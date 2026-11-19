//! Windows clipboard access: a message-only listener thread (`AddClipboardFormatListener`) that
//! turns clipboard changes into `ClipEvent`s, and a writer that puts stored formats back on the
//! clipboard. Win32 calls live in small helpers with SAFETY comments; the byte-level payload
//! builders and parsers are pure functions so they are tested on any OS (the module itself is
//! compiled only on Windows).

use std::cell::RefCell;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::OnceLock;

use anyhow::{anyhow, bail, Context, Result};
use clipsy_core::{ClipEvent, Format};
use windows::core::PCWSTR;
use windows::Win32::Foundation::{CloseHandle, GlobalFree, HANDLE, HGLOBAL, HINSTANCE, HWND, LPARAM, LRESULT, WPARAM};
use windows::Win32::System::DataExchange::{
    AddClipboardFormatListener, CloseClipboard, EmptyClipboard, GetClipboardData, GetClipboardOwner,
    GetClipboardSequenceNumber, IsClipboardFormatAvailable, OpenClipboard, RegisterClipboardFormatW,
    SetClipboardData,
};
use windows::Win32::System::Memory::{GlobalAlloc, GlobalLock, GlobalSize, GlobalUnlock, GMEM_MOVEABLE};
use windows::Win32::System::Ole::{CF_DIB, CF_HDROP, CF_UNICODETEXT};
use windows::Win32::System::Threading::{
    OpenProcess, QueryFullProcessImageNameW, PROCESS_NAME_WIN32, PROCESS_QUERY_LIMITED_INFORMATION,
};
use windows::Win32::UI::WindowsAndMessaging::{
    CreateWindowExW, DefWindowProcW, DispatchMessageW, GetMessageW, GetWindowThreadProcessId,
    RegisterClassW, TranslateMessage, CW_USEDEFAULT, HWND_MESSAGE, MSG, WM_CLIPBOARDUPDATE, WNDCLASSW,
    WINDOW_EX_STYLE, WINDOW_STYLE,
};

use crate::formats::{build_event, RawClip};
use crate::util::{cut_at_nul, dword_is_zero, percent_decode, retry};

/// Largest clipboard payload we copy out of another process (guards against huge allocations).
const MAX_PAYLOAD: usize = 256 * 1024 * 1024;

/// Registered clipboard format ids, looked up once per process.
struct FormatIds {
    png: u32,
    html: u32,
    exclude_monitor: u32,
    history_disallowed: u32,
    cloud_disallowed: u32,
}

fn format_ids() -> &'static FormatIds {
    static IDS: OnceLock<FormatIds> = OnceLock::new();
    IDS.get_or_init(|| FormatIds {
        png: format_id("PNG"),
        html: format_id("HTML Format"),
        exclude_monitor: format_id("ExcludeClipboardContentFromMonitorProcessing"),
        history_disallowed: format_id("CanIncludeInClipboardHistory"),
        cloud_disallowed: format_id("CanUploadToCloudClipboard"),
    })
}

/// Sequence number of our own last clipboard write, or -1 when there is none pending.
static OWN_WRITE_SEQ: AtomicI64 = AtomicI64::new(-1);

thread_local! {
    /// Where the listener window sends events. Only touched on the listener thread.
    static LISTENER_TX: RefCell<Option<Sender<ClipEvent>>> = const { RefCell::new(None) };
}

/// Start a message-only window on its own thread that forwards every clipboard change to `tx`.
/// Returns once the window exists, or with the Win32 error if it could not be created.
pub fn spawn_listener(tx: Sender<ClipEvent>) -> Result<()> {
    let (ready_tx, ready_rx) = std::sync::mpsc::channel::<Result<()>>();
    std::thread::Builder::new()
        .name("clipsy-clipboard".into())
        .spawn(move || {
            if let Err(e) = create_listener_window() {
                let _ = ready_tx.send(Err(e));
                return;
            }
            LISTENER_TX.with(|cell| *cell.borrow_mut() = Some(tx));
            let _ = ready_tx.send(Ok(()));
            message_loop();
        })
        .context("starting clipboard listener thread")?;
    ready_rx
        .recv()
        .map_err(|_| anyhow!("clipboard listener thread exited before it started"))?
}

