//! The global hotkey: parsing it, and holding the registration.
//!
//! This is the primary interface, not a shortcut for the tray menu. CLAUDE.md
//! is explicit that every function must be reachable without the tray, because
//! tray menus are awkward with a screen reader.
//!
//! `RegisterHotKey` takes a combination away from every other program on the
//! machine for as long as we hold it, so the default is deliberately obscure.
//! It is also why a bare key with no modifier is refused: registering, say,
//! F12 alone would swallow that key system-wide.

use std::fmt;

use windows::Win32::Foundation::HWND;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT,
    MOD_SHIFT, MOD_WIN,
};

/// The ids we register under. Two combinations are claimed: one to toggle
/// keep-alive, one to open the settings. Both are global, and every global
/// hotkey is taken away from every other program on the machine, so there is
/// no third and both defaults are deliberately obscure.
pub const TOGGLE_ID: i32 = 1;
pub const SETTINGS_ID: i32 = 2;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Hotkey {
    /// `MOD_*` bits, without `MOD_NOREPEAT` - that is added at registration.
    pub modifiers: u32,
    /// Virtual key code.
    pub vk: u32,
}

impl Default for Hotkey {
    fn default() -> Self {
        // Ctrl+Win+F12, chosen by the user. Windows claims a number of
        // Win+Ctrl combinations (the arrows switch virtual desktops, F4 closes
        // one) but not F12, and the Xbox Game Bar squats on Win+Alt rather
        // than Win+Ctrl.
        Hotkey {
            modifiers: MOD_CONTROL.0 | MOD_WIN.0,
            vk: VK_F12,
        }
    }
}

const VK_F11: u32 = 0x7A;
const VK_F12: u32 = 0x7B;

impl Hotkey {
    /// The default for opening the settings: `Ctrl+Win+F11`.
    ///
    /// Next to the toggle key and sharing its modifiers, so the pair is one
    /// thing to remember rather than two, and equally clear of anything
    /// Windows claims.
    pub const fn settings_default() -> Hotkey {
        Hotkey {
            modifiers: MOD_CONTROL.0 | MOD_WIN.0,
            vk: VK_F11,
        }
    }
}

impl Hotkey {
    /// Parse something like `ctrl+win+f12`. Returns `None` for anything it
    /// cannot make sense of, so a mistyped config falls back to the default
    /// rather than leaving the app with no way to toggle.
    pub fn parse(text: &str) -> Option<Hotkey> {
        let mut modifiers = 0u32;
        let mut vk = None;

        for part in text.split('+') {
            let part = part.trim().to_lowercase();
            if part.is_empty() {
                continue;
            }
            match part.as_str() {
                "ctrl" | "control" => modifiers |= MOD_CONTROL.0,
                "alt" => modifiers |= MOD_ALT.0,
                "shift" => modifiers |= MOD_SHIFT.0,
                "win" | "windows" | "super" | "meta" => modifiers |= MOD_WIN.0,
                key => {
                    // A second key, rather than a second modifier, is a
                    // mistake worth rejecting rather than silently ignoring.
                    if vk.is_some() {
                        return None;
                    }
                    vk = Some(key_code(key)?);
                }
            }
        }

        let vk = vk?;
        // At least one modifier: see the module note.
        if modifiers == 0 {
            return None;
        }
        Some(Hotkey { modifiers, vk })
    }

    /// Claim the combination system-wide under `id`. Fails if another program
    /// holds it.
    pub fn register(self, hwnd: HWND, id: i32) -> windows::core::Result<Registration> {
        unsafe {
            // MOD_NOREPEAT so holding the keys down toggles once rather than
            // ten times a second.
            RegisterHotKey(
                Some(hwnd),
                id,
                HOT_KEY_MODIFIERS(self.modifiers) | MOD_NOREPEAT,
                self.vk,
            )?;
        }
        Ok(Registration { hwnd, id })
    }
}

