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

use windows::Win32::System::SystemInformation::GetLocalTime;

/// Cheap to clone - a path and a flag - so the message loop can keep its own
/// handle after the engine has taken one.
#[derive(Clone)]
pub struct Log {
    path: Option<PathBuf>,
    /// Mirror to the console too. Off once there is a tray icon instead.
    echo: bool,
}

impl Log {
    pub fn new(path: Option<PathBuf>, echo: bool) -> Self {
        Log { path, echo }
    }

    pub fn path(&self) -> Option<&Path> {
        self.path.as_deref()
    }

    pub fn write(&self, message: &str) {
        let line = format!("{} {}", timestamp(), message);

        if self.echo {
            println!("{line}");
        }

        if let Some(path) = &self.path {
            // Logging is a convenience, never a reason to fail. If the file
            // cannot be written, carry on silently rather than interrupting.
            if let Ok(mut f) = OpenOptions::new().create(true).append(true).open(path) {
                let _ = writeln!(f, "{line}");
            }
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
