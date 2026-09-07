//! One StableSound at a time.
//!
//! This is a Milestone 6 problem, and specifically a problem the console
//! harness used to hide. A second copy started while the first is running does
//! four unhelpful things: it puts a second icon in the notification area, which
//! a screen reader reads out exactly like the live one; it fails to register
//! the global hotkey, because the first copy is holding it; it opens a second
//! keep-alive stream on the same device; and it competes to write the same log
//! and settings files. While there was a console, all of that announced itself
//! in the window that had just appeared. Now nothing would.
//!
//! It is not a far-fetched case either. "Start StableSound when I sign in" has
//! been built since Milestone 4, so the app is normally *already running* by
//! the time the user goes looking for it - and with no window of its own, the
//! obvious way to check is to run the exe again.
//!
//! # Why a mutex rather than looking for the window
//!
//! `FindWindow` on the tray window class would work most of the time and race
//! the rest of it: two copies started together can both look, both find
//! nothing, and both carry on. A named mutex is decided by the kernel at
//! creation, so exactly one caller is ever told it made it.
//!
//! The name is `Local\`-prefixed, which scopes it to the signed-in session
//! rather than the machine. Two users signed in at once should each get their
//! own StableSound: global hotkeys are per-session, so they are not in
//! competition, and each has their own headphones to think about.

use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GetLastError, ERROR_ALREADY_EXISTS, HANDLE};
use windows::Win32::System::Threading::CreateMutexW;

/// Chosen to be unmistakable rather than pretty. Anything shorter risks
/// colliding with an unrelated program's mutex, which would lock StableSound
/// out of starting at all - a far worse failure than the one being prevented.
const NAME: &str = "Local\\StableSound.SingleInstance.1a7f4c6e";

/// Held for as long as this process should be the only StableSound.
///
/// The kernel would release the mutex when the process ends whether this
/// existed or not. It exists so that the handle has an owner with a lifetime,
/// rather than being leaked and explained in a comment.
pub struct Claim(HANDLE);

impl Drop for Claim {
    fn drop(&mut self) {
        unsafe {
            let _ = CloseHandle(self.0);
        }
    }
}

/// Become the one StableSound, or find out somebody already is.
///
/// `None` means another copy is running and this one should stop. Note the
/// order: the mutex is created first and the error code read straight
/// afterwards, because anything in between could overwrite it.
pub fn claim() -> Option<Claim> {
    let name: Vec<u16> = NAME.encode_utf16().chain(std::iter::once(0)).collect();
    unsafe {
        // A handle comes back either way - owning the mutex and merely opening
        // it are the same call - so the last error is the only thing that says
        // which happened.
        let handle = CreateMutexW(None, true, PCWSTR(name.as_ptr())).ok()?;
        if GetLastError() == ERROR_ALREADY_EXISTS {
            let _ = CloseHandle(handle);
            return None;
        }
        Some(Claim(handle))
    }
}