/// Put `formats` on the clipboard, replacing its current contents. Supported: `text/plain`
/// (CF_UNICODETEXT), `image/png` ("PNG"), `text/html` ("HTML Format"), `text/uri-list` (CF_HDROP).
/// Other MIME types are skipped. Errors when nothing supported is present.
pub fn write_formats(formats: &[Format]) -> Result<()> {
    let payloads = clipboard_payloads(formats)?;
    // A null owner: the clipboard is written without an owner window.
    let open = open_clipboard(HWND(std::ptr::null_mut()))?;
    let result = fill_clipboard(&payloads);
    // Mark even after a partial write: whatever did land is still our write, and the listener
    // must not record it as a new history item. Assumption: WM_CLIPBOARDUPDATE is posted only
    // after CloseClipboard, and mark_own_write runs while the clipboard is still open.
    mark_own_write();
    drop(open);
    result
}

/// Empty the clipboard and hand it every payload. The caller must hold the clipboard open.
fn fill_clipboard(payloads: &[(u32, Vec<u8>)]) -> Result<()> {
    // SAFETY: the clipboard is open on this thread (the caller holds it).
    unsafe { EmptyClipboard() }.context("EmptyClipboard")?;
    for (format, bytes) in payloads {
        set_clipboard_bytes(*format, bytes)?;
    }
    Ok(())
}

/// Remember the current clipboard sequence number so the listener ignores the update caused by
/// our own write. Call it while the clipboard is open, after the last SetClipboardData;
/// `write_formats` does this.
pub fn mark_own_write() {
    // SAFETY: GetClipboardSequenceNumber takes no arguments and has no failure mode.
    let seq = unsafe { GetClipboardSequenceNumber() };
    OWN_WRITE_SEQ.store(i64::from(seq), Ordering::SeqCst);
}

/// True (once) when `seq` is the sequence number recorded by `mark_own_write`.
fn take_own_write(seq: u32) -> bool {
    OWN_WRITE_SEQ
        .compare_exchange(i64::from(seq), -1, Ordering::SeqCst, Ordering::SeqCst)
        .is_ok()
}

// ---------------------------------------------------------------------------------------------
// Listener side
// ---------------------------------------------------------------------------------------------

fn create_listener_window() -> Result<()> {
    // SAFETY: a null module name asks for the handle of the running executable.
    let module = unsafe { windows::Win32::System::LibraryLoader::GetModuleHandleW(PCWSTR::null()) }
        .context("GetModuleHandleW")?;
    let class_name = wide("ClipsyClipboardListener");
    let class = WNDCLASSW {
        lpfnWndProc: Some(wnd_proc),
        hInstance: HINSTANCE(module.0),
        lpszClassName: PCWSTR(class_name.as_ptr()),
        ..Default::default()
    };
    // SAFETY: `class` and the strings it points to outlive this call; the class is registered
    // once per process by the single listener. A zero atom means failure.
    if unsafe { RegisterClassW(&class) } == 0 {
        bail!("RegisterClassW failed");
    }
    // SAFETY: all pointers are valid for the call; HWND_MESSAGE makes a message-only window.
    let hwnd = unsafe {
        CreateWindowExW(
            WINDOW_EX_STYLE(0),
            PCWSTR(class_name.as_ptr()),
            PCWSTR::null(),
            WINDOW_STYLE(0),
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            CW_USEDEFAULT,
            HWND_MESSAGE,
            None,
            HINSTANCE(module.0),
            None,
        )
    }
    .context("CreateWindowExW")?;
    // SAFETY: `hwnd` was just created by this thread.
    unsafe { AddClipboardFormatListener(hwnd) }.context("AddClipboardFormatListener")?;
    Ok(())
}

fn message_loop() {
    let mut msg = MSG::default();
    loop {
        // SAFETY: `msg` is a valid, writable MSG for the duration of the call.
        let result = unsafe { GetMessageW(&mut msg, None, 0, 0) };
        // 0 means WM_QUIT, -1 means an error; both end the loop.
        if result.0 <= 0 {
            break;
        }
        // SAFETY: `msg` came from GetMessageW on this thread.
        unsafe {
            let _ = TranslateMessage(&msg);
            DispatchMessageW(&msg);
        }
    }
}

/// Window procedure. Runs on the listener thread. Must never panic (panics cannot unwind out of
/// a Win32 callback), so everything here is fallible without `unwrap`.
unsafe extern "system" fn wnd_proc(hwnd: HWND, msg: u32, wparam: WPARAM, lparam: LPARAM) -> LRESULT {
    if msg == WM_CLIPBOARDUPDATE {
        handle_update(hwnd);
        return LRESULT(0);
    }
    // SAFETY: forwarding the unmodified arguments Windows gave us to the default handler.
    unsafe { DefWindowProcW(hwnd, msg, wparam, lparam) }
}

