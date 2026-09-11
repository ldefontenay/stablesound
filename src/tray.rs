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
//! The window exists only to have a message queue. It is never shown. The
//! hotkey and the tray callback both arrive here, and `main` reads them off
//! the queue rather than in a window procedure - which keeps every decision in
//! one readable loop instead of in a callback that would need global state to
//! reach the engine.
//!
//! # Why there is a window procedure after all
//!
//! Reading everything off the queue was the second reason the tray did not
//! work, and the harder one to see. `GetMessage` returns **posted** messages.
//! Messages that are *sent* it dispatches straight to the window procedure
//! while it waits, and never returns to the caller at all. The hotkey and the
//! timer are posted, so they arrived and the loop looked healthy; the shell
//! sends the tray callback, so it went to a procedure that did nothing but call
//! `DefWindowProcW` and was thrown away. Every click, every `Enter`, every
//! Applications key, silently discarded.
//!
//! `wndproc` therefore re-posts the callback to the queue as `WM_TRAY_QUEUED`,
//! which puts the decision back in the loop where it belongs and works whether
//! the shell sends the message or posts it. It is also the right thing to do
//! regardless: opening a modal menu inside a sent message blocks the sender.
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
//!
//! The version 3 mouse messages are still accepted, and a failed
//! `NIM_SETVERSION` is no longer fatal. Handling both costs a few lines and
//! removes a whole class of question from the next hardware round: it no longer
//! matters which protocol is actually in force.

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
    DestroyMenu, DestroyWindow, GetCursorPos, PostMessageW, RegisterClassW, RegisterWindowMessageW,
    SetForegroundWindow, TrackPopupMenu, HICON, MF_SEPARATOR, MF_STRING, TPM_RETURNCMD,
    TPM_RIGHTBUTTON, WINDOW_EX_STYLE, WM_APP, WM_CONTEXTMENU, WM_LBUTTONUP, WM_NULL, WM_RBUTTONUP,
    WNDCLASSW, WS_OVERLAPPED,
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

/// How the pending-icon retry is paced, in ticks of the message loop's 100ms
/// timer and then in seconds of trying.
///
/// A minute is far longer than Explorer needs, and it costs nothing: the retry
/// only runs while the icon is actually missing. What it buys is the race
/// where a shell is up but broadcast `TaskbarCreated` before our window
/// existed to hear it, which is exactly the order a logon task can arrive in.
const ICON_RETRY_TICKS: u32 = 10;
const ICON_RETRY_SECONDS: u32 = 60;

/// Message the shell uses for the tray callback. Anything from `WM_APP` up is
/// ours to define.
pub const WM_TRAY: u32 = WM_APP + 1;

/// The same callback, re-posted by `wndproc` so the message loop can see it.
/// See the module note on sent versus posted messages.
pub const WM_TRAY_QUEUED: u32 = WM_APP + 3;

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
pub const CMD_SETTINGS: i32 = 4;
pub const CMD_HELP: i32 = 5;

/// Icon size. 16x16 is what the notification area asks for; Windows scales it
/// where the display needs something larger.
const SIZE: i32 = 16;

/// The label the icon is registered under, and the first half of what a screen
/// reader reads off it. See `add` for why it is registered separately from the
/// state.
const NAME: &str = "StableSound";

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
    /// Whether `NIM_SETVERSION` was accepted. Decides only where the menu's
    /// anchor point comes from; both protocols are handled either way.
    version4: bool,
    /// When the icon was last activated, and when the menu was last asked for,
    /// so that one press producing two messages is not read as two presses.
    /// See `ECHO`.
    last_select: Option<Instant>,
    last_menu: Option<Instant>,
    /// Set when the shell would not take the icon, which at sign-in means
    /// Explorer is not up yet rather than anything being wrong.
    pending: bool,
    /// Ticks of the message loop's timer since the icon went pending, so the
    /// retry can be paced and eventually given up on.
    tries: u32,
}

/// News about a pending icon, for the one place that reports it.
pub enum IconOutcome {
    /// A shell took the icon. Nothing more to do.
    Added,
    /// It never did. The app carries on - the hotkey is the primary
    /// interface - but this is worth telling somebody about.
    GaveUp,
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

