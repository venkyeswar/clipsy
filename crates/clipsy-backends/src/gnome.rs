//! GNOME Wayland backend. Talks to the Clipsy GNOME Shell extension over a Unix socket.
//!
//! The app is the server: it listens on `$XDG_RUNTIME_DIR/clipsy-shell.sock` and the extension
//! connects to it. Messages are one JSON object per line (see `docs/gnome-protocol.md`).
//! The extension does the things a normal Wayland app cannot: watch the clipboard, grab global
//! shortcuts, send key presses and place the popup window.

use std::collections::HashMap;
use std::io::{BufRead, BufReader, Read, Write};
use std::os::unix::fs::PermissionsExt;
use std::os::unix::net::{UnixListener, UnixStream};
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, AtomicU64, Ordering};
use std::sync::mpsc::Sender;
use std::sync::{Arc, Mutex, PoisonError};
use std::thread;

use anyhow::{anyhow, bail, Context, Result};
use base64::engine::general_purpose::STANDARD;
use base64::Engine as _;
use clipsy_core::{ClipEvent, Format};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::{Backend, HotkeyAction, PasteMode, SetupNeeded};

/// Longest accepted line (matches the protocol).
const MAX_LINE: usize = 48 * 1024 * 1024;
const NOT_RUNNING: &str = "The Clipsy GNOME extension is not running";

#[derive(Deserialize)]
struct WireFormat {
    mime: String,
    data: String,
}

/// Messages the extension sends us. Unknown types become `Unknown` and are ignored.
#[derive(Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum Incoming {
    Hello,
    Clip {
        #[serde(default)]
        formats: Vec<WireFormat>,
        #[serde(default)]
        source_app: Option<String>,
        #[serde(default)]
        sensitive: bool,
    },
    Hotkey { action: String },
    BindResult {
        action: String,
        ok: bool,
        #[serde(default)]
        error: Option<String>,
    },
    #[serde(other)]
    Unknown,
}

/// Parse one line. `None` means "not something we understand"; callers just skip it.
fn parse_line(line: &[u8]) -> Option<Incoming> {
    serde_json::from_slice(line).ok()
}

fn action_name(a: HotkeyAction) -> &'static str {
    match a {
        HotkeyAction::TogglePopup => "toggle_popup",
        HotkeyAction::ClearHistory => "clear_history",
        HotkeyAction::PauseRecording => "pause_recording",
    }
}

fn parse_action(s: &str) -> Option<HotkeyAction> {
    match s {
        "toggle_popup" => Some(HotkeyAction::TogglePopup),
        "clear_history" => Some(HotkeyAction::ClearHistory),
        "pause_recording" => Some(HotkeyAction::PauseRecording),
        _ => None,
    }
}

fn bind_msg(action: HotkeyAction, accel: &str) -> Value {
    json!({"type": "bind", "action": action_name(action), "accel": accel})
}

fn formats_msg(formats: &[Format]) -> Value {
    let list: Vec<Value> = formats
        .iter()
        .map(|f| json!({"mime": f.mime, "data": STANDARD.encode(&f.data)}))
        .collect();
    json!({"type": "set_clipboard", "formats": list})
}

/// Turn a `clip` message into a `ClipEvent`. Formats with bad base64 are skipped.
fn clip_event(formats: Vec<WireFormat>, source_app: Option<String>, sensitive: bool) -> ClipEvent {
    let formats = formats
        .into_iter()
        .filter_map(|f| STANDARD.decode(f.data.as_bytes()).ok().map(|data| Format { mime: f.mime, data }))
        .collect();
    ClipEvent { formats, source_app, sensitive }
}

enum Line {
    Data(Vec<u8>),
    TooLong,
    Eof,
}

