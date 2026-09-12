//! The settings dialog.
//!
//! The Milestone 2 hardware round asked for the idle timeout to be easy to
//! change, and said explicitly that editing a config file by hand does not
//! count. The third round added a request to customise the hotkey and a
//! settings item in the tray menu. This is all three.
//!
//! # Why a dialog template rather than a toolkit
//!
//! The controls here are real `EDIT`, `COMBOBOX` and `BUTTON` windows, built
//! from a template compiled by rc.exe (see `stablesound.rc`). CLAUDE.md rules
//! out drawn-UI toolkits because they reconstruct an accessibility tree rather
//! than exposing real controls, and behave worse under JAWS. A template goes
//! further than merely satisfying that: tab order, mnemonics, the default
//! button, Escape to cancel and the reading of a label as its control's name
//! all come from Windows, so a screen reader meets exactly the dialog it meets
//! everywhere else. `winsafe` was the other candidate; it wants to own the
//! window and message loop, which this app already has for the tray and the
//! hotkey, and it would have cost a dependency against a hard size budget for
//! controls we get either way.
//!
//! # Why it is modeless
//!
//! `DialogBoxParamW` would be simpler - it returns the answer directly instead
//! of handing it back over a channel. It also runs its own message loop, and
//! that loop dispatches `WM_HOTKEY` to a window procedure that does not handle
//! it. The global hotkey is the *primary interface* under CLAUDE.md, and
//! silently killing it for as long as a window happens to be open is not a
//! trade worth making to save a channel. So the dialog is modeless, `main`
//! keeps pumping, and `IsDialogMessageW` in that one loop gives the dialog its
//! keyboard behaviour.
//!
//! # The hotkey is typed, not captured
//!
//! Windows has a `HOTKEY` control that records the combination you press. It
//! is the wrong choice here twice over: a screen reader user pressing a
//! combination into it gets no readable confirmation of what was captured, and
//! it cannot express the Windows key at all - which the default `Ctrl+Win+F12`
//! needs. A plain edit box holding `ctrl+win+f12` is readable, reviewable,
//! correctable character by character, and parsed by the same `Hotkey::parse`
//! that reads the config file, so the dialog and the file cannot disagree.

use std::sync::mpsc::Sender;

use windows::core::PCWSTR;
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Diagnostics::Debug::MessageBeep;
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Controls::{
    CheckDlgButton, IsDlgButtonChecked, BST_CHECKED, BST_UNCHECKED,
};
use windows::Win32::UI::Input::KeyboardAndMouse::SetFocus;
use windows::Win32::UI::WindowsAndMessaging::{
    CreateDialogParamW, DestroyWindow, DialogBoxParamW, EndDialog, GetDlgItem, GetDlgItemInt,
    GetDlgItemTextW, GetWindowLongPtrW, IsDialogMessageW, IsWindow, PostQuitMessage,
    SendDlgItemMessageW, SetDlgItemInt, SetDlgItemTextW, SetForegroundWindow, SetWindowLongPtrW,
    ShowWindow, CB_ADDSTRING, CB_GETCURSEL, CB_SETCURSEL, GWLP_USERDATA, IDCANCEL, IDOK,
    MB_ICONEXCLAMATION, MB_ICONINFORMATION, MESSAGEBOX_STYLE, MSG, SW_SHOW, WM_CLOSE, WM_COMMAND,
    WM_HELP, WM_INITDIALOG, WM_NCDESTROY,
};

use crate::config::{Config, DeviceSelector, Release, Signal};
use crate::docs;
use crate::hotkey::{self, Hotkey};
use crate::startup;

/// Must match `stablesound.rc`.
const IDD_SETTINGS: u16 = 100;
const IDD_MESSAGE: u16 = 101;
const IDC_DEVICE: i32 = 1001;
const IDC_SIGNAL: i32 = 1002;
const IDC_RELEASE: i32 = 1003;
const IDC_TIMEOUT: i32 = 1004;
const IDC_WAKE_INPUT: i32 = 1005;
const IDC_EARCONS: i32 = 1007;
const IDC_VOLUME: i32 = 1008;
const IDC_HOTKEY: i32 = 1009;
const IDC_LOGGING: i32 = 1010;
const IDC_DIAGNOSTICS: i32 = 1011;
const IDC_SET_HOTKEY: i32 = 1012;
const IDC_STARTUP: i32 = 1013;
const IDC_USE_SET_HOTKEY: i32 = 1014;
const IDC_MESSAGE: i32 = 1015;
const IDC_MESSAGE_LABEL: i32 = 1016;
const IDC_OPEN_HELP: i32 = 1017;
const IDC_OPEN_LOG: i32 = 1018;
const IDC_EXIT: i32 = 1019;

