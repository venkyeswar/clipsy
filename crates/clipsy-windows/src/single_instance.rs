//! Single-instance guard and CLI verbs.
//!
//! The first process holds a named mutex and listens on a named pipe. A
//! second process (for example `clipsy-windows toggle`) writes one verb line
//! to the pipe and exits. If the primary is not reachable, the second process
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
use anyhow::{bail, Context};
#[cfg(windows)]
use std::fs::{File, OpenOptions};
#[cfg(windows)]
use std::io::{Read, Write};
#[cfg(windows)]
use std::os::windows::io::{AsRawHandle, FromRawHandle, OwnedHandle};
#[cfg(windows)]
use std::time::{Duration, Instant};
#[cfg(windows)]
use windows::core::{HRESULT, PCWSTR, PWSTR};
#[cfg(windows)]
use windows::Win32::Foundation::{
    GetLastError, LocalFree, BOOL, ERROR_ALREADY_EXISTS, ERROR_FILE_NOT_FOUND, ERROR_PIPE_BUSY,
    ERROR_PIPE_CONNECTED, HANDLE, HLOCAL,
};
#[cfg(windows)]
use windows::Win32::Security::Authorization::{
    ConvertSidToStringSidW, ConvertStringSecurityDescriptorToSecurityDescriptorW, SDDL_REVISION_1,
};
#[cfg(windows)]
use windows::Win32::Security::{
    GetTokenInformation, PSECURITY_DESCRIPTOR, SECURITY_ATTRIBUTES, TokenUser, TOKEN_QUERY,
    TOKEN_USER,
};
#[cfg(windows)]
use windows::Win32::Storage::FileSystem::PIPE_ACCESS_INBOUND;
#[cfg(windows)]
use windows::Win32::System::Pipes::{
    ConnectNamedPipe, CreateNamedPipeW, PIPE_READMODE_BYTE, PIPE_REJECT_REMOTE_CLIENTS,
    PIPE_TYPE_BYTE, PIPE_UNLIMITED_INSTANCES, PIPE_WAIT,
};
#[cfg(windows)]
use windows::Win32::System::Threading::{CreateMutexW, GetCurrentProcess, OpenProcessToken};

/// Mutex name in the session-local namespace (one per logged-in user session).
#[cfg(windows)]
const MUTEX_NAME: &str = r"Local\Clipsy-Instance";

/// How long a second process keeps retrying to reach the primary.
#[cfg(windows)]
const FORWARD_TIMEOUT: Duration = Duration::from_secs(2);

/// Pause between retries (client side) and after a failed pipe operation (server side).
#[cfg(windows)]
const RETRY_DELAY: Duration = Duration::from_millis(100);

/// Upper bound on bytes read from one client, so a misbehaving client cannot
/// make the server buffer without limit.
#[cfg(windows)]
const MAX_MESSAGE_BYTES: usize = 4096;

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

/// Sends one verb to the running instance.
///
/// Only "not listening yet" (`ERROR_FILE_NOT_FOUND`) and "all instances busy"
/// (`ERROR_PIPE_BUSY`) are retried, for up to 2 seconds. Any other error,
/// such as access denied, is returned at once.
#[cfg(windows)]
pub fn forward(v: Verb) -> anyhow::Result<()> {
    let name = pipe_name();
    let deadline = Instant::now() + FORWARD_TIMEOUT;
    loop {
        let err = match write_verb(&name, v) {
            Ok(()) => return Ok(()),
            Err(e) => e,
        };
        if !is_retryable(&err) {
            return Err(err).with_context(|| {
                format!("could not send '{}' to Clipsy on {name}", verb_name(v))
            });
        }
        if Instant::now() >= deadline {
            let msg = if err.raw_os_error() == Some(ERROR_FILE_NOT_FOUND.0 as i32) {
                format!("no running Clipsy is listening on {name} (is it running?)")
            } else {
                format!("Clipsy's pipe {name} stayed busy or unreachable for 2 seconds")
            };
            return Err(anyhow::Error::new(err).context(msg));
        }
        std::thread::sleep(RETRY_DELAY);
    }
}