/// Read one `\n`-terminated line without ever buffering more than `cap` bytes.
/// An oversize line is skipped to its end and reported as `TooLong`.
fn read_capped<R: BufRead>(r: &mut R, cap: usize) -> std::io::Result<Line> {
    let mut buf = Vec::new();
    let n = r.by_ref().take(cap as u64 + 1).read_until(b'\n', &mut buf)?;
    if n == 0 {
        return Ok(Line::Eof);
    }
    if buf.last() == Some(&b'\n') {
        buf.pop();
        return Ok(if buf.len() > cap { Line::TooLong } else { Line::Data(buf) });
    }
    if buf.len() > cap {
        // Discard the rest of this line in small pieces.
        loop {
            let mut sink = Vec::new();
            let n = r.by_ref().take(64 * 1024).read_until(b'\n', &mut sink)?;
            if n == 0 || sink.last() == Some(&b'\n') {
                break;
            }
        }
        return Ok(Line::TooLong);
    }
    Ok(Line::Eof) // partial line then EOF
}

struct Inner {
    writer: Mutex<Option<UnixStream>>,
    /// True between the extension's `hello` and the connection closing.
    connected: AtomicBool,
    /// Bumped per accepted connection so a stale reader cannot clobber a newer one.
    generation: AtomicU64,
    hotkeys: Mutex<HashMap<HotkeyAction, String>>,
    /// Last `bind_result` error per action (cleared when the action is registered again).
    bind_errors: Mutex<HashMap<HotkeyAction, String>>,
}

impl Inner {
    fn is_connected(&self) -> bool {
        self.connected.load(Ordering::SeqCst)
    }

    /// Write one JSON line. On failure the writer is dropped and we are marked disconnected.
    fn send(&self, msg: &Value) -> Result<()> {
        let mut line = serde_json::to_vec(msg)?;
        line.push(b'\n');
        let mut guard = self.writer.lock().unwrap_or_else(PoisonError::into_inner);
        let Some(w) = guard.as_mut() else { bail!(NOT_RUNNING) };
        if let Err(e) = w.write_all(&line).and_then(|_| w.flush()) {
            *guard = None;
            self.connected.store(false, Ordering::SeqCst);
            return Err(anyhow!("lost the Clipsy GNOME extension: {e}"));
        }
        Ok(())
    }

    fn send_if_connected(&self, msg: &Value) {
        if self.is_connected() {
            if let Err(e) = self.send(msg) {
                tracing::warn!("{e}");
            }
        }
    }

    fn handle(&self, msg: Incoming, clips: &Sender<ClipEvent>, hotkeys: &Sender<HotkeyAction>) {
        match msg {
            Incoming::Hello => {
                self.connected.store(true, Ordering::SeqCst);
                let binds: Vec<(HotkeyAction, String)> = self
                    .hotkeys
                    .lock()
                    .unwrap_or_else(PoisonError::into_inner)
                    .iter()
                    .map(|(a, s)| (*a, s.clone()))
                    .collect();
                for (a, accel) in binds {
                    if let Err(e) = self.send(&bind_msg(a, &accel)) {
                        tracing::warn!("{e}");
                        break;
                    }
                }
            }
            Incoming::Clip { formats, source_app, sensitive } => {
                let _ = clips.send(clip_event(formats, source_app, sensitive));
            }
            Incoming::Hotkey { action } => match parse_action(&action) {
                Some(a) => {
                    let _ = hotkeys.send(a);
                }
                None => tracing::debug!("ignoring unknown hotkey action {action:?}"),
            },
            Incoming::BindResult { action, ok, error } => {
                if ok {
                    return;
                }
                let error = error.unwrap_or_else(|| "unknown error".into());
                tracing::warn!("The extension could not bind {action}: {error}");
                if let Some(a) = parse_action(&action) {
                    self.bind_errors.lock().unwrap_or_else(PoisonError::into_inner).insert(a, error);
                }
            }
            Incoming::Unknown => {}
        }
    }
}

/// Backend for GNOME on Wayland, driven by the Clipsy GNOME Shell extension.
pub struct GnomeBackend {
    inner: Arc<Inner>,
    listener: Option<UnixListener>,
    socket_path: PathBuf,
    /// Extra terminal classes from the config, sent with every paste.
    terminal_apps: Mutex<Vec<String>>,
}

