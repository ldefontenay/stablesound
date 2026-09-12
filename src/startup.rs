//! "Start StableSound when I sign in."
//!
//! Two mechanisms, tried in that order, because the tester's only route to the
//! screen is sound and the first announcement of a session is the one that
//! matters most:
//!
//! 1. **A Task Scheduler logon task.** Fires around the logon event itself.
//! 2. **A value under the `Run` key.** The fallback, and what this module did
//!    on its own until Milestone 7.
//!
//! The `Run` key is not slow because StableSound is slow - it starts in a
//! fraction of a second. It is slow because Explorer works through `Run`
//! *after* the desktop is ready, with a delay before it begins and throttling
//! between the entries it finds, and the order within it is not something an
//! application can ask for. A logon task is not Explorer's to delay. The
//! Milestone 7 round measured the `Run` key at about 30 seconds on this
//! machine, with the first thing JAWS said clipped; see PLAN.md 6.7.
//!
//! Registering a task can be refused, which is the whole reason the `Run` key
//! is still here. It is refused as an `HRESULT` from a COM call, not as a UAC
//! prompt - nothing in this module can put a prompt on screen at sign-in.
//! Whichever of the two is actually in force is what `current` reports, so the
//! checkbox in the settings can never claim StableSound will start when it
//! will not.
//!
//! Deliberately *not* a line in `stablesound.conf`. What decides whether the
//! app starts with Windows lives in Windows, and a copy in the config file
//! could disagree with it - somebody clears the entry with an autoruns tool,
//! or copies a config file between machines, and the checkbox then reports
//! something untrue. Windows is the single source of truth and the dialog
//! reads it directly.
//!
//! `HKEY_CURRENT_USER` and an interactive-token task rather than anything
//! machine-wide: per-user, needs no elevation, and matches a portable app that
//! lives wherever the user put it.

use std::path::{Path, PathBuf};

use windows::core::{w, PCWSTR};
use windows::Win32::Foundation::ERROR_SUCCESS;
use windows::Win32::System::Com::{
    CoCreateInstance, CoInitializeEx, CLSCTX_INPROC_SERVER, COINIT_APARTMENTTHREADED,
};
use windows::Win32::System::Registry::{
    RegCloseKey, RegDeleteValueW, RegOpenKeyExW, RegQueryValueExW, RegSetValueExW, HKEY,
    HKEY_CURRENT_USER, KEY_READ, KEY_WRITE, REG_SZ,
};
use windows::Win32::System::TaskScheduler::{
    ITaskService, TaskScheduler, TASK_CREATE_OR_UPDATE, TASK_LOGON_INTERACTIVE_TOKEN,
};
use windows::Win32::System::Variant::VARIANT;

const RUN_KEY: PCWSTR = w!("Software\\Microsoft\\Windows\\CurrentVersion\\Run");
const VALUE_NAME: PCWSTR = w!("StableSound");

/// The task's name in the root folder of the Task Scheduler library.
///
/// Every function that touches a task takes the name as an argument rather
/// than reading this directly, so that the round-trip test at the bottom of
/// this file can exercise the real COM calls under a name of its own instead
/// of trampling the task a user is relying on.
const TASK_NAME: &str = "StableSound";

/// How StableSound is actually being started.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Method {
    /// A Task Scheduler logon task. What we want.
    Task,
    /// A value under the `Run` key. Works, but late.
    Run,
}

impl Method {
    /// A phrase for the log and for the one message the dialog may have to
    /// show. Written to be read aloud.
    pub fn describe(self) -> &'static str {
        match self {
            Method::Task => "a scheduled task that runs at sign-in",
            Method::Run => "an entry under the registry Run key",
        }
    }
}

/// What happened when autostart was switched on.
pub struct Enabled {
    pub method: Method,
    /// Why the scheduled task could not be used, when it could not. `None`
    /// means the task was registered and nothing needs saying.
    pub fell_back: Option<String>,
}

/// Whether Windows is currently set to start *this* copy of StableSound.
///
/// False for an entry that points somewhere else, which is the honest answer
/// and also the self-healing one: the box reads clear, and ticking it registers
/// this copy. An entry left behind by an exe that has since been moved or
/// deleted would otherwise be a box that says yes while nothing starts.
pub fn is_enabled() -> bool {
    current().is_some()
}

/// Which mechanism is in force for this copy, if either. The task wins if both
/// somehow exist - it is the one that fires first.
pub fn current() -> Option<Method> {
    let me = exe_path().ok()?;
    if task_command(TASK_NAME).is_some_and(|c| same_path(&c, &me)) {
        return Some(Method::Task);
    }
    if run_value().is_some_and(|c| same_path(&unquote(&c), &me)) {
        return Some(Method::Run);
    }
    None
}

