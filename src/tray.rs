//! The tray icon, its menu, and the hidden window that receives messages.
//!
//! A convenience, never the only route to anything. CLAUDE.md is explicit that
//! tray menus are awkward with a screen reader and that the global hotkey is
//! the primary interface. Everything here is reachable another way.
//!
//! It still earns its place: it gives sighted users somewhere to look, it gives
//! everybody a visible way to quit, and the notification area is reachable from
//! the keyboard with `Win+B`. The menu is a real Win32 menu, so JAWS reads it
//! without any help from us.
//!
//! The window exists only to have a message queue. It is never shown. Both the
//! hotkey and the tray callback post to it, and `main` reads them straight off
//! the queue rather than in a window procedure - which keeps every decision in
//! one readable loop instead of in a callback that would need global state to
//! reach the engine.
//!
//! # Version 4, and why it is not optional
//!
//! `NIM_SETVERSION` is the whole reason the keyboard works. Without it the
//! shell speaks the original Windows 95 protocol, in which a tray icon is told
//! about mouse buttons and nothing else - so an icon reached with `Win+B` and
//! then activated with `Enter`, or asked for its menu with the Applications
//! key, produces no message whatsoever. Milestone 3 testing found exactly that:
//! "No context menu appeared when pressing the applications key or f10. [...]
//! Pressing enter also did nothing."
//!
//! Version 4 adds `NIN_SELECT`, `NIN_KEYSELECT` and `WM_CONTEXTMENU`, which are
//! the keyboard's route in, and it puts the icon's own screen position in
//! `wParam` so the menu can be anchored to the icon rather than to wherever the
//! mouse pointer happens to be sitting. For an app whose primary user never
//! touches a mouse, both matter.

use std::mem::size_of;
use std::time::{Duration, Instant};

use windows::core::{Result as WinResult, PCWSTR};
use windows::Win32::Foundation::{HWND, LPARAM, LRESULT, POINT, WPARAM};
use windows::Win32::System::LibraryLoader::GetModuleHandleW;
use windows::Win32::UI::Shell::{
    Shell_NotifyIconW, NIF_ICON, NIF_MESSAGE, NIF_SHOWTIP, NIF_TIP, NIM_ADD, NIM_DELETE,
    NIM_MODIFY, NIM_SETVERSION, NINF_KEY, NIN_SELECT, NOTIFYICONDATAW, NOTIFYICON_VERSION_4,
};
use windows::Win32::UI::WindowsAndMessaging::{
    AppendMenuW, CreateIcon, CreatePopupMenu, CreateWindowExW, DefWindowProcW, DestroyIcon,
    DestroyMenu, DestroyWindow, PostMessageW, RegisterClassW, RegisterWindowMessageW,
    SetForegroundWindow, TrackPopupMenu, HICON, MF_SEPARATOR, MF_STRING, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU, WM_NULL, WNDCLASSW, WS_OVERLAPPED,
};

/// Sent when the icon is activated with `Enter`. Defined by shellapi.h as
/// `NIN_SELECT | NINF_KEY`; the windows crate exposes both halves but not the
/// combination.
const NIN_KEYSELECT: u32 = NIN_SELECT | NINF_KEY;

/// Our one icon.
const ICON_ID: u32 = 1;

/// How close together two activations have to be before the second is taken
/// for an echo rather than a second press.
///
/// The shell has a long-standing habit of sending `NIN_KEYSELECT` twice for a
/// single `Enter`. Two toggles in a row cancel out, which would look exactly
/// like the bug this version-4 work is fixing - the icon doing nothing - so it
/// is worth guarding rather than discovering on the next hardware round.
/// Comfortably longer than any echo, comfortably shorter than a deliberate
/// second press.
const ECHO: Duration = Duration::from_millis(250);

/// Message the tray icon posts to our window. Anything from `WM_APP` up is
/// ours to define.
pub const WM_TRAY: u32 = WM_APP + 1;

/// What the user asked the icon for.
pub enum TrayEvent {
    /// Activated: a left click, or `Enter` or `Space` on a focused icon.
    Toggle,
    /// Wants the menu, anchored at this point in screen coordinates. The shell
    /// supplies the icon's own position, which is what makes a keyboard-opened
    /// menu appear next to the icon instead of next to the mouse.
    Menu { x: i32, y: i32 },
}

/// Menu command ids. Also the values `TrackPopupMenu` hands back.
pub const CMD_TOGGLE: i32 = 1;
pub const CMD_OPEN_LOG: i32 = 2;
pub const CMD_QUIT: i32 = 3;

/// Icon size. 16x16 is what the notification area asks for; Windows scales it
/// where the display needs something larger.
const SIZE: i32 = 16;