/// Combo box order. The template is the other half of this contract; changing
/// one without the other silently mislabels a setting, so both are written out
/// rather than inferred.
const SIGNALS: [&str; 3] = [
    "Digital silence (recommended)",
    "Silence with a tick each second",
    "Inaudible tone (last resort)",
];
const RELEASES: [&str; 2] = [
    "When nothing has played for a while",
    "A fixed time after switching on",
];
const DEFAULT_DEVICE: &str = "Default output (follow Windows)";

/// Sine's numbers are not in the dialog. They are a last resort for hardware
/// the other signals fail on, they need a frequency and an amplitude to mean
/// anything, and putting two more numeric fields in front of every user to
/// serve that case is a poor trade. The config file still carries them, and
/// picking the tone here keeps whatever is already in the file.
const FALLBACK_SINE: (f32, f32) = (20_000.0, 0.01);

/// What the dialog needs while it is open. Lives in a `Box` owned by the
/// window, reached through `GWLP_USERDATA`, and dropped on `WM_NCDESTROY`.
struct State {
    /// The config as it was when the dialog opened. Carries the settings with
    /// no control of their own - the sine numbers, the audio threshold - so
    /// that OK cannot quietly discard them.
    original: Config,
    /// Device names offered, in combo order after the default entry.
    devices: Vec<String>,
    /// Where an accepted config goes. `main` picks it up on its next tick.
    applied: Sender<Config>,
}

/// Open the dialog, or bring the existing one forward if it is already up.
///
/// Returns the dialog window, which the caller must feed to
/// `IsDialogMessageW` - without that there is no Tab, no mnemonics and no
/// Escape, which is to say no accessible dialog at all.
pub fn open(
    owner: HWND,
    existing: Option<HWND>,
    cfg: &Config,
    devices: Vec<String>,
    applied: Sender<Config>,
) -> Option<HWND> {
    // Modeless means nothing stops a second one being asked for. Raising the
    // first is what every other Windows app does, and it avoids two dialogs
    // disagreeing about what the settings are.
    if let Some(hwnd) = existing {
        if unsafe { IsWindow(Some(hwnd)) }.as_bool() {
            unsafe {
                let _ = SetForegroundWindow(hwnd);
            }
            return Some(hwnd);
        }
    }

    let state = Box::new(State {
        original: cfg.clone(),
        devices,
        applied,
    });

    let hwnd = unsafe {
        let instance = GetModuleHandleW(None).ok()?;
        CreateDialogParamW(
            Some(instance.into()),
            PCWSTR(IDD_SETTINGS as usize as *const u16),
            Some(owner),
            Some(dialog_proc),
            LPARAM(Box::into_raw(state) as isize),
        )
        .ok()?
    };

    unsafe {
        let _ = ShowWindow(hwnd, SW_SHOW);
        let _ = SetForegroundWindow(hwnd);
    }
    Some(hwnd)
}

unsafe extern "system" fn dialog_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        WM_INITDIALOG => {
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, lparam.0);
            if let Some(state) = state(hwnd) {
                populate(hwnd, state);
            }
            // Non-zero: let Windows set the initial focus to the first
            // control, which is the device list.
            1
        }

        WM_COMMAND => {
            match (wparam.0 & 0xFFFF) as i32 {
                id if id == IDOK.0 => {
                    if let Some(state) = state(hwnd) {
                        if let Some(cfg) = read(hwnd, state) {
                            let _ = state.applied.send(cfg);
                            let _ = DestroyWindow(hwnd);
                        }
                        // Otherwise `read` has already said what is wrong and
                        // put the focus on the field that is wrong.
                    }
                    1
                }
                id if id == IDCANCEL.0 => {
                    let _ = DestroyWindow(hwnd);
                    1
                }
                // Both of these open something to read, and both are also in
                // the tray menu. They are here because dropping the console
                // in Milestone 6 would otherwise have left them tray-only,
                // and CLAUDE.md constraint 3 does not allow that.
                IDC_OPEN_HELP => {
                    docs::open_help(Some(hwnd));
                    1
                }
                IDC_OPEN_LOG => {
                    docs::open_log(Some(hwnd));
                    1
                }
                // The only way out of StableSound that is not the tray menu.
                //
                // The dialog goes first: the message loop is about to stop
                // reading messages, and a window whose keyboard handling has
                // stopped is worse than no window. `PostQuitMessage` is safe
                // to call from here because a modeless dialog's procedure runs
                // on the thread that owns the queue - which is the whole
                // reason this dialog is modeless.
                //
                // Settings typed but not accepted with OK are discarded, as
                // they are for Cancel. The help says so.
                IDC_EXIT => {
                    let _ = DestroyWindow(hwnd);
                    PostQuitMessage(0);
                    1
                }
                _ => 0,
            }
        }

        // F1, from wherever the focus happens to be. The conventional key for
        // this, and free: nothing else in the dialog wants it.
        WM_HELP => {
            docs::open_help(Some(hwnd));
            1
        }

        // Reached by the close button and by Escape once IDCANCEL has run.
        WM_CLOSE => {
            let _ = DestroyWindow(hwnd);
            1
        }

        WM_NCDESTROY => {
            let ptr = SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
            if ptr != 0 {
                drop(Box::from_raw(ptr as *mut State));
            }
            0
        }

        _ => 0,
    }
}