/// Turn autostart on or off.
///
/// `Ok(Some(..))` when it was switched on, saying which mechanism took effect;
/// `Ok(None)` when it was switched off. Errors are for reporting rather than
/// for failing on: this is a convenience, and the app runs perfectly well
/// without it.
pub fn set(on: bool) -> Result<Option<Enabled>, String> {
    if on {
        enable().map(Some)
    } else {
        disable().map(|()| None)
    }
}

/// Register the logon task, and fall back to the `Run` key if that is refused.
///
/// Exactly one of the two is left in place. Both at once would start two
/// copies, and while `instance::claim` stops the second it does so with a
/// message window - at sign-in, in front of somebody who cannot see it.
fn enable() -> Result<Enabled, String> {
    let exe = exe_path()?;

    match register_task(TASK_NAME, &exe) {
        Ok(()) => {
            // The task is in force now, so the older route must go or it will
            // start a second copy a few seconds later.
            let _ = write_run(false);
            Ok(Enabled {
                method: Method::Task,
                fell_back: None,
            })
        }
        Err(why) => {
            // Never leave a half-registered task behind claiming to be the
            // mechanism in force.
            let _ = delete_task(TASK_NAME);
            write_run(true)?;
            Ok(Enabled {
                method: Method::Run,
                fell_back: Some(why),
            })
        }
    }
}

/// Remove both mechanisms. Removing something that was not there is the state
/// we wanted, so only a real refusal is an error.
fn disable() -> Result<(), String> {
    let _ = delete_task(TASK_NAME);
    write_run(false)
}

/// Move an existing `Run` entry up to a scheduled task, once, at startup.
///
/// Without this, anybody who ticked the box before Milestone 7 keeps the slow
/// route for ever: the dialog only calls `set` when the checkbox *changes*, and
/// for them it is already ticked. Returns a line for the log, or `None` when
/// there was nothing to do.
///
/// Safe to attempt on every run. The worst case is a refused COM call and the
/// `Run` entry staying exactly as it was, which is what the app shipped with.
pub fn upgrade_run_to_task() -> Option<String> {
    if current() != Some(Method::Run) {
        return None;
    }
    let exe = exe_path().ok()?;
    match register_task(TASK_NAME, &exe) {
        Ok(()) => {
            let _ = write_run(false);
            Some("autostart: moved up from the Run key to a scheduled task, so the next sign-in starts StableSound earlier".to_string())
        }
        Err(why) => Some(format!(
            "autostart: still the Run key - a scheduled task could not be registered ({why})"
        )),
    }
}

// -- the scheduled task ---------------------------------------------------

/// Connect to the Task Scheduler service. Every call in this module goes
/// through here, so the COM setup is written once.
fn service() -> Result<ITaskService, String> {
    unsafe {
        // Harmless on a thread that is already initialised - it returns
        // S_FALSE and we never uninitialise, so we cannot pull the apartment
        // out from under the caller. The main thread is an STA already; this
        // is here so the module does not depend on having been called from it.
        let _ = CoInitializeEx(None, COINIT_APARTMENTTHREADED);

        let service: ITaskService = CoCreateInstance(&TaskScheduler, None, CLSCTX_INPROC_SERVER)
            .map_err(|e| format!("the Task Scheduler service is unreachable: {}", brief(&e)))?;
        // Empty arguments mean the local machine as the current user.
        service
            .Connect(
                &VARIANT::default(),
                &VARIANT::default(),
                &VARIANT::default(),
                &VARIANT::default(),
            )
            .map_err(|e| format!("connecting to Task Scheduler failed: {}", brief(&e)))?;
        Ok(service)
    }
}

/// Create or replace the logon task. Always a rewrite rather than a check:
/// moving the exe and ticking the box again then repairs the task instead of
/// leaving it pointing at where the app used to be.
fn register_task(name: &str, exe: &Path) -> Result<(), String> {
    let service = service()?;
    unsafe {
        let folder = service
            .GetFolder(&"\\".into())
            .map_err(|e| format!("the Task Scheduler library is unreadable: {}", brief(&e)))?;
        let task = service
            .NewTask(0)
            .map_err(|e| format!("a task could not be built: {}", brief(&e)))?;
        task.SetXmlText(&task_xml(name, exe).into())
            .map_err(|e| format!("the task description was rejected: {}", brief(&e)))?;
        folder
            .RegisterTaskDefinition(
                &name.into(),
                &task,
                TASK_CREATE_OR_UPDATE.0,
                // Empty user and password with an interactive token: the task
                // runs as whoever registered it, needing no stored password
                // and no elevation.
                &VARIANT::default(),
                &VARIANT::default(),
                TASK_LOGON_INTERACTIVE_TOKEN,
                &VARIANT::default(),
            )
            .map_err(|e| format!("Windows refused to register it: {}", brief(&e)))?;
    }
    Ok(())
}

