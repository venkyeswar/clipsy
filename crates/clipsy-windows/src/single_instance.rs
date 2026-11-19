//! Single-instance guard and CLI verbs.
//!
//! The first process holds a named mutex and listens on a named pipe. A
//! second process (for example `clipsy-windows toggle`) writes one verb per
//! line to the pipe and exits. If no primary answers, the second process
//! retries for a short time and then gives up instead of starting a second
//! tray icon.

#[cfg(windows)]
use std::sync::mpsc::Sender;

/// A command sent from a second process to the running instance.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Verb {
    Toggle,
    Settings,
    Pause,
    Resume,
    Clear,
    Quit,
}

/// Parses one verb line. Matching is exact; callers trim whitespace first.
pub fn parse_verb(arg: &str) -> Option<Verb> {
    match arg {
        "toggle" => Some(Verb::Toggle),
        "settings" => Some(Verb::Settings),
        "pause" => Some(Verb::Pause),
        "resume" => Some(Verb::Resume),
        "clear" => Some(Verb::Clear),
        "quit" => Some(Verb::Quit),
        _ => None,
    }
}

/// The wire name of a verb (the text written to the pipe).
pub fn verb_name(v: Verb) -> &'static str {
    match v {
        Verb::Toggle => "toggle",
        Verb::Settings => "settings",
        Verb::Pause => "pause",
        Verb::Resume => "resume",
        Verb::Clear => "clear",
        Verb::Quit => "quit",
    }
}

#[cfg(windows)]
use anyhow::Context;
#[cfg(windows)]
use std::fs::{File, OpenOptions};
#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::io::{FromRawHandle, OwnedHandle};
#[cfg(windows)]
use std::time::{Duration, Instant};
#[cfg(windows)]
use windows::core::PCWSTR;
#[cfg(windows)]
use windows::Win32::Foundation::{GetLastError, BOOL, ERROR_ALREADY_EXISTS, ERROR_PIPE_CONNECTED};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
#[cfg(windows)]
use windows::Win32::System::Pipes::{
    CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
#[cfg(windows)]
use windows::Win32::System::Threading::CreateMutexW;

/// Mutex name in the session-local namespace (one per logged-in user session).
#[cfg(windows)]
const MUTEX_NAME: &str = r"Local\Clipsy-Instance";

/// How long a second process keeps retrying to reach the primary.
#[cfg(windows)]
const FORWARD_TIMEOUT: Duration = Duration::from_secs(2);

/// Upper bound on bytes read from one client, so a misbehaving client cannot
/// make the server buffer without limit.
#[cfg(windows)]
const MAX_MESSAGE_BYTES: u64 = 4096;

/// Outcome of trying to become the running instance.
#[cfg(windows)]
pub enum Role {
    /// This process owns the mutex and must run the tray and pipe server.
    Primary(PrimaryGuard),
    /// Another instance already owns the mutex.
    Secondary,
}

/// Keeps the instance mutex alive for as long as the primary runs.
/// Dropping it releases the mutex.
#[cfg(windows)]
pub struct PrimaryGuard {
    _mutex: OwnedHandle,
}

#[cfg(windows)]
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// Pipe name for the current user, e.g. `\\.\pipe\clipsy-alice`.
#[cfg(windows)]
fn pipe_name() -> String {
    let user = std::env::var("USERNAME").unwrap_or_else(|_| "default".to_string());
    format!(r"\\.\pipe\clipsy-{user}")
}

// `ConnectNamedPipe` is declared here instead of imported from the `windows`
// crate because its binding is behind the `Win32_System_IO` feature, which is
// not enabled in Cargo.toml. The signature matches the Win32 API; the
// OVERLAPPED pointer is always passed as null (synchronous mode).
#[cfg(windows)]
#[link(name = "kernel32")]
extern "system" {
    fn ConnectNamedPipe(hnamedpipe: *mut std::ffi::c_void, lpoverlapped: *mut std::ffi::c_void) -> i32;
}

/// Tries to create the instance mutex. Returns `Primary` if we created it,
/// `Secondary` if another process already holds it.
#[cfg(windows)]
pub fn acquire() -> anyhow::Result<Role> {
    let name = wide(MUTEX_NAME);
    // SAFETY: `name` is a NUL-terminated UTF-16 buffer that stays alive for the
    // whole call. Null security attributes give the default ACL. The returned
    // handle is checked below and then owned by `OwnedHandle`.
    let handle = unsafe { CreateMutexW(None, BOOL(0), PCWSTR(name.as_ptr())) }
        .context("could not create the Clipsy instance mutex")?;
    // SAFETY: GetLastError has no preconditions. It is read immediately after
    // CreateMutexW, which sets ERROR_ALREADY_EXISTS when the mutex existed.
    let already_running = unsafe { GetLastError() } == ERROR_ALREADY_EXISTS;
    // SAFETY: `handle.0` is a valid handle returned by CreateMutexW that nothing
    // else owns. OwnedHandle closes it exactly once when dropped.
    let owned = unsafe { OwnedHandle::from_raw_handle(handle.0) };
    if already_running {
        drop(owned);
        Ok(Role::Secondary)
    } else {
        Ok(Role::Primary(PrimaryGuard { _mutex: owned }))
    }
}

/// Sends one verb to the running instance. Retries for up to 2 seconds in case
/// the primary is still starting or is busy with another client.
#[cfg(windows)]
pub fn forward(v: Verb) -> anyhow::Result<()> {
    let name = pipe_name();
    let deadline = Instant::now() + FORWARD_TIMEOUT;
    loop {
        match write_verb(&name, v) {
            Ok(()) => return Ok(()),
            Err(e) if Instant::now() >= deadline => {
                return Err(e).with_context(|| {
                    format!("no running Clipsy answered on {name} within 2 seconds")
                });
            }
            Err(_) => std::thread::sleep(Duration::from_millis(100)),
        }
    }
}

#[cfg(windows)]
fn write_verb(name: &str, v: Verb) -> std::io::Result<()> {
    let mut pipe = OpenOptions::new().write(true).open(name)?;
    pipe.write_all(format!("{}\n", verb_name(v)).as_bytes())
}

/// Starts the pipe server on a background thread. Every verb received is sent
/// on `tx`. The thread ends when the receiver is dropped or the pipe fails.
#[cfg(windows)]
pub fn serve(tx: Sender<Verb>) {
    let name = pipe_name();
    let spawned = std::thread::Builder::new()
        .name("clipsy-pipe-server".to_string())
        .spawn(move || {
            if let Err(e) = serve_loop(&name, &tx) {
                tracing::error!("pipe server stopped: {e:#}");
            }
        });
    if let Err(e) = spawned {
        tracing::error!("could not start pipe server thread: {e}");
    }
}

#[cfg(windows)]
fn serve_loop(name: &str, tx: &Sender<Verb>) -> anyhow::Result<()> {
    let wname = wide(name);
    loop {
        let pipe = create_pipe(&wname)?;
        // SAFETY: `pipe` is a valid pipe handle owned by us. A null overlapped
        // pointer selects synchronous mode.
        let connected = unsafe { ConnectNamedPipe(pipe_raw(&pipe), std::ptr::null_mut()) } != 0;
        // SAFETY: GetLastError has no preconditions; read right after ConnectNamedPipe.
        if !connected && unsafe { GetLastError() } != ERROR_PIPE_CONNECTED {
            tracing::warn!("pipe connect failed: {}", std::io::Error::last_os_error());
            continue;
        }
        let text = read_message(pipe);
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            match parse_verb(line) {
                Some(v) => {
                    if tx.send(v).is_err() {
                        return Ok(());
                    }
                }
                None => tracing::warn!("unknown verb from second instance: {line:?}"),
            }
        }
    }
}

