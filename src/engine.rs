//! The state machine.
//!
//! Runs on its own thread because WASAPI objects are not `Send`: that thread
//! owns every COM object, and the outside world talks to it over channels.
//!
//! # Intent versus stream
//!
//! Two separate things, and keeping them apart is what makes the app robust:
//!
//! - **Intent** is whether keep-alive *should* be running. Only the user, or a
//!   release timeout, changes it.
//! - **Stream** is whether a WASAPI client is actually open right now.
//!
//! Milestone 2 testing conflated them, and that was the Test 5 bug: when the
//! headphones disconnected, the render call failed, the engine stopped, and it
//! never came back. The user had to type `on` again. Now a lost device clears
//! the stream but leaves intent alone, so the engine simply reopens - on the
//! new default device if that is what changed.
//!
//! # What starts keep-alive
//!
//! Explicit request, or user input when `wake_on_input` is set. **Never
//! detected audio.** PLAN.md's original diagram had audio as a trigger, which
//! would defeat the point: after releasing the headset for the phone, the next
//! word JAWS spoke would take it straight back. Audio only postpones release.
//!
//! Input is different in kind: it is not a consequence of our own output, so it
//! cannot form that loop.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

use crate::audio::device::{self, OpenDevice};
use crate::audio::keepalive::KeepAlive;
use crate::audio::meter::Meter;
use crate::config::{Config, DeviceSelector, Release};
use crate::input::InputWatcher;
use crate::log::Log;

/// How often the engine wakes. Fast enough to keep the audio buffer fed and to
/// notice input promptly, slow enough to stay invisible on a CPU graph.
const TICK: Duration = Duration::from_millis(100);

/// How often to check whether the default output changed underneath us.
const DEVICE_CHECK: Duration = Duration::from_secs(1);

/// How long to wait before retrying a device that would not open. Bluetooth
/// reconnects take a few seconds, so retrying faster just spams the log.
const RETRY_DELAY: Duration = Duration::from_secs(2);