fn handle_update(owner: HWND) {
    // SAFETY: no arguments, no failure mode.
    let seq = unsafe { GetClipboardSequenceNumber() };
    if take_own_write(seq) {
        return;
    }
    let raw = match read_clipboard(owner) {
        Ok(raw) => raw,
        Err(e) => {
            tracing::warn!("reading clipboard failed: {e:#}");
            return;
        }
    };
    let Some(event) = build_event(raw) else { return };
    LISTENER_TX.with(|cell| {
        if let Some(tx) = cell.borrow().as_ref() {
            if tx.send(event).is_err() {
                tracing::warn!("clipboard event receiver is gone");
            }
        }
    });
}

/// Read every format we care about. The clipboard is opened for the duration of the call.
fn read_clipboard(owner: HWND) -> Result<RawClip> {
    let _open = open_clipboard(owner)?;
    let ids = format_ids();

    // A DWORD of 0 in either opt-out format means the copier does not want this in history or
    // in the cloud clipboard. Both count as sensitive, like the monitor-exclusion flag.
    let opted_out = |format: u32| read_format(format).is_some_and(|b| dword_is_zero(&b));
    let history_disallowed = opted_out(ids.history_disallowed) || opted_out(ids.cloud_disallowed);

    let png = read_format(ids.png);
    // CF_DIB is only needed when there is no PNG to use instead.
    let dib = if png.is_none() { read_format(u32::from(CF_DIB.0)) } else { None };

    Ok(RawClip {
        // GlobalSize is the allocation size, so the text may be followed by slack after its NUL.
        unicode_text: read_format(u32::from(CF_UNICODETEXT.0))
            .map(|b| cut_at_nul(&utf16_units(&b)).to_vec()),
        png,
        dib,
        html: read_format(ids.html),
        files: read_format(u32::from(CF_HDROP.0)).map(|b| parse_dropfiles(&b)).unwrap_or_default(),
        exclude_flag: format_available(ids.exclude_monitor),
        history_disallowed,
        source_exe: source_exe_name(),
    })
}

/// Bytes of a clipboard format, or `None` when the format is missing or empty.
fn read_format(format: u32) -> Option<Vec<u8>> {
    if format == 0 {
        return None;
    }
    // SAFETY: the caller holds the clipboard open. Windows returns an error (mapped to None) for
    // formats that are not present.
    let handle = unsafe { GetClipboardData(format) }.ok()?;
    read_global(handle)
}

fn format_available(format: u32) -> bool {
    // SAFETY: a plain query; the clipboard is open on this thread via the caller's guard.
    format != 0 && unsafe { IsClipboardFormatAvailable(format) }.is_ok()
}

/// Copy the bytes of a global memory handle owned by the clipboard.
fn read_global(handle: HANDLE) -> Option<Vec<u8>> {
    let hglobal = HGLOBAL(handle.0);
    // SAFETY: `hglobal` is a handle returned by GetClipboardData; GlobalSize is 0 on failure.
    let size = unsafe { GlobalSize(hglobal) };
    if size == 0 || size > MAX_PAYLOAD {
        return None;
    }
    // SAFETY: GlobalLock returns a pointer valid for `size` bytes until the matching unlock.
    let ptr = unsafe { GlobalLock(hglobal) } as *const u8;
    if ptr.is_null() {
        return None;
    }
    let _unlock = GlobalGuard(hglobal);
    // SAFETY: the block is locked, so it stays at `ptr` for `size` readable bytes; we copy out
    // before the guard unlocks it.
    let bytes = unsafe { std::slice::from_raw_parts(ptr, size) }.to_vec();
    Some(bytes)
}

/// Unlocks a global memory block when dropped.
struct GlobalGuard(HGLOBAL);

impl Drop for GlobalGuard {
    fn drop(&mut self) {
        // SAFETY: this guard only exists after a successful GlobalLock of the same handle.
        // GlobalUnlock reports "not locked anymore" as an error; that is expected and ignored.
        let _ = unsafe { GlobalUnlock(self.0) };
    }
}

/// Closes the clipboard when dropped, so every early return still releases it.
struct ClipboardGuard;

impl Drop for ClipboardGuard {
    fn drop(&mut self) {
        // SAFETY: this guard only exists while the clipboard is open on this thread.
        let _ = unsafe { CloseClipboard() };
    }
}