#[cfg(windows)]
fn is_retryable(e: &std::io::Error) -> bool {
    matches!(
        e.raw_os_error(),
        Some(code) if code == ERROR_PIPE_BUSY.0 as i32 || code == ERROR_FILE_NOT_FOUND.0 as i32
    )
}

#[cfg(windows)]
fn write_verb(name: &str, v: Verb) -> std::io::Result<()> {
    let mut pipe = OpenOptions::new().write(true).open(name)?;
    pipe.write_all(format!("{}\n", verb_name(v)).as_bytes())
}

/// Starts the pipe server on a background thread. Every verb received is sent
/// on `tx`. The server runs until the process exits.
#[cfg(windows)]
pub fn serve(tx: Sender<Verb>) {
    let name = pipe_name();
    let spawned = std::thread::Builder::new()
        .name("clipsy-pipe-server".to_string())
        .spawn(move || serve_loop(&name, tx));
    if let Err(e) = spawned {
        tracing::error!("could not start pipe server thread: {e}");
    }
}

/// Accept loop. Each connection is handled on its own thread, so a client that
/// stalls cannot block new connections.
#[cfg(windows)]
fn serve_loop(name: &str, tx: Sender<Verb>) {
    let wname = wide(name);
    loop {
        // Built on every pass so that a failure (for example a token query error)
        // is retried like the other failures below, instead of ending the server.
        let security = match PipeSecurity::for_current_user() {
            Ok(s) => s,
            Err(e) => {
                tracing::error!("pipe server cannot build its ACL, retrying: {e:#}");
                std::thread::sleep(RETRY_DELAY);
                continue;
            }
        };
        let pipe = match create_pipe(&wname, &security) {
            Ok(p) => p,
            Err(e) => {
                tracing::warn!("{e:#}");
                std::thread::sleep(RETRY_DELAY);
                continue;
            }
        };
        if let Err(e) = connect_pipe(&pipe) {
            tracing::warn!("pipe connect failed: {e}");
            std::thread::sleep(RETRY_DELAY);
            continue;
        }
        let client_tx = tx.clone();
        let file = File::from(pipe);
        let spawned = std::thread::Builder::new()
            .name("clipsy-pipe-client".to_string())
            .spawn(move || handle_client(file, client_tx));
        if let Err(e) = spawned {
            tracing::warn!("could not start pipe client thread: {e}");
        }
    }
}

/// Reads one verb line from a connected client and forwards it.
#[cfg(windows)]
fn handle_client(mut file: File, tx: Sender<Verb>) {
    let line = read_first_line(&mut file);
    let line = line.trim();
    if line.is_empty() {
        return;
    }
    match parse_verb(line) {
        // The receiver may be gone during shutdown; nothing to do then.
        Some(v) => {
            let _ = tx.send(v);
        }
        None => tracing::warn!("unknown verb from second instance: {line:?}"),
    }
}

/// Reads bytes up to the first newline, end of file, read error or the size cap.
/// The newline is not included. Reading stops at the newline, so a client that
/// keeps the pipe open after its line does not hold the reader.
#[cfg(windows)]
fn read_first_line(file: &mut File) -> String {
    let mut buf = Vec::new();
    let mut byte = [0u8; 1];
    while buf.len() < MAX_MESSAGE_BYTES {
        match file.read(&mut byte) {
            Ok(1) if byte[0] == b'\n' => break,
            Ok(1) => buf.push(byte[0]),
            _ => break,
        }
    }
    String::from_utf8_lossy(&buf).into_owned()
}