pub enum Command {
    Toggle,
    On,
    Off,
    /// Apply new settings. Restarts the stream if currently running.
    Reload(Box<Config>),
    Quit,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum StopReason {
    Requested,
    IdleTimeout,
    FixedTimeout,
}

impl StopReason {
    fn describe(&self) -> &'static str {
        match self {
            StopReason::Requested => "asked to stop",
            StopReason::IdleTimeout => "idle timeout reached",
            StopReason::FixedTimeout => "fixed timer expired",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Started {
        device: String,
        signal: String,
    },
    /// Started because the user touched the keyboard or mouse.
    WokenByInput {
        device: String,
    },
    Stopped {
        reason: StopReason,
    },
    /// The stream was lost but intent stands, so we are retrying.
    Interrupted {
        message: String,
    },
    /// Reopened on a different device than before.
    Moved {
        device: String,
    },
    /// A named device was not found, so the default was used instead.
    Substituted {
        wanted: String,
        used: String,
    },
    Error {
        message: String,
    },
}

/// Everything that exists only while a stream is actually open.
struct Active {
    device: OpenDevice,
    keepalive: KeepAlive,
    meter: Meter,
}

/// Engine state. Timers live here rather than in `Active` so that losing and
/// reopening a device does not silently restart the release countdown.
struct Runtime {
    /// Should keep-alive be running?
    intent: bool,
    /// May user input turn intent back on?
    ///
    /// Cleared when the user switches off by hand, so deliberately releasing
    /// the headset for the phone is not undone by the next keypress. An
    /// automatic release leaves it set, because that just means "you stopped
    /// for a bit".
    armed: bool,
    stream: Option<Active>,
    since_intent: Instant,
    last_audio: Instant,
    next_retry: Instant,
    /// So a device that stays away does not fill the log with retry failures.
    retry_reported: bool,
    /// Set when intent came from user input rather than an explicit request,
    /// so the next successful open can say so.
    started_by_input: bool,
    last_device_name: Option<String>,
    last_device_check: Instant,
}

pub struct Handle {
    pub commands: Sender<Command>,
    pub events: Receiver<Event>,
    pub thread: JoinHandle<()>,
}

pub fn spawn(config: Config, log: Log) -> Handle {
    let (cmd_tx, cmd_rx) = std::sync::mpsc::channel();
    let (evt_tx, evt_rx) = std::sync::mpsc::channel();

    let thread = thread::spawn(move || {
        unsafe {
            let _ = CoInitializeEx(None, COINIT_MULTITHREADED);
        }
        run(config, log, cmd_rx, evt_tx);
        unsafe {
            CoUninitialize();
        }
    });

    Handle {
        commands: cmd_tx,
        events: evt_rx,
        thread,
    }
}

fn run(mut config: Config, log: Log, commands: Receiver<Command>, events: Sender<Event>) {
    let now = Instant::now();
    let mut rt = Runtime {
        intent: false,
        armed: false,
        stream: None,
        since_intent: now,
        last_audio: now,
        next_retry: now,
        retry_reported: false,
        started_by_input: false,
        last_device_name: None,
        last_device_check: now,
    };
    let mut input = InputWatcher::new();

    loop {
        // --- commands -----------------------------------------------------
        match commands.try_recv() {
            Ok(Command::Quit) | Err(TryRecvError::Disconnected) => {
                set_intent_off(&mut rt, StopReason::Requested, &log, &events);
                break;
            }
            Ok(Command::Toggle) => {
                if rt.intent {
                    set_intent_off(&mut rt, StopReason::Requested, &log, &events);
                } else {
                    set_intent_on(&mut rt);
                }
            }
            Ok(Command::On) => set_intent_on(&mut rt),
            Ok(Command::Off) => set_intent_off(&mut rt, StopReason::Requested, &log, &events),
            Ok(Command::Reload(new_config)) => {
                config = *new_config;
                log.write("settings reloaded");
                // Drop any open stream so the new signal and device take
                // effect. Intent is untouched, so it reopens immediately.
                if rt.stream.take().is_some() {
                    rt.next_retry = Instant::now();
                }
            }
            Err(TryRecvError::Empty) => {}
        }

        let now = Instant::now();

        // --- user presence -------------------------------------------------
        // Poll unconditionally, even when the setting is off. Short-circuiting
        // on the config leaves the watcher's baseline frozen, so switching the
        // setting on later compares against an ancient timestamp and fires a
        // spurious wake immediately.
        let had_input = input.seen_input();
        if config.wake_on_input && had_input {
            if rt.intent {
                // Somebody is here and typing, so do not release out from
                // under them just because nothing happens to be speaking.
                rt.last_audio = now;
            } else if rt.armed {
                log.write("woken by keyboard or mouse input");
                set_intent_on(&mut rt);
                rt.started_by_input = true;
            }
        }

        if rt.intent {
            // --- open, or reopen after a loss ------------------------------
            if rt.stream.is_none() && now >= rt.next_retry {
                try_open(&mut rt, &config, &log, &events, now);
            }

            // --- feed the buffer -------------------------------------------
            if let Some(active) = rt.stream.as_mut() {
                if let Err(e) = active.keepalive.pump() {
                    // Almost always the device going away. Keep intent, drop
                    // the stream, and let the retry path pick up whatever
                    // Windows switches to. This is the Test 5 fix.
                    let message = format!("audio stream lost: {e}");
                    log.write(&format!("{message} - will retry"));
                    let _ = events.send(Event::Interrupted { message });
                    rt.stream = None;
                    rt.next_retry = now + RETRY_DELAY;
                    rt.retry_reported = false;
                }
            }

            // --- has anything real been playing? ---------------------------
            if matches!(config.release, Release::Idle { .. }) {
                if let Some(active) = rt.stream.as_ref() {
                    if active.meter.peak() > config.audio_threshold {
                        rt.last_audio = now;
                    }
                }
            }

            // --- time to let go? -------------------------------------------
            let expired = match config.release {
                Release::Idle { secs } => {
                    now.duration_since(rt.last_audio) >= Duration::from_secs(u64::from(secs))
                }
                Release::Fixed { secs } => {
                    now.duration_since(rt.since_intent) >= Duration::from_secs(u64::from(secs))
                }
            };
            if expired {
                let reason = match config.release {
                    Release::Idle { .. } => StopReason::IdleTimeout,
                    Release::Fixed { .. } => StopReason::FixedTimeout,
                };
                set_intent_off(&mut rt, reason, &log, &events);
                // An automatic release is not a decision to stay off.
                rt.armed = true;
                thread::sleep(TICK);
                continue;
            }

            // --- follow the default output if it changes -------------------
            if config.device == DeviceSelector::Default
                && now.duration_since(rt.last_device_check) >= DEVICE_CHECK
            {
                rt.last_device_check = now;
                if let (Some(active), Ok(current)) =
                    (rt.stream.as_ref(), device::default_output_id())
                {
                    if current != active.device.id {
                        log.write("default output changed, moving keep-alive");
                        rt.stream = None;
                        rt.next_retry = now;
                    }
                }
            }
        }

        thread::sleep(TICK);
    }

    log.write("engine stopped");
}

fn set_intent_on(rt: &mut Runtime) {
    if rt.intent {
        return;
    }
    let now = Instant::now();
    rt.intent = true;
    rt.armed = true;
    rt.since_intent = now;
    rt.last_audio = now;
    rt.next_retry = now;
    rt.retry_reported = false;
    rt.started_by_input = false;
}

fn set_intent_off(rt: &mut Runtime, reason: StopReason, log: &Log, events: &Sender<Event>) {
    if !rt.intent {
        return;
    }
    rt.intent = false;
    // Switching off by hand also disarms input waking, so releasing the headset
    // for the phone actually sticks.
    if reason == StopReason::Requested {
        rt.armed = false;
    }

    let held = rt.since_intent.elapsed().as_secs();
    // Dropping releases the IAudioClient, which is what frees the headset.
    let had_stream = rt.stream.take().is_some();
    rt.last_device_name = None;

    log.write(&format!(
        "keep-alive OFF ({}) after {}s{}",
        reason.describe(),
        held,
        if had_stream { " - device released" } else { "" }
    ));
    let _ = events.send(Event::Stopped { reason });
}

fn try_open(rt: &mut Runtime, config: &Config, log: &Log, events: &Sender<Event>, now: Instant) {
    let (open, substituted) = match device::open(&config.device) {
        Ok(v) => v,
        Err(e) => {
            retry_later(rt, now);
            if !rt.retry_reported {
                rt.retry_reported = true;
                let message = format!("could not open an output device: {e}");
                log.write(&format!(
                    "{message} - retrying every {}s",
                    RETRY_DELAY.as_secs()
                ));
                let _ = events.send(Event::Error { message });
            }
            return;
        }
    };

    if substituted {
        if let DeviceSelector::Named(wanted) = &config.device {
            log.write(&format!(
                "device '{wanted}' not found, using default '{}' instead",
                open.name
            ));
            let _ = events.send(Event::Substituted {
                wanted: wanted.clone(),
                used: open.name.clone(),
            });
        }
    }

    let meter = match Meter::open(&open.device) {
        Ok(m) => m,
        Err(e) => {
            retry_later(rt, now);
            if !rt.retry_reported {
                rt.retry_reported = true;
                let message = format!("could not open the audio meter: {e}");
                log.write(&message);
                let _ = events.send(Event::Error { message });
            }
            return;
        }
    };

    let keepalive = match KeepAlive::start(&open.device, config.signal) {
        Ok(k) => k,
        Err(e) => {
            retry_later(rt, now);
            if !rt.retry_reported {
                rt.retry_reported = true;
                let message = format!("could not start keep-alive: {e}");
                log.write(&message);
                let _ = events.send(Event::Error { message });
            }
            return;
        }
    };

    let moved = matches!(&rt.last_device_name, Some(previous) if *previous != open.name);
    log.write(&format!(
        "keep-alive ON  device='{}' signal={} release={}",
        open.name,
        config.signal,
        describe_release(config.release)
    ));
    let _ = events.send(if moved {
        Event::Moved {
            device: open.name.clone(),
        }
    } else if rt.started_by_input {
        Event::WokenByInput {
            device: open.name.clone(),
        }
    } else {
        Event::Started {
            device: open.name.clone(),
            signal: config.signal.to_string(),
        }
    });

    rt.last_device_name = Some(open.name.clone());
    rt.retry_reported = false;
    rt.started_by_input = false;
    rt.stream = Some(Active {
        device: open,
        keepalive,
        meter,
    });
}

fn retry_later(rt: &mut Runtime, now: Instant) {
    rt.next_retry = now + RETRY_DELAY;
}

fn describe_release(release: Release) -> String {
    match release {
        Release::Idle { secs } => format!("idle {secs}s"),
        Release::Fixed { secs } => format!("fixed {secs}s"),
    }
}