/// Open the clipboard, retrying because other programs often hold it for a moment.
fn open_clipboard(owner: HWND) -> Result<ClipboardGuard> {
    let opened = retry(10, 20, || {
        // SAFETY: `owner` is either null or a window handle owned by this thread.
        unsafe { OpenClipboard(owner) }.ok()
    });
    opened.ok_or_else(|| anyhow!("could not open the clipboard (held by another program)"))?;
    Ok(ClipboardGuard)
}

/// Window the clipboard was last changed by, as a process image file name (e.g. `chrome.exe`).
fn source_exe_name() -> Option<String> {
    // SAFETY: no arguments; fails when nothing owns the clipboard.
    let owner = unsafe { GetClipboardOwner() }.ok()?;
    let mut pid = 0u32;
    // SAFETY: `pid` is a valid, writable u32 for the duration of the call.
    unsafe { GetWindowThreadProcessId(owner, Some(&mut pid as *mut u32)) };
    if pid == 0 {
        return None;
    }
    // SAFETY: a query-only right on a process id; the handle is closed by HandleGuard.
    let process = unsafe { OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, false, pid) }.ok()?;
    let _close = HandleGuard(process);
    let mut buf = [0u16; 1024];
    let mut len = buf.len() as u32;
    // SAFETY: `buf` has `len` writable UTF-16 units, and `len` is updated to the length written.
    unsafe {
        QueryFullProcessImageNameW(process, PROCESS_NAME_WIN32, windows::core::PWSTR(buf.as_mut_ptr()), &mut len)
    }
    .ok()?;
    let written = buf.get(..len as usize)?;
    let full = String::from_utf16_lossy(written);
    full.rsplit('\\').next().filter(|s| !s.is_empty()).map(str::to_string)
}

/// Closes a kernel handle when dropped.
struct HandleGuard(HANDLE);

impl Drop for HandleGuard {
    fn drop(&mut self) {
        // SAFETY: the guard only wraps a handle returned by OpenProcess.
        let _ = unsafe { CloseHandle(self.0) };
    }
}

// ---------------------------------------------------------------------------------------------
// Writer side
// ---------------------------------------------------------------------------------------------

/// Turn stored formats into `(clipboard format id, bytes)` pairs. Only the first format of each
/// kind is used. Errors when none of the formats can be written.
fn clipboard_payloads(formats: &[Format]) -> Result<Vec<(u32, Vec<u8>)>> {
    let mut out: Vec<(u32, Vec<u8>)> = Vec::new();
    let mut seen_text = false;
    let mut seen_png = false;
    let mut seen_html = false;
    let mut seen_files = false;
    for f in formats {
        // Empty data would make GlobalAlloc(0); there is nothing useful to put on the clipboard.
        if f.data.is_empty() {
            continue;
        }
        let base = base_mime(&f.mime);
        if base == "text/plain" && !seen_text {
            seen_text = true;
            out.push((u32::from(CF_UNICODETEXT.0), utf16_nul_bytes(&String::from_utf8_lossy(&f.data))));
        } else if base == "image/png" && !seen_png {
            seen_png = true;
            push_registered(&mut out, format_ids().png, f.data.clone());
        } else if base == "text/html" && !seen_html {
            seen_html = true;
            push_registered(&mut out, format_ids().html, build_cf_html(&f.data));
        } else if base == "text/uri-list" && !seen_files {
            let paths: Vec<String> = uri_list_to_paths(&f.data);
            if !paths.is_empty() {
                seen_files = true;
                out.push((u32::from(CF_HDROP.0), paths_to_dropfiles(&paths)));
            }
        }
    }
    if out.is_empty() {
        bail!("no formats the Windows clipboard can take");
    }
    Ok(out)
}

fn push_registered(out: &mut Vec<(u32, Vec<u8>)>, id: u32, bytes: Vec<u8>) {
    if id != 0 {
        out.push((id, bytes));
    }
}

