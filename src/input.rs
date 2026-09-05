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
//!
//! # Waiting for the mouse to speak
//!
//! Raw input was right, and it still woke keep-alive on the third round. The
//! diagnostic log said why, in three consecutive lines as the trackpad was
//! swept: `no mouse activity ever recorded -> keyboard`, the wake, and then
//! `mouse active 0 ms before it -> mouse`. Every event of the sweep bar the
//! first was attributed correctly - and the first is the one that wakes.
//!
//! The two clocks do not tick together. `GetLastInputInfo` is stamped by the
//! system the instant the input lands; `WM_INPUT` has to be queued and
//! dispatched to us afterwards. Poll in the gap between them and the mouse has
//! provably not reported itself yet, so the input looks like a keypress. Once a
//! sweep is under way the pointer tick is never stale again, which is why only
//! the leading event was ever wrong and why the fault survived two rounds of
//! being reasoned about from the outside.
//!
//! So an input that no pointing device accounts for is no longer decided on the
//! spot. It is **held for [`GRACE`]**, and decided when that is up - by which
//! time the `WM_INPUT` that explains it has arrived, if one was coming. Input
//! the mouse already accounts for is still decided immediately; there is
//! nothing left to wait for.
//!
//! The cost falls on the keyboard, which now wakes keep-alive [`GRACE`] later
//! than it did. That is affordable precisely where it lands: waking from a
//! release means reconnecting Bluetooth, which hardware testing puts at around
//! three seconds, so a quarter of a second on the front makes no odds. When
//! keep-alive is already on, the wake path only postpones the idle timer, and a
//! delay there is invisible.

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

/// How long an input no pointing device accounts for is held before it is
/// called the keyboard.
///
/// It has to outlast the gap between the system stamping the input clock and
/// `WM_INPUT` reaching us - see the module note. That gap is milliseconds when
/// it is only queueing, but a trackpad has to decide what a contact meant
/// before it synthesises anything, and a tap that turns out not to be the start
/// of a drag is the slow case. A quarter of a second covers it with room to
/// spare, and the diagnostic log records the actual figure as a negative
/// pointer age, so this can be tightened against measurement rather than
/// against another guess.
const GRACE: Duration = Duration::from_millis(250);

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
    /// How long the decision was held waiting for a pointing device to own up,
    /// in milliseconds; zero if the mouse had already accounted for it. Carried
    /// for the log, so a negative age can be read against the wait that caught
    /// it and `GRACE` tuned on evidence.
    pub held_ms: u32,
}

impl Input {
    const NONE: Input = Input {
        any: false,
        keyboard: false,
        pointer_age_ms: None,
        held_ms: 0,
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
        let held = match self.held_ms {
            0 => String::new(),
            ms => format!(", held {ms} ms"),
        };
        match self.pointer_age_ms {
            Some(age) => format!("input seen{held}, mouse active {age} ms before it -> {source}"),
            None => format!("input seen{held}, no mouse activity ever recorded -> {source}"),
        }
    }
}

/// Reports whether new input has happened since the last call, and whether it
/// looked like the keyboard. Poll it; the first call establishes a baseline and
/// reports nothing.
pub struct InputWatcher {
    last_tick: u32,
    decider: Decider,
}

impl InputWatcher {
    pub fn new() -> Self {
        InputWatcher {
            last_tick: current_tick(),
            decider: Decider::new(),
        }
    }

    pub fn poll(&mut self) -> Input {
        let tick = current_tick();
        let fresh = (tick != self.last_tick).then(|| {
            self.last_tick = tick;
            tick
        });
        self.decider
            .step(fresh, pointer_tick(), unsafe { GetTickCount() })
    }
}

/// An input held back until a pointing device has had its chance to own up.
#[derive(Clone, Copy)]
struct Pending {
    /// The input-clock reading the held decision is about.
    input_tick: u32,
    /// When the hold started, by `GetTickCount`.
    held_since: u32,
}

