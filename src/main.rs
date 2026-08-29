//! StableSound.
//!
//! Milestone 2 exercises the engine from a console harness. The global hotkey
//! (Milestone 3) and the settings dialog (Milestone 4) replace this front end;
//! the engine underneath is the real thing.

mod audio;
mod config;
mod engine;
mod log;

use std::io::{self, BufRead, Write};

use windows::Win32::System::Com::{CoInitializeEx, COINIT_APARTMENTTHREADED};

use crate::config::{Config, DeviceSelector, Release, Signal};
use crate::engine::{Command, Event};
use crate::log::Log;

fn main() {
    // The engine thread initialises COM for itself, but this thread also calls
    // into WASAPI (listing devices, resolving a device name), and COM is
    // per-thread. Without this, list_outputs fails with 0x800401F0.
    unsafe {
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
    }

    let config_path = config::config_path();
    let (cfg, adjustments) = Config::load(&config_path);

    let log = Log::new(
        if cfg.logging {
            Some(config::log_path())
        } else {
            None
        },
        false,
    );

    println!("StableSound - Milestone 2 engine harness");
    println!();
    println!("Settings: {}", config_path.display());
    if !config_path.exists() {
        println!("  (no file yet - using defaults; 'save' writes one)");
    }
    match log.path() {
        Some(p) => println!("Log:      {}", p.display()),
        None => println!("Log:      disabled"),
    }
    println!();
    describe(&cfg);

    // Report anything validate() corrected, rather than silently changing
    // behaviour behind the user's back.
    for adjustment in &adjustments {
        println!();
        println!("ADJUSTED: {}", adjustment.what);
        println!("  Why: {}", adjustment.why);
        log.write(&format!(
            "config adjusted: {} ({})",
            adjustment.what, adjustment.why
        ));
    }

    println!();
    match audio::device::list_outputs() {
        Ok(devices) if !devices.is_empty() => {
            println!("Available outputs:");
            for d in &devices {
                println!("  {}", d.name);
            }
        }
        Ok(_) => println!("No active output devices found."),
        Err(e) => println!("Could not list output devices: {e}"),
    }

    println!();
    help();

    let handle = engine::spawn(cfg.clone(), log);
    let mut cfg = cfg;

    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
        drain_events(&handle.events);

        print!("> ");
        let _ = io::stdout().flush();

        let Some(Ok(line)) = lines.next() else { break };
        let input = line.trim_start_matches('\u{feff}').trim().to_lowercase();
        let (word, rest) = match input.split_once(' ') {
            Some((w, r)) => (w, r.trim()),
            None => (input.as_str(), ""),
        };

        match word {
            "" => continue,
            "on" => send(&handle, Command::On),
            "off" => send(&handle, Command::Off),
            "toggle" | "t" => send(&handle, Command::Toggle),

            "signal" => match rest {
                "zeros" => set_signal(&mut cfg, Signal::Zeros, &handle),
                "fluctuate" | "fluct" => set_signal(&mut cfg, Signal::Fluctuate, &handle),
                "sine" => set_signal(
                    &mut cfg,
                    Signal::Sine {
                        freq: 20_000.0,
                        amp: 0.01,
                    },
                    &handle,
                ),
                _ => println!("Usage: signal zeros | fluctuate | sine"),
            },

            "idle" | "fixed" => match rest.parse::<u32>() {
                Ok(secs) => {
                    cfg.release = if word == "idle" {
                        Release::Idle { secs }
                    } else {
                        Release::Fixed { secs }
                    };
                    apply(&mut cfg, &handle);
                }
                Err(_) => println!("Usage: {word} <seconds>   for example: {word} 60"),
            },

            "device" => {
                cfg.device = if rest.is_empty() || rest == "default" {
                    DeviceSelector::Default
                } else {
                    // Match the original casing from the device list rather
                    // than the lowercased input.
                    match original_case_name(rest) {
                        Some(name) => DeviceSelector::Named(name),
                        None => {
                            println!("No output device matches '{rest}'. Using default.");
                            DeviceSelector::Default
                        }
                    }
                };
                apply(&mut cfg, &handle);
            }

            "save" => match cfg.save(&config_path) {
                Ok(()) => println!("Saved to {}", config_path.display()),
                Err(e) => println!("Could not save: {e}"),
            },

            "status" => describe(&cfg),
            "help" | "?" => help(),
            "quit" | "exit" | "q" => break,
            other => println!("Unknown command: {other}. Type help for the list."),
        }
    }

    let _ = handle.commands.send(Command::Quit);
    let _ = handle.thread.join();
    println!("Stopped.");
}

fn send(handle: &engine::Handle, cmd: Command) {
    let _ = handle.commands.send(cmd);
    // Give the engine a moment so its event lands before the next prompt.
    std::thread::sleep(std::time::Duration::from_millis(250));
    drain_events(&handle.events);
}

fn set_signal(cfg: &mut Config, signal: Signal, handle: &engine::Handle) {
    cfg.signal = signal;
    apply(cfg, handle);
}

/// Push settings to the engine, reporting anything validation corrected.
fn apply(cfg: &mut Config, handle: &engine::Handle) {
    for adjustment in cfg.validate() {
        println!("ADJUSTED: {}", adjustment.what);
        println!("  Why: {}", adjustment.why);
    }
    let _ = handle.commands.send(Command::Reload(Box::new(cfg.clone())));
    describe(cfg);
}

fn original_case_name(lowercased: &str) -> Option<String> {
    audio::device::list_outputs().ok().and_then(|devices| {
        devices
            .into_iter()
            .find(|d| d.name.to_lowercase() == lowercased)
            .map(|d| d.name)
    })
}

fn describe(cfg: &Config) {
    let device = match &cfg.device {
        DeviceSelector::Default => "default output".to_string(),
        DeviceSelector::Named(n) => n.clone(),
    };
    let release = match cfg.release {
        Release::Idle { secs } => format!("idle - release after {secs}s with no audio"),
        Release::Fixed { secs } => format!("fixed - release {secs}s after switching on"),
    };
    println!("Device:  {device}");
    println!("Signal:  {}", cfg.signal);
    println!("Release: {release}");
}

fn drain_events(events: &std::sync::mpsc::Receiver<Event>) {
    while let Ok(event) = events.try_recv() {
        match event {
            Event::Started { device, signal } => {
                println!("  KEEP-ALIVE ON - {device}, signal {signal}")
            }
            Event::Stopped { reason } => {
                println!("  KEEP-ALIVE OFF - {reason:?}, device released")
            }
            Event::Moved { device } => println!("  MOVED to new default output: {device}"),
            Event::Substituted { wanted, used } => {
                println!("  '{wanted}' not found, using '{used}' instead")
            }
            Event::Error { message } => println!("  ERROR: {message}"),
        }
    }
}

fn help() {
    println!("Commands (type, then press Enter):");
    println!("  on / off / toggle     control keep-alive");
    println!("  signal zeros          pure silence (default, works on the AeroClip)");
    println!("  signal fluctuate      silence plus a tiny blip each second");
    println!("  signal sine           inaudible tone; forces fixed release mode");
    println!("  idle <seconds>        release after N seconds with no audio");
    println!("  fixed <seconds>       release N seconds after switching on");
    println!("  device <name>         target a device by name, or 'device default'");
    println!("  save                  write current settings to the config file");
    println!("  status                show current settings");
    println!("  quit                  exit");
}