unsafe fn state<'a>(hwnd: HWND) -> Option<&'a mut State> {
    let ptr = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *mut State;
    ptr.as_mut()
}

/// Put the current settings into the controls.
fn populate(hwnd: HWND, state: &State) {
    let cfg = &state.original;

    add_item(hwnd, IDC_DEVICE, DEFAULT_DEVICE);
    for name in &state.devices {
        add_item(hwnd, IDC_DEVICE, name);
    }
    // A device named in the config but not currently present - unplugged, or
    // a headset that is off - falls back to the default entry rather than
    // silently selecting the wrong one.
    let device_index = match &cfg.device {
        DeviceSelector::Default => 0,
        DeviceSelector::Named(wanted) => state
            .devices
            .iter()
            .position(|d| d == wanted)
            .map(|i| i + 1)
            .unwrap_or(0),
    };
    select(hwnd, IDC_DEVICE, device_index);

    for name in SIGNALS {
        add_item(hwnd, IDC_SIGNAL, name);
    }
    select(
        hwnd,
        IDC_SIGNAL,
        match cfg.signal {
            Signal::Zeros => 0,
            Signal::Fluctuate => 1,
            Signal::Sine { .. } => 2,
        },
    );

    for name in RELEASES {
        add_item(hwnd, IDC_RELEASE, name);
    }
    let (release_index, secs) = match cfg.release {
        Release::Idle { secs } => (0, secs),
        Release::Fixed { secs } => (1, secs),
    };
    select(hwnd, IDC_RELEASE, release_index);
    set_int(hwnd, IDC_TIMEOUT, secs);

    check(hwnd, IDC_WAKE_INPUT, cfg.wake_on_input);
    check(hwnd, IDC_EARCONS, cfg.earcons);
    // Stored as an amplitude, shown as a percentage: "10" is a far easier
    // thing to read out, hear and retype than "0.1".
    set_int(hwnd, IDC_VOLUME, (cfg.earcon_volume * 100.0).round() as u32);
    set_text(hwnd, IDC_HOTKEY, &cfg.hotkey.to_string());
    check(hwnd, IDC_USE_SET_HOTKEY, cfg.settings_hotkey_enabled);
    // Filled in even when the checkbox is clear, so turning the hotkey on is
    // one keystroke rather than one keystroke and a combination to invent.
    set_text(hwnd, IDC_SET_HOTKEY, &cfg.settings_hotkey.to_string());
    check(hwnd, IDC_LOGGING, cfg.logging);
    check(hwnd, IDC_DIAGNOSTICS, cfg.diagnostics);
    // Read from the registry rather than the config, because that is what
    // actually decides it - see `startup`.
    check(hwnd, IDC_STARTUP, startup::is_enabled());
}

