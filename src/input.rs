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
//! It is a heuristic, and it is wrong in three places:
//!
//! - A mouse click or wheel with no movement reads as keyboard. Acceptable:
//!   unlike a trackpad brush, clicking is deliberate.
//! - A trackpad touch too light to move the pointer reads as keyboard. In
//!   practice a brush that registers at all moves it.
//! - A keypress in the same poll as a pointer move is attributed to the mouse
//!   and missed. Worst case, waking waits for the next keypress.
//!
//! Deliberately *not* solved by polling `GetAsyncKeyState` across the key
//! range. That would be exact, but sweeping every virtual key code in a loop is
//! a textbook keylogger signature - worse for Milestone 6 than the hook already
//! rejected above.

use windows::Win32::Foundation::POINT;
use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};
use windows::Win32::UI::WindowsAndMessaging::GetCursorPos;

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
}

impl InputWatcher {
    pub fn new() -> Self {
        InputWatcher {
            last_tick: current_tick(),
            last_cursor: cursor(),
        }
    }

    pub fn poll(&mut self) -> Input {
        let tick = current_tick();
        let cursor = cursor();
        let moved = cursor.x != self.last_cursor.x || cursor.y != self.last_cursor.y;
        self.last_cursor = cursor;

        if tick == self.last_tick {
            // No input at all. A pointer that moved without the input
            // timestamp advancing was moved by software, not by the user.
            return Input::NONE;
        }
        self.last_tick = tick;

        Input {
            any: true,
            keyboard: !moved,
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
    use super::Input;

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
}
