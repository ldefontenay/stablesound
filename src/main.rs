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
mod settings;
mod startup;
mod tray;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, SetTimer, TranslateMessage, MSG, SW_SHOWNORMAL,
    WM_HOTKEY, WM_MOUSEMOVE, WM_TIMER,
};

use crate::config::Config;
use crate::engine::{Command, Event, StopReason};
use crate::hotkey::Registration;
use crate::log::Log;
use crate::tray::{Tray, TrayEvent};

/// Timer that pulls engine events off the channel. The engine cannot post to
/// this queue itself - it has no window - so the queue asks it instead. A tenth
/// of a second matches the engine's own tick and is far below the point where
/// anyone would notice the tray lagging behind the sound.
const EVENT_TIMER: usize = 1;
const EVENT_TIMER_MS: u32 = 100;

/// Everything the message loop owns.
///
/// This was four parameters until the settings dialog arrived and added a
/// window to keep track of, a channel to read answers off, a second hotkey
/// registration and a shared config. Eight positional arguments would be
/// worse than a struct.
struct Surface {
    tray: Tray,
    /// The settings dialog while it is open. Modeless, so it lives alongside
    /// the loop rather than inside a nested one - see `settings`.
    dialog: Option<HWND>,
    /// Accepted settings arrive here from the dialog's OK button.
    applied: Receiver<Config>,
    applied_tx: Sender<Config>,
    /// The live settings, shared with the console thread so the dialog and
    /// the typed commands cannot disagree about what is set.
    shared: Arc<Mutex<Config>>,
    toggle_key: Option<Registration>,
    settings_key: Option<Registration>,
    config_path: PathBuf,
    log_path: PathBuf,
    log: Log,
}

