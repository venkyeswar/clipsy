//! Shared data types. THE CONTRACT between core, backends and app. Do not change signatures
//! without updating all three crates.

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Kind { Text, Html, Image, Files, Other }

impl Kind {
    pub fn as_str(&self) -> &'static str {
        match self { Kind::Text => "text", Kind::Html => "html", Kind::Image => "image", Kind::Files => "files", Kind::Other => "other" }
    }
    pub fn from_str(s: &str) -> Kind {
        match s { "text" => Kind::Text, "html" => Kind::Html, "image" => Kind::Image, "files" => Kind::Files, _ => Kind::Other }
    }
}

/// One MIME representation of a clipboard entry.
#[derive(Debug, Clone)]
pub struct Format { pub mime: String, pub data: Vec<u8> }

/// What a backend emits when the clipboard changes (raw, unclassified).
#[derive(Debug, Clone)]
pub struct ClipEvent {
    pub formats: Vec<Format>,
    pub source_app: Option<String>,
    /// true if the source flagged it as secret (e.g. x-kde-passwordManagerHint)
    pub sensitive: bool,
}

/// A stored history item (metadata only; payloads loaded via Store::formats).
#[derive(Debug, Clone)]
pub struct ClipItem {
    pub id: i64,
    pub kind: Kind,
    pub preview: String,
    pub source_app: Option<String>,
    pub byte_size: i64,
    pub width: Option<u32>,
    pub height: Option<u32>,
    pub pinned: bool,
    pub created_at: i64,
    pub last_used_at: i64,
    pub thumb_path: Option<std::path::PathBuf>,
}
