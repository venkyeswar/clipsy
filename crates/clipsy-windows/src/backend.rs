//! Windows backend: clipboard watching, global hotkeys and paste behind the `Backend` trait.
//!
//! Hotkeys are registered with `RegisterHotKey`, which only works from the thread that owns
//! the message queue. A dedicated thread runs that queue; registration requests reach it over
//! a channel, and a thread message wakes it up to read them.

use std::collections::HashMap;
use std::ffi::c_void;
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::mpsc::{channel, Receiver, Sender};
use std::sync::{Mutex, PoisonError};
use std::time::Duration;

use anyhow::{bail, Context, Result};
use clipsy_backends::{Backend, HotkeyAction, PasteMode};
use clipsy_core::{ClipEvent, Format};
use windows::Win32::Foundation::{HWND, LPARAM, WPARAM};
use windows::Win32::System::Threading::GetCurrentThreadId;
use windows::Win32::UI::Input::KeyboardAndMouse::{
    RegisterHotKey, UnregisterHotKey, HOT_KEY_MODIFIERS, MOD_ALT, MOD_CONTROL, MOD_NOREPEAT, MOD_SHIFT, MOD_WIN,
};
use windows::Win32::UI::WindowsAndMessaging::{
    GetMessageW, PeekMessageW, PostThreadMessageW, MSG, PM_NOREMOVE, WM_APP, WM_HOTKEY,
};

use crate::clipboard;
use crate::hotkey::{parse_accel, Accel};
use crate::paste::win::{
    exe_name, focus_and_wait, foreground_window, from_raw, is_own_window, modifiers_held, send_chord, to_raw,
    wait_until, window_class, MODIFIER_TIMEOUT, POLL_EVERY,
};
use crate::paste::keys_for_apps;

/// Null window handle: hotkeys then post to the hotkey thread's own queue.
const NO_WINDOW: HWND = HWND(std::ptr::null_mut::<c_void>());

/// Thread message that tells the hotkey thread to read its command channel.
const WM_HOTKEY_COMMAND: u32 = WM_APP + 1;

/// Work for the hotkey thread. Every command that can fail carries a reply channel.
enum HotkeyCommand {
    /// Bind `action` to `accel`, replacing its previous binding. Replies `true` on success.
    /// On failure the previous binding (if any) is restored.
    Register { action: HotkeyAction, accel: Accel, reply: Sender<bool> },
    /// Remove the binding of `action`, if it has one.
    Unregister(HotkeyAction),
}

/// Handle to the running hotkey thread.
struct HotkeyThread {
    commands: Sender<HotkeyCommand>,
    thread_id: u32,
}

impl HotkeyThread {
    /// Queue a command and wake the hotkey thread. False when the command could not be queued
    /// or the wake-up message was refused (the thread is gone or its queue is unusable).
    fn send(&self, cmd: HotkeyCommand) -> bool {
        if self.commands.send(cmd).is_err() {
            return false;
        }
        // SAFETY: PostThreadMessageW only queues a message for the hotkey thread. An error means
        // the wake-up was not delivered, so the caller must not wait for a reply.
        unsafe { PostThreadMessageW(self.thread_id, WM_HOTKEY_COMMAND, WPARAM(0), LPARAM(0)) }.is_ok()
    }
}

/// How long `bind` waits for the hotkey thread to answer before giving up.
const HOTKEY_REPLY_TIMEOUT: Duration = Duration::from_secs(2);

/// Windows implementation of [`Backend`].
pub struct WindowsBackend {
    /// Window that had focus before the popup opened, as a raw handle value (0 = none).
    active: AtomicIsize,
    /// Extra terminal executables from the config, matched by `is_terminal`.
    terminal_apps: Mutex<Vec<String>>,
    /// Set by `start`; `None` until then.
    hotkeys: Option<HotkeyThread>,
    /// The accelerator the TogglePopup hotkey is bound to right now. May differ from the config
    /// when the fallback was used. `None` when no toggle hotkey is bound.
    toggle_bound: Mutex<Option<String>>,
}

impl Default for WindowsBackend {
    fn default() -> Self {
        Self::new()
    }
}

impl WindowsBackend {
    pub fn new() -> Self {
        Self {
            active: AtomicIsize::new(0),
            terminal_apps: Mutex::new(Vec::new()),
            hotkeys: None,
            toggle_bound: Mutex::new(None),
        }
    }

    /// The accelerator the popup shortcut is bound to now, or `None` if it is not bound.
    pub fn bound_toggle(&self) -> Option<String> {
        self.toggle_bound.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }

    /// Re-register the TogglePopup hotkey. The previous binding stays in place if `accel` fails.
    pub fn rebind_toggle(&self, accel: &str) -> Result<()> {
        self.bind(HotkeyAction::TogglePopup, accel)
    }

