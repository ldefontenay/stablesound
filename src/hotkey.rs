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

/// The ids we register under. At most two combinations are claimed: one to
/// toggle keep-alive, and one to open the settings if the user has asked for
/// it - it is off by default, because the settings are opened rarely and every
/// global hotkey is taken away from every other program on the machine. There
/// is no third, and both defaults are deliberately obscure.
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

/// Why a typed combination could not be used.
///
/// Separate variants rather than a bare `None` because the Milestone 4 round
/// asked for it: the tester met the refusal message, found that it read well,
/// and then asked that it say how to write a combination correctly. A message
/// that names the actual fault can do that; one written for every fault at
/// once cannot.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum ParseError {
    /// Nothing was typed at all.
    Empty,
    /// A word that is neither a modifier nor a key we know.
    Unknown(String),
    /// Two keys, rather than a key and its modifiers.
    TwoKeys,
    /// Modifiers, but nothing to press with them.
    NoKey,
    /// A key with no modifier. Refused on purpose - see the module note.
    NoModifier,
}

impl ParseError {
    /// The sentence that says what went wrong. The advice on how to write a
    /// combination is added by the caller, and is the same in every case.
    pub fn describe(&self, typed: &str) -> String {
        match self {
            ParseError::Empty => "No hotkey was typed.".to_string(),
            ParseError::Unknown(word) => {
                format!("'{word}' is not a modifier or a key StableSound recognises.")
            }
            ParseError::TwoKeys => format!(
                "'{typed}' names two keys. A hotkey is one key, with modifiers held down \
                 alongside it."
            ),
            ParseError::NoKey => {
                format!("'{typed}' is only modifiers. There has to be a key to press with them.")
            }
            ParseError::NoModifier => format!(
                "'{typed}' has no modifier, and a key on its own is refused on purpose: \
                 registering it would take that key away from every other program on the \
                 machine for as long as StableSound is running."
            ),
        }
    }
}

/// How to write a combination. One string, used by the dialog, the console and
/// the config loader, so the three cannot drift apart.
pub const HOW_TO_WRITE: &str = "\
Write a hotkey as modifiers and one key, joined by plus signs.

The modifiers, spelt any of these ways:
  ctrl   (or control)
  alt
  shift
  win    (or windows, super, meta)

At least one modifier is needed, and you can use several. The order does not \
matter, and neither does capitalisation or spacing: ctrl+win+f12, WIN+CTRL+F12 \
and Control + Windows + F12 are all the same combination.

The key can be a letter, a digit, f1 to f24, or one of: space, pause, break, \
insert, delete, home, end, pageup, pagedown, up, down, left, right.

For example: ctrl+win+f12, ctrl+alt+shift+s, win+shift+pageup";

impl Hotkey {
    /// Parse something like `ctrl+win+f12`, saying why if it cannot.
    ///
    /// Order, capitalisation and spacing are all free; only the modifier
    /// names and the key name have to be recognisable.
    pub fn parse_detailed(text: &str) -> Result<Hotkey, ParseError> {
        let mut modifiers = 0u32;
        let mut vk = None;
        let mut typed_anything = false;

        for part in text.split('+') {
            let part = part.trim().to_lowercase();
            if part.is_empty() {
                continue;
            }
            typed_anything = true;
            match part.as_str() {
                "ctrl" | "control" => modifiers |= MOD_CONTROL.0,
                "alt" => modifiers |= MOD_ALT.0,
                "shift" => modifiers |= MOD_SHIFT.0,
                "win" | "windows" | "super" | "meta" => modifiers |= MOD_WIN.0,
                key => {
                    // A second key, rather than a second modifier, is a
                    // mistake worth rejecting rather than silently ignoring.
                    if vk.is_some() {
                        return Err(ParseError::TwoKeys);
                    }
                    vk = Some(key_code(key).ok_or_else(|| ParseError::Unknown(key.to_string()))?);
                }
            }
        }

        if !typed_anything {
            return Err(ParseError::Empty);
        }
        let vk = vk.ok_or(ParseError::NoKey)?;
        // At least one modifier: see the module note.
        if modifiers == 0 {
            return Err(ParseError::NoModifier);
        }
        Ok(Hotkey { modifiers, vk })
    }

    /// Parse, discarding the reason. For callers that only need to know
    /// whether it worked - a mistyped config falls back to the default rather
    /// than leaving the app with no way to toggle.
    pub fn parse(text: &str) -> Option<Hotkey> {
        Hotkey::parse_detailed(text).ok()
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
    fn each_fault_is_named_rather_than_lumped_together() {
        // The Milestone 4 round asked the refusal message to say how to write
        // a combination correctly. It can only do that if it knows which
        // mistake was made.
        use ParseError::*;
        assert_eq!(Hotkey::parse_detailed(""), Err(Empty));
        assert_eq!(Hotkey::parse_detailed("  +  "), Err(Empty));
        assert_eq!(Hotkey::parse_detailed("ctrl"), Err(NoKey));
        assert_eq!(Hotkey::parse_detailed("ctrl+alt+win"), Err(NoKey));
        assert_eq!(Hotkey::parse_detailed("f12"), Err(NoModifier));
        assert_eq!(Hotkey::parse_detailed("ctrl+a+b"), Err(TwoKeys));
        assert_eq!(
            Hotkey::parse_detailed("ctrl+notakey"),
            Err(Unknown("notakey".into()))
        );
        assert_eq!(
            Hotkey::parse_detailed("ctrl+f99"),
            Err(Unknown("f99".into()))
        );
    }

    #[test]
    fn every_fault_says_something_specific() {
        for typed in ["", "ctrl", "f12", "ctrl+a+b", "ctrl+notakey"] {
            let fault = Hotkey::parse_detailed(typed).unwrap_err();
            let said = fault.describe(typed);
            assert!(!said.is_empty(), "{typed} explained itself with nothing");
            // The advice is added separately and must not be duplicated into
            // each fault, or the message box would say it twice.
            assert!(!said.contains("plus signs"), "{typed}: {said}");
        }
    }

    #[test]
    fn the_advice_covers_every_spelling_the_parser_accepts() {
        // The message and the parser drifting apart is exactly the failure the
        // tester met: a refusal that does not tell you what would work.
        for spelling in [
            "ctrl", "control", "alt", "shift", "win", "windows", "super", "meta",
        ] {
            assert!(
                HOW_TO_WRITE.contains(spelling),
                "{spelling} is accepted but never mentioned"
            );
            assert!(Hotkey::parse(&format!("{spelling}+f12")).is_some());
        }
        for named in NAMED {
            assert!(
                HOW_TO_WRITE.contains(named.0),
                "{} is accepted but never mentioned",
                named.0
            );
        }
        // And it says the thing the tester specifically asked about.
        assert!(HOW_TO_WRITE.contains("order does not"));
    }

    #[test]
    fn the_order_of_modifiers_really_does_not_matter() {
        let wanted = Hotkey::parse("ctrl+alt+shift+win+f9").unwrap();
        for text in [
            "f9+ctrl+alt+shift+win",
            "win+shift+alt+ctrl+f9",
            "shift+f9+win+ctrl+alt",
        ] {
            assert_eq!(Hotkey::parse(text), Some(wanted), "{text}");
        }
    }

    #[test]
    fn letters_and_digits_map_to_ascii() {
        assert_eq!(Hotkey::parse("ctrl+alt+k").unwrap().vk, u32::from(b'K'));
        assert_eq!(Hotkey::parse("ctrl+alt+7").unwrap().vk, u32::from(b'7'));
    }
}