fn main() {
    // The engine thread initialises COM for itself, but this thread also calls
    // into WASAPI (listing devices) and COM is per-thread. Without this,
    // list_outputs fails with 0x800401F0.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let config_path = config::config_path();
    let (cfg, adjustments) = Config::load(&config_path);

    let log = Log::new(config::log_path(), false, cfg.logging);
    let log_path = config::log_path();
    // The engine takes ownership of the log; the message loop keeps a handle
    // of its own so tray callbacks can be recorded where they arrive.
    let loop_log = log.clone();

    let tray = match Tray::create() {
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

    // This has been wrong on hardware before, so it says so rather than
    // failing quietly.
    log.write(&format!(
        "tray icon added, shell notification version 4 {}",
        if tray.is_version4() {
            "accepted"
        } else {
            "REFUSED, falling back to version 3 messages"
        }
    ));
    // Claim the hotkeys before anything else can want them, and say plainly
    // if somebody already has one. A hotkey that silently does nothing is the
    // worst possible failure for the app's primary interface.
    let toggle_key = claim(
        cfg.hotkey,
        tray.hwnd(),
        hotkey::TOGGLE_ID,
        "toggles keep-alive",
        &log,
    );
    // On by default, at the tester's request after the Milestone 5 round, and
    // switchable off in the dialog once the settings are settled - which is
    // what the help file will recommend. The tray menu and the console open
    // the settings too, so nothing is out of reach either way.
    let settings_key = cfg
        .settings_hotkey_enabled
        .then(|| {
            claim(
                cfg.settings_hotkey,
                tray.hwnd(),
                hotkey::SETTINGS_ID,
                "opens the settings",
                &log,
            )
        })
        .flatten();

    println!();
    console::help();
    println!();

    let shared = Arc::new(Mutex::new(cfg.clone()));
    let handle = engine::spawn(cfg, log);
    console::spawn(shared.clone(), handle.commands.clone(), tray.hwnd());

    let (applied_tx, applied) = channel();
    let mut surface = Surface {
        tray,
        dialog: None,
        applied,
        applied_tx,
        shared,
        toggle_key,
        settings_key,
        config_path,
        log_path,
        log: loop_log,
    };

    unsafe {
        SetTimer(Some(surface.tray.hwnd()), EVENT_TIMER, EVENT_TIMER_MS, None);
    }
    pump(&mut surface, &handle);
    unsafe {
        let _ = KillTimer(Some(surface.tray.hwnd()), EVENT_TIMER);
    }

    // Order matters on the way out. Releasing the hotkeys and the icon before
    // the engine stops would leave the tray showing a stale state during the
    // moment the engine spends draining the off-tone.
    let _ = handle.commands.send(Command::Quit);
    let _ = handle.thread.join();
    drop(surface);
    println!("Stopped.");
}

/// Register one global hotkey, reporting either way.
fn claim(
    key: crate::hotkey::Hotkey,
    hwnd: HWND,
    id: i32,
    what: &str,
    log: &Log,
) -> Option<Registration> {
    match key.register(hwnd, id) {
        Ok(r) => {
            println!("Hotkey:   {key} {what}.");
            log.write(&format!("hotkey {key} registered ({what})"));
            Some(r)
        }
        Err(e) => {
            println!("WARNING: could not register {key} ({e}).");
            println!("  Another program is probably already using it.");
            println!("  Pick a different one in the settings, or with: hotkey <combination>");
            log.write(&format!("hotkey {key} NOT registered ({what}): {e}"));
            None
        }
    }
}

/// The message loop.
///
/// Everything is handled here rather than in a window procedure. A procedure
/// would need global state to reach the engine, whereas this can simply borrow
/// it, and it keeps the whole control surface in one place a reader can follow.
fn pump(surface: &mut Surface, handle: &engine::Handle) {
    let mut message = MSG::default();

    loop {
        // Zero is WM_QUIT, and -1 is an error. `as_bool` would treat -1 as
        // success and spin forever on a closed queue.
        let result = unsafe { GetMessageW(&mut message, None, 0, 0) };
        if result.0 <= 0 {
            break;
        }

        // The settings dialog is modeless, so its keyboard behaviour is not
        // automatic: this one call is what gives it Tab, the arrow keys,
        // mnemonics, Enter and Escape. It declines anything belonging to
        // another window, which is how the hotkey and the tray go on working
        // while it is open.
        if let Some(dialog) = surface.dialog {
            if settings::is_dialog_message(dialog, &message) {
                continue;
            }
        }

        match message.message {
            WM_HOTKEY if message.wParam.0 as i32 == hotkey::TOGGLE_ID => {
                let _ = handle.commands.send(Command::Toggle);
            }
            WM_HOTKEY if message.wParam.0 as i32 == hotkey::SETTINGS_ID => {
                open_settings(surface);
            }
            settings::WM_OPEN_SETTINGS => open_settings(surface),

            // Re-posted by the window procedure, because the shell *sends*
            // the callback and a sent message never comes back out of
            // GetMessage. See `tray`.
            tray::WM_TRAY_QUEUED => {
                // Recorded before it is acted on. Two rounds have now ended
                // with "the tray does nothing", and that report cannot be told
                // apart from "the message never arrived" without this line.
                let event = tray::callback_event(message.lParam);
                if event != WM_MOUSEMOVE {
                    surface.log.write(&format!("tray callback 0x{event:04X}"));
                }
                if tray_event(surface, handle, message.wParam, message.lParam) {
                    break;
                }
            }
            console::WM_CONSOLE_QUIT => break,

            WM_TIMER if message.wParam.0 == EVENT_TIMER => {
                drain(&mut surface.tray, &handle.events);
                // Settings the dialog accepted, read on the same tick as the
                // engine's events so there is one place where the loop takes
                // in news from elsewhere.
                while let Ok(cfg) = surface.applied.try_recv() {
                    apply_settings(surface, handle, cfg);
                }
            }

            other if surface.tray.is_taskbar_restart(other) => surface.tray.readd(),

            _ => {}
        }

        unsafe {
            let _ = TranslateMessage(&message);
            DispatchMessageW(&message);
        }
    }
}

/// Act on a tray callback. Returns true if we should quit.
fn tray_event(
    surface: &mut Surface,
    handle: &engine::Handle,
    wparam: windows::Win32::Foundation::WPARAM,
    lparam: windows::Win32::Foundation::LPARAM,
) -> bool {
    match surface.tray.decode(wparam, lparam) {
        Some(TrayEvent::Toggle) => {
            let _ = handle.commands.send(Command::Toggle);
            false
        }
        // Blocks while the menu is open, which is fine: the engine keeps
        // pumping audio on its own thread.
        Some(TrayEvent::Menu { x, y }) => menu(surface, handle, POINT { x, y }),
        None => false,
    }
}

/// Show the tray menu and act on the choice. Returns true if we should quit.
fn menu(surface: &mut Surface, handle: &engine::Handle, at: POINT) -> bool {
    match surface.tray.show_menu(at) {
        Some(tray::CMD_TOGGLE) => {
            let _ = handle.commands.send(Command::Toggle);
            false
        }
        Some(tray::CMD_SETTINGS) => {
            open_settings(surface);
            false
        }
        Some(tray::CMD_OPEN_LOG) => {
            open_log(&surface.log_path);
            false
        }
        Some(tray::CMD_QUIT) => true,
        _ => false,
    }
}

/// Put the settings dialog on screen, or raise the one already there.
///
/// The device list is gathered here rather than inside the dialog because
/// listing outputs needs COM on the calling thread, and this thread has it.
fn open_settings(surface: &mut Surface) {
    let devices = audio::device::list_outputs()
        .map(|list| list.into_iter().map(|d| d.name).collect())
        .unwrap_or_default();
    let Ok(snapshot) = surface.shared.lock().map(|live| live.clone()) else {
        return;
    };

    surface.dialog = settings::open(
        surface.tray.hwnd(),
        surface.dialog,
        &snapshot,
        devices,
        surface.applied_tx.clone(),
    );
}

/// Take settings the user accepted: write them down, tell the engine, and
/// re-claim any hotkey that changed.
fn apply_settings(surface: &mut Surface, handle: &engine::Handle, cfg: Config) {
    // The dialog destroys itself on OK, so the handle we were holding is
    // already stale.
    surface.dialog = None;

    let Ok(previous) = surface.shared.lock().map(|mut live| {
        let previous = live.clone();
        *live = cfg.clone();
        previous
    }) else {
        return;
    };

    // Hotkeys first. Re-registering can fail - somebody else may hold the new
    // combination - and it is better to find that out before the rest of the
    // settings have moved.
    if cfg.hotkey != previous.hotkey {
        rebind(
            &mut surface.toggle_key,
            previous.hotkey,
            cfg.hotkey,
            surface.tray.hwnd(),
            hotkey::TOGGLE_ID,
            "toggle keep-alive",
            &surface.log,
        );
    }
    // Three cases, not one: switched off, switched on, or changed while on.
    // Dropping the registration is what hands the combination back to the
    // rest of the machine, which is the whole point of being able to turn it
    // off.
    match (
        previous.settings_hotkey_enabled,
        cfg.settings_hotkey_enabled,
    ) {
        (true, false) => {
            surface.settings_key = None;
            println!(
                "  HOTKEY: {} no longer opens the settings.",
                previous.settings_hotkey
            );
            println!("    Use the tray menu (Win+B), or type: settings");
            surface.log.write(&format!(
                "settings hotkey {} released",
                previous.settings_hotkey
            ));
        }
        (false, true) => {
            surface.settings_key = claim(
                cfg.settings_hotkey,
                surface.tray.hwnd(),
                hotkey::SETTINGS_ID,
                "opens the settings",
                &surface.log,
            );
        }
        (true, true) if cfg.settings_hotkey != previous.settings_hotkey => {
            rebind(
                &mut surface.settings_key,
                previous.settings_hotkey,
                cfg.settings_hotkey,
                surface.tray.hwnd(),
                hotkey::SETTINGS_ID,
                "open the settings",
                &surface.log,
            );
        }
        _ => {}
    }

    // Takes effect now rather than at the next start: the log is how this
    // project's behaviour gets checked, and it gets switched on precisely
    // when something has begun to go wrong.
    surface.log.set_enabled(cfg.logging);

    match cfg.save(&surface.config_path) {
        Ok(()) => {
            println!("  SETTINGS SAVED to {}", surface.config_path.display());
            surface
                .log
                .write("settings changed from the dialog and saved");
        }
        Err(e) => {
            println!("  ERROR: settings could not be saved: {e}");
            surface
                .log
                .write(&format!("settings could not be saved: {e}"));
        }
    }
    console::describe(&cfg);
    let _ = handle.commands.send(Command::Reload(Box::new(cfg)));
}

/// Swap one global hotkey for another, putting the old one back if the new
/// one cannot be had.
///
/// The old registration must be dropped before the new one is tried, or
/// re-registering the *same* combination would collide with itself. That
/// leaves a moment with no hotkey at all, which is why failure restores the
/// old one rather than merely reporting.
fn rebind(
    slot: &mut Option<Registration>,
    old: hotkey::Hotkey,
    new: hotkey::Hotkey,
    hwnd: HWND,
    id: i32,
    what: &str,
    log: &Log,
) {
    *slot = None;
    match new.register(hwnd, id) {
        Ok(registration) => {
            *slot = Some(registration);
            println!("  HOTKEY: {new} now {what}s.");
            log.write(&format!("hotkey for {what} changed from {old} to {new}"));
        }
        Err(e) => {
            println!("  ERROR: {new} could not be registered ({e}).");
            println!("    Another program is holding it. Keeping {old}.");
            log.write(&format!(
                "hotkey {new} refused for {what} ({e}), restoring {old}"
            ));
            *slot = old.register(hwnd, id).ok();
            if slot.is_none() {
                println!("    {old} could not be taken back either - nothing bound now.");
                log.write(&format!("hotkey {old} could not be restored for {what}"));
            }
        }
    }
}

/// Hand the log to whatever the user reads text files with.
///
/// Worth a menu item of its own: the log is how this project's behaviour gets
/// checked, because idle behaviour cannot be watched live - reading the output
/// with a screen reader makes the very sound being measured.
fn open_log(path: &Path) {
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

fn banner(cfg: &Config, config_path: &Path, log: &Log, adjustments: &[config::Adjustment]) {
    println!("StableSound - Milestone 5");
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
