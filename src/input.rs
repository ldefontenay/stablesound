//! Detecting that the user is at the machine.
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
//! # Why this module no longer tells the keyboard from the mouse
//!
//! It used to try, because the Milestone 2 round asked for the mouse to stop
//! waking keep-alive: the tester does not use one, and brushing the trackpad by
//! accident took the headset back off their phone. `GetLastInputInfo` reports
//! one timestamp for all input and cannot say what caused it, so three
//! mechanisms were built to supply the missing half, and **all three failed on
//! real hardware**:
//!
//! 1. Pair the timestamp with `GetCursorPos`: pointer moved, call it the
//!    mouse. Defeated by the *end* of a sweep - lifting a finger is itself an
//!    input event, arriving after the pointer has already stopped.
//! 2. The same, with a settling window requiring the pointer to have been still
//!    for half a second. A sweep still woke keep-alive every time.
//! 3. Raw mouse input, so the mouse reported itself rather than being inferred.
//!    That mechanism was correct - the diagnostic log showed events being
//!    attributed properly - and it still failed, because `GetLastInputInfo` is
//!    stamped the instant input lands while `WM_INPUT` must be queued and
//!    dispatched to us afterwards. Holding an unaccounted-for input for 250 ms
//!    to close that race was tried in the Milestone 4 round. The trackpad woke
//!    keep-alive anyway.
//!
//! Four hardware rounds, three mechanisms, and the tester's verdict after the
//! fourth: "Please give up on this feature now and remove all code that was
//! there to facilitate it. It was just a nice-to-have anyway."
//!
//! So **all input wakes keep-alive**, and nothing here tries to say what caused
//! it. What that costs is bounded and known: a trackpad brush takes the headset
//! back from the phone, and the hotkey takes it straight off again - and a
//! manual switch-off still disarms waking entirely, so a deliberate handover to
//! the phone is never undone by a stray finger.
//!
//! What it buys is the whole of the machinery above: no raw input registration,
//! no `WM_INPUT` for every pointer movement system-wide, no two-clock race, and
//! no held decision delaying the keyboard by a quarter of a second. Waking is
//! immediate again.

use windows::Win32::UI::Input::KeyboardAndMouse::{GetLastInputInfo, LASTINPUTINFO};

/// Reports whether new input has happened since the last call. Poll it; the
/// first call establishes a baseline and reports nothing.
pub struct InputWatcher {
    last_tick: u32,
}

impl InputWatcher {
    pub fn new() -> Self {
        InputWatcher {
            last_tick: current_tick(),
        }
    }

    /// True if the system has seen input since the previous poll.
    pub fn poll(&mut self) -> bool {
        self.step(current_tick())
    }

    /// The decision, split from the clock so it can be tested without a
    /// keyboard.
    fn step(&mut self, tick: u32) -> bool {
        if tick == self.last_tick {
            return false;
        }
        self.last_tick = tick;
        true
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
    use super::InputWatcher;

    /// A watcher with a known baseline, standing in for one built by `new` at
    /// a moment when the input clock read `tick`.
    fn watching(tick: u32) -> InputWatcher {
        InputWatcher { last_tick: tick }
    }

    #[test]
    fn an_unmoved_clock_is_not_input() {
        // The first poll after startup must report nothing, or keep-alive
        // would come on by itself the moment the app launched.
        assert!(!watching(1000).step(1000));
    }

    #[test]
    fn a_moved_clock_is_input() {
        assert!(watching(1000).step(1200));
    }

    #[test]
    fn the_same_input_is_not_reported_twice() {
        let mut watcher = watching(1000);
        assert!(watcher.step(1200));
        assert!(!watcher.step(1200), "one keypress woke keep-alive twice");
    }

    #[test]
    fn the_baseline_moves_across_a_wrap() {
        // GetTickCount wraps roughly every 49 days and a machine left running
        // will cross it. Equality is the whole test now, so a wrap is only a
        // different pair of numbers - but pin it, because the version of this
        // module that did arithmetic here got the wrap wrong twice.
        let mut watcher = watching(u32::MAX - 10);
        assert!(watcher.step(10));
        assert!(!watcher.step(10));
    }
}