/// Build a config from the controls, or explain what is wrong and return
/// `None` with the focus on the offending field.
fn read(hwnd: HWND, state: &State) -> Option<Config> {
    let mut cfg = state.original.clone();

    cfg.device = match selected(hwnd, IDC_DEVICE) {
        0 => DeviceSelector::Default,
        i => match state.devices.get(i - 1) {
            Some(name) => DeviceSelector::Named(name.clone()),
            None => DeviceSelector::Default,
        },
    };

    cfg.signal = match selected(hwnd, IDC_SIGNAL) {
        1 => Signal::Fluctuate,
        2 => match state.original.signal {
            // Keep whatever the file already had rather than resetting it.
            Signal::Sine { freq, amp } => Signal::Sine { freq, amp },
            _ => Signal::Sine {
                freq: FALLBACK_SINE.0,
                amp: FALLBACK_SINE.1,
            },
        },
        _ => Signal::Zeros,
    };

    let secs = get_int(hwnd, IDC_TIMEOUT);
    if secs == 0 {
        complain(
            hwnd,
            "The timeout needs to be at least one second.\n\n\
             At zero the headphones would be let go the moment they were \
             taken, which would make StableSound do nothing at all.",
            IDC_TIMEOUT,
        );
        return None;
    }
    cfg.release = match selected(hwnd, IDC_RELEASE) {
        1 => Release::Fixed { secs },
        _ => Release::Idle { secs },
    };

    cfg.wake_on_input = checked(hwnd, IDC_WAKE_INPUT);
    cfg.earcons = checked(hwnd, IDC_EARCONS);

    let percent = get_int(hwnd, IDC_VOLUME);
    if percent > 100 {
        complain(
            hwnd,
            "Tone volume runs from 0 to 100 percent.\n\n\
             Above 100 the tone would clip and come out as a rasp rather \
             than a note.",
            IDC_VOLUME,
        );
        return None;
    }
    cfg.earcon_volume = percent as f32 / 100.0;

    cfg.hotkey = read_hotkey(hwnd, IDC_HOTKEY)?;
    cfg.settings_hotkey_enabled = checked(hwnd, IDC_USE_SET_HOTKEY);
    cfg.settings_hotkey = read_hotkey(hwnd, IDC_SET_HOTKEY)?;
    // Windows gives a combination to one registration only, so the second of
    // two identical hotkeys would never fire. Caught here rather than left to
    // be met later as "the hotkey stopped working". Only a collision when the
    // second one is actually going to be claimed.
    if cfg.settings_hotkey_enabled && cfg.hotkey == cfg.settings_hotkey {
        complain(
            hwnd,
            "Both hotkeys are set to the same combination.\n\n\
             Windows can only give a combination to one thing at a time, so \
             the second would never fire and that function would be left with \
             no key at all.",
            IDC_SET_HOTKEY,
        );
        return None;
    }

    cfg.logging = checked(hwnd, IDC_LOGGING);
    cfg.diagnostics = checked(hwnd, IDC_DIAGNOSTICS);

    // The one combination that cannot work: a signal loud enough to trip our
    // own peak meter defeats idle release. `validate` corrects it; saying so
    // is the point, since the user picked something they will not otherwise
    // get.
    let adjustments = cfg.validate();
    if !adjustments.is_empty() {
        let mut text = String::from("StableSound had to change one thing:\n");
        for adjustment in &adjustments {
            text.push_str(&format!(
                "\n{}\n  Why: {}\n",
                adjustment.what, adjustment.why
            ));
        }
        notify(hwnd, &text);
    }

    // Last, and only once everything else has been accepted: this one writes
    // to the registry rather than into the config we are about to return, so
    // it must not happen on a path that then bails out.
    if checked(hwnd, IDC_STARTUP) != startup::is_enabled() {
        match startup::set(checked(hwnd, IDC_STARTUP)) {
            // Switched off, or switched on by the route we wanted. Nothing to
            // say - the checkbox is the feedback.
            Ok(None) => {}
            Ok(Some(enabled)) if enabled.fell_back.is_none() => {}
            // Switched on, but only by the slower of the two routes. Worth
            // saying, because the difference is whether the first thing the
            // screen reader says is clipped - but said as a note rather than a
            // failure, because StableSound will still start.
            Ok(Some(enabled)) => {
                let route = enabled.method.describe();
                let why = enabled.fell_back.unwrap_or_default();
                notify(
                    hwnd,
                    &format!(
                        "Your settings were saved, and StableSound will start when you sign in.\n\
                         \n\
                         It will start later than it could, though. StableSound asked Windows \
                         for a scheduled task, which runs at sign-in itself, and Windows refused \
                         - so it has used {} instead. Windows runs those only once the desktop \
                         is ready, after every other program set to start with you, which on \
                         this machine has meant about half a minute.\n\
                         \n\
                         Nothing is broken and there is nothing you need to do. Windows said: {}",
                        route, why
                    ),
                );
            }
            Err(e) => {
                // Not worth refusing the whole dialog over. Everything else
                // the user changed is good, and saying so beats silently doing
                // nothing.
                notify(
                    hwnd,
                    &format!(
                        "Your other settings were saved, but StableSound could not change \
                         whether it starts when you sign in.\n\n{e}"
                    ),
                );
            }
        }
    }

    Some(cfg)
}

