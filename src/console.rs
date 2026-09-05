//! The line-based console harness.
//!
//! Milestone 2's front end, kept rather than replaced. The hotkey and tray are
//! now the real interface, but this is still the only way to change a setting
//! or see what the engine thinks, and the tester is already fluent in it.
//! Throwing it away before the GUI is proven on hardware would leave nothing to
//! fall back on. Milestone 4 gives the settings a dialog and Milestone 6
//! removes this.
//!
//! Runs on its own thread, because `main` is busy pumping the message queue for
//! the hotkey and the tray. It talks to the engine over the same command
//! channel they do.

use std::io::{self, BufRead, Write};
use std::sync::mpsc::Sender;

use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Com::{CoInitializeEx, CoUninitialize, COINIT_APARTMENTTHREADED};
use windows::Win32::UI::WindowsAndMessaging::PostMessageW;

use crate::audio::device;
use crate::config::{self, Config, DeviceSelector, Release, Signal};
use crate::engine::Command;
use crate::hotkey::Hotkey;

/// Posted to the message loop when the user types `quit`, since only the
/// thread that owns the queue may end it.
pub const WM_CONSOLE_QUIT: u32 = windows::Win32::UI::WindowsAndMessaging::WM_APP + 2;

/// `HWND` is not `Send`, so the window is handed over as a raw value and put
/// back together on this side. Sound because the handle is only ever used to
/// post a message, which is explicitly safe from any thread.
pub fn spawn(cfg: Config, commands: Sender<Command>, hwnd: HWND) {
    let hwnd_bits = hwnd.0 as isize;
    std::thread::spawn(move || {
        // COM is per-thread, and listing devices needs it here too.
        unsafe {
            let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);
        }
        run(cfg, &commands);
        unsafe {
            let _ = PostMessageW(
                Some(HWND(hwnd_bits as *mut std::ffi::c_void)),
                WM_CONSOLE_QUIT,
                WPARAM(0),
                LPARAM(0),
            );
            CoUninitialize();
        }
    });
}

fn run(mut cfg: Config, commands: &Sender<Command>) {
    let config_path = config::config_path();
    let stdin = io::stdin();
    let mut lines = stdin.lock().lines();

    loop {
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
            "on" => send(commands, Command::On),
            "off" => send(commands, Command::Off),
            "toggle" | "t" => send(commands, Command::Toggle),

            "signal" => match rest {
                "zeros" => set_signal(&mut cfg, Signal::Zeros, commands),
                "fluctuate" | "fluct" => set_signal(&mut cfg, Signal::Fluctuate, commands),
                "sine" => set_signal(
                    &mut cfg,
                    Signal::Sine {
                        freq: 20_000.0,
                        amp: 0.01,
                    },
                    commands,
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
                    apply(&mut cfg, commands);
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
                apply(&mut cfg, commands);
            }

            "wake" => match rest {
                "on" | "" => {
                    cfg.wake_on_input = true;
                    apply(&mut cfg, commands);
                }
                "off" => {
                    cfg.wake_on_input = false;
                    apply(&mut cfg, commands);
                }
                _ => println!("Usage: wake on | wake off"),
            },

            "mouse" => match rest {
                "on" => {
                    cfg.wake_on_mouse = true;
                    apply(&mut cfg, commands);
                }
                "off" | "" => {
                    cfg.wake_on_mouse = false;
                    apply(&mut cfg, commands);
                }
                _ => println!("Usage: mouse on | mouse off"),
            },

            "earcons" => match rest {
                "on" | "" => {
                    cfg.earcons = true;
                    apply(&mut cfg, commands);
                }
                "off" => {
                    cfg.earcons = false;
                    apply(&mut cfg, commands);
                }
                _ => println!("Usage: earcons on | earcons off"),
            },

            "volume" => match rest.parse::<f32>() {
                Ok(v) => {
                    cfg.earcon_volume = v;
                    apply(&mut cfg, commands);
                }
                Err(_) => println!("Usage: volume <0.0 to 1.0>   for example: volume 0.2"),
            },

            "hotkey" => match Hotkey::parse(rest) {
                Some(key) => {
                    cfg.hotkey = key;
                    println!("Hotkey set to {key}.");
                    println!("Save, then restart - the combination is claimed at startup.");
                }
                None => {
                    println!("Usage: hotkey ctrl+win+f12");
                    println!("At least one of ctrl, alt, shift, win, then one key.");
                }
            },

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

    let _ = commands.send(Command::Quit);
}

fn send(commands: &Sender<Command>, cmd: Command) {
    let _ = commands.send(cmd);
}

fn set_signal(cfg: &mut Config, signal: Signal, commands: &Sender<Command>) {
    cfg.signal = signal;
    apply(cfg, commands);
}

/// Push settings to the engine, reporting anything validation corrected.
fn apply(cfg: &mut Config, commands: &Sender<Command>) {
    for adjustment in cfg.validate() {
        println!("ADJUSTED: {}", adjustment.what);
        println!("  Why: {}", adjustment.why);
    }
    let _ = commands.send(Command::Reload(Box::new(cfg.clone())));
    describe(cfg);
}

fn original_case_name(lowercased: &str) -> Option<String> {
    device::list_outputs().ok().and_then(|devices| {
        devices
            .into_iter()
            .find(|d| d.name.to_lowercase() == lowercased)
            .map(|d| d.name)
    })
}

pub fn describe(cfg: &Config) {
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
    println!("Hotkey:  {}", cfg.hotkey);
    println!(
        "Wake:    {}",
        match (cfg.wake_on_input, cfg.wake_on_mouse) {
            (false, _) => "off - only the hotkey starts it".to_string(),
            (true, false) => "keyboard only".to_string(),
            (true, true) => "keyboard and mouse".to_string(),
        }
    );
    println!(
        "Earcons: {}",
        if cfg.earcons {
            format!("on at {:.0}%", cfg.earcon_volume * 100.0)
        } else {
            "off".to_string()
        }
    );
}

pub fn help() {
    println!("Commands (type, then press Enter):");
    println!("  on / off / toggle     control keep-alive");
    println!("  signal zeros          pure silence (default, works on the AeroClip)");
    println!("  signal fluctuate      silence plus a tiny blip each second");
    println!("  signal sine           inaudible tone; forces fixed release mode");
    println!("  idle <seconds>        release after N seconds with no audio");
    println!("  fixed <seconds>       release N seconds after switching on");
    println!("  device <name>         target a device by name, or 'device default'");
    println!("  wake on | wake off    bring keep-alive back on keyboard input");
    println!("  mouse on | mouse off  let the mouse wake it too (off by default)");
    println!("  earcons on | off      the tones that mark each state change");
    println!("  volume <0.0-1.0>      how loud those tones are");
    println!("  hotkey <combination>  for example: hotkey ctrl+win+f12");
    println!("  save                  write current settings to the config file");
    println!("  status                show current settings");
    println!("  quit                  exit");
}