    /// Bind `action` to `accel` through the hotkey thread.
    fn bind(&self, action: HotkeyAction, accel: &str) -> Result<()> {
        let taken = || anyhow::anyhow!("hotkey {accel} is already in use or invalid");
        let hotkeys = self
            .hotkeys
            .as_ref()
            .context("clipboard backend is not running, so shortcuts cannot be used")?;
        let parsed = match parse_accel(accel) {
            Ok(parsed) => parsed,
            Err(why) => {
                tracing::warn!("{why}");
                return Err(taken());
            }
        };
        let (reply, answer) = channel();
        if !hotkeys.send(HotkeyCommand::Register { action, accel: parsed, reply }) {
            bail!("the hotkey thread has stopped, so shortcuts cannot be used");
        }
        match answer.recv_timeout(HOTKEY_REPLY_TIMEOUT) {
            Ok(true) => {
                if action == HotkeyAction::TogglePopup {
                    *self.toggle_bound.lock().unwrap_or_else(PoisonError::into_inner) =
                        Some(accel.to_string());
                }
                Ok(())
            }
            Ok(false) => Err(taken()),
            // Timed out or the thread dropped the reply: not a conflict, the thread is unresponsive.
            Err(_) => bail!("the hotkey thread did not answer, so shortcut {accel} was not changed"),
        }
    }

    fn terminal_apps(&self) -> Vec<String> {
        self.terminal_apps.lock().unwrap_or_else(PoisonError::into_inner).clone()
    }
}

/// Hotkey thread body: owns the message queue, answers commands and forwards WM_HOTKEY.
fn hotkey_thread(commands: Receiver<HotkeyCommand>, fired: Sender<HotkeyAction>, ready: Sender<u32>) {
    // SAFETY: GetCurrentThreadId takes no arguments.
    let thread_id = unsafe { GetCurrentThreadId() };
    let mut msg = MSG::default();
    // SAFETY: `msg` is a valid MSG. PM_NOREMOVE only creates this thread's message queue, so that
    // PostThreadMessageW can reach it, and removes nothing.
    let _ = unsafe { PeekMessageW(&mut msg, NO_WINDOW, 0, 0, PM_NOREMOVE) };
    if ready.send(thread_id).is_err() {
        return;
    }

    let mut bound: HashMap<HotkeyAction, Accel> = HashMap::new();
    loop {
        // SAFETY: `msg` is a valid out-pointer. A null window takes this thread's messages.
        let got = unsafe { GetMessageW(&mut msg, NO_WINDOW, 0, 0) }.0;
        if got <= 0 {
            // 0 is WM_QUIT and -1 an error; either way nothing more can arrive.
            break;
        }
        match msg.message {
            WM_HOTKEY => {
                if let Some(action) = action_for_id(msg.wParam.0) {
                    // The receiver is gone only when the app is shutting down.
                    let _ = fired.send(action);
                }
            }
            WM_HOTKEY_COMMAND => {
                while let Ok(cmd) = commands.try_recv() {
                    handle_command(cmd, &mut bound);
                }
            }
            _ => {}
        }
    }
}

fn handle_command(cmd: HotkeyCommand, bound: &mut HashMap<HotkeyAction, Accel>) {
    match cmd {
        HotkeyCommand::Register { action, accel, reply } => {
            let previous = bound.get(&action).copied();
            // The old id must be free before the new binding can use it.
            unregister_raw(action);
            let ok = register_raw(action, accel);
            if ok {
                bound.insert(action, accel);
            } else {
                // Keep the old shortcut working when the new one is refused.
                let restored = previous.is_some_and(|old| register_raw(action, old));
                if !restored {
                    bound.remove(&action);
                }
            }
            // The caller may have given up waiting; nothing to undo then.
            let _ = reply.send(ok);
        }
        HotkeyCommand::Unregister(action) => {
            unregister_raw(action);
            bound.remove(&action);
        }
    }
}

fn register_raw(action: HotkeyAction, accel: Accel) -> bool {
    // SAFETY: a null window asks for WM_HOTKEY on this thread's queue. The id is a constant
    // unique per action, and the key code is a plain number.
    match unsafe { RegisterHotKey(NO_WINDOW, hotkey_id(action), modifier_bits(&accel), accel.vk) } {
        Ok(()) => true,
        Err(e) => {
            tracing::warn!("RegisterHotKey failed for {action:?}: {e}");
            false
        }
    }
}

fn unregister_raw(action: HotkeyAction) {
    // SAFETY: the id was registered (or not) on this thread by `register_raw`. Unregistering an
    // unknown id is harmless and reports an error that we do not need.
    let _ = unsafe { UnregisterHotKey(NO_WINDOW, hotkey_id(action)) };
}

