//! Clipboard format mapping: turns raw Windows clipboard data into `clipsy_core::ClipEvent`s
//! and back. Pure logic (no Win32 calls), so it compiles and is tested on every OS.

use std::io::Cursor;

use clipsy_core::{ClipEvent, Format};

/// Clipboard contents as read from Win32, before mapping to MIME types.
#[derive(Debug, Clone, Default)]
pub struct RawClip {
    /// CF_UNICODETEXT, UTF-16 code units (may end with NUL).
    pub unicode_text: Option<Vec<u16>>,
    /// Registered "PNG" format bytes.
    pub png: Option<Vec<u8>>,
    /// CF_DIB bytes (BITMAPINFO without the BMP file header).
    pub dib: Option<Vec<u8>>,
    /// CF_HTML bytes (with the "Version:/StartFragment:" header).
    pub html: Option<Vec<u8>>,
    /// CF_HDROP file paths (Windows paths such as `C:\dir\file.txt`).
    pub files: Vec<String>,
    /// ExcludeClipboardContentFromMonitorProcessing or similar secret marker.
    pub exclude_flag: bool,
    /// CanIncludeInClipboardHistory = 0 or CanUploadToCloudClipboard = 0 (Windows opt-outs).
    pub history_disallowed: bool,
    /// Executable name of the app that owns the clipboard, e.g. `chrome.exe`.
    pub source_exe: Option<String>,
}

/// Build a ClipEvent from raw clipboard data. Returns `None` when nothing usable is present.
pub fn build_event(raw: RawClip) -> Option<ClipEvent> {
    let RawClip { unicode_text, png, dib, html, files, exclude_flag, history_disallowed, source_exe } = raw;
    let mut formats = Vec::new();

    if let Some(units) = unicode_text {
        let text = String::from_utf16_lossy(&units);
        let text = text.trim_end_matches('\0');
        if !text.is_empty() {
            formats.push(Format { mime: "text/plain;charset=utf-8".into(), data: text.as_bytes().to_vec() });
        }
    }
    // A PNG from the app wins; a DIB is converted only when there is no PNG.
    let image = png.filter(|p| !p.is_empty()).or_else(|| dib.as_deref().and_then(dib_to_png));
    if let Some(data) = image {
        formats.push(Format { mime: "image/png".into(), data });
    }
    if let Some(data) = html.as_deref().and_then(cf_html_fragment) {
        formats.push(Format { mime: "text/html".into(), data });
    }
    if !files.is_empty() {
        let uri_list = files_to_uri_list(&files);
        if !uri_list.is_empty() {
            formats.push(Format { mime: "text/uri-list".into(), data: uri_list.into_bytes() });
        }
    }

    if formats.is_empty() {
        return None;
    }
    Some(ClipEvent {
        formats,
        source_app: source_exe,
        sensitive: exclude_flag || history_disallowed,
    })
}

/// Convert a CF_DIB payload into PNG bytes by prepending a BMP file header.
pub fn dib_to_png(dib: &[u8]) -> Option<Vec<u8>> {
    let bmp = dib_to_bmp(dib)?;
    let img = image::load_from_memory_with_format(&bmp, image::ImageFormat::Bmp).ok()?;
    let mut png = Vec::new();
    img.write_to(&mut Cursor::new(&mut png), image::ImageFormat::Png).ok()?;
    Some(png)
}

/// Clipboard format id of CF_DIBV5 (BITMAPV5HEADER with colour masks and colour space).
pub const CF_DIBV5: u32 = 17;

/// Convert PNG bytes into a CF_DIB payload: a 40-byte BITMAPINFOHEADER followed by bottom-up
/// 32-bit BGRA rows (BI_RGB). Returns None when the bytes are not a PNG image.
pub fn png_to_dib(png: &[u8]) -> Option<Vec<u8>> {
    let (width, height, pixels) = decode_png_bgra(png)?;
    let mut out = Vec::with_capacity(40 + pixels.len());
    out.extend_from_slice(&40u32.to_le_bytes()); // biSize
    out.extend_from_slice(&width.to_le_bytes()); // biWidth
    out.extend_from_slice(&height.to_le_bytes()); // biHeight: positive means bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
    out.extend_from_slice(&32u16.to_le_bytes()); // biBitCount
    out.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
    out.extend_from_slice(&(pixels.len() as u32).to_le_bytes()); // biSizeImage
    out.extend_from_slice(&[0u8; 16]); // resolution and colour counts (unused)
    out.extend_from_slice(&pixels);
    Some(out)
}

