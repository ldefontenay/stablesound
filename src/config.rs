//! Settings, stored as a plain `key = value` text file.
//!
//! Hand-rolled rather than using serde + toml. The format is trivial, and the
//! project has a hard size budget (see CLAUDE.md), so two extra dependency
//! trees would not pay for themselves. The file is also plain enough to read
//! and edit by hand with a screen reader, which is a bonus.

use std::fmt;
use std::fs;
use std::path::{Path, PathBuf};

use crate::hotkey::Hotkey;

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
    /// Append state transitions to a log file.
    ///
    /// Off by default, which is the tester's decision and not the one argued
    /// for here. The case put to them was that a line per state change costs a
    /// few hundred bytes a day and is the only evidence that exists when
    /// something goes wrong, in an app whose behaviour cannot be watched as it
    /// happens. Having seen both, they chose: "Please have logging off by
    /// default and it can then be turned on when needed for development or
    /// trouble-shooting." It is one checkbox in the dialog.
    ///
    /// The cost is real and worth stating: a hardware round run without
    /// turning it on first leaves nothing behind to read. Every test script
    /// from here on has to say so. See [`Config::diagnostics`] for the
    /// detailed variety, which sits behind the same switch again.
    pub logging: bool,
    /// Bring keep-alive back automatically when the user touches the machine.
    ///
    /// Requested after Milestone 2 testing. It fixes a real gap: once idle
    /// release has fired, the next thing to make a sound is often a
    /// notification, and that first word gets clipped. Sitting down and
    /// touching a key is a reliable signal that speech is about to be wanted.
    ///
    /// This is *not* the same as waking on detected audio, which was rejected
    /// in `engine`: audio is a consequence of our own output, so it forms a
    /// loop. User input is independent of anything the app does.
    ///
    /// Any input counts, the mouse included. Excluding the mouse was tried
    /// three ways over four hardware rounds and abandoned - see `input`.
    pub wake_on_input: bool,
    /// The global toggle combination. The primary interface, not a shortcut.
    pub hotkey: Hotkey,
    /// Whether to claim [`Config::settings_hotkey`] at all.
    ///
    /// On by default. The Milestone 4 round asked for it off - "it won't be
    /// used often enough to warrant a hotkey" - and the Milestone 5 round,
    /// having lived with it off, changed its mind: "Actually, ship with the
    /// settings hotkey on. In the documentation later, we can recommend that
    /// the hotkey can be disabled once the user has completed tweaking the
    /// application settings to their satisfaction."
    ///
    /// So the reasoning survives, only inverted. A global hotkey is a
    /// combination taken away from every other program on the machine for as
    /// long as StableSound runs, and it is worth handing back - but the moment
    /// that is worth doing is after the settings have been set the way you
    /// want them, not before you have ever opened them. The help file owes
    /// this a paragraph; see the Milestone 6 notes in PLAN.md.
    pub settings_hotkey_enabled: bool,
    /// The global combination that opens the settings dialog, when
    /// [`Config::settings_hotkey_enabled`] is on.
    ///
    /// Kept in the config, and offered in the dialog, even while it is off, so
    /// that switching it on does not also mean thinking up a combination.
    pub settings_hotkey: Hotkey,
    /// Play a tone when *you* switch keep-alive on or off. On by default:
    /// CLAUDE.md requires state changes to be audible, and for a hotkey pressed
    /// with no window in front of you the tone is the only feedback there is.
    ///
    /// Automatic transitions - an idle release, and the wake that follows the
    /// next keypress - are always silent, whatever this is set to. They were
    /// not, and the Milestone 3 round found them a nuisance: they happen many
    /// times an hour and tell the user nothing they need to act on.
    pub earcons: bool,
    /// Earcon amplitude, 0.0 to 1.0.
    pub earcon_volume: f32,
    /// Log the measurements behind each decision, not just the decision.
    ///
    /// Off by default; it makes the log several times longer. The tester asked
    /// after the Milestone 4 round whether a log is worth keeping at all, and
    /// the answer this settles on is: an ordinary log yes, a detailed one only
    /// while something is being looked into.
    ///
    /// The reason for the split is that this project's behaviour cannot be
    /// watched live. Reading the app's own output with a screen reader makes
    /// the very sound the idle detector is measuring, so anything about audio
    /// or timing has to be read back afterwards. Two rounds were lost guessing
    /// at input handling from the outside before a detailed log settled it in
    /// three lines.
    ///
    /// What it adds now that input handling has gone: the peak meter crossing
    /// the audio threshold in each direction, and how long a device took to
    /// open. Those are the two measurements behind the questions still open in
    /// PLAN.md - whether digital silence still works after a long gap, and
    /// what a sleep or resume does to the headset.
    pub diagnostics: bool,
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
            logging: false,
            wake_on_input: true,
            hotkey: Hotkey::default(),
            settings_hotkey_enabled: true,
            settings_hotkey: Hotkey::settings_default(),
            earcons: true,
            // The tester's figure from the Milestone 3 round, having compared
            // 0.1, 0.2 and 0.35 on the AeroClip. The first guess of 0.2 was
            // twice as loud as wanted.
            earcon_volume: 0.1,
            diagnostics: false,
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

        // Two identical combinations means the second registration fails and
        // one of the two functions silently has no key at all. Caught here
        // rather than left to be discovered as "the hotkey stopped working".
        // Only a problem when both are actually claimed.
        if self.settings_hotkey_enabled && self.hotkey == self.settings_hotkey {
            self.settings_hotkey = Hotkey::settings_default();
            if self.settings_hotkey == self.hotkey {
                self.settings_hotkey = Hotkey::default();
            }
            out.push(Adjustment {
                what: format!(
                    "settings hotkey changed to {}, because it was the same as the toggle",
                    self.settings_hotkey
                ),
                why: "a combination can only be registered once, so the second of two identical hotkeys would never fire and that function would have no key at all"
                    .into(),
            });
        }

        if !(0.0..=1.0).contains(&self.earcon_volume) {
            let was = self.earcon_volume;
            self.earcon_volume = self.earcon_volume.clamp(0.0, 1.0);
            out.push(Adjustment {
                what: format!("earcon volume clamped from {was} to {}", self.earcon_volume),
                why: "amplitude runs from 0.0 to 1.0; anything above 1.0 clips, and the earcon would come out as a rasp rather than a tone"
                    .into(),
            });
        }

        out
    }

    pub fn load(path: &Path) -> (Config, Vec<Adjustment>) {
        let (mut cfg, mut adjustments) = match fs::read_to_string(path) {
            Ok(text) => Config::parse(&text),
            Err(_) => (Config::default(), Vec::new()),
        };
        adjustments.extend(cfg.validate());
        (cfg, adjustments)
    }

    /// Reads the file. Anything unparseable keeps its default rather than
    /// failing the load, but says so - a mistyped hotkey that silently did
    /// nothing would be maddening to diagnose without sight of the file.
    fn parse(text: &str) -> (Config, Vec<Adjustment>) {
        // A byte order mark, if an editor left one. This file is documented as
        // safe to edit by hand, and several Windows editors offer "UTF-8 with
        // BOM"; without this the mark glues itself to the first key, so that
        // one setting is silently ignored and every other one works. That is
        // about the most confusing way a config file can fail.
        let text = text.strip_prefix('\u{feff}').unwrap_or(text);
        let mut cfg = Config::default();
        let mut adjustments = Vec::new();
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
                "settings_hotkey_enabled" => {
                    cfg.settings_hotkey_enabled =
                        parse_bool(value).unwrap_or(cfg.settings_hotkey_enabled)
                }
                "settings_hotkey" => match Hotkey::parse(value) {
                    Some(key) => cfg.settings_hotkey = key,
                    None => adjustments.push(Adjustment {
                        what: format!(
                            "settings_hotkey '{value}' ignored, keeping {}",
                            cfg.settings_hotkey
                        ),
                        why: "it should read like 'ctrl+win+f11' - at least one of ctrl, alt, shift or win, then one key"
                            .into(),
                    }),
                },
                "hotkey" => match Hotkey::parse(value) {
                    Some(key) => cfg.hotkey = key,
                    None => adjustments.push(Adjustment {
                        what: format!("hotkey '{value}' ignored, keeping {}", cfg.hotkey),
                        why: "it should read like 'ctrl+win+f12' - at least one of ctrl, alt, shift or win, then one key. A key with no modifier is refused because registering it would take that key away from every other program"
                            .into(),
                    }),
                },
                "earcons" => cfg.earcons = parse_bool(value).unwrap_or(cfg.earcons),
                "diagnostics" => {
                    cfg.diagnostics = parse_bool(value).unwrap_or(cfg.diagnostics)
                }
                "earcon_volume" => {
                    cfg.earcon_volume = value.parse().unwrap_or(cfg.earcon_volume)
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
        (cfg, adjustments)
    }

    pub fn save(&self, path: &Path) -> std::io::Result<()> {
        if let Some(dir) = path.parent() {
            fs::create_dir_all(dir)?;
        }
        fs::write(path, self.serialise())
    }

    /// The settings in force, one line each, for the log.
    ///
    /// This was the console's `status` command, which printed the same eight
    /// lines. Milestone 6 took the console away and the log inherited them:
    /// they go down at startup and again whenever the dialog changes
    /// something, so a log read afterwards says what the app was actually
    /// configured to do at the time - which the console could only ever tell
    /// somebody sitting in front of it.
    pub fn summary(&self) -> Vec<String> {
        let device = match &self.device {
            DeviceSelector::Default => "default output".to_string(),
            DeviceSelector::Named(n) => n.clone(),
        };
        let release = match self.release {
            Release::Idle { secs } => format!("idle - release after {secs}s with no audio"),
            Release::Fixed { secs } => format!("fixed - release {secs}s after switching on"),
        };
        vec![
            format!("device:  {device}"),
            format!("signal:  {}", self.signal),
            format!("release: {release}"),
            format!("hotkey:  {} toggles keep-alive", self.hotkey),
            format!(
                "dialog:  {}",
                if self.settings_hotkey_enabled {
                    format!(
                        "{} opens the settings, as does the tray",
                        self.settings_hotkey
                    )
                } else {
                    "tray menu (Win+B) only - no hotkey".to_string()
                }
            ),
            format!(
                "wake:    {}",
                if self.wake_on_input {
                    "on - any input brings keep-alive back"
                } else {
                    "off - only the hotkey starts it"
                }
            ),
            format!(
                "earcons: {}",
                if self.earcons {
                    format!(
                        "on at {:.0}% - when switched on or off by hand; automatic releases are silent",
                        self.earcon_volume * 100.0
                    )
                } else {
                    "off".to_string()
                }
            ),
            format!(
                "logging: {}",
                match (self.logging, self.diagnostics) {
                    (false, _) => "off",
                    (true, false) => "on - state changes",
                    (true, true) => "on - state changes, plus detailed troubleshooting lines",
                }
            ),
        ]
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

# Log state changes to stablesound.log. Off by default; turn it on
# before doing anything you might want to ask about afterwards. Idle
# behaviour cannot be watched live - reading output with a screen reader
# makes the very sound being measured - so the file is the only record.
logging = {logging}

# Bring keep-alive back when you touch the machine, so the first word
# after a pause is not clipped. Any input counts, the trackpad included.
# Switching keep-alive off by hand disables this until you switch it on
# again - so releasing the headset for your phone is not undone by the
# next keypress.
wake_on_input = {wake_on_input}

# Global toggle. At least one of ctrl, alt, shift, win - then one key
# (a letter, a digit, f1 to f24, a punctuation key such as ; , . / - =,
# or space, pause, insert, delete, home, end, pageup, pagedown, up,
# down, left, right).
# This combination is taken from every other program while StableSound
# runs, so obscure is good.
hotkey = {hotkey}

# A second global hotkey that opens the settings dialog from anywhere.
# On by default, so the settings can be found before you know where the
# tray is. Every global hotkey is a combination taken away from every
# other program while StableSound runs, so once the settings are the way
# you want them this is worth switching off: the tray menu opens them
# too, with Win+B.
settings_hotkey_enabled = {settings_hotkey_enabled}

# Which combination that would be. Same rules as the toggle above. Kept
# here even while it is off, so switching it on does not also mean
# thinking up a combination.
settings_hotkey = {settings_hotkey}

# Play a short tone when you switch keep-alive on or off: rising for on,
# falling for off. It plays through the headphones being kept awake, so
# hearing it also proves the headset is up.
# Automatic releases, and waking again when you next type, are always
# silent - they happen often and there is nothing to act on.
earcons = {earcons}

# Earcon loudness, 0.0 to 1.0.
earcon_volume = {earcon_volume}

# Detailed logging, for troubleshooting. Off by default - it makes the
# log several times longer. Turn it on while looking into a problem: it
# adds the moments when audio starts and stops, with the level measured,
# and how long each device took to open.
diagnostics = {diagnostics}
",
            threshold = self.audio_threshold,
            logging = self.logging,
            wake_on_input = self.wake_on_input,
            hotkey = self.hotkey,
            settings_hotkey_enabled = self.settings_hotkey_enabled,
            settings_hotkey = self.settings_hotkey,
            earcons = self.earcons,
            earcon_volume = self.earcon_volume,
            diagnostics = self.diagnostics,
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
        let (parsed, adjustments) = Config::parse(&cfg.serialise());
        assert!(adjustments.is_empty());
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn a_byte_order_mark_does_not_eat_the_first_setting() {
        // Found by writing a test config with PowerShell's -Encoding utf8,
        // which adds one: the first line was ignored and every other line
        // worked, which looked exactly like a parser bug in one key.
        let (cfg, adjustments) = Config::parse("\u{feff}timeout = 45\nearcons = off\n");
        assert_eq!(cfg.release, Release::Idle { secs: 45 });
        assert!(!cfg.earcons);
        assert!(adjustments.is_empty());
    }

    #[test]
    fn unknown_keys_and_comments_are_ignored() {
        let (cfg, _) = Config::parse("# comment\nnonsense = 1\nsignal = fluctuate\n");
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
    fn new_milestone_3_settings_round_trip() {
        let cfg = Config {
            wake_on_input: false,
            hotkey: Hotkey::parse("ctrl+alt+shift+s").unwrap(),
            earcons: false,
            earcon_volume: 0.35,
            ..Config::default()
        };
        let (parsed, adjustments) = Config::parse(&cfg.serialise());
        assert!(adjustments.is_empty());
        assert_eq!(parsed, cfg);
    }

    #[test]
    fn a_bad_hotkey_is_reported_not_silently_dropped() {
        // Silently ignoring it would leave the user pressing a combination
        // that does nothing, with no way to tell why.
        let (cfg, adjustments) = Config::parse("hotkey = f12\n");
        assert_eq!(cfg.hotkey, Hotkey::default());
        assert_eq!(adjustments.len(), 1);
    }

    #[test]
    fn a_good_hotkey_is_taken() {
        let (cfg, adjustments) = Config::parse("hotkey = ctrl+alt+shift+k\n");
        assert_eq!(cfg.hotkey, Hotkey::parse("ctrl+alt+shift+k").unwrap());
        assert!(adjustments.is_empty());
    }

    #[test]
    fn earcon_volume_is_clamped() {
        let mut cfg = Config {
            earcon_volume: 4.0,
            ..Config::default()
        };
        let adjustments = cfg.validate();
        assert_eq!(cfg.earcon_volume, 1.0);
        assert_eq!(adjustments.len(), 1);
    }

    #[test]
    fn the_settings_hotkey_round_trips_and_is_reported_when_wrong() {
        let cfg = Config {
            settings_hotkey: Hotkey::parse("ctrl+alt+p").unwrap(),
            ..Config::default()
        };
        let (parsed, adjustments) = Config::parse(&cfg.serialise());
        assert!(adjustments.is_empty());
        assert_eq!(parsed.settings_hotkey, cfg.settings_hotkey);

        let (fallback, adjustments) = Config::parse(
            "settings_hotkey = f11
",
        );
        assert_eq!(fallback.settings_hotkey, Hotkey::settings_default());
        assert_eq!(adjustments.len(), 1);
    }

    #[test]
    fn two_identical_hotkeys_are_pulled_apart() {
        let mut cfg = Config {
            hotkey: Hotkey::settings_default(),
            settings_hotkey: Hotkey::settings_default(),
            settings_hotkey_enabled: true,
            ..Config::default()
        };
        let adjustments = cfg.validate();
        assert_eq!(adjustments.len(), 1);
        assert_ne!(cfg.hotkey, cfg.settings_hotkey);
    }

    #[test]
    fn a_settings_hotkey_that_is_never_claimed_may_match_the_toggle() {
        // Nothing registers it, so there is no collision to pull apart, and
        // correcting it would silently change a setting the user cannot even
        // see the effect of.
        let mut cfg = Config {
            hotkey: Hotkey::settings_default(),
            settings_hotkey: Hotkey::settings_default(),
            settings_hotkey_enabled: false,
            ..Config::default()
        };
        assert!(cfg.validate().is_empty());
        assert_eq!(cfg.settings_hotkey, Hotkey::settings_default());
    }

    #[test]
    fn the_settings_hotkey_is_on_by_default_and_can_be_handed_back() {
        // Shipped on, at the tester's request after the Milestone 5 round, and
        // switchable off once the settings are settled - so both directions
        // have to survive a round trip through the file.
        let cfg = Config::default();
        assert!(cfg.settings_hotkey_enabled);
        assert_eq!(cfg.settings_hotkey, Hotkey::settings_default());
        assert!(
            !Config::parse(
                "settings_hotkey_enabled = no
"
            )
            .0
            .settings_hotkey_enabled
        );
    }

    #[test]
    fn logging_is_off_by_default_and_can_be_turned_on() {
        // The tester's call after seeing both: "Please have logging off by
        // default and it can then be turned on when needed for development or
        // trouble-shooting."
        let cfg = Config::default();
        assert!(!cfg.logging);
        assert!(!cfg.diagnostics);
        assert!(
            Config::parse(
                "logging = yes
"
            )
            .0
            .logging
        );
    }

    #[test]
    fn the_two_hotkeys_do_not_default_to_the_same_combination() {
        // Registering the same combination twice fails the second time, which
        // would silently cost whichever function lost the race.
        let cfg = Config::default();
        assert_ne!(cfg.hotkey, cfg.settings_hotkey);
    }

    #[test]
    fn wake_on_input_round_trips() {
        let cfg = Config {
            wake_on_input: false,
            ..Config::default()
        };
        assert!(!Config::parse(&cfg.serialise()).0.wake_on_input);
    }
}