/// Read and parse one of the two hotkey fields, complaining in place if what
/// was typed does not make sense.
fn read_hotkey(hwnd: HWND, id: i32) -> Option<Hotkey> {
    let typed = get_text(hwnd, id);
    match Hotkey::parse_detailed(&typed) {
        Ok(key) => Some(key),
        Err(fault) => {
            // What went wrong, then how to write one. The Milestone 4 round
            // found the old message readable but short of the examples needed
            // to get it right on the second attempt - so the examples are now
            // the same text everywhere, from `hotkey`.
            complain(
                hwnd,
                &format!("{}\n\n{}", fault.describe(&typed), hotkey::HOW_TO_WRITE),
                id,
            );
            None
        }
    }
}

/// Say what is wrong and put the focus back on the field that is wrong.
///
/// Moving the focus is the part that matters for a screen reader: the message
/// box says what happened, and returning to the control makes the next thing
/// JAWS reads be the field to correct, rather than leaving the user to hunt
/// for it.
fn complain(hwnd: HWND, text: &str, focus_on: i32) {
    problem(Some(hwnd), text);
    unsafe {
        if let Ok(control) = GetDlgItem(Some(hwnd), focus_on) {
            let _ = SetFocus(Some(control));
        }
    }
}

fn notify(hwnd: HWND, text: &str) {
    note(Some(hwnd), text);
}

/// Say that something went wrong, with the exclamation ding in front of it.
///
/// Public because Milestone 6 took the console away. Everything that used to
/// be a `println!` the user could scroll back to now goes to the log - except
/// the failures, which have nowhere else to be seen: logging is off by
/// default, so a hotkey that could not be claimed would otherwise be silent,
/// and a silent primary interface is the worst failure this app has.
///
/// The owner is optional because the earliest of those failures happens before
/// there is a window to own it.
pub fn problem(owner: Option<HWND>, text: &str) {
    show_message(owner, text, MB_ICONEXCLAMATION, PROBLEM_HEADING);
}

/// Say that something happened, with the information ding in front of it.
pub fn note(owner: Option<HWND>, text: &str) {
    show_message(owner, text, MB_ICONINFORMATION, NOTE_HEADING);
}

/// What the heading over the message says, which is how a screen reader user
/// learns which kind of message this is.
///
/// The Milestone 5.1 round asked for this. The heading used to describe the
/// control - "Message, which the arrow keys will read back" - and JAWS then
/// said "read only edit" straight after it, so the words were spent twice on
/// the same fact: "This already tells the user how to interact with the
/// dialogue, so the message box label could rather be something like: 'error,
/// and how to fix'."
///
/// Both start with "What" so the `W` mnemonic in the template holds whichever
/// is showing. That mnemonic is the way back to the text once Tab has moved
/// on to OK.
const PROBLEM_HEADING: &str = "&What went wrong, and how to fix it:";
const NOTE_HEADING: &str = "&What happened:";

/// Say something, in a window whose text can be read back with the arrow keys.
///
/// This is a `MessageBoxW` replacement, and the reason it exists is the
/// Milestone 5 round: the advice shown when a hotkey is refused was judged
/// worth reading in full but too long to take in from one announcement, and a
/// message box offers no way to go back over it short of the JAWS cursor.
/// `IDD_MESSAGE` puts the same text in a read-only edit, which reads on focus
/// the way a message box does and then re-reads a line at a time.
///
/// `sound` keeps the one thing a message box gave that a plain dialog does
/// not: the system's own exclamation or information ding, which arrives before
/// any speech and says which kind of message this is. `heading` says the same
/// thing in words, for the announcement that follows the ding.
///
/// Modal, like the message box it replaces, so the global hotkey is not
/// serviced while it is up. Unchanged from before, and it is a window that
/// exists to be dismissed.
fn show_message(owner: Option<HWND>, text: &str, sound: MESSAGEBOX_STYLE, heading: &str) {
    unsafe {
        let _ = MessageBeep(sound);
    }
    // Edit controls want CRLF. A bare newline reaches one as a stray control
    // character rather than a line break, which the screen reader then has to
    // read around.
    let shown = Message {
        heading: wide(heading),
        body: wide(&text.replace('\n', "\r\n")),
    };
    unsafe {
        let Ok(instance) = GetModuleHandleW(None) else {
            return;
        };
        DialogBoxParamW(
            Some(instance.into()),
            PCWSTR(IDD_MESSAGE as usize as *const u16),
            owner,
            Some(message_proc),
            LPARAM(&shown as *const Message as isize),
        );
    }
}

