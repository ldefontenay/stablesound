//! Detecting that the user is at the machine, and whether it was the keyboard.
//!
//! Uses `GetLastInputInfo`, which reports when the system last saw input of any
//! kind, rather than installing a low-level keyboard hook.
//!
//! That choice is deliberate. A `WH_KEYBOARD_LL` hook sees the actual keys, and
//! a small unsigned binary that reads every keystroke is precisely the shape
//! antivirus heuristics flag - a problem this project already expects to face
//! when distributing (PLAN.md, Milestone 6). `GetLastInputInfo` returns only a
//! timestamp, never key data, needs no hook, and cannot be mistaken for a
//! keylogger. For "is somebody there?" the timestamp is all we need.
//!
//! # Telling the keyboard from the mouse
//!
//! Milestone 2's round asked for the mouse to stop waking keep-alive: the
//! tester does not use one, and brushing the trackpad by accident takes the
//! headset back off their phone. `GetLastInputInfo` reports one timestamp for
//! all input and cannot say what caused it, so something has to supply that.
//!
//! **Two attempts at inferring it from the cursor failed on hardware**, and the
//! second failure is why this module now measures instead of guessing. The idea
//! was to pair the timestamp with `GetCursorPos`: pointer moved, call it the
//! mouse; pointer still, call it the keyboard. Round one showed that the *end*
//! of a trackpad sweep defeats it - lifting a finger is itself an input event
//! and arrives after the pointer has already stopped, so it reads as a
//! keypress. Round two added a settling window, requiring the pointer to have
//! been still for half a second, and a trackpad sweep still woke keep-alive
//! every time. A pointing device evidently produces input that the cursor
//! position does not account for.
//!
//! So the mouse now reports itself. `watch_pointer` registers for **raw mouse
//! input** - usage page 1, usage 2 - with `RIDEV_INPUTSINK`, so every event
//! from a mouse or trackpad reaches our window even in the background. When one
//! arrives, `note_pointer_event` records the time and nothing else.
//!
//! That is exact where the cursor was not: it sees buttons, wheels and contacts
//! that move the pointer nowhere. And it keeps the property the whole design
//! rests on - **only the mouse is registered, and the payload is never read.**
//! We call neither `GetRawInputData` nor anything else that could say what
//! happened; that a pointing device did something is the entire content. A
//! keystroke never enters this process in any form, so the objection that ruled
//! out a keyboard hook does not apply here.
//!
//! The rule is then simply: input whose timestamp coincides with recent
//! pointing-device activity is the mouse, and anything else is the keyboard.

use std::sync::atomic::{AtomicBool, AtomicU32, Ordering};
use std::time::Duration;

use windows::core::Result as WinResult;
use windows::Win32::Foundation::HWND;
use windows::Win32::System::SystemInformation::GetTickCount;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::Input::{RegisterRawInputDevices, RAWINPUTDEVICE, RIDEV_INPUTSINK};

/// How long after a pointing-device event input still counts as the mouse.
///
/// Not a fudge for imprecision any more - the raw stream is exact about when
/// the mouse moved. It covers the trailing events a pointing device produces
/// around an interaction: a trackpad contact ending, a click landing after a
/// movement, a gesture the system turns into something other than mouse input.
/// A second is generous, and it costs a keyboard-only user nothing at all,
/// because they produce no pointing-device events for it to hang off.
const SETTLING: Duration = Duration::from_millis(1000);

/// Tick count of the most recent pointing-device event.
static POINTER_TICK: AtomicU32 = AtomicU32::new(0);
/// Whether one has ever arrived. Separate from the tick because zero is a
/// legitimate reading in the first millisecond after boot, and because it tells
/// "the mouse has been quiet" apart from "raw input never registered".
static POINTER_SEEN: AtomicBool = AtomicBool::new(false);

/// Ask Windows to report mouse activity to `hwnd`, in the background as well as
/// the foreground.
///
/// Call once, from the thread that owns the window's message queue. `hwnd` must
/// be an ordinary window: a message-only window does not receive raw input.
pub fn watch_pointer(hwnd: HWND) -> WinResult<()> {
    let devices = [RAWINPUTDEVICE {
        // Generic desktop page, mouse. Nothing else is registered, which is
        // the point - see the module note.
        usUsagePage: 0x01,
        usUsage: 0x02,
        // Deliver even when we are not the foreground window. We never are:
        // our only window is hidden.
        dwFlags: RIDEV_INPUTSINK,
        hwndTarget: hwnd,
    }];
    unsafe { RegisterRawInputDevices(&devices, size_of::<RAWINPUTDEVICE>() as u32) }
}

/// Record that a pointing device did something. Called for every `WM_INPUT`.
///
/// Deliberately does not touch the message payload. See the module note: the
/// time is the whole of what we want to know.
pub fn note_pointer_event() {
    POINTER_TICK.store(unsafe { GetTickCount() }, Ordering::Relaxed);
    POINTER_SEEN.store(true, Ordering::Relaxed);
}

fn pointer_tick() -> Option<u32> {
    POINTER_SEEN
        .load(Ordering::Relaxed)
        .then(|| POINTER_TICK.load(Ordering::Relaxed))
}

