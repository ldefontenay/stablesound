//! Settings, stored as a plain `key = value` text file.
//!
//! Hand-rolled rather than using serde + toml. The format is trivial, and the
//! project has a hard size budget (see CLAUDE.md), so two extra dependency
//! trees would not pay for themselves. The file is also plain enough to read
//! and edit by hand with a screen reader, which is a bonus.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

/// Which output device to keep awake.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum DeviceSelector {
    /// Follow whatever Windows currently calls the default output.
    Default,
    /// A specific device, matched on its friendly name. Stored by name rather
    /// than by ID so it survives re-pairing, which changes the ID.
    Named(String),
}

/// What we actually push through the audio stream.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Signal {
    /// Pure digital silence. Confirmed sufficient on the AeroClip, and the
    /// default because a peak of 0.0 cannot interfere with idle detection.
    Zeros,
    /// Silence plus one tiny non-zero sample per second. Sound Keeper's
    /// default. Fallback for hardware that optimises pure silence away.
    Fluctuate,
    /// Inaudible tone. Last resort: it is loud enough to trip our own peak
    /// meter, so it cannot be combined with idle-based release.
    Sine { freq: f32, amp: f32 },
}

impl Signal {
    /// Peak amplitude this signal produces, for reasoning about whether the
    /// device meter will see us as "real audio".
    pub fn peak(self) -> f32 {
        match self {
            Signal::Zeros => 0.0,
            Signal::Fluctuate => 1.0 / 32768.0,
            Signal::Sine { amp, .. } => amp,
        }
    }
}

impl fmt::Display for Signal {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Signal::Zeros => write!(f, "zeros"),
            Signal::Fluctuate => write!(f, "fluctuate"),
            Signal::Sine { freq, amp } => write!(f, "sine {freq} Hz at {:.1}%", amp * 100.0),
        }
    }
}

/// When to stop keeping the device awake.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Release {
    /// Release after this many seconds with no real audio. Any speech resets
    /// the countdown.
    Idle { secs: u32 },
    /// Release this many seconds after being switched on, regardless.
    Fixed { secs: u32 },
}

#[derive(Clone, Debug, PartialEq)]
pub struct Config {
    pub device: DeviceSelector,
    pub signal: Signal,
    pub release: Release,
    /// Peak level above which we call it real audio rather than our own signal.
    pub audio_threshold: f32,
    /// Append state transitions to a log file. On by default: testing showed
    /// idle behaviour cannot be observed live, because reading the output with
    /// a screen reader generates the very audio being measured.
    pub logging: bool,
    /// Bring keep-alive back automatically when the user touches the keyboard
    /// or mouse.
    ///
    /// Requested after Milestone 2 testing. It fixes a real gap: once idle
    /// release has fired, the next thing to make a sound is often a
    /// notification, and that first word gets clipped. Sitting down and
    /// touching a key is a reliable signal that speech is about to be wanted.
    ///
    /// This is *not* the same as waking on detected audio, which was rejected
    /// in `engine`: audio is a consequence of our own output, so it forms a
    /// loop. Keyboard input is independent of anything the app does.
    pub wake_on_input: bool,
}

impl Default for Config {
    fn default() -> Self {
        Config {
            device: DeviceSelector::Default,
            signal: Signal::Zeros,
            // 30 s after Milestone 2 testing: 60 s felt too long once waking
            // on input made re-arming cheap.
            release: Release::Idle { secs: 30 },
            // JAWS speech measured around 0.56 on the AeroClip, so this has
            // roughly a 1000x margin.
            audio_threshold: 0.0005,
            logging: true,
            wake_on_input: true,
        }
    }
}

/// A problem with a config that we corrected rather than rejected.
pub struct Adjustment {
    pub what: String,
    pub why: String,
}

