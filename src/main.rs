// No console window. Milestone 6, at the tester's word: "The dialogue is
// enough and I would be comfortable with the console window removed."
//
// This one line is also what stops a console flashing up at every sign-in,
// which is what autostart did for as long as this was a console binary.
#![windows_subsystem = "windows"]

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
//! # Where the output went
//!
//! A windows subsystem binary since Milestone 6: no console, no window of its
//! own, nothing on screen but a tray icon until something is asked of it. The
//! console harness that carried Milestones 2 to 5 is gone, at the tester's
//! word that "the dialogue is enough and I would be comfortable with the
//! console window removed".
//!
//! What it printed had to go somewhere, and it went two ways rather than one:
//!
//! - **Anything routine goes to the log** - the settings in force, the hotkeys
//!   claimed, the devices available. The engine already logged every state
//!   change from its own thread, so `drain` no longer repeats it and only
//!   moves the tray icon.
//! - **Anything that failed opens the message window**, because logging is off
//!   by default and a log nobody switched on is not a report. A hotkey that
//!   could not be claimed leaves the primary interface silently dead, which is
//!   the worst failure this app has; it says so instead.
//!
//! The one deliberate exception is [`engine::Event::Error`]. The engine raises
//! it whenever no output device can be opened, retries on its own, and
//! recovers - proven on hardware when the headset was disconnected and brought
//! back. A dialog on every disconnection would be exactly the message spam
//! that round was checking for, so it stays in the log.

mod audio;
mod config;
mod docs;
mod engine;
mod hotkey;
mod input;
mod instance;
mod log;
mod settings;
mod startup;
mod tray;

use std::path::{Path, PathBuf};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Arc, Mutex};

use windows::Win32::Foundation::{HWND, POINT};
use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::{
    DispatchMessageW, GetMessageW, KillTimer, SetTimer, TranslateMessage, MSG, WM_HOTKEY,
    WM_MOUSEMOVE, WM_TIMER,
};

use crate::config::Config;
use crate::engine::{Command, Event};
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
    /// The live settings. Shared behind a lock because the dialog reads them
    /// on this thread while the engine works from its own copy.
    shared: Arc<Mutex<Config>>,
    toggle_key: Option<Registration>,
    settings_key: Option<Registration>,
    config_path: PathBuf,
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

    // Before anything is opened, claimed or written. The settings are read
    // first only so that the message below can name the hotkey this machine
    // is actually set up with, rather than the default.
    let Some(_only_copy) = instance::claim() else {
        already_running(&cfg);
        return;
    };

    let log = Log::new(config::log_path(), cfg.logging);
    // The engine takes ownership of the log; the message loop keeps a handle
    // of its own so tray callbacks can be recorded where they arrive.
    let loop_log = log.clone();

    let tray = match Tray::create() {
        Ok(t) => t,
        Err(e) => {
            // No window means no hotkey, which is not something to limp along
            // with silently - and with no console to print to, this window is
            // the only way to say so.
            //
            // A missing *icon* no longer comes through here. That is handled
            // further down, because at sign-in it means Explorer has not
            // started yet and the right answer is to wait, not to give up.
            settings::problem(
                None,
                &format!(
                    "StableSound could not create its tray icon, and cannot run without one.\n\
                     \n\
                     Windows said: {e}\n\
                     \n\
                     Try signing out and back in. If it keeps happening, switch logging on in \
                     the settings first, so there is something to read afterwards."
                ),
            );
            return;
        }
    };

    record_startup(&cfg, &config_path, &log, &adjustments);

    // Which of the two autostart routes is in force, and an attempt to move an
    // older Run entry up to a scheduled task. Done here rather than in the
    // dialog because the dialog only acts when the checkbox *changes*, and for
    // anybody who ticked it before Milestone 7 it is already ticked. See
    // `startup`.
    match startup::current() {
        Some(method) => log.write(&format!("autostart: on, by {}", method.describe())),
        None => log.write("autostart: off"),
    }
    if let Some(line) = startup::upgrade_run_to_task() {
        log.write(&line);
    }

    // This has been wrong on hardware before, so it says so rather than
    // failing quietly.
    if tray.is_icon_pending() {
        // The expected state when a logon task beats Explorer to it. Recorded
        // either way, because "there was no tray icon" is a report that cannot
        // be told apart from "the icon was added and then vanished" without
        // this line.
        log.write(
            "tray icon refused for now - no shell to put it in yet, which is \
             normal this early at sign-in. Retrying for a minute; the hotkey \
             works regardless.",
        );
    } else {
        log.write(&format!(
            "tray icon added, shell notification version 4 {}",
            if tray.is_version4() {
                "accepted"
            } else {
                "REFUSED, falling back to version 3 messages"
            }
        ));
    }
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
    // what the help file recommends. The tray menu opens the settings too, so
    // nothing is out of reach either way.
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

    let shared = Arc::new(Mutex::new(cfg.clone()));
    let handle = engine::spawn(cfg, log);

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
    surface.log.write("stopped");
    drop(surface);
}

