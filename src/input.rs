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

use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Reports whether new keyboard or mouse input has happened since the last
/// call. Poll it; the first call establishes a baseline and reports nothing.
pub struct InputWatcher {
    last_tick: Option<u32>,
}

impl InputWatcher {
    pub fn new() -> Self {
        InputWatcher {
            last_tick: Some(current_tick()),
        }
    }

    /// True if there has been input since the previous poll.
    pub fn seen_input(&mut self) -> bool {
        let tick = current_tick();
        match self.last_tick {
            Some(previous) if previous == tick => false,
            _ => {
                self.last_tick = Some(tick);
                true
            }
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