/// Creates one inbound byte-mode pipe instance that only the owner and SYSTEM
/// can open, and that rejects clients from remote machines.
#[cfg(windows)]
fn create_pipe(wname: &[u16], security: &PipeSecurity) -> anyhow::Result<OwnedHandle> {
    // SAFETY: `wname` is NUL-terminated and alive for the call. The security
    // attributes point at a descriptor owned by `security`, which outlives the
    // call; CreateNamedPipeW copies what it needs. Buffer sizes 0 use the system
    // defaults. The result is checked for an invalid handle before it is owned.
    let handle = unsafe {
        CreateNamedPipeW(
            PCWSTR(wname.as_ptr()),
            PIPE_ACCESS_INBOUND,
            PIPE_TYPE_BYTE | PIPE_READMODE_BYTE | PIPE_WAIT | PIPE_REJECT_REMOTE_CLIENTS,
            PIPE_UNLIMITED_INSTANCES,
            0,
            0,
            0,
            Some(security.attributes()),
        )
    };
    if handle.is_invalid() {
        return Err(windows::core::Error::from_win32()).context("could not create the Clipsy pipe");
    }
    // SAFETY: `handle.0` is a valid pipe handle that nothing else owns. OwnedHandle
    // closes it when dropped, which also disconnects the client.
    Ok(unsafe { OwnedHandle::from_raw_handle(handle.0) })
}

/// Waits for a client to connect. A client that connected before this call
/// (ERROR_PIPE_CONNECTED) counts as success.
#[cfg(windows)]
fn connect_pipe(pipe: &OwnedHandle) -> windows::core::Result<()> {
    // SAFETY: `pipe` is a valid pipe handle owned by us. None selects synchronous mode.
    match unsafe { ConnectNamedPipe(HANDLE(pipe.as_raw_handle()), None) } {
        Err(e) if e.code() == HRESULT::from_win32(ERROR_PIPE_CONNECTED.0) => Ok(()),
        other => other,
    }
}

/// Builds the pipe ACL SDDL for one user SID: full access for that user and for
/// SYSTEM, nothing for anyone else (including other local users).
///
/// The user's SID is used rather than the `OW` (owner rights) alias: the pipe's
/// owner is the elevated token's default owner (Administrators) when Clipsy runs
/// elevated, which would lock out the same user's non-elevated `clipsy-windows`.
#[cfg_attr(not(windows), allow(dead_code))]
pub fn sddl_for_user(sid: &str) -> String {
    format!("D:P(A;;GA;;;{sid})(A;;GA;;;SY)")
}

/// True when `text` looks like a SID such as `S-1-5-21-1-2-3-1001`: an `S-` prefix
/// followed only by digits and dashes. The text is embedded into an SDDL string, so
/// anything else must be refused.
#[cfg_attr(not(windows), allow(dead_code))]
fn is_sid_text(text: &str) -> bool {
    match text.strip_prefix("S-") {
        Some(rest) => !rest.is_empty() && rest.bytes().all(|b| b.is_ascii_digit() || b == b'-'),
        None => false,
    }
}

/// SID of the current process token's user, as text (for example `S-1-5-21-...`).
#[cfg(windows)]
fn user_sid_string() -> anyhow::Result<String> {
    // SAFETY: GetCurrentProcess returns a pseudo-handle that needs no closing.
    let process = unsafe { GetCurrentProcess() };
    let mut token = HANDLE(std::ptr::null_mut());
    // SAFETY: `token` is a valid out-pointer. On success it holds a new handle
    // that OwnedHandle closes on every path below.
    unsafe { OpenProcessToken(process, TOKEN_QUERY, &mut token) }
        .context("could not open the process token")?;
    // SAFETY: `token.0` is a handle we just received and nothing else owns.
    let token = unsafe { OwnedHandle::from_raw_handle(token.0) };
    let token_handle = HANDLE(token.as_raw_handle());

    // First call with no buffer: only asks for the required size. It fails with
    // ERROR_INSUFFICIENT_BUFFER by design, so its result is ignored.
    let mut len = 0u32;
    // SAFETY: a null buffer with length 0 is allowed; `len` is a valid out-pointer.
    let _ = unsafe { GetTokenInformation(token_handle, TokenUser, None, 0, &mut len) };
    if len == 0 {
        bail!("could not size the process token information");
    }

    // Vec<usize> gives the pointer alignment that TOKEN_USER needs.
    let words = (len as usize).div_ceil(std::mem::size_of::<usize>());
    let mut buf = vec![0usize; words];
    // SAFETY: buf holds at least `len` bytes with TOKEN_USER alignment. On success
    // it contains a TOKEN_USER whose Sid points into buf itself.
    unsafe {
        GetTokenInformation(
            token_handle,
            TokenUser,
            Some(buf.as_mut_ptr().cast()),
            len,
            &mut len,
        )
    }
    .context("could not read the process user SID")?;
    // SAFETY: after the successful call above, buf starts with a valid TOKEN_USER.
    // buf is still alive, so the Sid pointer it holds is valid to read here.
    let sid = unsafe { (*buf.as_ptr().cast::<TOKEN_USER>()).User.Sid };

    let mut sid_text = PWSTR::null();
    // SAFETY: `sid` is valid (see above) and `sid_text` is a valid out-pointer.
    // On success the system allocates the string, which is freed below.
    unsafe { ConvertSidToStringSidW(sid, &mut sid_text) }
        .context("could not convert the user SID to text")?;
    // SAFETY: sid_text is a NUL-terminated UTF-16 string that the system allocated
    // and that is freed only after this read.
    let text = unsafe { sid_text.to_string() };
    // SAFETY: frees the buffer allocated by ConvertSidToStringSidW, exactly once.
    let _ = unsafe { LocalFree(HLOCAL(sid_text.0.cast())) };
    let text = text.context("user SID text is not valid UTF-16")?;

    // The text goes into the SDDL string, so accept only the SID shape
    // S-<digits and dashes>. Anything else is refused rather than embedded.
    if !is_sid_text(&text) {
        bail!("unexpected user SID text: {text:?}");
    }
    Ok(text)
}