impl GnomeBackend {
    /// Listen on `$XDG_RUNTIME_DIR/clipsy-shell.sock` (temp dir if unset).
    pub fn new() -> Result<Self> {
        let dir = std::env::var_os("XDG_RUNTIME_DIR")
            .filter(|d| !d.is_empty())
            .map(PathBuf::from)
            .unwrap_or_else(std::env::temp_dir);
        Self::with_socket_path(dir.join("clipsy-shell.sock"))
    }

    /// Listen on a specific socket path (removing any stale file first).
    pub fn with_socket_path(path: impl AsRef<Path>) -> Result<Self> {
        let socket_path = path.as_ref().to_path_buf();
        let _ = std::fs::remove_file(&socket_path);
        let listener = UnixListener::bind(&socket_path)
            .with_context(|| format!("could not listen on {}", socket_path.display()))?;
        std::fs::set_permissions(&socket_path, std::fs::Permissions::from_mode(0o600))
            .with_context(|| format!("could not restrict {}", socket_path.display()))?;
        Ok(Self {
            inner: Arc::new(Inner {
                writer: Mutex::new(None),
                connected: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                hotkeys: Mutex::new(HashMap::new()),
                bind_errors: Mutex::new(HashMap::new()),
            }),
            listener: Some(listener),
            socket_path,
            terminal_apps: Mutex::new(Vec::new()),
        })
    }

    /// Why the extension last refused to bind `action` (e.g. the key is already taken).
    pub fn bind_error(&self, action: HotkeyAction) -> Option<String> {
        self.inner.bind_errors.lock().unwrap_or_else(PoisonError::into_inner).get(&action).cloned()
    }
}

fn serve_connection(
    inner: Arc<Inner>,
    stream: UnixStream,
    gen: u64,
    clips: Sender<ClipEvent>,
    hotkeys: Sender<HotkeyAction>,
) {
    let mut reader = BufReader::new(stream);
    loop {
        match read_capped(&mut reader, MAX_LINE) {
            Ok(Line::Data(line)) => {
                if let Some(msg) = parse_line(&line) {
                    inner.handle(msg, &clips, &hotkeys);
                }
            }
            Ok(Line::TooLong) => tracing::warn!("dropped an oversize line from the GNOME extension"),
            Ok(Line::Eof) | Err(_) => break,
        }
    }
    if inner.generation.load(Ordering::SeqCst) == gen {
        *inner.writer.lock().unwrap_or_else(PoisonError::into_inner) = None;
        inner.connected.store(false, Ordering::SeqCst);
        tracing::info!("the Clipsy GNOME extension disconnected");
    }
}