impl Config {
    /// Correct combinations that cannot work, returning what was changed so the
    /// caller can log it.
    ///
    /// The one that matters: a signal loud enough to trip our own peak meter
    /// cannot be used with idle-based release, because the engine would see its
    /// own keep-alive as "real audio" and never release. Rather than leave that
    /// as a trap for whoever switches to sine for other hardware, fix it here.
    pub fn validate(&mut self) -> Vec<Adjustment> {
        let mut out = Vec::new();

        if let Release::Idle { secs } = self.release {
            if self.signal.peak() >= self.audio_threshold {
                self.release = Release::Fixed { secs: secs.max(60) };
                out.push(Adjustment {
                    what: format!(
                        "release mode changed from idle to fixed ({} s)",
                        secs.max(60)
                    ),
                    why: format!(
                        "the {} signal peaks at {:.5}, at or above the {:.5} detection \
                         threshold, so idle detection would see our own keep-alive as real \
                         audio and never release",
                        self.signal,
                        self.signal.peak(),
                        self.audio_threshold
                    ),
                });
            }
        }

        let secs = match self.release {
            Release::Idle { secs } | Release::Fixed { secs } => secs,
        };
        if secs == 0 {
            self.release = match self.release {
                Release::Idle { .. } => Release::Idle { secs: 30 },
                Release::Fixed { .. } => Release::Fixed { secs: 30 },
            };
            out.push(Adjustment {
                what: "timeout changed from 0 to 30 s".into(),
                why: "a zero timeout would release immediately, making the app pointless".into(),
            });
        }

        out
    }

    pub fn load(path: &Path) -> (Config, Vec<Adjustment>) {
        let mut cfg = match fs::read_to_string(path) {
            Ok(text) => Config::parse(&text),
            Err(_) => Config::default(),
        };
        let adjustments = cfg.validate();
        (cfg, adjustments)
    }

    fn parse(text: &str) -> Config {
        let mut cfg = Config::default();
        let mut sine_freq = 20_000.0f32;
        let mut sine_amp = 0.01f32;
        let mut signal_name = String::from("zeros");
        let mut release_name = String::from("idle");
        let mut release_secs = 30u32;

        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() || line.starts_with('#') {
                continue;
            }
            let Some((key, value)) = line.split_once('=') else {
                continue;
            };
            let key = key.trim().to_lowercase();
            let value = value.trim();

            match key.as_str() {
                "device" => {
                    cfg.device = if value.is_empty() || value.eq_ignore_ascii_case("default") {
                        DeviceSelector::Default
                    } else {
                        DeviceSelector::Named(value.to_string())
                    }
                }
                "signal" => signal_name = value.to_lowercase(),
                "sine_freq" => sine_freq = value.parse().unwrap_or(sine_freq),
                "sine_amp" => sine_amp = value.parse().unwrap_or(sine_amp),
                "release" => release_name = value.to_lowercase(),
                "timeout" => release_secs = value.parse().unwrap_or(release_secs),
                "threshold" => cfg.audio_threshold = value.parse().unwrap_or(cfg.audio_threshold),
                "logging" => cfg.logging = parse_bool(value).unwrap_or(cfg.logging),
                "wake_on_input" => {
                    cfg.wake_on_input = parse_bool(value).unwrap_or(cfg.wake_on_input)
                }
                _ => {}
            }
        }

        cfg.signal = match signal_name.as_str() {
            "fluctuate" | "fluct" => Signal::Fluctuate,
            "sine" => Signal::Sine {
                freq: sine_freq,
                amp: sine_amp,
            },
            _ => Signal::Zeros,
        };
        cfg.release = match release_name.as_str() {
            "fixed" => Release::Fixed { secs: release_secs },
            _ => Release::Idle { secs: release_secs },
        };
        cfg
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, self.serialise())
    }

    fn serialise(&self) -> String {
        let (release_name, secs) = match self.release {
            Release::Idle { secs } => ("idle", secs),
            Release::Fixed { secs } => ("fixed", secs),
        };
        let (signal_name, freq, amp) = match self.signal {
            Signal::Zeros => ("zeros", 20_000.0, 0.01),
            Signal::Fluctuate => ("fluctuate", 20_000.0, 0.01),
            Signal::Sine { freq, amp } => ("sine", freq, amp),
        };
        let device = match &self.device {
            DeviceSelector::Default => "default".to_string(),
            DeviceSelector::Named(n) => n.clone(),
        };

        format!(
            "\
# StableSound settings.
# Plain text on purpose - safe to edit by hand.

# Which output to keep awake: 'default', or an exact device name.
device = {device}

# zeros | fluctuate | sine
# zeros is quietest and works on the Soundcore AeroClip.
# Only switch to sine if the others fail: it is loud enough to defeat
# idle detection, and release will be forced to fixed mode.
signal = {signal_name}
sine_freq = {freq}
sine_amp = {amp}

# idle  = release after 'timeout' seconds with no audio
# fixed = release 'timeout' seconds after switching on
release = {release_name}
timeout = {secs}

# Peak level counted as real audio. JAWS speech measures around 0.56.
threshold = {threshold}

# Log state changes to stablesound.log. Recommended: idle behaviour
# cannot be watched live, because reading output with a screen reader
# makes the very sound being measured.
logging = {logging}

# Bring keep-alive back when you touch the keyboard or mouse, so the
# first word after a pause is not clipped. Switching keep-alive off by
# hand disables this until you switch it on again - so releasing the
# headset for your phone is not undone by the next keypress.
wake_on_input = {wake_on_input}
",
            threshold = self.audio_threshold,
            logging = self.logging,
            wake_on_input = self.wake_on_input,
        )
    }
}

