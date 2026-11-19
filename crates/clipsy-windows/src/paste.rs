//! Windows paste injection: the key chord that pastes, plus (Windows only) focusing the
//! remembered window and sending the chord with `SendInput`.

/// Virtual-key code of Ctrl (`VK_CONTROL`).
pub const VK_CONTROL: u16 = 0x11;
/// Virtual-key code of Shift (`VK_SHIFT`).
pub const VK_SHIFT: u16 = 0x10;
/// Virtual-key code of the V key.
pub const VK_V: u16 = 0x56;

/// Key events for one paste, as (virtual key, is_key_down) pairs in the order they are sent.
///
/// Normal paste is Ctrl+V. Terminal paste (`terminal == true`) is Ctrl+Shift+V. Keys go
/// down in the order Ctrl, Shift, V and come up in the reverse order.
pub fn key_sequence(terminal: bool) -> Vec<(u16, bool)> {
    let mut held = vec![VK_CONTROL];
    if terminal {
        held.push(VK_SHIFT);
    }
    held.push(VK_V);
    let mut events: Vec<(u16, bool)> = held.iter().map(|&vk| (vk, true)).collect();
    events.extend(held.iter().rev().map(|&vk| (vk, false)));
    events
}

/// Win32 side of paste: focusing a window, reading its executable name and sending keys.
/// The whole module is compiled on Windows only.
#[cfg(windows)]
pub(crate) mod win {
    use std::ffi::c_void;
    use std::time::{Duration, Instant};

    use anyhow::{ensure, Result};
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
        GetForegroundWindow, GetWindowThreadProcessId, IsWindow, SetForegroundWindow,
    };

    use super::{key_sequence, VK_CONTROL, VK_SHIFT};

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

    /// Send the paste chord (Ctrl+V, or Ctrl+Shift+V when `terminal`) with SendInput.
    pub(crate) fn send_chord(terminal: bool) -> Result<()> {
        let inputs: Vec<INPUT> = key_sequence(terminal)
            .into_iter()
            .map(|(vk, down)| INPUT {
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
            })
            .collect();
        let size = i32::try_from(std::mem::size_of::<INPUT>())?;
        // SAFETY: `inputs` is a valid slice of INPUT values, and `size` is the size of one INPUT.
        let sent = unsafe { SendInput(&inputs, size) } as usize;
        ensure!(
            sent == inputs.len(),
            "SendInput sent {sent} of {} key events (the target window may be running as administrator)",
            inputs.len()
        );
        Ok(())
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

#[cfg(test)]
mod tests {
    use super::*;

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
}