#[cfg(windows)]
fn pipe_raw(pipe: &OwnedHandle) -> *mut std::ffi::c_void {
    use std::os::windows::io::AsRawHandle;
    pipe.as_raw_handle()
}

/// Creates one inbound byte-mode pipe instance.
#[cfg(windows)]
fn create_pipe(wname: &[u16]) -> anyhow::Result<OwnedHandle> {
    // SAFETY: `wname` is NUL-terminated and alive for the call. Null security
    // attributes give the default ACL. Buffer sizes 0 use the system defaults.
    // The result is checked for an invalid handle before it is owned.
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wname.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT,
            PIPE_UNLIMITED_INSTANCES,
            0,
            0,
            0,
            None,
        )
    };
    if handle.is_invalid() {
        return Err(windows::core::Error::from_win32()).context("could not create the Clipsy pipe");
    }
    // SAFETY: `handle.0` is a valid pipe handle that nothing else owns. OwnedHandle
    // closes it when dropped, which also disconnects the client.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.0) })
}

/// Reads the client's message until it closes the pipe (or the size cap is hit).
/// Read errors such as a broken pipe end the message and are not reported.
#[cfg(windows)]
fn read_message(pipe: OwnedHandle) -> String {
    let mut file = File::from(pipe);
    let mut buf = Vec::new();
    let _ = (&mut file).take(MAX_MESSAGE_BYTES).read_to_end(&mut buf);
    String::from_utf8_lossy(&buf).into_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_all_verbs() {
        assert_eq!(parse_verb("toggle"), Some(Verb::Toggle));
        assert_eq!(parse_verb("settings"), Some(Verb::Settings));
        assert_eq!(parse_verb("pause"), Some(Verb::Pause));
        assert_eq!(parse_verb("resume"), Some(Verb::Resume));
        assert_eq!(parse_verb("clear"), Some(Verb::Clear));
        assert_eq!(parse_verb("quit"), Some(Verb::Quit));
    }

    #[test]
    fn unknown_verb_is_none() {
        assert_eq!(parse_verb(""), None);
        assert_eq!(parse_verb("explode"), None);
        assert_eq!(parse_verb("TOGGLE"), None);
    }

    #[test]
    fn verb_round_trip_names() {
        for v in [
            Verb::Toggle,
            Verb::Settings,
            Verb::Pause,
            Verb::Resume,
            Verb::Clear,
            Verb::Quit,
        ] {
            assert_eq!(parse_verb(verb_name(v)), Some(v));
        }
    }
}