/// Convert PNG bytes into a CF_DIBV5 payload: a 124-byte BITMAPV5HEADER using BI_BITFIELDS with
/// red, green, blue and alpha masks, sRGB colour space, then bottom-up 32-bit BGRA rows.
/// Returns None when the bytes are not a PNG image.
pub fn png_to_dibv5(png: &[u8]) -> Option<Vec<u8>> {
    let (width, height, pixels) = decode_png_bgra(png)?;
    let mut out = Vec::with_capacity(124 + pixels.len());
    out.extend_from_slice(&124u32.to_le_bytes()); // bV5Size
    out.extend_from_slice(&width.to_le_bytes()); // bV5Width
    out.extend_from_slice(&height.to_le_bytes()); // bV5Height: positive means bottom-up
    out.extend_from_slice(&1u16.to_le_bytes()); // bV5Planes
    out.extend_from_slice(&32u16.to_le_bytes()); // bV5BitCount
    out.extend_from_slice(&3u32.to_le_bytes()); // bV5Compression = BI_BITFIELDS
    out.extend_from_slice(&(pixels.len() as u32).to_le_bytes()); // bV5SizeImage
    out.extend_from_slice(&[0u8; 8]); // bV5XPelsPerMeter, bV5YPelsPerMeter
    out.extend_from_slice(&[0u8; 8]); // bV5ClrUsed, bV5ClrImportant
    out.extend_from_slice(&0x00FF_0000u32.to_le_bytes()); // bV5RedMask
    out.extend_from_slice(&0x0000_FF00u32.to_le_bytes()); // bV5GreenMask
    out.extend_from_slice(&0x0000_00FFu32.to_le_bytes()); // bV5BlueMask
    out.extend_from_slice(&0xFF00_0000u32.to_le_bytes()); // bV5AlphaMask
    out.extend_from_slice(&0x7352_4742u32.to_le_bytes()); // bV5CSType = LCS_sRGB
    out.extend_from_slice(&[0u8; 36]); // bV5Endpoints (CIEXYZTRIPLE, unused for sRGB)
    out.extend_from_slice(&[0u8; 12]); // bV5GammaRed, bV5GammaGreen, bV5GammaBlue
    out.extend_from_slice(&4u32.to_le_bytes()); // bV5Intent = LCS_GM_IMAGES
    out.extend_from_slice(&[0u8; 12]); // bV5ProfileData, bV5ProfileSize, bV5Reserved
    debug_assert_eq!(out.len(), 124);
    out.extend_from_slice(&pixels);
    Some(out)
}

/// Decode PNG bytes to (width, height, bottom-up BGRA pixel rows) as DIB stores them.
fn decode_png_bgra(png: &[u8]) -> Option<(i32, i32, Vec<u8>)> {
    let img = image::load_from_memory_with_format(png, image::ImageFormat::Png).ok()?.into_rgba8();
    let (w, h) = img.dimensions();
    let width = i32::try_from(w).ok()?;
    let height = i32::try_from(h).ok()?;
    let mut pixels = Vec::with_capacity(w as usize * h as usize * 4);
    for row in img.rows().rev() {
        for px in row {
            let [r, g, b, a] = px.0;
            pixels.extend_from_slice(&[b, g, r, a]);
        }
    }
    Some((width, height, pixels))
}

/// Turn Windows paths into a `text/uri-list` body: one `file:///` URI per line, CRLF endings.
pub fn files_to_uri_list(paths: &[String]) -> String {
    paths
        .iter()
        .filter(|p| !p.is_empty())
        .map(|p| path_to_file_uri(p) + "\r\n")
        .collect()
}

/// Text to paste from stored formats: the `text/plain` representation, if any.
pub fn clip_text_from_formats(formats: &[Format]) -> Option<String> {
    formats
        .iter()
        .find(|f| base_mime(&f.mime) == "text/plain")
        .map(|f| String::from_utf8_lossy(&f.data).into_owned())
}

/// MIME type without parameters, lower case (`Text/Plain; charset=utf-8` -> `text/plain`).
fn base_mime(mime: &str) -> String {
    mime.split(';').next().unwrap_or("").trim().to_ascii_lowercase()
}

