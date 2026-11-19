//! Windows paste injection: the key chord that pastes, plus (Windows only) focusing the
//! remembered window and sending the chord with `SendInput`.

use clipsy_backends::PasteMode;

use crate::terminals::is_terminal;

/// Virtual-key code of Ctrl (`VK_CONTROL`).
pub const VK_CONTROL: u16 = 0x11;
/// Virtual-key code of Shift (`VK_SHIFT`).
pub const VK_SHIFT: u16 = 0x10;
/// Virtual-key code of the V key.
pub const VK_V: u16 = 0x56;

/// Virtual-key code of Insert (`VK_INSERT`).
pub const VK_INSERT: u16 = 0x2D;

/// Which key chord pastes into the target window.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PasteKeys {
    /// Ctrl+V: most apps, and every binary (image) paste.
    CtrlV,
    /// Ctrl+Shift+V: Windows Terminal and other terminal emulators.
    CtrlShiftV,
    /// Shift+Insert: classic console windows (cmd.exe, PowerShell in conhost).
    ShiftInsert,
}

/// Window class of classic console windows (conhost).
pub const CONSOLE_WINDOW_CLASS: &str = "ConsoleWindowClass";
/// Window class of the Windows Terminal host window.
pub const TERMINAL_WINDOW_CLASS: &str = "CASCADIA_HOSTING_WINDOW_CLASS";

/// Chooses the paste chord for a target window. Pure: `class` is the window class name (empty
/// when unknown), `exe` the executable file name or path when it could be read.
///
/// Binary (image) pastes are always Ctrl+V. Then the window class decides: console windows use
/// Shift+Insert and Windows Terminal uses Ctrl+Shift+V. Otherwise a terminal executable (or
/// `PasteMode::Terminal`) uses Ctrl+Shift+V, and everything else uses Ctrl+V.
pub fn keys_for(class: &str, exe: Option<&str>, mode: PasteMode) -> PasteKeys {
    keys_for_apps(class, exe, mode, &[])
}

/// `keys_for` plus the user's own terminal executables (`terminal_apps` from the config).
pub fn keys_for_apps(class: &str, exe: Option<&str>, mode: PasteMode, terminal_apps: &[String]) -> PasteKeys {
    if mode == PasteMode::Binary {
        return PasteKeys::CtrlV;
    }
    if class == CONSOLE_WINDOW_CLASS {
        return PasteKeys::ShiftInsert;
    }
    if class == TERMINAL_WINDOW_CLASS {
        return PasteKeys::CtrlShiftV;
    }
    let terminal_exe = exe.is_some_and(|e| is_terminal(e, terminal_apps));
    if mode == PasteMode::Terminal || terminal_exe {
        PasteKeys::CtrlShiftV
    } else {
        PasteKeys::CtrlV
    }
}

/// Key events for a paste chord, as (virtual key, is_key_down) pairs in the order they are sent.
/// Modifiers go down first and come up in the reverse order.
pub fn keys_sequence_for(keys: PasteKeys) -> Vec<(u16, bool)> {
    let held: Vec<u16> = match keys {
        PasteKeys::CtrlV => vec![VK_CONTROL, VK_V],
        PasteKeys::CtrlShiftV => vec![VK_CONTROL, VK_SHIFT, VK_V],
        PasteKeys::ShiftInsert => vec![VK_SHIFT, VK_INSERT],
    };
    let mut events: Vec<(u16, bool)> = held.iter().map(|&vk| (vk, true)).collect();
    events.extend(held.iter().rev().map(|&vk| (vk, false)));
    events
}

/// Key events for one paste, as (virtual key, is_key_down) pairs in the order they are sent.
///
/// Normal paste is Ctrl+V. Terminal paste (`terminal == true`) is Ctrl+Shift+V.
pub fn key_sequence(terminal: bool) -> Vec<(u16, bool)> {
    keys_sequence_for(if terminal { PasteKeys::CtrlShiftV } else { PasteKeys::CtrlV })
}

/// Win32 side of paste: focusing a window, reading its executable name and sending keys.
/// The whole module is compiled on Windows only.
#[cfg(windows)]
pub(crate) mod win {
    use std::ffi::c_void;
    use std::time::{Duration, Instant};