fn delete_task(name: &str) -> Result<(), String> {
    let service = service()?;
    unsafe {
        let folder = service
            .GetFolder(&"\\".into())
            .map_err(|e| format!("the Task Scheduler library is unreadable: {}", brief(&e)))?;
        folder
            .DeleteTask(&name.into(), 0)
            .map_err(|e| format!("the task could not be removed: {}", brief(&e)))?;
    }
    Ok(())
}

/// The command the registered task runs, if there is a task at all.
fn task_command(name: &str) -> Option<PathBuf> {
    let service = service().ok()?;
    let xml = unsafe {
        let folder = service.GetFolder(&"\\".into()).ok()?;
        let task = folder.GetTask(&task_path(name).into()).ok()?;
        task.Xml().ok()?.to_string()
    };
    // Read back as text rather than through the action collection. One call
    // instead of four, and the element we want is unambiguous: the task has a
    // single `Exec` action because we are the only thing that writes it.
    element(&xml, "Command").map(PathBuf::from)
}

/// The task's own description of itself.
///
/// Schema version 1.2, so nothing here needs a Windows newer than the ones
/// `stablesound.manifest` claims support for. Two settings matter more than
/// they look:
///
/// - `ExecutionTimeLimit` **must** be `PT0S`. The default is three days, and
///   Task Scheduler ends a task that outlives its limit - which for an app
///   that is meant to sit in the tray indefinitely means keep-alive silently
///   stopping after a long uptime.
/// - The battery settings must both be off. This is a laptop, and a task that
///   refuses to start on battery, or stops when the charger comes out, is a
///   screen reader going quiet at the worst moment.
fn task_xml(name: &str, exe: &Path) -> String {
    let uri = escape(&task_path(name));
    let command = escape(&exe.display().to_string());
    let folder = exe
        .parent()
        .map(|p| escape(&p.display().to_string()))
        .unwrap_or_default();
    // Naming the user keeps the trigger to this account. Left out if the
    // environment cannot say who that is, in which case the trigger fires for
    // any logon - harmless, because the task still runs only with this user's
    // interactive token and simply does not run without one.
    //
    // Windows rewrites the trigger's copy of this as a SID when it stores the
    // task, and keeps the principal's as the name we gave. Neither is read
    // back by anything here; `task_command` wants the command and nothing
    // else.
    let user = match (std::env::var("USERDOMAIN"), std::env::var("USERNAME")) {
        (Ok(domain), Ok(who)) if !domain.is_empty() && !who.is_empty() => {
            format!("<UserId>{}\\{}</UserId>", escape(&domain), escape(&who))
        }
        _ => String::new(),
    };

    format!(
        r#"<?xml version="1.0" encoding="UTF-16"?>
<Task version="1.2" xmlns="http://schemas.microsoft.com/windows/2004/02/mit/task">
  <RegistrationInfo>
    <Author>StableSound</Author>
    <Description>Starts StableSound at sign-in, so screen reader speech is not clipped by Bluetooth headphones going to sleep. Created by StableSound's own settings; untick "Start StableSound when I sign in" to remove it.</Description>
    <URI>{uri}</URI>
  </RegistrationInfo>
  <Triggers>
    <LogonTrigger>
      <Enabled>true</Enabled>
      {user}
    </LogonTrigger>
  </Triggers>
  <Principals>
    <Principal id="Author">
      {user}
      <LogonType>InteractiveToken</LogonType>
      <RunLevel>LeastPrivilege</RunLevel>
    </Principal>
  </Principals>
  <Settings>
    <MultipleInstancesPolicy>IgnoreNew</MultipleInstancesPolicy>
    <DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>
    <StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>
    <AllowHardTerminate>true</AllowHardTerminate>
    <StartWhenAvailable>false</StartWhenAvailable>
    <RunOnlyIfNetworkAvailable>false</RunOnlyIfNetworkAvailable>
    <IdleSettings>
      <StopOnIdleEnd>false</StopOnIdleEnd>
      <RestartOnIdle>false</RestartOnIdle>
    </IdleSettings>
    <AllowStartOnDemand>true</AllowStartOnDemand>
    <Enabled>true</Enabled>
    <Hidden>false</Hidden>
    <RunOnlyIfIdle>false</RunOnlyIfIdle>
    <WakeToRun>false</WakeToRun>
    <ExecutionTimeLimit>PT0S</ExecutionTimeLimit>
    <Priority>4</Priority>
  </Settings>
  <Actions Context="Author">
    <Exec>
      <Command>{command}</Command>
      <WorkingDirectory>{folder}</WorkingDirectory>
    </Exec>
  </Actions>
</Task>
"#
    )
}