/// Wrap a DIB in a BITMAPFILEHEADER so that an image decoder can read it as a BMP file.
fn dib_to_bmp(dib: &[u8]) -> Option<Vec<u8>> {
    let u32_at = |off: usize| -> Option<u32> {
        dib.get(off..off + 4).map(|b| u32::from_le_bytes([b[0], b[1], b[2], b[3]]))
    };
    let header_size = u32_at(0)? as usize;
    if header_size < 12 || dib.len() < header_size {
        return None;
    }
    let bit_count = u16::from_le_bytes([*dib.get(14)?, *dib.get(15)?]) as u32;
    let (compression, clr_used) = if header_size >= 40 {
        (u32_at(16)?, u32_at(32)?)
    } else {
        // OS/2 BITMAPCOREHEADER: no compression or colors-used fields.
        (0, 0)
    };
    // Bit masks follow a plain BITMAPINFOHEADER; larger headers already contain them.
    let masks = match (header_size, compression) {
        (40, 3) => 12,
        (40, 6) => 16,
        _ => 0,
    };
    let palette_entries = if clr_used != 0 {
        clr_used as usize
    } else if bit_count <= 8 {
        1usize << bit_count
    } else {
        0
    };
    let entry_size = if header_size == 12 { 3 } else { 4 };
    let pixel_offset = 14 + header_size + masks + palette_entries * entry_size;
    let file_size = u32::try_from(14 + dib.len()).ok()?;

    let mut bmp = Vec::with_capacity(14 + dib.len());
    bmp.extend_from_slice(b"BM");
    bmp.extend_from_slice(&file_size.to_le_bytes());
    bmp.extend_from_slice(&0u32.to_le_bytes());
    bmp.extend_from_slice(&u32::try_from(pixel_offset).ok()?.to_le_bytes());
    bmp.extend_from_slice(dib);
    Some(bmp)
}

/// The HTML between the StartFragment and EndFragment byte offsets of a CF_HTML payload.
fn cf_html_fragment(data: &[u8]) -> Option<Vec<u8>> {
    let start = header_offset(data, b"StartFragment:")?;
    let end = header_offset(data, b"EndFragment:")?;
    if start >= end || end > data.len() {
        return None;
    }
    let fragment = &data[start..end];
    (!fragment.is_empty()).then(|| fragment.to_vec())
}

/// Parse the decimal value after `key` in the CF_HTML header.
fn header_offset(data: &[u8], key: &[u8]) -> Option<usize> {
    let pos = data.windows(key.len()).position(|w| w == key)? + key.len();
    let digits: String = data[pos..]
        .iter()
        .take_while(|b| b.is_ascii_digit())
        .map(|&b| b as char)
        .collect();
    digits.parse().ok()
}