    use anyhow::Result;
    use windows::core::PWSTR;
    use windows::Win32::Foundation::{CloseHandle, HWND};
    use windows::Win32::System::Threading::{
        AttachThreadInput, GetCurrentThreadId, OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32,
        PROCESS_QUERY_LIMITED_INFORMATION,
    };
    use windows::Win32::UI::Input::KeyboardAndMouse::{
        GetAsyncKeyState, SendInput, INPUT, INPUT_0, INPUT_KEYBOARD, KEYBDINPUT, KEYBD_EVENT_FLAGS, KEYEVENTF_KEYUP,
        VIRTUAL_KEY,
    };
    use windows::Win32::UI::WindowsAndMessaging::{
        GetClassNameW, GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
    };

    use super::{keys_sequence_for, release_sequence, PasteKeys, SendBlocked, VK_CONTROL, VK_SHIFT};

    /// How long to wait for the target window to become the foreground window.
    pub(crate) const FOCUS_TIMEOUT: Duration = Duration::from_millis(500);
    /// How long to wait for the user to release Ctrl/Shift/Alt/Win before sending keys.
    pub(crate) const MODIFIER_TIMEOUT: Duration = Duration::from_millis(300);
    /// Polling interval for the waits above.
    pub(crate) const POLL_EVERY: Duration = Duration::from_millis(10);

    const VK_MENU: u16 = 0x12; // Alt
    const VK_LWIN: u16 = 0x5B;
    const VK_RWIN: u16 = 0x5C;

    /// The window that currently has keyboard focus (null when there is none).
    pub(crate) fn foreground_window() -> HWND {
        // SAFETY: GetForegroundWindow takes no arguments and returns a handle value or null.
        unsafe { GetForegroundWindow() }
    }

    /// True while `hwnd` is an existing window.
    pub(crate) fn window_exists(hwnd: HWND) -> bool {
        // SAFETY: IsWindow only inspects the handle value; an invalid handle yields FALSE.
        unsafe { IsWindow(hwnd) }.as_bool()
    }

    /// Process id that owns `hwnd`, or 0 when the handle is not a window.
    fn owner_pid(hwnd: HWND) -> u32 {
        let mut pid = 0u32;
        // SAFETY: `pid` is a live u32 for the whole call and the handle is only looked up by the OS.
        unsafe { GetWindowThreadProcessId(hwnd, Some(&mut pid as *mut u32)) };
        pid
    }

    /// True when `hwnd` belongs to this process (Clipsy's own popup or settings windows).
    pub(crate) fn is_own_window(hwnd: HWND) -> bool {
        let pid = owner_pid(hwnd);
        pid != 0 && pid == std::process::id()
    }

    /// Window class name of `hwnd` (for example `ConsoleWindowClass`), or None when it cannot be read.
    pub(crate) fn window_class(hwnd: HWND) -> Option<String> {
        let mut buf = [0u16; 256];
        // SAFETY: `buf` is a writable buffer of `buf.len()` UTF-16 units for the whole call.
        let len = unsafe { GetClassNameW(hwnd, &mut buf) };
        if len <= 0 {
            return None;
        }
        Some(String::from_utf16_lossy(&buf[..len as usize]))
    }