/// Hand `bytes` to the clipboard as `format`. The memory block is freed on failure only,
/// because after a successful SetClipboardData the system owns it.
fn set_clipboard_bytes(format: u32, bytes: &[u8]) -> Result<()> {
    // SAFETY: a movable block of `bytes.len()` bytes; `bytes` is never empty (callers always
    // include a terminator or header). Freed below if it is not handed to the clipboard.
    let hglobal = unsafe { GlobalAlloc(GMEM_MOVEABLE, bytes.len()) }.context("GlobalAlloc")?;
    // SAFETY: `hglobal` is a fresh block of at least `bytes.len()` bytes.
    let ptr = unsafe { GlobalLock(hglobal) } as *mut u8;
    if ptr.is_null() {
        // SAFETY: we still own `hglobal` because it was never given to the clipboard.
        let _ = unsafe { GlobalFree(hglobal) };
        bail!("GlobalLock failed");
    }
    // SAFETY: `ptr` addresses `bytes.len()` writable bytes, and the source slice does not overlap
    // the fresh allocation.
    unsafe { std::ptr::copy_nonoverlapping(bytes.as_ptr(), ptr, bytes.len()) };
    // SAFETY: the block was locked above by this thread.
    let _ = unsafe { GlobalUnlock(hglobal) };

    // SAFETY: the clipboard is open and emptied on this thread; on success the system takes the
    // block, on failure we free it below.
    if let Err(e) = unsafe { SetClipboardData(format, HANDLE(hglobal.0)) } {
        // SAFETY: the clipboard refused the block, so we still own it.
        let _ = unsafe { GlobalFree(hglobal) };
        return Err(e).context("SetClipboardData");
    }
    Ok(())
}

// ---------------------------------------------------------------------------------------------
// Pure helpers (byte layouts, parsing, conversions)
// ---------------------------------------------------------------------------------------------

/// Registered clipboard format id for `name`, or 0 when Windows refuses.
fn format_id(name: &str) -> u32 {
    let wide = wide(name);
    // SAFETY: `wide` is a NUL-terminated UTF-16 string that lives for the call.
    unsafe { RegisterClipboardFormatW(PCWSTR(wide.as_ptr())) }
}

/// UTF-16 string with a terminating NUL, as a `Vec<u16>`.
fn wide(s: &str) -> Vec<u16> {
    s.encode_utf16().chain(std::iter::once(0)).collect()
}

/// UTF-16LE bytes of `text` followed by a 16-bit NUL, the layout of CF_UNICODETEXT.
fn utf16_nul_bytes(text: &str) -> Vec<u8> {
    text.encode_utf16()
        .chain(std::iter::once(0))
        .flat_map(|u| u.to_le_bytes())
        .collect()
}

/// Little-endian UTF-16 units from raw clipboard bytes (a trailing odd byte is ignored).
fn utf16_units(bytes: &[u8]) -> Vec<u16> {
    let mut units = Vec::with_capacity(bytes.len() / 2);
    for pair in bytes.chunks(2) {
        if let [lo, hi] = *pair {
            units.push(u16::from_le_bytes([lo, hi]));
        }
    }
    units
}

