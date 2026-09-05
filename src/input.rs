//! Detecting that the user is at the machine.
//!
//! Uses `GetLastInputInfo`, which reports when the system last saw keyboard or
//! mouse input, rather than installing a low-level keyboard hook.
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
//! Milestone 2's hardware round asked for the mouse to stop waking keep-alive:
//! the tester does not use one, and brushing the trackpad by accident takes the
//! headset back off their phone.
//!
//! `GetLastInputInfo` reports one timestamp for all input and cannot say what
//! caused it. So the source is inferred by pairing it with `GetCursorPos`: if
//! the timestamp advanced and the pointer also moved, call it the mouse; if it
//! advanced and the pointer sat still, call it the keyboard. That keeps both
//! properties that mattered above - a timestamp and a cursor coordinate are
//! still not key data, and there is still no hook.
//!
//! Comparing against only the *previous* poll is not enough, and the Milestone
//! 3 hardware round proved it: `mouse off` did not work, and a deliberate
//! trackpad sweep still woke keep-alive every time. The sweep itself was
//! classified correctly - what was not was the moment the finger left the pad.
//! Lifting off a precision trackpad is itself an input event, it arrives a few
//! milliseconds after the pointer has already stopped, and so the poll that saw
//! it found the timestamp advanced and the pointer still: keyboard.
//!
//! So the pointer has to be still for a while, not merely still since the last
//! poll. `SETTLING` below is that window. Anything arriving within it of a
//! pointer movement is called the mouse.
//!
//! It is a heuristic, and it is wrong in three places:
//!
//! - A mouse click or wheel more than `SETTLING` after the last movement reads
//!   as keyboard. Acceptable: unlike a trackpad brush, clicking is deliberate.
//! - A trackpad touch too light to move the pointer reads as keyboard. In
//!   practice a brush that registers at all moves it.
//! - A keypress within `SETTLING` of a pointer move is attributed to the mouse
//!   and missed. Worst case, waking waits for the next keypress - and the user
//!   this defaults for does not use a pointing device at all.
//!
//! Deliberately *not* solved by polling `GetAsyncKeyState` across the key
//! range. That would be exact, but sweeping every virtual key code in a loop is
//! a textbook keylogger signature - worse for Milestone 6 than the hook already
//! rejected above.

use std::time::{Duration, Instant};

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

/// How long after a pointer movement input still counts as the mouse.
///
/// Sized for the trailing events a pointing device emits once the pointer has
/// stopped - a trackpad finger lift, a click at the end of a movement. Long
/// enough to cover them, short enough that typing after using the mouse is not
/// ignored for any noticeable time.
const SETTLING: Duration = Duration::from_millis(500);

/// What the user did since the last poll.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Input {
    /// Any input at all: keyboard, mouse, touch, pen.
    pub any: bool,
    /// Input that did not move the pointer, so most likely the keyboard.
    /// See the module note for where this inference is wrong.
    pub keyboard: bool,
}

impl Input {
    const NONE: Input = Input {
        any: false,
        keyboard: false,
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
}

/// Reports whether new input has happened since the last call, and whether it
/// looked like the keyboard. Poll it; the first call establishes a baseline and
/// reports nothing.
pub struct InputWatcher {
    last_tick: u32,
    last_cursor: POINT,
    /// When the pointer was last seen in a new place. `None` until it moves.
    last_moved: Option<Instant>,
}

impl InputWatcher {
    pub fn new() -> Self {
        InputWatcher {
            last_tick: current_tick(),
            last_cursor: cursor(),
            last_moved: None,
        }
    }

    pub fn poll(&mut self) -> Input {
        self.observe(Instant::now(), current_tick(), cursor())
    }

    /// The whole decision, with the three readings passed in so it can be
    /// tested. `poll` is only this plus the calls that take them.
    fn observe(&mut self, now: Instant, tick: u32, cursor: POINT) -> Input {
        if cursor.x != self.last_cursor.x || cursor.y != self.last_cursor.y {
            self.last_cursor = cursor;
            self.last_moved = Some(now);
        }

        if tick == self.last_tick {
            // No input at all. A pointer that moved without the input
            // timestamp advancing was moved by software, not by the user.
            return Input::NONE;
        }
        self.last_tick = tick;

        let settling = self
            .last_moved
            .is_some_and(|at| now.duration_since(at) < SETTLING);

        Input {
            any: true,
            keyboard: !settling,
        }
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

fn cursor() -> POINT {
    let mut point = POINT { x: 0, y: 0 };
    unsafe {
        let _ = GetCursorPos(&mut point);
    }
    point
}

#[cfg(test)]
mod tests {
    use super::{Input, InputWatcher, POINT, SETTLING};
    use std::time::{Duration, Instant};

    /// A watcher with a known baseline, so tests can drive `observe` directly
    /// instead of moving a real mouse.
    fn watcher(at: Instant) -> (InputWatcher, Instant) {
        (
            InputWatcher {
                last_tick: 1_000,
                last_cursor: POINT { x: 100, y: 100 },
                last_moved: None,
            },
            at,
        )
    }

    const KEY: Input = Input {
        any: true,
        keyboard: true,
    };
    const MOUSE: Input = Input {
        any: true,
        keyboard: false,
    };

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
    fn a_still_pointer_and_a_new_timestamp_is_the_keyboard() {
        let (mut w, t0) = watcher(Instant::now());
        let seen = w.observe(t0, 1_001, POINT { x: 100, y: 100 });
        assert_eq!(seen, KEY);
    }

    #[test]
    fn a_moving_pointer_is_the_mouse() {
        let (mut w, t0) = watcher(Instant::now());
        let seen = w.observe(t0, 1_001, POINT { x: 140, y: 100 });
        assert_eq!(seen, MOUSE);
    }

    #[test]
    fn nothing_is_reported_when_the_timestamp_stands_still() {
        let (mut w, t0) = watcher(Instant::now());
        // A pointer that moves without the input clock advancing was moved by
        // software, not by a person.
        assert_eq!(w.observe(t0, 1_000, POINT { x: 300, y: 300 }), Input::NONE);
    }

    #[test]
    fn the_lift_at_the_end_of_a_trackpad_sweep_is_not_the_keyboard() {
        // The Milestone 3 bug. The sweep is classified correctly; the finger
        // coming off the pad is a further input event, arriving after the
        // pointer has already stopped. Judged on the previous poll alone it
        // looks exactly like a keypress, and it woke keep-alive every time.
        let (mut w, t0) = watcher(Instant::now());

        assert_eq!(w.observe(t0, 1_001, POINT { x: 140, y: 120 }), MOUSE);
        let t1 = t0 + Duration::from_millis(100);
        assert_eq!(w.observe(t1, 1_002, POINT { x: 190, y: 160 }), MOUSE);

        // Pointer now stationary, but the lift lands in the next poll.
        let t2 = t1 + Duration::from_millis(100);
        assert_eq!(
            w.observe(t2, 1_003, POINT { x: 190, y: 160 }),
            MOUSE,
            "the finger lift was attributed to the keyboard"
        );
        assert!(!w
            .observe(t2, 1_004, POINT { x: 190, y: 160 })
            .wakes(true, false));
    }

    #[test]
    fn typing_after_the_pointer_settles_still_counts() {
        let (mut w, t0) = watcher(Instant::now());
        assert_eq!(w.observe(t0, 1_001, POINT { x: 140, y: 120 }), MOUSE);

        let later = t0 + SETTLING + Duration::from_millis(1);
        assert_eq!(w.observe(later, 1_002, POINT { x: 140, y: 120 }), KEY);
    }
}