/// `C:\My Docs\a b.pdf` -> `file:///C:/My%20Docs/a%20b.pdf`. UNC paths become `file://server/...`.
fn path_to_file_uri(path: &str) -> String {
    let p = path.replace('\\', "/");
    let prefix = if p.starts_with("//") {
        "file:"
    } else if p.starts_with('/') {
        "file://"
    } else {
        "file:///"
    };
    let mut out = String::from(prefix);
    for &b in p.as_bytes() {
        if b.is_ascii_alphanumeric() || b"-._~/:".contains(&b) {
            out.push(b as char);
        } else {
            out.push_str(&format!("%{b:02X}"));
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    fn find<'a>(ev: &'a ClipEvent, mime: &str) -> Option<&'a Format> {
        ev.formats.iter().find(|f| f.mime == mime)
    }

    /// A 2x2 24-bit BI_RGB DIB (BITMAPINFOHEADER, bottom-up). Each row is 6 bytes of pixels,
    /// padded to 8.
    fn dib_2x2() -> Vec<u8> {
        let mut d = Vec::new();
        d.extend_from_slice(&40u32.to_le_bytes()); // biSize
        d.extend_from_slice(&2i32.to_le_bytes()); // biWidth
        d.extend_from_slice(&2i32.to_le_bytes()); // biHeight
        d.extend_from_slice(&1u16.to_le_bytes()); // biPlanes
        d.extend_from_slice(&24u16.to_le_bytes()); // biBitCount
        d.extend_from_slice(&0u32.to_le_bytes()); // biCompression = BI_RGB
        d.extend_from_slice(&16u32.to_le_bytes()); // biSizeImage
        d.extend_from_slice(&0i32.to_le_bytes()); // biXPelsPerMeter
        d.extend_from_slice(&0i32.to_le_bytes()); // biYPelsPerMeter
        d.extend_from_slice(&0u32.to_le_bytes()); // biClrUsed
        d.extend_from_slice(&0u32.to_le_bytes()); // biClrImportant
        for _ in 0..2 {
            d.extend_from_slice(&[255, 0, 0, 0, 255, 0, 0, 0]); // blue, green, pad
        }
        d
    }

    #[test]
    fn text_maps_to_plain_mime_and_strips_nul() {
        let raw = RawClip {
            unicode_text: Some("héllo\u{0}\u{0}".encode_utf16().collect()),
            ..Default::default()
        };
        let ev = build_event(raw).unwrap();
        let f = find(&ev, "text/plain;charset=utf-8").unwrap();
        assert_eq!(f.data, "héllo".as_bytes());
        assert!(!ev.sensitive);
    }

    #[test]
    fn png_passes_through() {
        let png = vec![0x89, b'P', b'N', b'G', 1, 2, 3];
        let raw = RawClip { png: Some(png.clone()), ..Default::default() };
        let ev = build_event(raw).unwrap();
        assert_eq!(find(&ev, "image/png").unwrap().data, png);
    }

    #[test]
    fn dib_converts_to_png() {
        let png = dib_to_png(&dib_2x2()).unwrap();
        assert_eq!(&png[..8], &[0x89, b'P', b'N', b'G', 0x0d, 0x0a, 0x1a, 0x0a]);
        let img = image::load_from_memory_with_format(&png, image::ImageFormat::Png).unwrap();
        assert_eq!((img.width(), img.height()), (2, 2));

        let raw = RawClip { dib: Some(dib_2x2()), ..Default::default() };
        let ev = build_event(raw).unwrap();
        assert_eq!(find(&ev, "image/png").unwrap().data, png);
    }

    #[test]
    fn dib_garbage_yields_none() {
        assert!(dib_to_png(&[1, 2, 3]).is_none());
        let raw = RawClip { dib: Some(vec![1, 2, 3]), ..Default::default() };
        assert!(build_event(raw).is_none());
    }

    #[test]
    fn html_fragment_extracted() {
        // CF_HTML: a header with byte offsets (fixed width, so its length is stable), then the
        // document. The fragment is the part between the StartFragment and EndFragment offsets.
        let body = "<html><body><!--StartFragment--><b>bold</b> word<!--EndFragment--></body></html>";
        let header = |s: usize, e: usize| {
            format!("Version:0.9\r\nStartHTML:00000000\r\nEndHTML:00000000\r\nStartFragment:{s:08}\r\nEndFragment:{e:08}\r\n")
        };
        let hlen = header(0, 0).len();
        let start = hlen + body.find("<b>").unwrap();
        let end = hlen + body.find("<!--EndFragment").unwrap();
        let doc = format!("{}{body}", header(start, end));
        assert_eq!(&doc[start..end], "<b>bold</b> word");

        let raw = RawClip { html: Some(doc.into_bytes()), ..Default::default() };
        let ev = build_event(raw).unwrap();
        assert_eq!(find(&ev, "text/html").unwrap().data, b"<b>bold</b> word");
    }

    #[test]
    fn files_become_uri_list_with_percent_encoding() {
        let paths = vec![
            r"C:\Users\me\My Docs\report 1.pdf".to_string(),
            r"D:\plain.txt".to_string(),
        ];
        assert_eq!(
            files_to_uri_list(&paths),
            "file:///C:/Users/me/My%20Docs/report%201.pdf\r\nfile:///D:/plain.txt\r\n"
        );
        let raw = RawClip { files: paths, ..Default::default() };
        let ev = build_event(raw).unwrap();
        let f = find(&ev, "text/uri-list").unwrap();
        assert_eq!(
            f.data,
            b"file:///C:/Users/me/My%20Docs/report%201.pdf\r\nfile:///D:/plain.txt\r\n"
        );
    }

    #[test]
    fn files_unicode_and_unc_paths_are_encoded() {
        let paths = vec![
            r"C:\Notes\café.txt".to_string(),
            r"\\server\share\a#b.txt".to_string(),
        ];
        assert_eq!(
            files_to_uri_list(&paths),
            "file:///C:/Notes/caf%C3%A9.txt\r\nfile://server/share/a%23b.txt\r\n"
        );
    }

    #[test]
    fn sensitive_flags_set_sensitive() {
        let text = || RawClip { unicode_text: Some(vec![b'x' as u16]), ..Default::default() };
        let ev = build_event(RawClip { exclude_flag: true, ..text() }).unwrap();
        assert!(ev.sensitive);
        let ev = build_event(RawClip { history_disallowed: true, ..text() }).unwrap();
        assert!(ev.sensitive);
        let ev = build_event(text()).unwrap();
        assert!(!ev.sensitive);
    }

    #[test]
    fn source_exe_becomes_source_app() {
        let raw = RawClip {
            unicode_text: Some(vec![b'x' as u16]),
            source_exe: Some("chrome.exe".into()),
            ..Default::default()
        };
        assert_eq!(build_event(raw).unwrap().source_app.as_deref(), Some("chrome.exe"));
    }

    #[test]
    fn empty_clip_returns_none() {
        assert!(build_event(RawClip::default()).is_none());
        let blank = RawClip {
            unicode_text: Some(vec![0, 0]),
            files: Vec::new(),
            ..Default::default()
        };
        assert!(build_event(blank).is_none());
    }

    #[test]
    fn clip_text_from_formats_reads_plain_text() {
        let formats = vec![
            Format { mime: "text/html".into(), data: b"<b>x</b>".to_vec() },
            Format { mime: "text/plain;charset=utf-8".into(), data: "hi".as_bytes().to_vec() },
        ];
        assert_eq!(clip_text_from_formats(&formats).as_deref(), Some("hi"));
        assert_eq!(clip_text_from_formats(&formats[..1]), None);
    }

    /// A 2x2 RGBA PNG: top row red, green; bottom row blue, fully transparent.
    fn png_2x2() -> Vec<u8> {
        let mut img = image::RgbaImage::new(2, 2);
        img.put_pixel(0, 0, image::Rgba([255, 0, 0, 255]));
        img.put_pixel(1, 0, image::Rgba([0, 255, 0, 255]));
        img.put_pixel(0, 1, image::Rgba([0, 0, 255, 255]));
        img.put_pixel(1, 1, image::Rgba([0, 0, 0, 0]));
        let mut out = Vec::new();
        img.write_to(&mut Cursor::new(&mut out), image::ImageFormat::Png).unwrap();
        out
    }

    fn u32_le_at(data: &[u8], off: usize) -> u32 {
        u32::from_le_bytes([data[off], data[off + 1], data[off + 2], data[off + 3]])
    }

    #[test]
    fn png_to_dib_has_info_header_and_pixel_bytes() {
        let dib = png_to_dib(&png_2x2()).unwrap();
        assert_eq!(u32_le_at(&dib, 0), 40, "biSize");
        assert_eq!(u32_le_at(&dib, 4), 2, "biWidth");
        assert_eq!(u32_le_at(&dib, 8), 2, "biHeight");
        assert_eq!(u16::from_le_bytes([dib[14], dib[15]]), 32, "biBitCount");
        assert_eq!(dib.len(), 40 + 16);
        // Bottom-up: the first stored row is the image's bottom row, blue then transparent, as BGRA.
        assert_eq!(&dib[40..44], &[255, 0, 0, 255]);
        assert_eq!(&dib[44..48], &[0, 0, 0, 0]);
        // The top row (red, green) comes last.
        assert_eq!(&dib[48..52], &[0, 0, 255, 255]);
    }

    #[test]
    fn png_to_dibv5_header_size_is_124() {
        let dib = png_to_dibv5(&png_2x2()).unwrap();
        assert_eq!(u32_le_at(&dib, 0), 124, "bV5Size");
        assert_eq!(u32_le_at(&dib, 16), 3, "BI_BITFIELDS");
        assert_eq!(u32_le_at(&dib, 40), 0x00FF_0000, "red mask");
        assert_eq!(u32_le_at(&dib, 44), 0x0000_FF00, "green mask");
        assert_eq!(u32_le_at(&dib, 48), 0x0000_00FF, "blue mask");
        assert_eq!(u32_le_at(&dib, 52), 0xFF00_0000, "alpha mask");
        assert_eq!(u32_le_at(&dib, 56), 0x7352_4742, "LCS_sRGB");
        assert_eq!(dib.len(), 124 + 16);
        assert_eq!(&dib[124..128], &[255, 0, 0, 255]);
    }

    #[test]
    fn png_to_dib_garbage_yields_none() {
        assert!(png_to_dib(&[1, 2, 3]).is_none());
        assert!(png_to_dibv5(&[1, 2, 3]).is_none());
    }
}