/// The deferral, split from the clocks so it can be tested without a trackpad.
struct Decider {
    pending: Option<Pending>,
}

impl Decider {
    const fn new() -> Self {
        Decider { pending: None }
    }

    /// Advance by one poll. `fresh` is the input-clock reading if it has moved
    /// since the last call, `pointer` the most recent pointing-device tick, and
    /// `now` the current tick count.
    fn step(&mut self, fresh: Option<u32>, pointer: Option<u32>, now: u32) -> Input {
        if let Some(tick) = fresh {
            let seen = classify(tick, pointer, 0);
            if !seen.keyboard {
                // A pointing device already accounts for it. Nothing to wait
                // for, and anything held is part of the same interaction.
                self.pending = None;
                return seen;
            }
            match &mut self.pending {
                // Still unaccounted for, and the hold already running covers
                // it. Take the newer reading but leave the deadline alone:
                // pushing it back on every keystroke would mean somebody
                // typing steadily never paused long enough to be noticed.
                Some(p) => p.input_tick = tick,
                None => {
                    self.pending = Some(Pending {
                        input_tick: tick,
                        held_since: now,
                    })
                }
            }
        }
        // Resolve whatever is being held once its grace is up - which is the
        // point of the exercise, so it has to happen on polls that brought no
        // new input as well as on those that did.
        match self.pending {
            Some(p) => {
                let held = now.wrapping_sub(p.held_since);
                if held >= GRACE.as_millis() as u32 {
                    self.pending = None;
                    classify(p.input_tick, pointer, held)
                } else {
                    Input::NONE
                }
            }
            None => Input::NONE,
        }
    }
}

