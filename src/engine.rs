//! The state machine.
//!
//! Runs on its own thread because WASAPI objects are not `Send`: that thread
//! owns every COM object, and the outside world talks to it over channels.
//!
//! Deliberately there is **no auto-start on detected audio.** PLAN.md's state
//! diagram sketched one, but it would defeat the app's main purpose: after
//! releasing the headset for the phone, the next word JAWS spoke would grab it
//! straight back. Starting is always explicit; only stopping is automatic.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

use crate::audio::device::{self, OpenDevice};
use crate::audio::keepalive::KeepAlive;
use crate::audio::meter::Meter;
use crate::config::{Config, DeviceSelector, Release};
use crate::log::Log;

/// How often the engine wakes. Fast enough to keep the audio buffer fed and to
/// notice audio promptly, slow enough to stay invisible on a CPU graph.
const TICK: Duration = Duration::from_millis(100);

/// How often to check whether the default output has changed underneath us.
const DEVICE_CHECK: Duration = Duration::from_secs(1);

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
    DeviceLost,
}

impl StopReason {
    fn describe(&self) -> &'static str {
        match self {
            StopReason::Requested => "asked to stop",
            StopReason::IdleTimeout => "idle timeout reached",
            StopReason::FixedTimeout => "fixed timer expired",
            StopReason::DeviceLost => "device went away",
        }
    }
}

#[derive(Clone, Debug)]
pub enum Event {
    Started {
        device: String,
        signal: String,
    },
    Stopped {
        reason: StopReason,
    },
    /// The default output changed while we were running, so we moved.
    Moved {
        device: String,
    },
    /// A named device was not found and the default was used instead.
    Substituted {
        wanted: String,
        used: String,
    },
    Error {
        message: String,
    },
}

/// Everything that exists only while keep-alive is running.
struct Active {
    device: OpenDevice,
    keepalive: KeepAlive,
    meter: Meter,
    started: Instant,
    last_audio: Instant,
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
    let mut active: Option<Active> = None;
    let mut last_device_check = Instant::now();

    loop {
        // --- commands -----------------------------------------------------
        match commands.try_recv() {
            Ok(Command::Quit) | Err(TryRecvError::Disconnected) => {
                if active.is_some() {
                    stop(&mut active, StopReason::Requested, &log, &events);
                }
                break;
            }
            Ok(Command::Toggle) => {
                if active.is_some() {
                    stop(&mut active, StopReason::Requested, &log, &events);
                } else {
                    start(&mut active, &config, &log, &events);
                }
            }
            Ok(Command::On) => {
                if active.is_none() {
                    start(&mut active, &config, &log, &events);
                }
            }
            Ok(Command::Off) => {
                if active.is_some() {
                    stop(&mut active, StopReason::Requested, &log, &events);
                }
            }
            Ok(Command::Reload(new_config)) => {
                let was_running = active.is_some();
                if was_running {
                    stop(&mut active, StopReason::Requested, &log, &events);
                }
                config = *new_config;
                log.write("settings reloaded");
                if was_running {
                    start(&mut active, &config, &log, &events);
                }
            }
            Err(TryRecvError::Empty) => {}
        }

        // --- keep the stream fed and decide whether to release -------------
        if let Some(state) = active.as_mut() {
            if let Err(e) = state.keepalive.pump() {
                let message = format!("keep-alive stopped: {e}");
                log.write(&message);
                let _ = events.send(Event::Error { message });
                stop(&mut active, StopReason::DeviceLost, &log, &events);
                continue;
            }

            let now = Instant::now();

            // Idle mode only: any real audio postpones the release.
            if let Release::Idle { .. } = config.release {
                if state.meter.peak() > config.audio_threshold {
                    state.last_audio = now;
                }
            }

            let expired = match config.release {
                Release::Idle { secs } => {
                    now.duration_since(state.last_audio) >= Duration::from_secs(u64::from(secs))
                }
                Release::Fixed { secs } => {
                    now.duration_since(state.started) >= Duration::from_secs(u64::from(secs))
                }
            };

            if expired {
                let reason = match config.release {
                    Release::Idle { .. } => StopReason::IdleTimeout,
                    Release::Fixed { .. } => StopReason::FixedTimeout,
                };
                stop(&mut active, reason, &log, &events);
                continue;
            }

            // Follow the default output if it changes under us. Only meaningful
            // when tracking the default; a named device should stay put.
            if config.device == DeviceSelector::Default
                && now.duration_since(last_device_check) >= DEVICE_CHECK
            {
                last_device_check = now;
                if let Ok(current_id) = device::default_output_id() {
                    if current_id != state.device.id {
                        log.write("default output changed, moving keep-alive");
                        stop(&mut active, StopReason::Requested, &log, &events);
                        start(&mut active, &config, &log, &events);
                        if let Some(state) = active.as_ref() {
                            let _ = events.send(Event::Moved {
                                device: state.device.name.clone(),
                            });
                        }
                        continue;
                    }
                }
            }
        }

        thread::sleep(TICK);
    }

    log.write("engine stopped");
}

fn start(active: &mut Option<Active>, config: &Config, log: &Log, events: &Sender<Event>) {
    let (open, substituted) = match device::open(&config.device) {
        Ok(v) => v,
        Err(e) => {
            let message = format!("could not open an output device: {e}");
            log.write(&message);
            let _ = events.send(Event::Error { message });
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
            let message = format!("could not open the audio meter: {e}");
            log.write(&message);
            let _ = events.send(Event::Error { message });
            return;
        }
    };

    let keepalive = match KeepAlive::start(&open.device, config.signal) {
        Ok(k) => k,
        Err(e) => {
            let message = format!("could not start keep-alive: {e}");
            log.write(&message);
            let _ = events.send(Event::Error { message });
            return;
        }
    };

    let now = Instant::now();
    log.write(&format!(
        "keep-alive ON  device='{}' signal={} release={}",
        open.name,
        config.signal,
        describe_release(config.release)
    ));
    let _ = events.send(Event::Started {
        device: open.name.clone(),
        signal: config.signal.to_string(),
    });

    *active = Some(Active {
        device: open,
        keepalive,
        meter,
        started: now,
        last_audio: now,
    });
}

fn stop(active: &mut Option<Active>, reason: StopReason, log: &Log, events: &Sender<Event>) {
    if let Some(state) = active.take() {
        let held = state.started.elapsed().as_secs();
        // Dropping releases the IAudioClient, which is what frees the headset.
        drop(state);
        log.write(&format!(
            "keep-alive OFF ({}) after {}s - device released",
            reason.describe(),
            held
        ));
        let _ = events.send(Event::Stopped { reason });
    }
}

fn describe_release(release: Release) -> String {
    match release {
        Release::Idle { secs } => format!("idle {secs}s"),
        Release::Fixed { secs } => format!("fixed {secs}s"),
    }
}