        let mut tray = Tray {
            hwnd,
            // Green while holding the device awake, grey once released.
            on_icon: make_icon(0x2E, 0xB8, 0x4C)?,
            off_icon: make_icon(0x80, 0x80, 0x80)?,
            taskbar_created: unsafe {
                RegisterWindowMessageW(PCWSTR(wide("TaskbarCreated").as_ptr()))
            },
            active: false,
            version4: false,
            last_select: None,
            last_menu: None,
            pending: false,
            tries: 0,
        };
        // Deliberately not fatal. See the module note on signing in before
        // Explorer: the window above is what the hotkey needs, and it exists
        // by now whatever the shell is doing.
        tray.pending = tray.add().is_err();
        Ok(tray)
    }

    /// Whether the icon is still waiting for a shell to put it in.
    pub fn is_icon_pending(&self) -> bool {
        self.pending
    }

    /// Try again for a pending icon, and say so once there is news.
    ///
    /// Called from the message loop's existing tick rather than from a timer
    /// of its own. Cheap: one `Shell_NotifyIcon` a second at most, and only
    /// while the icon is actually missing.
    pub fn poll_icon(&mut self) -> Option<IconOutcome> {
        if !self.pending {
            return None;
        }
        self.tries += 1;
        // The tick is 100ms, so this is one attempt a second.
        if !self.tries.is_multiple_of(ICON_RETRY_TICKS) {
            return None;
        }
        if self.add().is_ok() {
            self.pending = false;
            return Some(IconOutcome::Added);
        }
        if self.tries >= ICON_RETRY_TICKS * ICON_RETRY_SECONDS {
            self.pending = false;
            return Some(IconOutcome::GaveUp);
        }
        None
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
        self.pending = self.add().is_err();
        self.tries = 0;
    }

    /// Reflect the current state in the icon and its tooltip.
    ///
    /// The tooltip carries the state in words, which is what a screen reader
    /// reads; the colour is for everyone else.
    pub fn set_active(&mut self, active: bool) {
        self.active = active;
        let _ = self.notify(NIM_MODIFY);
    }

    fn add(&mut self) -> WinResult<()> {
        // Added under the app's name rather than its state, which matters more
        // than it looks. Windows 11 keeps whatever `szTip` said at `NIM_ADD` as
        // the icon's label, and a screen reader is then given "<label>
        // <tooltip>" for the rest of the session. Adding it as "StableSound:
        // headphones free" therefore made switching keep-alive on read as
        // "StableSound: headphones free StableSound: headphones awake", which
        // is what the Milestone 5 round reported. Registering the name once and
        // leaving the state to the tooltip gives "StableSound Headphones
        // awake" - the shape every built-in icon already uses, as in "Volume
        // Headphones (soundcore AeroClip): 44%".
        //
        // Measured on Windows 11 build 26200. Microsoft's own Windows Security
        // icon reads "Windows Security - No actions needed. Windows Security -
        // Actions recommended." on the same machine, so this is the shell's
        // rule and not something we can switch off.
        self.notify_with_tip(NIM_ADD, NAME)?;

        // Must follow the add, and must happen before any keyboard use. See
        // the module note: without it the icon is deaf to everything but the
        // mouse. Not fatal if it is refused - the version 3 messages are
        // handled too - but worth knowing about, so `create` reports it.
        let mut data = NOTIFYICONDATAW {
            cbSize: size_of::<NOTIFYICONDATAW>() as u32,
            hWnd: self.hwnd,
            uID: ICON_ID,
            ..Default::default()
        };
        data.Anonymous.uVersion = NOTIFYICON_VERSION_4;
        self.version4 = unsafe { Shell_NotifyIconW(NIM_SETVERSION, &data) }.as_bool();

        // Only now say what the state is. The label is cached by this point, so
        // this and every later change move the second half of the name alone.
        self.notify(NIM_MODIFY)?;
        Ok(())
    }

    /// Whether the shell accepted version 4. Reported at startup: if the tray
    /// misbehaves again, this is the first thing worth knowing.
    pub fn is_version4(&self) -> bool {
        self.version4
    }

    fn notify(&self, action: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE) -> WinResult<()> {
        // Short on purpose. The Milestone 3 verdict on the first attempt was
        // simply "too verbose". The name of the app is deliberately absent -
        // the shell puts it in front of this; see `NAME`.
        let tip = if self.active {
            "Headphones awake"
        } else {
            "Headphones free"
        };
        self.notify_with_tip(action, tip)
    }

    /// The one place `Shell_NotifyIconW` is called, so the flags and the icon
    /// are decided once.
    fn notify_with_tip(
        &self,
        action: windows::Win32::UI::Shell::NOTIFY_ICON_MESSAGE,
        tip: &str,
    ) -> WinResult<()> {
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
        for (slot, ch) in data.szTip.iter_mut().zip(tip.encode_utf16()) {
            *slot = ch;
        }

        unsafe { Shell_NotifyIconW(action, &data).ok() }
    }

    /// Turn a callback message into what the user meant by it.
    ///
    /// Version 4 layout: `wParam` holds the anchor point, `lParam` holds the
    /// event in its low word and the icon id in its high word. Note that this
    /// is the reverse of the version 3 layout, where `wParam` was the id - so
    /// the anchor is only trustworthy when version 4 was accepted.
    ///
    /// Both protocols are accepted. The version 3 mouse messages should not
    /// arrive once version 4 is in force, and the echo guard makes it harmless
    /// if they do.
    pub fn decode(&mut self, wparam: WPARAM, lparam: LPARAM) -> Option<TrayEvent> {
        let now = Instant::now();
        match callback_event(lparam) {
            NIN_SELECT | NIN_KEYSELECT | WM_LBUTTONUP => {
                if fresh(&mut self.last_select, now) {
                    Some(TrayEvent::Toggle)
                } else {
                    None
                }
            }
            WM_CONTEXTMENU | WM_RBUTTONUP => {
                if fresh(&mut self.last_menu, now) {
                    let at = self.anchor(wparam);
                    Some(TrayEvent::Menu { x: at.x, y: at.y })
                } else {
                    None
                }
            }
            _ => None,
        }
    }

    /// Where to put the menu. Under version 4 the shell hands us the icon's own
    /// position, which is what makes a keyboard-opened menu appear beside the
    /// icon; without it there is nothing to go on but the pointer.
    fn anchor(&self, wparam: WPARAM) -> POINT {
        if self.version4 {
            return POINT {
                x: loword(wparam.0 as u32),
                y: hiword(wparam.0 as u32),
            };
        }
        let mut point = POINT { x: 0, y: 0 };
        unsafe {
            let _ = GetCursorPos(&mut point);
        }
        point
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
            // Asked for by name in the third hardware round. The dialog has
            // a hotkey of its own as well - CLAUDE.md does not allow the tray
            // to be the only route to anything.
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                CMD_SETTINGS as usize,
                PCWSTR(wide("&Settings...").as_ptr()),
            );
            // Both of these are in the settings dialog as well. The tray is
            // never the only route to anything - CLAUDE.md constraint 3 - and
            // with the console gone these two would otherwise have been.
            let _ = AppendMenuW(
                menu,
                MF_STRING,
                CMD_HELP as usize,
                PCWSTR(wide("&Help").as_ptr()),
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
        remove_icon(self.hwnd);
        unsafe {
            let _ = DestroyIcon(self.on_icon);
            let _ = DestroyIcon(self.off_icon);
            let _ = DestroyWindow(self.hwnd);
        }
    }
}