    /// Executable file name (full path) of the process that owns `hwnd`, if it can be read.
    /// Processes that run elevated cannot be opened, so this returns None for them.
    pub(crate) fn exe_name(hwnd: HWND) -> Option<String> {
        let pid = owner_pid(hwnd);
        if pid == 0 {
            return None;
        }
        // SAFETY: OpenProcess only reads its arguments and returns a handle that is closed below.
        let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
        let mut buf = vec![0u16; 1024];
        let mut len = buf.len() as u32;
        // SAFETY: `buf` has room for `len` UTF-16 units and `process` is the live handle opened above.
        let queried = unsafe {
            QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, PWSTR(buf.as_mut_ptr()), &mut len)
        }
        .is_ok();
        // SAFETY: `process` came from OpenProcess above and is closed exactly once, here.
        let _ = unsafe { CloseHandle(process) };
        queried.then(|| String::from_utf16_lossy(&buf[..len as usize]))
    }

    /// Make `target` the foreground window and wait up to `FOCUS_TIMEOUT` for it to take effect.
    /// Returns true when it is focused in time.
    pub(crate) fn focus_and_wait(target: HWND) -> bool {
        // SAFETY: GetCurrentThreadId takes no arguments.
        let me = unsafe { GetCurrentThreadId() };
        let fg = foreground_window();
        // SAFETY: a null `fg` is allowed; the call then returns 0 and the id is not used.
        let fg_thread = unsafe { GetWindowThreadProcessId(fg, None) };
        // Windows refuses SetForegroundWindow unless this process is already in the foreground.
        // Sharing input with the current foreground thread grants that right while we switch focus.
        let attached = fg_thread != 0 && fg_thread != me;
        if attached {
            // SAFETY: both thread ids belong to live threads; the link is removed again below.
            let _ = unsafe { AttachThreadInput(me, fg_thread, true) };
        }
        // SAFETY: `target` is a window handle value; SetForegroundWindow only changes focus.
        let _ = unsafe { SetForegroundWindow(target) };
        if attached {
            // SAFETY: undoes the AttachThreadInput call made above with the same thread ids.
            let _ = unsafe { AttachThreadInput(me, fg_thread, false) };
        }
        wait_until(FOCUS_TIMEOUT, POLL_EVERY, || foreground_window().0 == target.0)
    }

    /// True while Ctrl, Shift, Alt or Win is physically held down.
    pub(crate) fn modifiers_held() -> bool {
        [VK_CONTROL, VK_SHIFT, VK_MENU, VK_LWIN, VK_RWIN].into_iter().any(|vk| {
            // SAFETY: GetAsyncKeyState takes a virtual-key code and only reads keyboard state.
            unsafe { GetAsyncKeyState(i32::from(vk)) < 0 }
        })
    }

    /// One keyboard INPUT for a key-down (`down == true`) or key-up event.
    fn key_input(vk: u16, down: bool) -> INPUT {
        INPUT {
            r#type: INPUT_KEYBOARD,
            Anonymous: INPUT_0 {
                ki: KEYBDINPUT {
                    wVk: VIRTUAL_KEY(vk),
                    wScan: 0,
                    dwFlags: if down { KEYBD_EVENT_FLAGS(0) } else { KEYEVENTF_KEYUP },
                    time: 0,
                    dwExtraInfo: 0,
                },
            },
        }
    }

    /// Hand `events` to SendInput in one batch. Returns how many events were injected.
    fn send_events(events: &[(u16, bool)]) -> Result<usize> {
        let inputs: Vec<INPUT> = events.iter().map(|&(vk, down)| key_input(vk, down)).collect();
        let size = i32::try_from(std::mem::size_of::<INPUT>())?;
        // SAFETY: `inputs` is a valid slice of INPUT values, and `size` is the size of one INPUT.
        Ok(unsafe { SendInput(&inputs, size) } as usize)
    }

    /// Send a paste chord with SendInput.
    ///
    /// If only part of the chord is injected, the keys that did go down are released again
    /// before the error is returned (a `SendBlocked`), so no modifier stays stuck.
    pub(crate) fn send_chord(keys: PasteKeys) -> Result<()> {
        let events = keys_sequence_for(keys);
        let sent = send_events(&events)?;
        if sent == events.len() {
            return Ok(());
        }
        // Key-ups for keys that were never pressed are harmless, so release the whole chord.
        let released = send_events(&release_sequence(keys)).unwrap_or(0);
        tracing::warn!("SendInput sent {sent} of {} paste key events; released {released}", events.len());
        Err(SendBlocked { sent, expected: events.len() }.into())
    }

    /// Call `done` every `every` until it returns true or `timeout` passes. Returns the last result.
    pub(crate) fn wait_until(timeout: Duration, every: Duration, mut done: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        loop {
            if done() {
                return true;
            }
            if Instant::now() >= deadline {
                return false;
            }
            std::thread::sleep(every);
        }
    }

    /// Raw handle value used to store a window in an atomic.
    pub(crate) fn to_raw(hwnd: HWND) -> isize {
        hwnd.0 as isize
    }

    /// Window handle from a value stored with `to_raw`.
    pub(crate) fn from_raw(raw: isize) -> HWND {
        HWND(raw as *mut c_void)
    }
}

/// Key-up events that release every key a paste chord may have pressed. Sent after a partly
/// failed SendInput so no key stays held.
pub fn release_sequence(keys: PasteKeys) -> Vec<(u16, bool)> {
    // The up events of the chord, already in reverse order of the downs.
    keys_sequence_for(keys).into_iter().filter(|&(_, down)| !down).collect()
}