/// Security attributes for the pipe, granting access only to the current user
/// and SYSTEM (see `sddl_for_user`).
#[cfg(windows)]
struct PipeSecurity {
    descriptor: PSECURITY_DESCRIPTOR,
    attrs: SECURITY_ATTRIBUTES,
}

#[cfg(windows)]
impl PipeSecurity {
    fn for_current_user() -> anyhow::Result<Self> {
        let sddl = wide(&sddl_for_user(&user_sid_string()?));
        let mut descriptor = PSECURITY_DESCRIPTOR(std::ptr::null_mut());
        // SAFETY: `sddl` is NUL-terminated and alive for the call, and `descriptor`
        // is a valid out-pointer. On success Windows allocates the descriptor, and
        // `Drop` frees it with LocalFree.
        unsafe {
            ConvertStringSecurityDescriptorToSecurityDescriptorW(
                PCWSTR(sddl.as_ptr()),
                SDDL_REVISION_1,
                &mut descriptor,
                None,
            )
        }
        .context("could not build the pipe security descriptor")?;
        let attrs = SECURITY_ATTRIBUTES {
            nLength: std::mem::size_of::<SECURITY_ATTRIBUTES>() as u32,
            lpSecurityDescriptor: descriptor.0,
            bInheritHandle: BOOL(0),
        };
        Ok(Self { descriptor, attrs })
    }

    fn attributes(&self) -> *const SECURITY_ATTRIBUTES {
        &self.attrs
    }
}

#[cfg(windows)]
impl Drop for PipeSecurity {
    fn drop(&mut self) {
        // SAFETY: the descriptor was allocated by ConvertStringSecurityDescriptor...
        // and is freed exactly once, here.
        let _ = unsafe { LocalFree(HLOCAL(self.descriptor.0)) };
    }
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
    fn sddl_contains_user_sid_and_system() {
        let sddl = sddl_for_user("S-1-5-21-1-2-3-1001");
        assert!(sddl.starts_with("D:P"));
        assert!(sddl.contains("(A;;GA;;;S-1-5-21-1-2-3-1001)"));
        assert!(sddl.contains("(A;;GA;;;SY)"));
        assert!(!sddl.contains(";OW)"));
    }

    #[test]
    fn accepts_real_sid_text() {
        assert!(is_sid_text("S-1-5-21-3473395470-3281294518-677509544-1001"));
        assert!(is_sid_text("S-1-5-18"));
    }

    #[test]
    fn rejects_non_sid_text() {
        assert!(!is_sid_text(""));
        assert!(!is_sid_text("S-"));
        assert!(!is_sid_text("1-5-21"));
        assert!(!is_sid_text("S-1-5)(A;;GA;;;WD"));
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