/// Hotkey id used with RegisterHotKey, one per action.
fn hotkey_id(action: HotkeyAction) -> i32 {
    match action {
        HotkeyAction::TogglePopup => 1,
        HotkeyAction::ClearHistory => 2,
        HotkeyAction::PauseRecording => 3,
    }
}

fn action_for_id(id: usize) -> Option<HotkeyAction> {
    match id {
        1 => Some(HotkeyAction::TogglePopup),
        2 => Some(HotkeyAction::ClearHistory),
        3 => Some(HotkeyAction::PauseRecording),
        _ => None,
    }
}

/// RegisterHotKey modifier flags for `accel`. MOD_NOREPEAT stops key-repeat from firing it again.
fn modifier_bits(accel: &Accel) -> HOT_KEY_MODIFIERS {
    let mut bits = MOD_NOREPEAT.0;
    if accel.alt {
        bits |= MOD_ALT.0;
    }
    if accel.ctrl {
        bits |= MOD_CONTROL.0;
    }
    if accel.shift {
        bits |= MOD_SHIFT.0;
    }
    if accel.win {
        bits |= MOD_WIN.0;
    }
    HOT_KEY_MODIFIERS(bits)
}

impl Backend for WindowsBackend {
    fn name(&self) -> &'static str {
        "windows"
    }

    fn start(&mut self, clips: Sender<ClipEvent>, hotkeys: Sender<HotkeyAction>) -> Result<()> {
        if self.hotkeys.is_some() {
            bail!("the Windows backend is already started");
        }
        clipboard::spawn_listener(clips).context("starting the clipboard watcher")?;
        let (commands, command_rx) = channel();
        let (ready_tx, ready_rx) = channel();
        std::thread::Builder::new()
            .name("clipsy-hotkeys".into())
            .spawn(move || hotkey_thread(command_rx, hotkeys, ready_tx))
            .context("starting the hotkey thread")?;
        let thread_id = ready_rx.recv().context("the hotkey thread stopped before it was ready")?;
        self.hotkeys = Some(HotkeyThread { commands, thread_id });
        Ok(())
    }

    fn set_clipboard(&self, formats: &[Format]) -> Result<()> {
        clipboard::write_formats(formats)
    }

    /// Remember the window that has focus now, unless it is Clipsy's own. When Clipsy's own window
    /// is in front, the previous target is kept. So after focusing Clipsy's settings window, the
    /// paste target is still the older app, not the one the user last looked at.
    fn remember_active_window(&self) {
        let fg = foreground_window();
        // Clipsy's own window (a popup already open) is not a paste target: keep the old one.
        if fg.0.is_null() || is_own_window(fg) {
            return;
        }
        self.active.store(to_raw(fg), Ordering::SeqCst);
    }

    fn paste(&self, mode: PasteMode) -> Result<()> {
        let remembered = self.active.load(Ordering::SeqCst);
        let target = if remembered == 0 { foreground_window() } else { from_raw(remembered) };
        if target.0.is_null() {
            bail!("there is no window to paste into, so nothing was pasted");
        }
        if !crate::paste::win::window_exists(target) {
            bail!("the window that was active before the popup is gone, so nothing was pasted");
        }
        if is_own_window(target) {
            bail!("Clipsy's own window is the paste target, so nothing was pasted");
        }

        // Keys sent while the popup or another window still has focus would land in that window.
        if !focus_and_wait(target) {
            bail!("the target window did not take focus, so nothing was pasted");
        }
        // A key still held by the user (Shift from Shift+Enter, say) would be added to our chord.
        if modifiers_held() && !wait_until(MODIFIER_TIMEOUT, POLL_EVERY, || !modifiers_held()) {
            tracing::warn!("windows paste: a modifier is still held after {MODIFIER_TIMEOUT:?}, pasting anyway");
        }

        // Console windows, Windows Terminal and terminal executables each need their own chord.
        let class = window_class(target).unwrap_or_default();
        let exe = exe_name(target);
        let keys = keys_for_apps(&class, exe.as_deref(), mode, &self.terminal_apps());
        tracing::info!(
            "windows paste: window={:#x} class={class:?} exe={:?} mode={mode:?} keys={keys:?}",
            to_raw(target),
            exe.as_deref().unwrap_or("unknown"),
        );
        send_chord(keys)
    }

    fn supports_paste(&self) -> bool {
        true
    }

    fn register_hotkey(&mut self, action: HotkeyAction, accel: &str) -> Result<()> {
        self.bind(action, accel)
    }

    fn unregister_hotkey(&mut self, action: HotkeyAction) {
        if let Some(hotkeys) = &self.hotkeys {
            let _ = hotkeys.send(HotkeyCommand::Unregister(action));
        }
    }

    fn set_terminal_apps(&self, apps: &[String]) {
        *self.terminal_apps.lock().unwrap_or_else(PoisonError::into_inner) = apps.to_vec();
    }
}
