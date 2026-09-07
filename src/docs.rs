//! The two files StableSound can hand the user to read: the help, and the log.
//!
//! They live together because they are the same gesture from the user's side -
//! a button in the settings dialog and an item in the tray menu, each opening
//! something in whatever program reads that kind of file - and because
//! Milestone 6 gave both of them two callers instead of one.
//!
//! Asked for after the Milestone 4 round, for "eventually", with a button in
//! both the settings dialog and the tray menu. This is it.
//!
//! # Why HTML in a browser
//!
//! The other two candidates were a plain text file opened in Notepad, and a
//! read-only edit control like the message window built in Milestone 5.1.
//! HTML was chosen for one reason: **a browser is the only one of the three
//! that gives a screen reader real heading navigation.** JAWS moves heading to
//! heading with `H`, and lists every heading in a document with `Insert+F6`.
//! Neither Notepad nor an edit control offers anything of the sort - in both,
//! a manual is a single wall of text walked a line at a time.
//!
//! The message window is right for twenty lines and wrong for a document with
//! a dozen sections.
//!
//! # Why it is written out rather than shipped alongside
//!
//! CLAUDE.md constraint 5 is a single self-contained exe, no installer. A help
//! file sitting next to the exe would be a second thing to copy, and a second
//! thing to lose. So the document is compiled into the binary and written out
//! only when somebody asks to read it.
//!
//! It is rewritten on every request rather than only when missing. That costs
//! nothing - the file is a few tens of kilobytes - and it means the help can
//! never be older than the program it describes, which is the failure that
//! matters: a help file left behind by an earlier version, describing settings
//! that have since moved.

use std::path::PathBuf;

use windows::core::PCWSTR;
use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Shell::ShellExecuteW;
use windows::Win32::UI::WindowsAndMessaging::SW_SHOWNORMAL;

/// The document itself, compiled in. See `help.html` at the repo root.
const DOCUMENT: &str = include_str!("../help.html");

/// Where it is written. Beside the settings file, so all of StableSound's
/// files are in one place and the same fallback to `%APPDATA%` applies when
/// the exe's own folder cannot be written to.
pub fn path() -> PathBuf {
    crate::config::config_path().with_file_name("stablesound-help.html")
}

/// Write the help out and hand it to the browser.
///
/// `owner` is the window to hang a failure on, if there is one.
pub fn open_help(owner: Option<HWND>) {
    let path = path();
    if let Err(e) = std::fs::write(&path, DOCUMENT) {
        crate::settings::problem(
            owner,
            &format!(
                "StableSound could not write its help file, so there is nothing to open.\n\
                 \n\
                 It tried to write: {}\n\
                 \n\
                 Windows said: {e}\n\
                 \n\
                 This usually means the folder StableSound is in cannot be written to. Moving \
                 StableSound.exe somewhere in your own user folder, rather than in Program \
                 Files, will fix it.",
                path.display()
            ),
        );
        return;
    }
    // As a `file:///` URL, not as a path. This is measured, not preferred:
    // handing `ShellExecuteW` the path itself put an "Open with" chooser on
    // screen instead of a browser, because this machine's `.html` entry still
    // pointed at Internet Explorer, which Windows 11 does not have. The URL
    // goes through the protocol handler instead - the default browser - and
    // opened correctly.
    //
    // The chooser is the worse of the two failures, because `ShellExecuteW`
    // reports it as success: it did launch something. No amount of checking
    // the return value would have caught it.
    if !shell_open(&file_url(&path)) {
        crate::settings::problem(
            owner,
            &format!(
                "StableSound could not open its help in your browser.\n\
                 \n\
                 The file itself was written, so you can open it yourself. It is at:\n\
                 \n\
                 {}",
                path.display()
            ),
        );
    }
}

/// A path as a `file:///` URL.
///
/// Percent-encoding is deliberately minimal: everything outside the unreserved
/// set, keeping the drive colon and turning both separators into `/`. A space
/// is the case that actually turns up - a user folder with a space in its name
/// is ordinary - and a bare space ends a URL early.
fn file_url(path: &std::path::Path) -> String {
    let mut url = String::from("file:///");
    for c in path.to_string_lossy().chars() {
        match c {
            '\\' | '/' => url.push('/'),
            ':' | '-' | '.' | '_' | '~' => url.push(c),
            c if c.is_ascii_alphanumeric() => url.push(c),
            c => {
                let mut buf = [0u8; 4];
                for byte in c.encode_utf8(&mut buf).as_bytes() {
                    url.push_str(&format!("%{byte:02X}"));
                }
            }
        }
    }
    url
}

/// Hand the log to whatever the user reads text files with.
///
/// The log is how this project's behaviour gets checked, because idle
/// behaviour cannot be watched live: reading the output with a screen reader
/// makes the very sound being measured.
///
/// A missing file almost always means logging is switched off, which is the
/// default, so it says that rather than "file not found" - the thing the user
/// needs to know is which box to tick.
pub fn open_log(owner: Option<HWND>) {
    let path = crate::config::log_path();
    if !path.exists() {
        crate::settings::note(
            owner,
            &format!(
                "There is no log file yet.\n\
                 \n\
                 StableSound does not write one unless you ask it to, so that it leaves nothing \
                 behind on a machine that is working fine.\n\
                 \n\
                 To start one, open the settings and tick \"Write a log file, so a problem can \
                 be looked into later\". If somebody is helping you look into a problem, tick \
                 \"Detailed logging\" as well. The file will then appear at:\n\
                 \n\
                 {}",
                path.display()
            ),
        );
        return;
    }
    // By path, unlike the help: a log is a text file and belongs in whatever
    // reads text files, not in a browser.
    if !shell_open(&path.to_string_lossy()) {
        crate::settings::problem(
            owner,
            &format!(
                "StableSound could not open the log file.\n\
                 \n\
                 It is there, so you can open it yourself in any text editor. It is at:\n\
                 \n\
                 {}",
                path.display()
            ),
        );
    }
}

/// Open a path or a URL with whatever the user has associated with it.
///
/// Returns whether the shell took it. `ShellExecuteW` answers with a number
/// that is an error code at or below 32 and a meaningless positive value above
/// it - a leftover from the days when it really did return an instance handle.
fn shell_open(target: &str) -> bool {
    let wide: Vec<u16> = target.encode_utf16().chain(std::iter::once(0)).collect();
    let verb: Vec<u16> = "open".encode_utf16().chain(std::iter::once(0)).collect();
    let result = unsafe {
        ShellExecuteW(
            None,
            PCWSTR(verb.as_ptr()),
            PCWSTR(wide.as_ptr()),
            PCWSTR::null(),
            PCWSTR::null(),
            SW_SHOWNORMAL,
        )
    };
    result.0 as isize > 32
}

#[cfg(test)]
mod tests {
    use super::file_url;
    use std::path::Path;

    #[test]
    fn a_path_becomes_a_browsable_url() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\me\stablesound-help.html")),
            "file:///C:/Users/me/stablesound-help.html"
        );
    }

    #[test]
    fn a_space_in_a_folder_name_does_not_end_the_url() {
        assert_eq!(
            file_url(Path::new(r"C:\Users\Jo Smith\help.html")),
            "file:///C:/Users/Jo%20Smith/help.html"
        );
    }

    #[test]
    fn a_non_ascii_folder_name_survives_as_utf8() {
        // Percent-encoded byte by byte, which is what a URL wants.
        assert_eq!(
            file_url(Path::new(r"C:\Users\Zoë\help.html")),
            "file:///C:/Users/Zo%C3%AB/help.html"
        );
    }
}
