//! Append-only log of state transitions.
//!
//! This exists for a specific reason found during Milestone 1 testing: idle
//! behaviour cannot be observed live. Reading the program's own output with a
//! screen reader produces speech, which trips the audio detector, which changes
//! the very thing being measured. A log read afterwards, while the headset is
//! quiet, is the only way to see what actually happened.

use std::fs::OpenOptions;
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

use windows::Win32::System::SystemInformation::GetLocalTime;

/// Cheap to clone, so the message loop can keep its own handle after the
/// engine has taken one.
///
/// The path is kept even while logging is off, and the switch is *shared*
/// between clones. Milestone 4 put logging in the settings dialog, and a
/// checkbox that only took effect after a restart would be worse than no
/// checkbox: this log is how the project's behaviour gets checked, so the
/// tester turns it on precisely when something has started going wrong.
#[derive(Clone)]
pub struct Log {
    path: PathBuf,
    /// Mirror to the console too. Off once there is a tray icon instead.
    echo: bool,
    enabled: Arc<AtomicBool>,
}

impl Log {
    pub fn new(path: PathBuf, echo: bool, enabled: bool) -> Self {
        Log {
            path,
            echo,
            enabled: Arc::new(AtomicBool::new(enabled)),
        }
    }

    /// Turn writing on or off for this log and every clone of it.
    pub fn set_enabled(&self, on: bool) {
        self.enabled.store(on, Ordering::Relaxed);
    }

    /// Where lines are going, or `None` while logging is off.
    pub fn path(&self) -> Option<&Path> {
        self.enabled
            .load(Ordering::Relaxed)
            .then_some(self.path.as_path())
    }

    pub fn write(&self, message: &str) {
        if !self.enabled.load(Ordering::Relaxed) {
            return;
        }
        let line = format!("{} {}", timestamp(), message);

        if self.echo {
            println!("{line}");
        }

        // Logging is a convenience, never a reason to fail. If the file
        // cannot be written, carry on silently rather than interrupting.
        if let Ok(mut f) = OpenOptions::new()
            .create(true)
            .append(true)
            .open(&self.path)
        {
            let _ = writeln!(f, "{line}");
        }
    }
}

fn timestamp() -> String {
    let t = unsafe { GetLocalTime() };
    format!(
        "{:04}-{:02}-{:02} {:02}:{:02}:{:02}",
        t.wYear, t.wMonth, t.wDay, t.wHour, t.wMinute, t.wSecond
    )
}