// -- the Run key ----------------------------------------------------------

/// The `Run` value's command line, if the value is there at all.
fn run_value() -> Option<String> {
    let key = open(KEY_READ.0)?;
    let mut kind = windows::Win32::System::Registry::REG_VALUE_TYPE::default();
    let mut size = 0u32;
    let read = unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            Some(&mut kind),
            None,
            Some(&mut size),
        )
    };
    if read != ERROR_SUCCESS || size == 0 {
        unsafe {
            let _ = RegCloseKey(key);
        }
        return None;
    }
    // `size` is bytes, and the value is UTF-16 including its terminator.
    let mut bytes = vec![0u8; size as usize];
    let read = unsafe {
        RegQueryValueExW(
            key,
            VALUE_NAME,
            None,
            Some(&mut kind),
            Some(bytes.as_mut_ptr().cast()),
            Some(&mut size),
        )
    };
    unsafe {
        let _ = RegCloseKey(key);
    }
    if read != ERROR_SUCCESS {
        return None;
    }
    let wide: Vec<u16> = bytes
        .as_chunks::<2>()
        .0
        .iter()
        .map(|pair| u16::from_le_bytes(*pair))
        .take_while(|unit| *unit != 0)
        .collect();
    Some(String::from_utf16_lossy(&wide))
}