/// What the user did since the last poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    /// Any input at all: keyboard, mouse, touch, pen.
    pub any: bool,
    /// Input that no pointing device accounts for, so the keyboard.
    pub keyboard: bool,
    /// How long before this input the mouse was last active, in milliseconds -
    /// negative if it reported itself just afterwards, `None` if no
    /// pointing-device event has ever been seen. Carried only so the
    /// diagnostic log can show the number the decision was made on.
    pub pointer_age_ms: Option<i32>,
}

impl Input {
    const NONE: Input = Input {
        any: false,
        keyboard: false,
        pointer_age_ms: None,
    };

    /// Whether this should wake keep-alive, given the user's settings.
    pub fn wakes(self, on_input: bool, on_mouse: bool) -> bool {
        if !on_input {
            return false;
        }
        if on_mouse {
            self.any
        } else {
            self.keyboard
        }
    }

    /// One line for the log when `diagnostics` is on. Two rounds of testing
    /// have now been spent guessing at this decision from the outside; the
    /// third should be able to read what it actually decided, and on what.
    pub fn explain(self) -> String {
        let source = if self.keyboard { "keyboard" } else { "mouse" };
        match self.pointer_age_ms {
            Some(age) => format!("input seen, mouse active {age} ms before it -> {source}"),
            None => format!("input seen, no mouse activity ever recorded -> {source}"),
        }
    }
}

/// Reports whether new input has happened since the last call, and whether it
/// looked like the keyboard. Poll it; the first call establishes a baseline and
/// reports nothing.
pub struct InputWatcher {
    last_tick: u32,
}

impl InputWatcher {
    pub fn new() -> Self {
        InputWatcher {
            last_tick: current_tick(),
        }
    }

    pub fn poll(&mut self) -> Input {
        let tick = current_tick();
        if tick == self.last_tick {
            return Input::NONE;
        }
        self.last_tick = tick;
        classify(tick, pointer_tick())
    }
}

/// The decision, split out so it can be tested without a mouse.
fn classify(input_tick: u32, pointer: Option<u32>) -> Input {
    // Signed on purpose. `WM_INPUT` reaches us a moment after the system
    // stamped the input clock, so the mouse can legitimately look newer than
    // the input it caused, and the subtraction has to survive that as well as
    // the tick counter wrapping.
    let age = pointer.map(|p| input_tick.wrapping_sub(p) as i32);
    let from_pointer = age.is_some_and(|age| age <= SETTLING.as_millis() as i32);
    Input {
        any: true,
        keyboard: !from_pointer,
        pointer_age_ms: age,
    }
}

fn current_tick() -> u32 {
    let mut info = LASTINPUTINFO {
        cbSize: std::mem::size_of::<LASTINPUTINFO>() as u32,
        dwTime: 0,
    };
    unsafe {
        if GetLastInputInfo(&mut info).as_bool() {
            info.dwTime
        } else {
            0
        }
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, Input, SETTLING};

    const KEY: Input = Input {
        any: true,
        keyboard: true,
        pointer_age_ms: None,
    };
    const MOUSE: Input = Input {
        any: true,
        keyboard: false,
        pointer_age_ms: None,
    };

    fn settling() -> u32 {
        SETTLING.as_millis() as u32
    }

    #[test]
    fn keyboard_wakes_but_mouse_does_not_by_default() {
        assert!(KEY.wakes(true, false));
        assert!(!MOUSE.wakes(true, false));
    }

    #[test]
    fn mouse_wakes_when_asked_for() {
        assert!(MOUSE.wakes(true, true));
        assert!(KEY.wakes(true, true));
    }

    #[test]
    fn nothing_wakes_when_waking_is_off() {
        assert!(!KEY.wakes(false, false));
        assert!(!KEY.wakes(false, true));
        assert!(!MOUSE.wakes(false, true));
    }

    #[test]
    fn silence_never_wakes() {
        assert!(!Input::NONE.wakes(true, true));
    }

    #[test]
    fn input_with_no_mouse_behind_it_is_the_keyboard() {
        assert!(classify(500_000, None).keyboard);
        // A mouse last active far longer ago than the settling window has
        // nothing to do with this input.
        assert!(classify(500_000, Some(500_000 - settling() - 1)).keyboard);
    }

    #[test]
    fn input_the_mouse_accounts_for_is_the_mouse() {
        assert!(!classify(500_000, Some(500_000)).keyboard);
        assert!(!classify(500_000, Some(500_000 - settling() + 1)).keyboard);
    }

    #[test]
    fn a_mouse_event_recorded_just_after_the_input_clock_still_counts() {
        // WM_INPUT arrives a moment after the system stamps the input clock,
        // so the pointer tick can be the newer of the two. Read unsigned, that
        // subtraction underflows to a huge positive number and the event comes
        // out as the keyboard - which is how a trackpad kept waking keep-alive.
        let seen = classify(500_000, Some(500_020));
        assert!(
            !seen.keyboard,
            "a mouse event 20 ms late was read as typing"
        );
        assert_eq!(seen.pointer_age_ms, Some(-20));
    }

    #[test]
    fn the_decision_survives_the_tick_counter_wrapping() {
        // GetTickCount wraps roughly every 49 days, and a machine left running
        // will cross it.
        let just_before = u32::MAX - 10;
        let just_after = 10u32;
        assert!(!classify(just_after, Some(just_before)).keyboard);
    }

    #[test]
    fn the_explanation_names_the_number_it_decided_on() {
        assert!(classify(500_000, Some(499_900))
            .explain()
            .contains("100 ms"));
        assert!(classify(500_000, None).explain().contains("no mouse"));
    }
}