pub struct Tray {
    hwnd: HWND,
    /// Icons for the two states, so a sighted user can tell at a glance.
    on_icon: HICON,
    off_icon: HICON,
    /// Message Explorer broadcasts when it restarts and every tray icon has to
    /// be added again. Without handling it, the icon vanishes for good the
    /// first time Explorer crashes.
    taskbar_created: u32,
    active: bool,
    /// When the icon was last activated, so a repeated `NIN_KEYSELECT` is not
    /// taken for a second press. See `ECHO`.
    last_select: Option<Instant>,
}

impl Tray {
    /// Create the hidden window and add the icon.
    pub fn create() -> WinResult<Tray> {
        let hwnd = unsafe {
            let instance = GetModuleHandleW(None)?;
            let class_name = wide("StableSoundTray");

            let class = WNDCLASSW {
                lpfnWndProc: Some(wndproc),
                hInstance: instance.into(),
                lpszClassName: PCWSTR(class_name.as_ptr()),
                ..Default::default()
            };
            // A zero return means the class could not be registered. There is
            // no recovery from that, and no icon without a window.
            if RegisterClassW(&class) == 0 {
                return Err(windows::core::Error::from_thread());
            }

            CreateWindowExW(
                WINDOW_EX_STYLE(0),
                PCWSTR(class_name.as_ptr()),
                PCWSTR(wide("StableSound").as_ptr()),
                WS_OVERLAPPED,
                0,
                0,
                0,
                0,
                None,
                None,
                Some(instance.into()),
                None,
            )?
        };

        let tray = Tray {
            hwnd,
            // Green while holding the device awake, grey once released.
            on_icon: make_icon(0x2E, 0xB8, 0x4C)?,
            off_icon: make_icon(0x80, 0x80, 0x80)?,
            taskbar_created: unsafe {
                RegisterWindowMessageW(PCWSTR(wide("TaskbarCreated").as_ptr()))
            },
            active: false,
            last_select: None,
        };
        tray.add()?;
        Ok(tray)
    }

    pub fn hwnd(&self) -> HWND {
        self.hwnd
    }

    /// True if this message means Explorer restarted and the icon needs
    /// putting back.
    pub fn is_taskbar_restart(&self, message: u32) -> bool {
        // A registered message id of 0 means registration failed; never match
        // that, or every WM_NULL would look like a restart.
        self.taskbar_created != 0 && message == self.taskbar_created
    }

    pub fn readd(&mut self) {
        let _ = self.add();
    }

    /// Reflect the current state in the icon and its tooltip.
    ///
    /// The tooltip carries the state in words, which is what a screen reader
    /// reads; the colour is for everyone else.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        let _ = self.notify(NIM_MODIFY);
    }

    fn add(&self) -> WinResult<()> {
        self.notify(NIM_ADD)?;

        // Must follow the add, and must happen before any keyboard use. See
        // the module note: without it the icon is deaf to everything but the
        // mouse.
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ICON_ID,
            ..Default::default()
        };
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data).ok() }
    }

    fn notify(&self, action: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) -> WinResult<()> {
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ICON_ID,
            // NIF_SHOWTIP is needed under version 4 to keep the ordinary
            // tooltip; without it the shell assumes the app draws its own.
            uFlags: NIF_ICON | NIF_MESSAGE | NIF_TIP | NIF_SHOWTIP,
            uCallbackMessage: WM_TRAY,
            hIcon: if self.active {
                self.on_icon
            } else {
                self.off_icon
            },
            ..Default::default()
        };

        // Short on purpose. This string is the icon's accessible name, so it is
        // read out in full every time the user arrows onto the icon - the
        // Milestone 3 verdict on the first attempt was simply "too verbose".
        let tip = if self.active {
            "StableSound: headphones awake"
        } else {
            "StableSound: headphones free"
        };
        for (slot, ch) in data.szTip.iter_mut().zip(tip.encode_utf16()) {
            *slot = ch;
        }

        unsafe { Shell_NotifyIconW(action, &data).ok() }
    }

    /// Turn a callback message into what the user meant by it.
    ///
    /// Version 4 layout: `wParam` holds the anchor point, `lParam` holds the
    /// event in its low word and the icon id in its high word. Note that this
    /// is the reverse of the version 3 layout, where `wParam` was the id.
    pub fn decode(&mut self, wparam: WPARAM, lparam: LPARAM) -> Option<TrayEvent> {
        match (lparam.0 as u32) & 0xFFFF {
            NIN_SELECT | NIN_KEYSELECT => {
                let now = Instant::now();
                if self.last_select.is_some_and(|at| now - at < ECHO) {
                    return None;
                }
                self.last_select = Some(now);
                Some(TrayEvent::Toggle)
            }
            // Both the right mouse button and the Applications key arrive here
            // under version 4; WM_RBUTTONUP is deliberately not handled as
            // well, or a right click would open two menus in a row.
            WM_CONTEXTMENU => Some(TrayEvent::Menu {
                x: loword(wparam.0 as u32),
                y: hiword(wparam.0 as u32),
            }),
            _ => None,
        }
    }

    /// Show the context menu at a point and return the command the user picked.
    ///
    /// Blocks until the menu closes. `TPM_RETURNCMD` hands the id straight
    /// back, which avoids routing `WM_COMMAND` through a window procedure that
    /// would then need a way to reach the engine.
    pub fn show_menu(&self, at: POINT) -> Option<i32> {
        unsafe {
            let menu = CreatePopupMenu().ok()?;
            let toggle = if self.active {
                "&Release the headphones"
            } else {
                "&Keep the headphones awake"
            };
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                CMD_TOGGLE as usize,
                PCWSTR(wide(toggle).as_ptr()),
            );
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                CMD_OPEN_LOG as usize,
                PCWSTR(wide("Open the &log file").as_ptr()),
            );
            let _ = AppendMenuW(menu, MF_SEPARATOR, 0, PCWSTR::null());
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                CMD_QUIT as usize,
                PCWSTR(wide("E&xit StableSound").as_ptr()),
            );

            // Both of these are long-standing tray menu requirements. Without
            // the first the menu cannot take focus; without the second it
            // refuses to close when the user clicks elsewhere.
            let _ = SetForegroundWindow(self.hwnd);
            let choice = TrackPopupMenu(
                menu,
                TPM_RETURNCMD | TPM_RIGHTBUTTON,
                at.x,
                at.y,
                None,
                self.hwnd,
                None,
            );
            let _ = PostMessageW(Some(self.hwnd), WM_NULL, WPARAM(0), LPARAM(0));
            let _ = DestroyMenu(menu);

            match choice.0 {
                0 => None,
                id => Some(id),
            }
        }
    }
}