impl Backend for GnomeBackend {
    fn name(&self) -> &'static str {
        "gnome"
    }

    fn start(&mut self, clips: Sender<ClipEvent>, hotkeys: Sender<HotkeyAction>) -> Result<()> {
        let listener = self.listener.take().ok_or_else(|| anyhow!("the GNOME backend is already started"))?;
        let inner = self.inner.clone();
        thread::Builder::new().name("gnome-accept".into()).spawn(move || {
            for stream in listener.incoming() {
                let Ok(stream) = stream else { continue };
                let Ok(write_half) = stream.try_clone() else { continue };
                let gen = inner.generation.fetch_add(1, Ordering::SeqCst) + 1;
                {
                    // A new connection replaces the old one.
                    let mut w = inner.writer.lock().unwrap_or_else(PoisonError::into_inner);
                    if let Some(old) = w.replace(write_half) {
                        let _ = old.shutdown(std::net::Shutdown::Both);
                    }
                    inner.connected.store(false, Ordering::SeqCst);
                }
                let (inner, clips, hotkeys) = (inner.clone(), clips.clone(), hotkeys.clone());
                let _ = thread::Builder::new()
                    .name("gnome-reader".into())
                    .spawn(move || serve_connection(inner, stream, gen, clips, hotkeys));
            }
        })?;
        Ok(())
    }

    fn set_clipboard(&self, formats: &[Format]) -> Result<()> {
        if !self.inner.is_connected() {
            bail!(NOT_RUNNING);
        }
        self.inner.send(&formats_msg(formats))
    }

    fn remember_active_window(&self) {
        self.inner.send_if_connected(&json!({"type": "remember_window"}));
    }

    fn paste(&self, mode: PasteMode) -> Result<()> {
        if !self.inner.is_connected() {
            bail!(NOT_RUNNING);
        }
        let mode = match mode {
            PasteMode::Normal => "normal",
            PasteMode::Terminal => "terminal",
        };
        let terminal_apps = self.terminal_apps.lock().unwrap_or_else(PoisonError::into_inner).clone();
        // Older extensions ignore `terminal_apps`.
        self.inner.send(&json!({"type": "paste", "mode": mode, "terminal_apps": terminal_apps}))
    }

    fn supports_paste(&self) -> bool {
        self.inner.is_connected()
    }

    fn set_terminal_apps(&self, apps: &[String]) {
        *self.terminal_apps.lock().unwrap_or_else(PoisonError::into_inner) = apps.to_vec();
    }

    fn register_hotkey(&mut self, action: HotkeyAction, accel: &str) -> Result<()> {
        self.inner.hotkeys.lock().unwrap_or_else(PoisonError::into_inner).insert(action, accel.to_string());
        self.inner.bind_errors.lock().unwrap_or_else(PoisonError::into_inner).remove(&action);
        self.inner.send_if_connected(&bind_msg(action, accel));
        Ok(())
    }

    fn unregister_hotkey(&mut self, action: HotkeyAction) {
        self.inner.hotkeys.lock().unwrap_or_else(PoisonError::into_inner).remove(&action);
        self.inner.send_if_connected(&json!({"type": "unbind", "action": action_name(action)}));
    }

    fn popup_will_show(&self, position: &str) {
        self.inner.send_if_connected(&json!({"type": "show_popup", "position": position}));
    }

    fn setup_needed(&self) -> Option<SetupNeeded> {
        (!self.inner.is_connected()).then_some(SetupNeeded::GnomeExtension)
    }
}

impl Drop for GnomeBackend {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(&self.socket_path);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Cursor;
    use std::sync::mpsc::channel;
    use std::time::{Duration, Instant};

    #[test]
    fn clip_round_trip() {
        let msg = json!({"type":"clip","formats":[
            {"mime":"text/plain;charset=utf-8","data": STANDARD.encode("héllo")},
            {"mime":"image/png","data": STANDARD.encode([0u8, 1, 255])},
            {"mime":"bad","data":"!!!not base64"}],
            "source_app":"kitty","sensitive":true});
        let Some(Incoming::Clip { formats, source_app, sensitive }) = parse_line(msg.to_string().as_bytes()) else {
            panic!("not a clip")
        };
        let ev = clip_event(formats, source_app, sensitive);
        assert_eq!(ev.formats.len(), 2);
        assert_eq!(ev.formats[0].data, "héllo".as_bytes());
        assert_eq!(ev.formats[1].data, vec![0, 1, 255]);
        assert_eq!(ev.source_app.as_deref(), Some("kitty"));
        assert!(ev.sensitive);
    }

    #[test]
    fn set_clipboard_encoding() {
        let v = formats_msg(&[Format { mime: "image/png".into(), data: vec![1, 2, 3] }]);
        assert_eq!(v["type"], "set_clipboard");
        assert_eq!(v["formats"][0]["data"], STANDARD.encode([1u8, 2, 3]));
    }