impl fmt::Display for Hotkey {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        // Written in the order people say them, which is also the order
        // Windows documents them in.
        for (bit, name) in [
            (MOD_CONTROL.0, "Ctrl"),
            (MOD_WIN.0, "Win"),
            (MOD_ALT.0, "Alt"),
            (MOD_SHIFT.0, "Shift"),
        ] {
            if self.modifiers & bit != 0 {
                write!(f, "{name}+")?;
            }
        }
        f.write_str(&key_name(self.vk))
    }
}

/// Holds the registration for as long as it lives, so the combination is
/// handed back to the rest of the system on exit.
pub struct Registration {
    hwnd: HWND,
    id: i32,
}

impl Drop for Registration {
    fn drop(&mut self) {
        unsafe {
            let _ = UnregisterHotKey(Some(self.hwnd), self.id);
        }
    }
}

/// Named keys worth allowing beyond letters, digits and function keys. Kept
/// short on purpose: an obscure combination is the point, and every extra name
/// is another thing to get wrong in a config file.
const NAMED: &[(&str, u32)] = &[
    ("space", 0x20),
    ("pause", 0x13),
    ("break", 0x13),
    ("insert", 0x2D),
    ("delete", 0x2E),
    ("home", 0x24),
    ("end", 0x23),
    ("pageup", 0x21),
    ("pagedown", 0x22),
    ("up", 0x26),
    ("down", 0x28),
    ("left", 0x25),
    ("right", 0x27),
];

fn key_code(key: &str) -> Option<u32> {
    if let Some((_, code)) = NAMED.iter().find(|(name, _)| *name == key) {
        return Some(*code);
    }

    let bytes = key.as_bytes();
    // Single letter or digit: the virtual key code is the ASCII value.
    if bytes.len() == 1 {
        let c = bytes[0].to_ascii_uppercase();
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return Some(u32::from(c));
        }
        return None;
    }

    // F1 to F24.
    if let Some(number) = key.strip_prefix('f') {
        if let Ok(n) = number.parse::<u32>() {
            if (1..=24).contains(&n) {
                return Some(0x70 + n - 1);
            }
        }
    }
    None
}

fn key_name(vk: u32) -> String {
    if let Some((name, _)) = NAMED.iter().find(|(_, code)| *code == vk) {
        return (*name).to_string();
    }
    if (0x70..=0x87).contains(&vk) {
        return format!("F{}", vk - 0x70 + 1);
    }
    if let Some(c) = char::from_u32(vk) {
        if c.is_ascii_uppercase() || c.is_ascii_digit() {
            return c.to_string();
        }
    }
    format!("key {vk}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_the_default() {
        assert_eq!(Hotkey::parse("ctrl+win+f12"), Some(Hotkey::default()));
    }

    #[test]
    fn is_forgiving_about_spelling_and_spacing() {
        let wanted = Hotkey::default();
        assert_eq!(Hotkey::parse("Control + Windows + F12"), Some(wanted));
        assert_eq!(Hotkey::parse("WIN+CTRL+F12"), Some(wanted));
    }

    #[test]
    fn round_trips_through_display() {
        for text in ["ctrl+win+f12", "ctrl+alt+shift+s", "win+shift+pageup"] {
            let key = Hotkey::parse(text).expect(text);
            assert_eq!(Hotkey::parse(&key.to_string()), Some(key), "{text}");
        }
    }

    #[test]
    fn refuses_a_key_with_no_modifier() {
        // Would swallow the key for every other program on the machine.
        assert_eq!(Hotkey::parse("f12"), None);
        assert_eq!(Hotkey::parse("a"), None);
    }

    #[test]
    fn refuses_nonsense() {
        assert_eq!(Hotkey::parse("ctrl+notakey"), None);
        assert_eq!(Hotkey::parse("ctrl+f99"), None);
        assert_eq!(Hotkey::parse("ctrl"), None);
        assert_eq!(Hotkey::parse(""), None);
        // Two keys is a mistake, not a combination.
        assert_eq!(Hotkey::parse("ctrl+a+b"), None);
    }

    #[test]
    fn letters_and_digits_map_to_ascii() {
        assert_eq!(Hotkey::parse("ctrl+alt+k").unwrap().vk, u32::from(b'K'));
        assert_eq!(Hotkey::parse("ctrl+alt+7").unwrap().vk, u32::from(b'7'));
    }
}