impl Drop for Tray {
    fn drop(&mut self) {
        unsafe {
            let data = NOTIFYICONDATAW {
                cbSize: size_of::<NOTIFYICONDATAW>() as u32,
                hWnd: self.hwnd,
                uID: ICON_ID,
                ..Default::default()
            };
            // Without this the icon stays in the notification area as a ghost
            // until something makes Windows notice the process is gone.
            let _ = Shell_NotifyIconW(NIM_DELETE, &data);
            let _ = DestroyIcon(self.on_icon);
            let _ = DestroyIcon(self.off_icon);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// A filled circle in the given colour.
///
/// Drawn in code rather than embedded as an `.ico`. The project has a hard size
/// budget, this is a couple of hundred bytes of logic against a few kilobytes
/// of resource, and a solid disc is all the detail a 16-pixel icon can hold
/// anyway. The state is carried by the tooltip in words regardless, since that
/// is what a screen reader reads.
fn make_icon(r: u8, g: u8, b: u8) -> WinResult<HICON> {
    // 1 bit per pixel, 16 pixels to a row, so exactly two bytes per row. A set
    // bit means "leave the screen alone", which is how the corners outside the
    // circle stay transparent.
    let mut and_mask = [0xFFu8; (SIZE * SIZE / 8) as usize];
    // 32 bits per pixel, laid out blue, green, red, alpha.
    let mut xor_bits = [0u8; (SIZE * SIZE * 4) as usize];

    let centre = (SIZE as f32 - 1.0) / 2.0;
    let radius = SIZE as f32 / 2.0 - 0.5;

    for y in 0..SIZE {
        for x in 0..SIZE {
            let dx = x as f32 - centre;
            let dy = y as f32 - centre;
            if dx * dx + dy * dy > radius * radius {
                continue;
            }
            let index = (y * SIZE + x) as usize;
            // Clear the mask bit so this pixel is drawn rather than skipped.
            and_mask[index / 8] &= !(0x80 >> (index % 8));
            xor_bits[index * 4] = b;
            xor_bits[index * 4 + 1] = g;
            xor_bits[index * 4 + 2] = r;
            // Opaque, for the paths that honour alpha instead of the mask.
            xor_bits[index * 4 + 3] = 0xFF;
        }
    }

    unsafe {
        CreateIcon(
            None,
            SIZE,
            SIZE,
            1,
            32,
            and_mask.as_ptr(),
            xor_bits.as_ptr(),
        )
    }
}

/// Nothing interesting happens here. Every message this app cares about is
/// read directly off the queue in `main`, so the window procedure only has to
/// satisfy Windows.
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    DefWindowProcW(hwnd, message, wparam, lparam)
}

/// Screen coordinates arrive packed into one word each, and can be negative on
/// a multi-monitor desktop, so they have to go through `i16` rather than being
/// masked straight into an `i32`.
fn loword(packed: u32) -> i32 {
    i32::from(packed as u16 as i16)
}

fn hiword(packed: u32) -> i32 {
    i32::from((packed >> 16) as u16 as i16)
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(std::iter::once(0)).collect()
}