/// Say that the tray icon never turned up, once, after a minute of trying.
///
/// Not fatal, and deliberately not shown at once. Until Milestone 7 a refused
/// icon ended the run with a message window, which was defensible while the
/// only way to start was Explorer's `Run` key - if Explorer had put us there,
/// Explorer was up. A logon task can beat the shell by seconds, so the same
/// refusal is now the ordinary case and waiting is the right answer.
///
/// But silence would be worse than either. Everything except the icon and its
/// menu still works, and a user who goes looking for the icon should find out
/// from StableSound why it is not there rather than concluding the app is
/// broken. By the time this can fire the desktop has long been up, so a window
/// is readable.
fn icon_never_arrived(surface: &mut Surface) {
    surface
        .log
        .write("tray icon never accepted after a minute of retrying - carrying on without it");
    let settings_route = if surface.settings_key.is_some() {
        "its settings hotkey"
    } else {
        "the Settings button, if you can reach it"
    };
    settings::problem(
        None,
        &format!(
            "StableSound is running, but Windows would not give it a tray icon.\n\
             \n\
             Everything else works. Your hotkey still switches keep-alive on and off, \
             {settings_route} still opens the settings, and the headphones are being held \
             awake exactly as usual. What you have lost is the icon in the system tray and \
             the menu on it.\n\
             \n\
             This usually means Windows Explorer is not running. Signing out and back in \
             normally fixes it. StableSound does not need restarting for the icon to come \
             back - it puts the icon back by itself the moment Explorer returns."
        ),
    );
}

/// Say that this is the second copy, and where the first one is.
///
/// A window rather than a silent exit, because a silent exit is
/// indistinguishable from the app failing to start - and running the exe again
/// is exactly what somebody does when they are not sure whether it is running,
/// which with no console and no window of its own is most of the time.
///
/// It names the combination in force rather than the default, which is why
/// this is worth reading the settings file for.
fn already_running(cfg: &Config) {
    let settings_route = if cfg.settings_hotkey_enabled {
        format!("Press {} to open its settings.", cfg.settings_hotkey)
    } else {
        "To open its settings, press Windows+B to reach the system tray, use the arrow \
         keys to find StableSound, then press the Applications key and choose Settings."
            .to_string()
    };
    settings::note(
        None,
        &format!(
            "StableSound is already running, so this second copy has stopped. Only one can run \
             at a time: two would fight over the same hotkey and the same headphones.\n\
             \n\
             The copy that is running is working normally. Press {} to switch keep-alive on \
             and off.\n\
             \n\
             {settings_route}\n\
             \n\
             If StableSound starts by itself when you sign in, it will usually be running \
             already. You can turn that off in its settings.",
            cfg.hotkey
        ),
    );
}