fn parse_bool(v: &str) -> Option<bool> {
    match v.to_lowercase().as_str() {
        "1" | "true" | "yes" | "on" => Some(true),
        "0" | "false" | "no" | "off" => Some(false),
        _ => None,
    }
}

/// Config lives next to the exe when that is writable, keeping the portable
/// build genuinely portable, and falls back to `%APPDATA%` when it is not
/// (Program Files, a read-only share).
pub fn config_path() -> PathBuf {
    if let Some(dir) = exe_dir() {
        if is_writable(&dir) {
            return dir.join("stablesound.conf");
        }
    }
    appdata_dir().join("stablesound.conf")
}

pub fn log_path() -> PathBuf {
    let cfg = config_path();
    cfg.with_file_name("stablesound.log")
}

fn exe_dir() -> Option<PathBuf> {
    std::env::current_exe()
        .ok()
        .and_then(|p| p.parent().map(Path::to_path_buf))
}

fn appdata_dir() -> PathBuf {
    std::env::var("APPDATA")
        .map(PathBuf::from)
        .unwrap_or_else(|_| PathBuf::from("."))
        .join("StableSound")
}

fn is_writable(dir: &Path) -> bool {
    let probe = dir.join(".stablesound-write-test");
    match fs::write(&probe, b"") {
        Ok(()) => {
            let _ = fs::remove_file(&probe);
            true
        }
        Err(_) => false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trips() {
        let cfg = Config {
            device: DeviceSelector::Named("Soundcore AeroClip".into()),
            release: Release::Fixed { secs: 120 },
            ..Config::default()
        };
        let parsed = Config::parse(&cfg.serialise());
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn unknown_keys_and_comments_are_ignored() {
        let cfg = Config::parse("# comment\nnonsense = 1\nsignal = fluctuate\n");
        assert_eq!(cfg.signal, Signal::Fluctuate);
    }

    #[test]
    fn sine_forces_fixed_release() {
        // The trap this guard exists for: sine is louder than the detection
        // threshold, so idle release would never fire.
        let mut cfg = Config {
            signal: Signal::Sine {
                freq: 20_000.0,
                amp: 0.01,
            },
            release: Release::Idle { secs: 90 },
            ..Config::default()
        };
        let adjustments = cfg.validate();
        assert_eq!(cfg.release, Release::Fixed { secs: 90 });
        assert_eq!(adjustments.len(), 1);
    }

    #[test]
    fn quiet_signals_keep_idle_release() {
        let mut cfg = Config::default();
        assert!(cfg.validate().is_empty());
        assert_eq!(cfg.release, Release::Idle { secs: 30 });

        let mut cfg = Config {
            signal: Signal::Fluctuate,
            ..Config::default()
        };
        assert!(cfg.validate().is_empty());
    }

    #[test]
    fn zero_timeout_is_corrected() {
        let mut cfg = Config {
            release: Release::Idle { secs: 0 },
            ..Config::default()
        };
        let adjustments = cfg.validate();
        assert_eq!(cfg.release, Release::Idle { secs: 30 });
        assert_eq!(adjustments.len(), 1);
    }

    #[test]
    fn wake_on_input_round_trips() {
        let cfg = Config {
            wake_on_input: false,
            ..Config::default()
        };
        assert!(!Config::parse(&cfg.serialise()).wake_on_input);
    }
}