    #[test]
    fn hotkey_and_unknown_parsing() {
        match parse_line(br#"{"type":"hotkey","action":"clear_history"}"#) {
            Some(Incoming::Hotkey { action }) => assert_eq!(parse_action(&action), Some(HotkeyAction::ClearHistory)),
            _ => panic!(),
        }
        assert_eq!(parse_action("nope"), None);
        assert!(matches!(parse_line(br#"{"type":"from_the_future","x":1}"#), Some(Incoming::Unknown)));
        assert!(parse_line(b"not json").is_none());
        assert!(parse_line(br#"{"type":"hotkey"}"#).is_none());
    }

    #[test]
    fn oversize_line_rejected_and_stream_recovers() {
        let mut data = vec![b'a'; 100];
        data.extend_from_slice(b"\nshort\n");
        let mut r = Cursor::new(data);
        assert!(matches!(read_capped(&mut r, 10).unwrap(), Line::TooLong));
        assert!(matches!(read_capped(&mut r, 10).unwrap(), Line::Data(d) if d == b"short"));
        assert!(matches!(read_capped(&mut r, 10).unwrap(), Line::Eof));
        // Oversize with no newline before EOF.
        let mut r = Cursor::new(vec![b'a'; 100]);
        assert!(matches!(read_capped(&mut r, 10).unwrap(), Line::TooLong));
    }

    fn wait_for(mut f: impl FnMut() -> bool) -> bool {
        let end = Instant::now() + Duration::from_secs(3);
        while Instant::now() < end {
            if f() {
                return true;
            }
            thread::sleep(Duration::from_millis(10));
        }
        false
    }

    #[test]
    fn fake_extension_session() {
        let dir = std::env::temp_dir().join(format!("clipsy-gnome-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let path = dir.join("shell.sock");
        let mut b = GnomeBackend::with_socket_path(&path).unwrap();
        assert_eq!(std::fs::metadata(&path).unwrap().permissions().mode() & 0o777, 0o600);
        assert_eq!(b.name(), "gnome");
        let (ctx, crx) = channel();
        let (htx, hrx) = channel();
        b.start(ctx, htx).unwrap();
        b.register_hotkey(HotkeyAction::TogglePopup, "<Super><Shift>v").unwrap();
        assert_eq!(b.setup_needed(), Some(SetupNeeded::GnomeExtension));
        assert!(b.set_clipboard(&[]).is_err());
        assert!(b.paste(PasteMode::Normal).is_err());

        let mut client = UnixStream::connect(&path).unwrap();
        client.set_read_timeout(Some(Duration::from_secs(3))).unwrap();
        let mut lines = BufReader::new(client.try_clone().unwrap());
        client.write_all(b"{\"type\":\"hello\",\"version\":1,\"shell\":\"46.0\"}\n").unwrap();
        let mut l = String::new();
        lines.read_line(&mut l).unwrap();
        let v: Value = serde_json::from_str(&l).unwrap();
        assert_eq!(v, json!({"type":"bind","action":"toggle_popup","accel":"<Super><Shift>v"}));
        assert!(b.setup_needed().is_none());
        assert!(b.supports_paste());

        let clip = json!({"type":"clip","formats":[{"mime":"text/plain;charset=utf-8","data":STANDARD.encode("hi")}],"source_app":null,"sensitive":false});
        writeln!(client, "{clip}").unwrap();
        let ev = crx.recv_timeout(Duration::from_secs(3)).unwrap();
        assert_eq!(ev.formats[0].data, b"hi");
        assert_eq!(ev.source_app, None);

        client.write_all(b"{\"type\":\"bogus\"}\ngarbage\n{\"type\":\"hotkey\",\"action\":\"toggle_popup\"}\n").unwrap();
        assert_eq!(hrx.recv_timeout(Duration::from_secs(3)).unwrap(), HotkeyAction::TogglePopup);

        client.write_all(b"{\"type\":\"bind_result\",\"action\":\"toggle_popup\",\"ok\":false,\"error\":\"taken\"}\n").unwrap();
        assert!(wait_for(|| b.bind_error(HotkeyAction::TogglePopup).is_some()));

        b.popup_will_show("center");
        l.clear();
        lines.read_line(&mut l).unwrap();
        assert_eq!(serde_json::from_str::<Value>(&l).unwrap(), json!({"type":"show_popup","position":"center"}));
        b.paste(PasteMode::Terminal).unwrap();
        l.clear();
        lines.read_line(&mut l).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&l).unwrap(),
            json!({"type":"paste","mode":"terminal","terminal_apps":[]})
        );
        b.set_terminal_apps(&["Code".to_string()]);
        b.paste(PasteMode::Normal).unwrap();
        l.clear();
        lines.read_line(&mut l).unwrap();
        assert_eq!(
            serde_json::from_str::<Value>(&l).unwrap(),
            json!({"type":"paste","mode":"normal","terminal_apps":["Code"]})
        );

        drop(lines);
        drop(client);
        assert!(wait_for(|| b.setup_needed().is_some()));
        drop(b);
        assert!(!path.exists());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