fn write_run(on: bool) -> Result<(), String> {
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

// -- odds and ends --------------------------------------------------------

fn exe_path() -> Result<PathBuf, String> {
    std::env::current_exe().map_err(|e| format!("the program's own path is unknown: {e}"))
}

/// A task's full path in the library. `GetTask` wants this, `RegisterTaskDefinition`
/// wants the bare name, and the difference is easy to get the wrong way round.
fn task_path(name: &str) -> String {
    format!("\\{name}")
}

/// The command Windows should run, quoted so a path with spaces survives -
/// which `C:\Program Files\...` and most Desktop paths have.
fn quoted_exe() -> Result<String, String> {
    Ok(format!("\"{}\"", exe_path()?.display()))
}

/// Strip one pair of surrounding quotes, which is how `write_run` stores the
/// path and not how the task does.
fn unquote(command: &str) -> PathBuf {
    PathBuf::from(command.trim().trim_matches('"'))
}

/// Whether two paths name the same file.
///
/// Compared case-insensitively, because Windows paths are, and a mismatch of
/// case alone would read as "something else starts at sign-in" and quietly
/// clear the checkbox.
fn same_path(a: &Path, b: &Path) -> bool {
    let text = |p: &Path| p.display().to_string().to_lowercase();
    text(a) == text(b)
}

/// The text of the first `<name>` element, for reading the task back.
fn element(xml: &str, name: &str) -> Option<String> {
    let open = format!("<{name}>");
    let close = format!("</{name}>");
    let start = xml.find(&open)? + open.len();
    let end = xml[start..].find(&close)? + start;
    Some(unescape(xml[start..end].trim()))
}

/// XML-escape a path on the way in. `&` is the one that actually turns up -
/// `C:\Tools & Utils\` is a perfectly ordinary folder name, and unescaped it
/// makes the whole task description unparseable.
fn escape(text: &str) -> String {
    text.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&apos;")
}

fn unescape(text: &str) -> String {
    text.replace("&lt;", "<")
        .replace("&gt;", ">")
        .replace("&quot;", "\"")
        .replace("&apos;", "'")
        // Last, or an escaped "&amp;lt;" would come back as "<".
        .replace("&amp;", "&")
}

/// An `HRESULT` in the form somebody can search for, without the crate's own
/// wording wrapped round it.
fn brief(error: &windows::core::Error) -> String {
    let message = error.message();
    if message.is_empty() {
        format!("0x{:08X}", error.code().0)
    } else {
        format!("{message} (0x{:08X})", error.code().0)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn escaping_survives_a_round_trip() {
        for path in [
            r"C:\Program Files\StableSound\stablesound.exe",
            r"C:\Tools & Utils\stablesound.exe",
            r"C:\it's <here>\stablesound.exe",
        ] {
            assert_eq!(unescape(&escape(path)), path);
        }
    }

    #[test]
    fn the_command_can_be_read_back_out_of_the_task() {
        let exe = Path::new(r"C:\Tools & Utils\stablesound.exe");
        let xml = task_xml(TASK_NAME, exe);
        assert_eq!(
            element(&xml, "Command").as_deref(),
            Some(exe.to_str().unwrap())
        );
    }

    #[test]
    fn a_task_that_never_ends_and_never_minds_the_battery() {
        // Three settings the app is broken without. Asserted here because
        // getting them wrong fails slowly - after three days of uptime, or
        // the moment somebody unplugs the charger - which is exactly the kind
        // of thing a hardware round will not catch.
        let xml = task_xml(TASK_NAME, Path::new(r"C:\stablesound.exe"));
        assert!(xml.contains("<ExecutionTimeLimit>PT0S</ExecutionTimeLimit>"));
        assert!(xml.contains("<DisallowStartIfOnBatteries>false</DisallowStartIfOnBatteries>"));
        assert!(xml.contains("<StopIfGoingOnBatteries>false</StopIfGoingOnBatteries>"));
    }

    /// Register a real task, read it back through the real COM calls, and
    /// remove it again.
    ///
    /// `#[ignore]`d because it writes to the machine's Task Scheduler library,
    /// which a plain `cargo test` has no business doing. Run it deliberately:
    ///
    /// ```text
    /// cargo test -- --ignored --nocapture
    /// ```
    ///
    /// It is the only test here that answers the question Milestone 7 was
    /// actually blocked on - whether a task can be registered at all without
    /// elevation - and it does so under a name of its own, so it cannot
    /// disturb the task a user is relying on. Run as a standard, unelevated
    /// user on Windows 11 build 26200 on 2026-09-12: it passed.
    #[test]
    #[ignore = "writes a real scheduled task; run with --ignored"]
    fn a_real_task_can_be_registered_read_back_and_removed() {
        const PROBE: &str = "StableSoundSelfTest";
        let exe = Path::new(r"C:\Windows\System32\cmd.exe");

        // Never inherit a failed earlier run's leftovers.
        let _ = delete_task(PROBE);

        register_task(PROBE, exe).expect("a standard user should be able to register a logon task");
        let read_back = task_command(PROBE);
        // Remove it before asserting, so a failed assertion does not leave a
        // task behind on the machine.
        let removed = delete_task(PROBE);

        assert_eq!(read_back.as_deref(), Some(exe), "the task read back wrong");
        removed.expect("the task should be removable");
        assert!(
            task_command(PROBE).is_none(),
            "the task was still there after being deleted"
        );
    }

    /// Read whatever this machine actually has, and say so.
    ///
    /// `#[ignore]`d because it reports on the machine rather than on the code,
    /// so it has no business in a plain `cargo test`. It writes nothing. Its
    /// value is that `run_value` parses a real `REG_SZ` - counted bytes, a
    /// UTF-16 payload and a terminator to find - and getting that wrong would
    /// show up as a checkbox reading the wrong way rather than as a crash.
    ///
    /// ```text
    /// cargo test -- --ignored --nocapture
    /// ```
    #[test]
    #[ignore = "reports on this machine's own state; run with --ignored"]
    fn what_this_machine_is_set_up_with() {
        match run_value() {
            Some(command) => {
                let path = unquote(&command);
                println!("Run value: {command}");
                println!("  parsed as: {}", path.display());
                println!("  exists:    {}", path.exists());
                assert!(
                    path.extension()
                        .is_some_and(|e| e.eq_ignore_ascii_case("exe")),
                    "a Run value that does not parse to an .exe means the reader is wrong"
                );
            }
            None => println!("Run value: absent"),
        }
        match task_command(TASK_NAME) {
            Some(path) => println!("Task command: {}", path.display()),
            None => println!("Task: absent"),
        }
    }

    #[test]
    fn the_run_keys_quoting_is_undone_before_comparing() {
        let exe = Path::new(r"C:\Program Files\StableSound\stablesound.exe");
        assert!(same_path(&unquote(&format!("\"{}\"", exe.display())), exe));
        // And case is not a difference.
        assert!(same_path(
            Path::new(r"C:\SS\StableSound.EXE"),
            Path::new(r"c:\ss\stablesound.exe")
        ));
    }
}
