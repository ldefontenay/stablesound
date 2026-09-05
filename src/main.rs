//! StableSound.
//!
//! Keeps Bluetooth headphones awake so screen reader speech is not clipped,
//! and lets go of them again so a phone can take over.
//!
//! This thread owns the message queue. It does nothing but wait for the hotkey,
//! the tray and the engine, which is why the engine runs elsewhere: WASAPI
//! objects are not `Send`, so the audio work owns its own thread and everything
//! reaches it over channels.
//!
//! Still a console subsystem binary. The window here is hidden, but the console
//! harness is deliberately still present - see `console`. Milestone 6 drops it
//! and switches subsystem.

mod audio;
mod config;
mod console;
mod engine;
mod hotkey;
mod input;
mod log;
mod tray;

use windows::core::PCWSTR;
use windows::Win32::Foundation::POINT;
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, SetTimer, TranslateMessage, MSG, SW_SHOWNORMAL,
    WM_HOTKEY, WM_TIMER,
};

use crate::config::Config;
use crate::engine::{Command, Event, StopReason};
use crate::log::Log;
use crate::tray::{Tray, TrayEvent};

/// Timer that pulls engine events off the channel. The engine cannot post to
/// this queue itself - it has no window - so the queue asks it instead. A tenth
/// of a second matches the engine's own tick and is far below the point where
/// anyone would notice the tray lagging behind the sound.
const EVENT_TIMER: usize = 1;
const EVENT_TIMER_MS: u32 = 100;

fn main() {
    // The engine thread initialises COM for itself, but this thread also calls
    // into WASAPI (listing devices) and COM is per-thread. Without this,
    // list_outputs fails with 0x800401F0.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let config_path = config::config_path();
    let (cfg, adjustments) = Config::load(&config_path);

    let log = Log::new(
        if cfg.logging {
            Some(config::log_path())
        } else {
            None
        },
        false,
    );
    let log_path = config::log_path();

    let mut tray = match Tray::create() {
        Ok(t) => t,
        Err(e) => {
            // No tray means no window, and no window means no hotkey. That is
            // not something to limp along with silently.
            eprintln!("Could not create the tray icon or its window: {e}");
            eprintln!("StableSound cannot run without it.");
            return;
        }
    };

    banner(&cfg, &config_path, &log, &adjustments);

    // Claim the hotkey before anything else can want it, and say plainly if
    // somebody already has it. A hotkey that silently does nothing is the
    // worst possible failure for the app's primary interface.
    let registration = match cfg.hotkey.register(tray.hwnd()) {
        Ok(r) => {
            println!("Hotkey:   {} toggles keep-alive.", cfg.hotkey);
            log.write(&format!("hotkey {} registered", cfg.hotkey));
            Some(r)
        }
        Err(e) => {
            println!("WARNING: could not register {} ({e}).", cfg.hotkey);
            println!("  Another program is probably already using it.");
            println!("  Set a different one with: hotkey <combination>, then save and restart.");
            println!("  Until then, use the console commands or the tray icon.");
            log.write(&format!("hotkey {} NOT registered: {e}", cfg.hotkey));
            None
        }
    };

    println!();
    console::help();
    println!();

    let handle = engine::spawn(cfg.clone(), log);
    console::spawn(cfg, handle.commands.clone(), tray.hwnd());

    unsafe {
        SetTimer(Some(tray.hwnd()), EVENT_TIMER, EVENT_TIMER_MS, None);
    }
    pump(&mut tray, &handle, &log_path);
    unsafe {
        let _ = KillTimer(Some(tray.hwnd()), EVENT_TIMER);
    }

    // Order matters on the way out. Releasing the hotkey and the icon before
    // the engine stops would leave the tray showing a stale state during the
    // moment the engine spends draining the off-tone.
    let _ = handle.commands.send(Command::Quit);
    let _ = handle.thread.join();
    drop(registration);
    drop(tray);
    println!("Stopped.");
}