/// The two strings `IDD_MESSAGE` needs, through the one `LPARAM` a dialog
/// template gets. Borrowed for the life of the modal call, which is entirely
/// inside `show_message`.
struct Message {
    heading: Vec<u16>,
    body: Vec<u16>,
}

unsafe extern "system" fn message_proc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> isize {
    match message {
        WM_INITDIALOG => {
            let shown = &*(lparam.0 as *const Message);
            SetDlgItemTextW(hwnd, IDC_MESSAGE_LABEL, PCWSTR(shown.heading.as_ptr())).ok();
            SetDlgItemTextW(hwnd, IDC_MESSAGE, PCWSTR(shown.body.as_ptr())).ok();
            // Non-zero: the first tab stop is the message itself, which is
            // where the focus belongs - it is the whole point of the window.
            1
        }
        WM_COMMAND => {
            let id = (wparam.0 & 0xFFFF) as i32;
            if id == IDOK.0 || id == IDCANCEL.0 {
                let _ = EndDialog(hwnd, id as isize);
                return 1;
            }
            0
        }
        _ => 0,
    }
}

// --- thin wrappers over the dialog API ------------------------------------
//
// Named for what they do to a setting rather than for the message they send,
// so `populate` and `read` read as the two halves of one mapping.

fn add_item(hwnd: HWND, id: i32, text: &str) {
    let text = wide(text);
    unsafe {
        SendDlgItemMessageW(
            hwnd,
            id,
            CB_ADDSTRING,
            WPARAM(0),
            LPARAM(text.as_ptr() as isize),
        );
    }
}

fn select(hwnd: HWND, id: i32, index: usize) {
    unsafe {
        SendDlgItemMessageW(hwnd, id, CB_SETCURSEL, WPARAM(index), LPARAM(0));
    }
}

fn selected(hwnd: HWND, id: i32) -> usize {
    let result = unsafe { SendDlgItemMessageW(hwnd, id, CB_GETCURSEL, WPARAM(0), LPARAM(0)) };
    // CB_ERR is -1, meaning nothing is selected. Treat it as the first entry,
    // which is the safe default for every list here.
    if result.0 < 0 {
        0
    } else {
        result.0 as usize
    }
}

fn check(hwnd: HWND, id: i32, on: bool) {
    unsafe {
        let _ = CheckDlgButton(hwnd, id, if on { BST_CHECKED } else { BST_UNCHECKED });
    }
}

fn checked(hwnd: HWND, id: i32) -> bool {
    unsafe { IsDlgButtonChecked(hwnd, id) == BST_CHECKED.0 }
}

fn set_int(hwnd: HWND, id: i32, value: u32) {
    unsafe {
        let _ = SetDlgItemInt(hwnd, id, value, false);
    }
}

fn get_int(hwnd: HWND, id: i32) -> u32 {
    unsafe { GetDlgItemInt(hwnd, id, None, false) }
}

fn set_text(hwnd: HWND, id: i32, text: &str) {
    let text = wide(text);
    unsafe {
        let _ = SetDlgItemTextW(hwnd, id, PCWSTR(text.as_ptr()));
    }
}

fn get_text(hwnd: HWND, id: i32) -> String {
    let mut buffer = [0u16; 128];
    let len = unsafe { GetDlgItemTextW(hwnd, id, &mut buffer) } as usize;
    String::from_utf16_lossy(&buffer[..len]).trim().to_string()
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Offer a message to the dialog. True if the dialog consumed it.
///
/// This one call is the whole of the dialog's keyboard behaviour - Tab and
/// Shift+Tab, the arrow keys within a group, mnemonics, Enter for OK and
/// Escape for Cancel. Messages that belong to any other window are declined,
/// which is what lets the hotkey and the tray go on working while it is open.
pub fn is_dialog_message(dialog: HWND, message: &MSG) -> bool {
    unsafe { IsDialogMessageW(dialog, message) }.as_bool()
}