/// The decision, split out so it can be tested without a mouse.
fn classify(input_tick: u32, pointer: Option<u32>, held_ms: u32) -> Input {
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
        held_ms,
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
    use super::{classify, Decider, Input, GRACE, SETTLING};

    const KEY: Input = Input {
        any: true,
        keyboard: true,
        pointer_age_ms: None,
        held_ms: 0,
    };
    const MOUSE: Input = Input {
        any: true,
        keyboard: false,
        pointer_age_ms: None,
        held_ms: 0,
    };

    fn settling() -> u32 {
        SETTLING.as_millis() as u32
    }

    fn grace() -> u32 {
        GRACE.as_millis() as u32
    }

    /// `classify` at its old arity, for the tests that predate the hold.
    fn at(input_tick: u32, pointer: Option<u32>) -> Input {
        classify(input_tick, pointer, 0)
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
        assert!(at(500_000, None).keyboard);
        // A mouse last active far longer ago than the settling window has
        // nothing to do with this input.
        assert!(at(500_000, Some(500_000 - settling() - 1)).keyboard);
    }

    #[test]
    fn input_the_mouse_accounts_for_is_the_mouse() {
        assert!(!at(500_000, Some(500_000)).keyboard);
        assert!(!at(500_000, Some(500_000 - settling() + 1)).keyboard);
    }

    #[test]
    fn a_mouse_event_recorded_just_after_the_input_clock_still_counts() {
        // WM_INPUT arrives a moment after the system stamps the input clock,
        // so the pointer tick can be the newer of the two. Read unsigned, that
        // subtraction underflows to a huge positive number and the event comes
        // out as the keyboard - which is how a trackpad kept waking keep-alive.
        let seen = at(500_000, Some(500_020));
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
        assert!(!at(just_after, Some(just_before)).keyboard);
    }

    // --- holding an unattributed input --------------------------------------
    //
    // The third hardware round's failure, reproduced: the trackpad is swept,
    // the input clock is stamped, and `WM_INPUT` has not arrived yet.

    #[test]
    fn a_trackpad_sweep_no_longer_wakes_on_its_leading_event() {
        let mut d = Decider::new();
        // The sweep starts. The mouse has said nothing at all yet - this is
        // exactly the log's "no mouse activity ever recorded".
        assert_eq!(d.step(Some(1000), None, 1000), Input::NONE, "decided early");
        // WM_INPUT lands 40 ms later, stamped at the moment of the input.
        let seen = d.step(None, Some(1040), 1000 + grace());
        assert!(!seen.keyboard, "the sweep was still read as typing");
        assert!(seen.any);
    }

    #[test]
    fn a_keypress_is_still_the_keyboard_once_the_hold_is_up() {
        let mut d = Decider::new();
        assert_eq!(d.step(Some(1000), None, 1000), Input::NONE);
        let seen = d.step(None, None, 1000 + grace());
        assert!(seen.keyboard);
        assert!(seen.any);
        assert_eq!(seen.held_ms, grace());
    }

    #[test]
    fn nothing_is_reported_while_the_hold_is_still_running() {
        let mut d = Decider::new();
        d.step(Some(1000), None, 1000);
        for elapsed in [1, grace() / 2, grace() - 1] {
            assert_eq!(
                d.step(None, None, 1000 + elapsed),
                Input::NONE,
                "reported after only {elapsed} ms"
            );
        }
    }

    #[test]
    fn input_the_mouse_already_accounts_for_is_not_held_at_all() {
        let mut d = Decider::new();
        let seen = d.step(Some(1000), Some(1000), 1000);
        assert!(!seen.keyboard);
        assert_eq!(seen.held_ms, 0, "waited on a decision already made");
    }

    #[test]
    fn steady_typing_is_noticed_without_waiting_for_a_pause() {
        // Every poll brings fresh input, so a hold that restarted on each one
        // would never expire and keep-alive would never wake at all.
        let mut d = Decider::new();
        let mut woken = false;
        for step in 0..10u32 {
            let now = 1000 + step * 100;
            if d.step(Some(now), None, now).keyboard {
                woken = true;
            }
        }
        assert!(woken, "typing steadily never woke keep-alive");
    }

    #[test]
    fn a_sweep_that_keeps_going_stays_the_mouse() {
        let mut d = Decider::new();
        let mut woke_as_keyboard = false;
        for step in 0..10u32 {
            let now = 1000 + step * 100;
            // The pointer reports itself 40 ms behind the input clock
            // throughout, as it did on hardware.
            let pointer = (step > 0).then(|| now - 60);
            if d.step(Some(now), pointer, now).keyboard {
                woke_as_keyboard = true;
            }
        }
        assert!(!woke_as_keyboard, "a sweep in progress woke keep-alive");
    }

    #[test]
    fn the_hold_survives_the_tick_counter_wrapping() {
        let mut d = Decider::new();
        let before = u32::MAX - 10;
        assert_eq!(d.step(Some(before), None, before), Input::NONE);
        // `now` has wrapped past zero while the input was held.
        let now = before.wrapping_add(grace());
        assert!(now < before, "the test no longer crosses the wrap");
        let seen = d.step(None, None, now);
        assert!(seen.any, "the hold was lost across the wrap");
        assert_eq!(seen.held_ms, grace());
    }

    #[test]
    fn the_explanation_names_the_number_it_decided_on() {
        assert!(at(500_000, Some(499_900)).explain().contains("100 ms"));
        assert!(at(500_000, None).explain().contains("no mouse"));
    }

    #[test]
    fn the_explanation_reports_the_wait_that_caught_a_late_mouse() {
        // The pair of numbers a fourth round would need: how late the pointer
        // was, and how long we waited for it.
        let held = classify(500_000, Some(500_040), grace()).explain();
        assert!(held.contains("-40 ms"), "{held}");
        assert!(held.contains(&format!("held {} ms", grace())), "{held}");
        // Nothing was waited for, so nothing is said about waiting.
        assert!(!at(500_000, Some(500_000)).explain().contains("held"));
    }
}
