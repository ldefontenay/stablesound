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
//! cannot form that loop. By default only *keyboard* input counts - see
//! `input` for why, and for how the two are told apart.
//!
//! # Earcons and release
//!
//! The off-tone has to be heard before the device is let go, which means the
//! release path blocks briefly while the stream drains. Playing it after the
//! release instead would mean reopening the device, taking the headset back
//! off the phone a moment after handing it over.
//!
//! Tones mark what the *user* did, never what the engine did on its own. That
//! is the Milestone 3 hardware verdict, and it was unambiguous: an idle release
//! and the wake that follows it happen many times an hour, and a tone on each
//! was "annoying". They are also the transitions the user has no reason to
//! hear: nothing has changed about what they can do, only about which device
//! currently holds the headset. A hotkey press is the opposite - it has no
//! other feedback at all, so it must be answered. See `Source`.

use std::sync::mpsc::{Receiver, Sender, TryRecvError};
use std::thread::{self, JoinHandle};
use std::time::{Duration, Instant};

use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_MULTITHREADED};

use crate::audio::device::{self, OpenDevice};
use crate::audio::earcon;
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

/// Ceiling on how long a release waits for the off-tone to be heard.
///
/// The tone itself is under 150 ms, plus up to another 150 ms of already
/// queued audio ahead of it, so this is roughly three times what it should
/// ever need. It exists because draining blocks this thread: a device that has
/// stopped consuming must not be able to wedge the engine, and a release that
/// is late is far worse than an earcon that is cut short.
const EARCON_TIMEOUT: Duration = Duration::from_secs(1);

/// Who asked for keep-alive to come on.
///
/// The only thing this changes is whether the on-tone plays. It is a
/// distinction the user can hear, so it is worth a type rather than a bare
/// bool at the call sites.
#[derive(Clone, Copy, PartialEq, Eq)]
enum Source {
    /// The hotkey or the tray.
    User,
    /// The user touched the keyboard and keep-alive re-armed itself.
    Input,
}