/// Error from `send_chord` when SendInput injected only part of a paste chord. The usual cause
/// is a target window running as administrator, which blocks input from a normal process.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SendBlocked {
    pub sent: usize,
    pub expected: usize,
}

impl std::fmt::Display for SendBlocked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(
            f,
            "SendInput sent {} of {} key events (the target window may be running as administrator)",
            self.sent, self.expected
        )
    }
}

impl std::error::Error for SendBlocked {}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn release_sequence_normal_releases_v_then_ctrl() {
        assert_eq!(release_sequence(PasteKeys::CtrlV), vec![(0x56, false), (0x11, false)]);
    }

    #[test]
    fn release_sequence_terminal_releases_v_shift_ctrl() {
        assert_eq!(
            release_sequence(PasteKeys::CtrlShiftV),
            vec![(0x56, false), (0x10, false), (0x11, false)]
        );
    }

    #[test]
    fn key_sequence_normal_is_ctrl_v() {
        assert_eq!(
            key_sequence(false),
            vec![(0x11, true), (0x56, true), (0x56, false), (0x11, false)]
        );
    }

    #[test]
    fn key_sequence_terminal_is_ctrl_shift_v() {
        assert_eq!(
            key_sequence(true),
            vec![
                (0x11, true),
                (0x10, true),
                (0x56, true),
                (0x56, false),
                (0x10, false),
                (0x11, false),
            ]
        );
    }

    #[test]
    fn console_window_class_uses_shift_insert() {
        assert_eq!(keys_for("ConsoleWindowClass", Some("cmd.exe"), PasteMode::Normal), PasteKeys::ShiftInsert);
        assert_eq!(keys_for("ConsoleWindowClass", None, PasteMode::Normal), PasteKeys::ShiftInsert);
    }

    #[test]
    fn cascadia_class_uses_ctrl_shift_v() {
        assert_eq!(
            keys_for("CASCADIA_HOSTING_WINDOW_CLASS", Some("WindowsTerminal.exe"), PasteMode::Normal),
            PasteKeys::CtrlShiftV
        );
        assert_eq!(keys_for("CASCADIA_HOSTING_WINDOW_CLASS", None, PasteMode::Normal), PasteKeys::CtrlShiftV);
    }

    #[test]
    fn binary_mode_is_plain_ctrl_v_even_in_terminal() {
        assert_eq!(keys_for("", Some("WindowsTerminal.exe"), PasteMode::Binary), PasteKeys::CtrlV);
        assert_eq!(keys_for("CASCADIA_HOSTING_WINDOW_CLASS", None, PasteMode::Binary), PasteKeys::CtrlV);
        assert_eq!(keys_for("ConsoleWindowClass", Some("cmd.exe"), PasteMode::Binary), PasteKeys::CtrlV);
    }

    #[test]
    fn unknown_app_uses_ctrl_v() {
        assert_eq!(keys_for("Chrome_WidgetWin_1", Some("chrome.exe"), PasteMode::Normal), PasteKeys::CtrlV);
        assert_eq!(keys_for("Notepad", None, PasteMode::Normal), PasteKeys::CtrlV);
    }

    #[test]
    fn terminal_exe_without_known_class_uses_ctrl_shift_v() {
        assert_eq!(keys_for("", Some(r"C:\Program Files\PowerShell\7\pwsh.exe"), PasteMode::Normal), PasteKeys::CtrlShiftV);
        assert_eq!(keys_for("", Some("kitty.exe"), PasteMode::Normal), PasteKeys::CtrlShiftV);
    }

    #[test]
    fn user_terminal_list_is_used() {
        let extra = vec!["code".to_string()];
        assert_eq!(keys_for_apps("", Some("Code.exe"), PasteMode::Normal, &extra), PasteKeys::CtrlShiftV);
        assert_eq!(keys_for_apps("", Some("Code.exe"), PasteMode::Binary, &extra), PasteKeys::CtrlV);
        assert_eq!(keys_for_apps("", Some("chrome.exe"), PasteMode::Normal, &extra), PasteKeys::CtrlV);
    }

    #[test]
    fn shift_insert_sequence_is_shift_ins_chord() {
        assert_eq!(
            keys_sequence_for(PasteKeys::ShiftInsert),
            vec![(0x10, true), (0x2D, true), (0x2D, false), (0x10, false)]
        );
    }
}
