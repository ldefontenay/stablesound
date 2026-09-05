//! "Start StableSound when I sign in."
//!
//! Deliberately *not* a line in `stablesound.conf`. What actually decides
//! whether the app starts with Windows is a value under the `Run` key, and if
//! the config file carried a copy of that they could disagree - somebody
//! clears the value with an autoruns tool, or copies a config file between
//! machines, and the checkbox then reports something that is not true. So the
//! registry is the single source of truth and the dialog reads it directly.
//!
//! `HKEY_CURRENT_USER` rather than `HKEY_LOCAL_MACHINE`: per-user, needs no
//! elevation, and matches a portable app that lives wherever the user put it.

use std::path::PathBuf;

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("StableSound");

/// Whether Windows is currently set to start us.
pub fn is_enabled() -> bool {
    let Some(key) = open(KEY_READ.0) else {
        return false;
    };
    let mut size = 0u32;
    let result = unsafe { RegQueryValueExW(key, VALUE_NAME, None, None, None, Some(&mut size)) };
    unsafe {
        let _ = RegCloseKey(key);
    }
    result == ERROR_SUCCESS
}

/// Turn it on or off. Returns what went wrong, for reporting rather than
/// failing: this is a convenience, and the app runs perfectly well without it.
pub fn set(on: bool) -> Result<(), String> {
    let key = open(KEY_WRITE.0).ok_or_else(|| "the Run key could not be opened".to_string())?;
    let result = if on {
        // Always rewritten rather than left alone, so moving the exe and
        // ticking the box again repairs the entry rather than leaving it
        // pointing at where the app used to be.
        let command = quoted_exe()?;
        let wide: Vec<u16> = command.encode_utf16().chain(std::iter::once(0)).collect();
        let bytes = unsafe {
            std::slice::from_raw_parts(wide.as_ptr() as *const u8, std::mem::size_of_val(&wide[..]))
        };
        unsafe { RegSetValueExW(key, VALUE_NAME, None, REG_SZ, Some(bytes)) }
    } else {
        let deleted = unsafe { RegDeleteValueW(key, VALUE_NAME) };
        // Removing something that was not there is the state we wanted.
        if deleted == ERROR_SUCCESS {
            deleted
        } else {
            ERROR_SUCCESS
        }
    };
    unsafe {
        let _ = RegCloseKey(key);
    }

    if result == ERROR_SUCCESS {
        Ok(())
    } else {
        Err(format!("Windows reported error {}", result.0))
    }
}

fn open(access: u32) -> Option<HKEY> {
    let mut key = HKEY::default();
    let result = unsafe {
        RegOpenKeyExW(
            HKEY_CURRENT_USER,
            RUN_KEY,
            None,
            windows::Win32::System::Registry::REG_SAM_FLAGS(access),
            &mut key,
        )
    };
    (result == ERROR_SUCCESS).then_some(key)
}

/// The command Windows should run, quoted so a path with spaces survives -
/// which `C:\Program Files\...` and most Desktop paths have.
fn quoted_exe() -> Result<String, String> {
    let path: PathBuf =
        std::env::current_exe().map_err(|e| format!("the program's own path is unknown: {e}"))?;
    Ok(format!("\"{}\"", path.display()))
}