fn u32_at(data: &[u8], off: usize) -> Option<u32> {
    let b = data.get(off..off.checked_add(4)?)?;
    Some(u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
}

/// Wrap an HTML fragment in the CF_HTML header ("Version:0.9" plus byte offsets).
fn build_cf_html(fragment: &[u8]) -> Vec<u8> {
    const PREFIX: &[u8] = b"<html><body>\r\n<!--StartFragment-->";
    const SUFFIX: &[u8] = b"<!--EndFragment-->\r\n</body>\r\n</html>";
    let header = |start_html: usize, end_html: usize, start_frag: usize, end_frag: usize| {
        format!(
            "Version:0.9\r\nStartHTML:{start_html:010}\r\nEndHTML:{end_html:010}\r\n\
             StartFragment:{start_frag:010}\r\nEndFragment:{end_frag:010}\r\n"
        )
    };
    // The header has fixed width (10-digit offsets), so its length is known up front.
    let header_len = header(0, 0, 0, 0).len();
    let start_frag = header_len + PREFIX.len();
    let end_frag = start_frag + fragment.len();
    let end_html = end_frag + SUFFIX.len();

    let mut out = header(header_len, end_html, start_frag, end_frag).into_bytes();
    out.extend_from_slice(PREFIX);
    out.extend_from_slice(fragment);
    out.extend_from_slice(SUFFIX);
    out
}

/// CF_HDROP payload (DROPFILES header followed by a double-NUL-terminated UTF-16 path list).
fn paths_to_dropfiles(paths: &[String]) -> Vec<u8> {
    const HEADER_LEN: u32 = 20;
    let mut out = Vec::new();
    out.extend_from_slice(&HEADER_LEN.to_le_bytes()); // pFiles: offset of the list
    out.extend_from_slice(&[0u8; 8]); // pt: drop point (unused)
    out.extend_from_slice(&0u32.to_le_bytes()); // fNC
    out.extend_from_slice(&1u32.to_le_bytes()); // fWide: UTF-16 list
    for p in paths {
        for unit in p.encode_utf16().chain(std::iter::once(0)) {
            out.extend_from_slice(&unit.to_le_bytes());
        }
    }
    out.extend_from_slice(&[0u8; 2]); // list terminator
    out
}

/// Paths from a CF_HDROP payload. Handles both UTF-16 (fWide) and ANSI lists.
fn parse_dropfiles(data: &[u8]) -> Vec<String> {
    let (Some(offset), Some(wide_flag)) = (u32_at(data, 0), u32_at(data, 16)) else {
        return Vec::new();
    };
    let Some(list) = usize::try_from(offset).ok().and_then(|o| data.get(o..)) else {
        return Vec::new();
    };
    if wide_flag != 0 {
        let units = utf16_units(list);
        units
            .split(|&u| u == 0)
            .take_while(|s| !s.is_empty())
            .map(String::from_utf16_lossy)
            .collect()
    } else {
        list.split(|&b| b == 0)
            .take_while(|s| !s.is_empty())
            .map(|s| String::from_utf8_lossy(s).into_owned())
            .collect()
    }
}

/// Windows paths from a `text/uri-list` body. Lines that are not `file://` URIs are skipped.
fn uri_list_to_paths(body: &[u8]) -> Vec<String> {
    String::from_utf8_lossy(body)
        .lines()
        .map(str::trim)
        .filter(|l| !l.is_empty() && !l.starts_with('#'))
        .filter_map(uri_to_path)
        .collect()
}

/// `file:///C:/My%20Docs/a.txt` -> `C:\My Docs\a.txt`; `file://server/share/x` -> `\\server\share\x`.
fn uri_to_path(uri: &str) -> Option<String> {
    let rest = uri.strip_prefix("file://")?;
    let decoded = percent_decode(rest);
    Some(match decoded.strip_prefix('/') {
        Some(local) => local.replace('/', "\\"),
        None => format!("\\\\{}", decoded.replace('/', "\\")),
    })
}

/// MIME type without parameters, lower case (`Text/Plain; charset=utf-8` -> `text/plain`).
fn base_mime(mime: &str) -> String {
    mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::formats::files_to_uri_list;

    #[test]
    fn utf16_nul_bytes_ends_with_terminator() {
        let bytes = utf16_nul_bytes("hé");
        assert_eq!(bytes, vec![b'h', 0, 0xE9, 0, 0, 0]);
        assert_eq!(utf16_units(&bytes), vec![u16::from(b'h'), 0xE9, 0]);
    }

    #[test]
    fn cf_html_round_trips_through_build_event() {
        let fragment = b"<b>bold</b> word";
        let doc = build_cf_html(fragment);
        let raw = RawClip { html: Some(doc), ..Default::default() };
        let ev = build_event(raw).unwrap();
        let html = ev.formats.iter().find(|f| f.mime == "text/html").unwrap();
        assert_eq!(html.data, fragment);
    }

    #[test]
    fn dropfiles_round_trip() {
        let paths = vec![r"C:\My Docs\café 1.pdf".to_string(), r"\\server\share\x.txt".to_string()];
        assert_eq!(parse_dropfiles(&paths_to_dropfiles(&paths)), paths);
    }

    #[test]
    fn dropfiles_garbage_is_empty() {
        assert!(parse_dropfiles(&[1, 2, 3]).is_empty());
        assert!(parse_dropfiles(&[0xff; 20]).is_empty());
    }

    #[test]
    fn uri_list_round_trips_paths() {
        let paths = vec![
            r"C:\Users\me\My Docs\report 1.pdf".to_string(),
            r"\\server\share\a#b.txt".to_string(),
            r"D:\plain.txt".to_string(),
        ];
        let body = files_to_uri_list(&paths);
        assert_eq!(uri_list_to_paths(body.as_bytes()), paths);
    }

    #[test]
    fn uri_list_skips_comments_and_non_file_uris() {
        let body = b"# comment\r\nhttps://example.com/x\r\nfile:///C:/a.txt\r\n";
        assert_eq!(uri_list_to_paths(body), vec![r"C:\a.txt".to_string()]);
    }

    #[test]
    fn own_write_is_taken_once() {
        OWN_WRITE_SEQ.store(42, Ordering::SeqCst);
        assert!(!take_own_write(41));
        assert!(take_own_write(42));
        assert!(!take_own_write(42));
    }
}