/// What the message loop asks the engine for.
///
/// There is no `On` or `Off`. There was, for as long as the console harness
/// let the tester type them; the hotkey, the tray icon and the tray menu have
/// only ever toggled, and Milestone 6 removed the console.
pub enum Command {
    Toggle,
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

/// What the engine tells the message loop.
///
/// Two variants, because two is all the message loop can act on: the tray
/// icon reads either "Headphones awake" or "Headphones free".
///
/// It used to have seven, carrying device names, signal names, stop reasons
/// and error text. Every one of those payloads existed for the console
/// harness to print, and every one of them is written to the log by the
/// engine at the same moment it is sent - so when Milestone 6 removed the
/// console, the strings were being cloned across a channel to be dropped
/// unread. What is *not* a state change no longer sends anything at all: a
/// lost stream, a substituted device and a device that will not open are all
/// logged and retried, and none of them moves the icon.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Event {
    KeepAliveOn,
    KeepAliveOff,
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
    /// Whether the peak meter was above the threshold on the previous tick.
    ///
    /// Only so detailed logging can record the two *transitions* rather than
    /// ten lines a second. See the note on `Config::diagnostics`.
    audio_present: bool,
    /// Set when intent came from user input rather than an explicit request,
    /// so the next successful open can say so.
    started_by_input: bool,
    /// Whether the next successful open should announce itself.
    ///
    /// One-shot, and set only by an explicit request. It survives a failed
    /// open, so a hotkey pressed while the headset is still reconnecting is
    /// answered late rather than not at all - but it does not survive the open
    /// it belongs to, so a reconnect or a move to another device later on
    /// stays silent.
    announce: bool,
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
        audio_present: false,
        started_by_input: false,
        announce: false,
        last_device_name: None,
        last_device_check: now,
    };
    let mut input = InputWatcher::new();

    loop {
        // --- commands -----------------------------------------------------
        match commands.try_recv() {
            Ok(Command::Quit) | Err(TryRecvError::Disconnected) => {
                set_intent_off(&mut rt, &config, StopReason::Requested, &log, &events);
                break;
            }
            Ok(Command::Toggle) => {
                if rt.intent {
                    set_intent_off(&mut rt, &config, StopReason::Requested, &log, &events);
                } else {
                    set_intent_on(&mut rt, Source::User);
                }
            }
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
        let seen = input.poll();
        if seen && config.wake_on_input {
            if rt.intent {
                // Somebody is here and using the machine, so do not release
                // out from under them just because nothing happens to be
                // speaking.
                rt.last_audio = now;
            } else if rt.armed {
                log.write("woken by user input");
                set_intent_on(&mut rt, Source::Input);
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
                    log.write(&format!("audio stream lost: {e} - will retry"));
                    rt.stream = None;
                    rt.next_retry = now + RETRY_DELAY;
                    rt.retry_reported = false;
                }
            }

            // --- has anything real been playing? ---------------------------
            if matches!(config.release, Release::Idle { .. }) {
                if let Some(active) = rt.stream.as_ref() {
                    let peak = active.meter.peak();
                    let present = peak > config.audio_threshold;
                    if present {
                        rt.last_audio = now;
                    }
                    // The two transitions, not the ten readings a second in
                    // between. This is what idle release actually rests on,
                    // and it is invisible from outside the app - watching it
                    // live is impossible, because reading the output with a
                    // screen reader makes the very sound being measured.
                    if config.diagnostics && present != rt.audio_present {
                        log.write(&if present {
                            format!("audio detected, peak {peak:.4}")
                        } else {
                            format!(
                                "audio stopped, peak {peak:.4}; letting go in {}s unless something plays",
                                match config.release {
                                    Release::Idle { secs } | Release::Fixed { secs } => secs,
                                }
                            )
                        });
                    }
                    rt.audio_present = present;
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
                set_intent_off(&mut rt, &config, reason, &log, &events);
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

fn set_intent_on(rt: &mut Runtime, source: Source) {
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
    rt.started_by_input = source == Source::Input;
    rt.announce = source == Source::User;
}

fn set_intent_off(
    rt: &mut Runtime,
    config: &Config,
    reason: StopReason,
    log: &Log,
    events: &Sender<Event>,
) {
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

    // A stale announcement must not outlive the intent that set it.
    rt.announce = false;

    // Say goodbye while the stream is still open. Once it is dropped the
    // device is gone and anything we played would have to seize it back.
    //
    // Only for a release the user asked for. An idle timeout is silent: it
    // happens several times an hour, and draining the tone is also the only
    // thing that makes a release slow, so staying quiet makes the automatic
    // path both quieter and quicker.
    if config.earcons && reason == StopReason::Requested {
        if let Some(active) = rt.stream.as_mut() {
            active.keepalive.play(earcon::OFF, config.earcon_volume);
            active.keepalive.drain(EARCON_TIMEOUT);
        }
    }

    // Dropping releases the IAudioClient, which is what frees the headset.
    let had_stream = rt.stream.take().is_some();
    rt.last_device_name = None;
    rt.audio_present = false;

    log.write(&format!(
        "keep-alive OFF ({}) after {}s{}",
        reason.describe(),
        held,
        if had_stream { " - device released" } else { "" }
    ));
    let _ = events.send(Event::KeepAliveOff);
}

fn try_open(rt: &mut Runtime, config: &Config, log: &Log, events: &Sender<Event>, now: Instant) {
    let began = Instant::now();
    let (open, substituted) = match device::open(&config.device) {
        Ok(v) => v,
        Err(e) => {
            retry_later(rt, now);
            if !rt.retry_reported {
                rt.retry_reported = true;
                log.write(&format!(
                    "could not open an output device: {e} - retrying every {}s",
                    RETRY_DELAY.as_secs()
                ));
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
        }
    }

    let meter = match Meter::open(&open.device) {
        Ok(m) => m,
        Err(e) => {
            retry_later(rt, now);
            if !rt.retry_reported {
                rt.retry_reported = true;
                log.write(&format!("could not open the audio meter: {e}"));
            }
            return;
        }
    };

    let mut keepalive = match KeepAlive::start(&open.device, config.signal) {
        Ok(k) => k,
        Err(e) => {
            retry_later(rt, now);
            if !rt.retry_reported {
                rt.retry_reported = true;
                log.write(&format!("could not start keep-alive: {e}"));
            }
            return;
        }
    };

    // Only for an open the user asked for - never a wake on input, and never
    // a reopen after the headset came back or the default output moved.
    // Hearing it through the device is still the point: it says both
    // "keep-alive is on" and "this is where it is on".
    if config.earcons && rt.announce {
        keepalive.play(earcon::ON, config.earcon_volume);
        rt.announce = false;
    }

    // Reopening somewhere else is the shape of both device-loss recovery and
    // a default-output change, and both have cost this project a hardware
    // round. It used to travel as `Event::Moved` for the console to print;
    // with the console gone it belongs on the line that is kept.
    let moved = matches!(&rt.last_device_name, Some(previous) if *previous != open.name);
    log.write(&format!(
        "keep-alive ON  device='{}'{} signal={} release={}",
        open.name,
        if moved { " (moved)" } else { "" },
        config.signal,
        describe_release(config.release)
    ));
    // How long the device took to hand itself over. A Bluetooth headset that
    // has to reconnect is the slow case, and two of the questions still open
    // in PLAN.md - a long idle gap, and a sleep or resume - would show up here
    // first as an open that suddenly takes much longer than usual.
    if config.diagnostics {
        log.write(&format!(
            "device opened in {} ms",
            began.elapsed().as_millis()
        ));
    }
    // A fresh stream has heard nothing yet, so the next thing that plays is a
    // transition worth recording.
    rt.audio_present = false;
    let _ = events.send(Event::KeepAliveOn);

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