/// Register one global hotkey, reporting either way.
///
/// Success is a log line. Failure is a window, because this is the primary
/// interface and the alternative is the user pressing a combination that does
/// nothing, forever, with no way to find out why.
fn claim(
    key: crate::hotkey::Hotkey,
    hwnd: HWND,
    id: i32,
    what: &str,
    log: &Log,
) -> Option<Registration> {
    match key.register(hwnd, id) {
        Ok(r) => {
            log.write(&format!("hotkey {key} registered ({what})"));
            Some(r)
        }
        Err(e) => {
            log.write(&format!("hotkey {key} NOT registered ({what}): {e}"));
            settings::problem(
                Some(hwnd),
                &format!(
                    "StableSound could not claim {key}, the hotkey that {what}. Another program \
                     is already using that combination, so pressing it will do nothing.\n\
                     \n\
                     Windows said: {e}\n\
                     \n\
                     To pick a different combination, open the settings from the StableSound \
                     icon in the system tray. Press Windows+B, then the arrow keys to \
                     reach it, then the Applications key for its menu, and choose Settings."
                ),
            );
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

            WM_TIMER if message.wParam.0 == EVENT_TIMER => {
                drain(surface, &handle.events);
                match surface.tray.poll_icon() {
                    Some(tray::IconOutcome::Added) => surface.log.write("tray icon added late"),
                    Some(tray::IconOutcome::GaveUp) => icon_never_arrived(surface),
                    None => {}
                }
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
            docs::open_log(Some(surface.tray.hwnd()));
            false
        }
        Some(tray::CMD_HELP) => {
            docs::open_help(Some(surface.tray.hwnd()));
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
fn apply_settings(surface: &mut Surface, handle: &engine::Handle, mut cfg: Config) {
    // The dialog destroys itself on OK, so the handle we were holding is
    // already stale.
    surface.dialog = None;

    let Ok(previous) = surface.shared.lock().map(|mut live| {
        let previous = live.clone();
        // The remembered keep-alive state is not the dialog's to change. It
        // has no control for it, so what comes back is whatever was true when
        // the dialog opened - and the hotkey goes on working while it is open,
        // which is a tested behaviour, so that snapshot can be minutes stale.
        // Taking the live value here is what stops pressing OK from undoing a
        // toggle made while the dialog was up.
        cfg.keep_alive_on = live.keep_alive_on;
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
            surface
                .log
                .write("settings changed from the dialog and saved");
            for line in cfg.summary() {
                surface.log.write(&format!("  {line}"));
            }
        }
        Err(e) => {
            surface
                .log
                .write(&format!("settings could not be saved: {e}"));
            // The settings are in force but will not survive a restart, and
            // there is nothing on screen that would otherwise say so.
            settings::problem(
                Some(surface.tray.hwnd()),
                &format!(
                    "StableSound is using the settings you chose, but could not write them to \
                     its settings file, so they will be forgotten when it next starts.\n\
                     \n\
                     The file is: {}\n\
                     \n\
                     Windows said: {e}\n\
                     \n\
                     This usually means the folder StableSound is in cannot be written to. \
                     Moving StableSound.exe somewhere in your own user folder, rather than in \
                     Program Files, will fix it.",
                    surface.config_path.display()
                ),
            );
        }
    }
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
            log.write(&format!("hotkey for {what} changed from {old} to {new}"));
        }
        Err(e) => {
            log.write(&format!(
                "hotkey {new} refused for {what} ({e}), restoring {old}"
            ));
            *slot = old.register(hwnd, id).ok();
            let fallback = if slot.is_some() {
                format!("StableSound has gone back to {old}, which still works.")
            } else {
                log.write(&format!("hotkey {old} could not be restored for {what}"));
                format!(
                    "StableSound could not take {old} back either, so nothing will {what} until \
                     you choose a combination that is free."
                )
            };
            settings::problem(
                Some(hwnd),
                &format!(
                    "StableSound could not claim {new} to {what}. Another program is already \
                     using that combination.\n\
                     \n\
                     Windows said: {e}\n\
                     \n\
                     {fallback}\n\
                     \n\
                     Open the settings again and try a different combination. Adding or removing \
                     Shift is usually enough."
                ),
            );
        }
    }
}

/// Take whatever the engine has said since the last tick and reflect it.
///
/// Two things happen here and nothing else. The tray icon follows the sound,
/// and a change the user made by hand is written to the settings file so it
/// survives to the next run. The engine writes its own log lines from its own
/// thread, which is why the log stayed complete through Milestone 2's testing
/// while the console did not; and the *audible* feedback is the engine's job
/// too, played through the device being kept awake, because an off-tone has
/// to be heard before the stream closes.
///
/// Saving belongs on this thread rather than in the engine because this thread
/// owns the settings file: the dialog writes it from here too, and one writer
/// means the two can never race to produce a half-written file.
fn drain(surface: &mut Surface, events: &std::sync::mpsc::Receiver<Event>) {
    while let Ok(event) = events.try_recv() {
        match event {
            Event::KeepAliveOn => surface.tray.set_active(true),
            Event::KeepAliveOff => surface.tray.set_active(false),
            Event::Remember(on) => remember(surface, on),
        }
    }
}

/// Write down that keep-alive was switched on, or off by hand.
///
/// A failure here only reaches the log, unlike the same failure from the
/// settings dialog, which opens a window. The difference is what the user was
/// doing: pressing OK is a request to save, and it failing is news, whereas
/// this happens on a hotkey press whose whole point is that it needs no
/// attention. A window every time the headphones were switched on, because a
/// folder is read-only, would be worse than quietly forgetting between runs.
fn remember(surface: &mut Surface, on: bool) {
    let Ok(cfg) = surface.shared.lock().map(|mut live| {
        live.keep_alive_on = on;
        live.clone()
    }) else {
        return;
    };
    match cfg.save(&surface.config_path) {
        Ok(()) => surface.log.write(&format!(
            "remembered for next time: keep-alive {}",
            if on { "on" } else { "off" }
        )),
        Err(e) => surface.log.write(&format!(
            "could not remember the keep-alive state ({e}) - it will start off next time"
        )),
    }
}

/// Write down what the app started as, for a log read afterwards.
///
/// This was the console banner. It is worth keeping as log lines rather than
/// dropping: every question still open in PLAN.md is answered by reading a log
/// after the fact, and "what was it set to at the time" is the first thing
/// anyone reading one needs to know.
fn record_startup(cfg: &Config, config_path: &Path, log: &Log, adjustments: &[config::Adjustment]) {
    log.write(&format!(
        "StableSound {} starting",
        env!("CARGO_PKG_VERSION")
    ));
    log.write(&format!(
        "settings file: {}{}",
        config_path.display(),
        if config_path.exists() {
            ""
        } else {
            " (none yet - using defaults)"
        }
    ));
    for line in cfg.summary() {
        log.write(&format!("  {line}"));
    }

    // Anything validate() corrected. This is the one thing the console said
    // out loud that now only reaches the log, and it is a deliberate choice:
    // adjustments happen when the settings file has been hand-edited out of
    // range, and a dialog at every sign-in would be a poor trade for a rare
    // case the settings dialog already shows the truth of.
    for adjustment in adjustments {
        log.write(&format!(
            "config adjusted: {} ({})",
            adjustment.what, adjustment.why
        ));
    }

    match audio::device::list_outputs() {
        Ok(devices) if !devices.is_empty() => {
            log.write("output devices available:");
            for d in &devices {
                log.write(&format!("  {}", d.name));
            }
        }
        Ok(_) => log.write("no active output devices found"),
        Err(e) => log.write(&format!("could not list output devices: {e}")),
    }
}

#[cfg(test)]
mod tests {
    /// Catch a message string whose line continuations have been flattened.
    ///
    /// This defect has now reached this project six times. Milestone 5 found
    /// five at once - "five message strings had had their line continuations
    /// flattened into runs of literal spaces, four of them long before this
    /// branch" - and Milestone 6 added a sixth while writing the second-copy
    /// message.
    ///
    /// It happens when the `\` at the end of a line inside a string literal is
    /// lost. Rust then keeps the newline and the next line's indentation, so
    /// "one can run \" + newline + fourteen spaces + "at a time" becomes "one
    /// can run              at a time". It compiles, it reads fine in the
    /// source, and nothing but running the program shows it - and the person
    /// this program is for cannot see the runs of spaces, only hear whatever
    /// the screen reader makes of them.
    ///
    /// So it is worth a test, even though testing the source text of a program
    /// from inside that program is an odd thing to do. Six spaces is the
    /// threshold: a flattened continuation carries a whole line's indentation,
    /// thirteen or fourteen spaces here, while the deliberate alignment this
    /// codebase does use - the columns in `Config::summary` and the key names
    /// in `hotkey::HOW_TO_WRITE` - never exceeds four.
    #[test]
    fn no_message_has_a_flattened_line_continuation() {
        let sources = [
            ("main.rs", include_str!("main.rs")),
            ("settings.rs", include_str!("settings.rs")),
            ("hotkey.rs", include_str!("hotkey.rs")),
            ("config.rs", include_str!("config.rs")),
            ("tray.rs", include_str!("tray.rs")),
            ("instance.rs", include_str!("instance.rs")),
        ];
        let mut found = Vec::new();
        for (name, text) in sources {
            for (n, line) in text.lines().enumerate() {
                let body = line.trim_start();
                // Comments wrap prose too, and a wrapped comment is just a
                // comment. Only string content can be flattened.
                if body.starts_with("//") || body.starts_with('*') {
                    continue;
                }
                if has_run_of_spaces(body) {
                    found.push(format!("{name}:{} {body}", n + 1));
                }
            }
        }
        assert!(
            found.is_empty(),
            "a line continuation looks flattened - the backslash at the end of \
             the previous line has been lost:\n{}",
            found.join("\n")
        );
    }

    /// A flattened continuation carries a whole line's indentation, so the
    /// giveaway is a long run of spaces. Six, against the four that the
    /// deliberate alignment in this codebase reaches.
    const RUN: usize = 6;

    /// True if `RUN` or more spaces sit between two non-space characters.
    ///
    /// Trimming first is what makes the run an *interior* one: indentation and
    /// trailing whitespace are neither a mistake nor readable text.
    fn has_run_of_spaces(line: &str) -> bool {
        let mut run = 0;
        for c in line.trim().chars() {
            run = if c == ' ' { run + 1 } else { 0 };
            if run >= RUN {
                return true;
            }
        }
        false
    }

    #[test]
    fn the_flattening_check_knows_what_it_is_looking_for() {
        // Built rather than written out, because a literal run of spaces here
        // would be found by the test above - which reads this very file.
        let spaces = " ".repeat(14);
        assert!(has_run_of_spaces(&format!(
            "\"one can run{spaces}at a time\""
        )));
        // Deliberate alignment, which this codebase does use.
        assert!(!has_run_of_spaces("\"wake:    {}\""));
        assert!(!has_run_of_spaces("  ctrl   (or control)"));
        assert!(!has_run_of_spaces("let x = 1;"));
        // Indentation alone is not a run between two characters.
        assert!(!has_run_of_spaces(&spaces));
    }
}