/// The message loop.
///
/// Everything is handled here rather than in a window procedure. A procedure
/// would need global state to reach the engine, whereas this can simply borrow
/// it, and it keeps the whole control surface in one place a reader can follow.
fn pump(tray: &mut Tray, handle: &engine::Handle, log_path: &std::path::Path) {
    let mut message = MSG::default();

    loop {
        // Zero is WM_QUIT, and -1 is an error. `as_bool` would treat -1 as
        // success and spin forever on a closed queue.
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }

        match message.message {
            WM_HOTKEY if message.wParam.0 as i32 == hotkey::TOGGLE_ID => {
                let _ = handle.commands.send(Command::Toggle);
            }

            tray::WM_TRAY => match tray.decode(message.wParam, message.lParam) {
                Some(TrayEvent::Toggle) => {
                    let _ = handle.commands.send(Command::Toggle);
                }
                Some(TrayEvent::Menu { x, y }) => {
                    // Blocks while the menu is open, which is fine: the engine
                    // keeps pumping audio on its own thread.
                    let chose_quit = menu(tray, handle, log_path, POINT { x, y });
                    if chose_quit {
                        break;
                    }
                }
                None => {}
            },

            console::WM_CONSOLE_QUIT => break,

            WM_TIMER if message.wParam.0 == EVENT_TIMER => drain(tray, &handle.events),

            other if tray.is_taskbar_restart(other) => tray.readd(),

            _ => {}
        }

        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Show the tray menu and act on the choice. Returns true if we should quit.
fn menu(tray: &Tray, handle: &engine::Handle, log_path: &std::path::Path, at: POINT) -> bool {
    match tray.show_menu(at) {
        Some(tray::CMD_TOGGLE) => {
            let _ = handle.commands.send(Command::Toggle);
            false
        }
        Some(tray::CMD_OPEN_LOG) => {
            open_log(log_path);
            false
        }
        Some(tray::CMD_QUIT) => true,
        _ => false,
    }
}

/// Hand the log to whatever the user reads text files with.
///
/// Worth a menu item of its own: the log is how this project's behaviour gets
/// checked, because idle behaviour cannot be watched live - reading the output
/// with a screen reader makes the very sound being measured.
fn open_log(path: &std::path::Path) {
    if !path.exists() {
        println!("No log file yet at {}", path.display());
        return;
    }
    let wide: Vec<u16> = path
        .as_os_str()
        .to_string_lossy()
        .encode_utf16()
        .chain(std::iter::once(0))
        .collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        );
    }
}

/// Take whatever the engine has said since the last tick and reflect it.
///
/// The tray only learns the state here. The *audible* feedback is the engine's
/// job, played through the device being kept awake, because an off-tone has to
/// be heard before the stream closes.
fn drain(tray: &mut Tray, events: &std::sync::mpsc::Receiver<Event>) {
    while let Ok(event) = events.try_recv() {
        match event {
            Event::Started { device, signal } => {
                println!("  KEEP-ALIVE ON - {device}, signal {signal}");
                tray.set_active(true);
            }
            Event::WokenByInput { device } => {
                println!("  KEEP-ALIVE ON - woken by input, {device}");
                tray.set_active(true);
            }
            Event::Moved { device } => {
                println!("  MOVED to new default output: {device}");
                tray.set_active(true);
            }
            Event::Stopped { reason } => {
                println!("  KEEP-ALIVE OFF - {}, device released", describe(&reason));
                tray.set_active(false);
            }
            Event::Interrupted { message } => {
                println!("  INTERRUPTED - {message} (retrying)");
            }
            Event::Substituted { wanted, used } => {
                println!("  '{wanted}' not found, using '{used}' instead");
            }
            Event::Error { message } => println!("  ERROR: {message}"),
        }
    }
}

fn describe(reason: &StopReason) -> &'static str {
    match reason {
        StopReason::Requested => "asked to stop",
        StopReason::IdleTimeout => "idle timeout reached",
        StopReason::FixedTimeout => "fixed timer expired",
    }
}

fn banner(
    cfg: &Config,
    config_path: &std::path::Path,
    log: &Log,
    adjustments: &[config::Adjustment],
) {
    println!("StableSound - Milestone 3");
    println!();
    println!("Settings: {}", config_path.display());
    if !config_path.exists() {
        println!("  (no file yet - using defaults; 'save' writes one)");
    }
    match log.path() {
        Some(p) => println!("Log:      {}", p.display()),
        None => println!("Log:      disabled"),
    }
    println!();
    console::describe(cfg);

    // Report anything validate() corrected, rather than silently changing
    // behaviour behind the user's back.
    for adjustment in adjustments {
        println!();
        println!("ADJUSTED: {}", adjustment.what);
        println!("  Why: {}", adjustment.why);
        log.write(&format!(
            "config adjusted: {} ({})",
            adjustment.what, adjustment.why
        ));
    }

    println!();
    match audio::device::list_outputs() {
        Ok(devices) if !devices.is_empty() => {
            println!("Available outputs:");
            for d in &devices {
                println!("  {}", d.name);
            }
        }
        Ok(_) => println!("No active output devices found."),
        Err(e) => println!("Could not list output devices: {e}"),
    }
    println!();
}