/// Take the icon out of the notification area.
///
/// Without this the icon stays there as a ghost pointing at a dead window: it
/// does nothing when activated, and a screen reader still reads it out, so the
/// next run is easy to mistake for the corpse of the last one.
///
/// Milestone 3 also hung this off a console control handler, for the case
/// where closing the console window killed the process without unwinding.
/// Milestone 6 removed the console, and with it that route: the only ways out
/// now are `Exit`, which unwinds, and being terminated outright, which no
/// handler of ours would survive either.
fn remove_icon(hwnd: HWND) {
    let data = NOTIFYICONDATAW {
        cbSize: size_of::<NOTIFYICONDATAW>() as u32,
        hWnd: hwnd,
        uID: ICON_ID,
        ..Default::default()
    };
    unsafe {
        let _ = Shell_NotifyIconW(NIM_DELETE, &data);
    }
}

/// The notification event, in the low word of `lParam` under both protocols.
/// Public so the caller can log what actually arrived.
pub fn callback_event(lparam: LPARAM) -> u32 {
    (lparam.0 as u32) & 0xFFFF
}

/// True if this is a real press rather than a second message for the same one.
///
/// The shell has a long-standing habit of sending `NIN_KEYSELECT` twice for a
/// single `Enter`, and accepting both version 3 and version 4 gives a second
/// way for one press to arrive twice. Two toggles in a row cancel out, which
/// looks exactly like an icon that does nothing - the very symptom being fixed.
fn fresh(last: &mut Option<Instant>, now: Instant) -> bool {
    if last.is_some_and(|at| now - at < ECHO) {
        return false;
    }
    *last = Some(now);
    true
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

/// The two messages that cannot be handled in the loop, and nothing else.
///
/// Both are here for the same reason: `GetMessage` hands sent messages straight
/// to this procedure and never returns them to the caller, so anything the
/// system sends rather than posts is invisible to a loop-only design. See the
/// module note.
unsafe extern "system" fn wndproc(
    hwnd: HWND,
    message: u32,
    wparam: WPARAM,
    lparam: LPARAM,
) -> LRESULT {
    match message {
        // Put it back on the queue, where the loop can make the decision with
        // the engine in reach. Also the correct order of events for the menu:
        // showing a modal popup inside a sent message blocks whoever sent it.
        WM_TRAY => {
            unsafe {
                let _ = PostMessageW(Some(hwnd), WM_TRAY_QUEUED, wparam, lparam);
            }
            LRESULT(0)
        }
        _ => unsafe { DefWindowProcW(hwnd, message, wparam, lparam) },
    }
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
