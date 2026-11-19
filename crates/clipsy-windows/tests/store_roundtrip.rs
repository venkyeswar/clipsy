//! Events built by `formats::build_event` must be accepted and classified by `clipsy_core::Store`.

use clipsy_core::{Config, Filter, Kind, Store};
use clipsy_windows::formats::{build_event, RawClip};
use tempfile::tempdir;

fn store() -> (tempfile::TempDir, Store) {
    let dir = tempdir().unwrap();
    let store = Store::open(dir.path()).unwrap();
    (dir, store)
}

#[test]
fn text_event_is_stored_and_listed() {
    let (_dir, mut store) = store();
    let raw = RawClip {
        unicode_text: Some("hello windows".encode_utf16().collect()),
        source_exe: Some("notepad.exe".into()),
        ..Default::default()
    };
    let ev = build_event(raw).unwrap();
    let id = store.insert_event(&ev, &Config::default()).unwrap().unwrap();

    let items = store.list("", Filter::Text, 10).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].id, id);
    assert_eq!(items[0].kind, Kind::Text);
    assert_eq!(items[0].preview, "hello windows");
    assert_eq!(items[0].source_app.as_deref(), Some("notepad.exe"));
    assert_eq!(store.plain_text(id).unwrap().as_deref(), Some("hello windows"));
}

#[test]
fn file_event_is_stored_as_files() {
    let (_dir, mut store) = store();
    let raw = RawClip {
        files: vec![r"C:\Users\me\My Docs\report 1.pdf".into(), r"D:\notes.txt".into()],
        ..Default::default()
    };
    let ev = build_event(raw).unwrap();
    store.insert_event(&ev, &Config::default()).unwrap().unwrap();

    let items = store.list("", Filter::Files, 10).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, Kind::Files);
    assert_eq!(items[0].preview, "report 1.pdf, notes.txt (2 files)");
}

#[test]
fn png_event_is_stored_as_image() {
    let (_dir, mut store) = store();
    let mut png = Vec::new();
    {
        let img = image::RgbImage::from_pixel(3, 2, image::Rgb([10, 20, 30]));
        img.write_to(&mut std::io::Cursor::new(&mut png), image::ImageFormat::Png).unwrap();
    }
    let raw = RawClip { png: Some(png), ..Default::default() };
    let ev = build_event(raw).unwrap();
    store.insert_event(&ev, &Config::default()).unwrap().unwrap();

    let items = store.list("", Filter::Images, 10).unwrap();
    assert_eq!(items.len(), 1);
    assert_eq!(items[0].kind, Kind::Image);
    assert_eq!((items[0].width, items[0].height), (Some(3), Some(2)));
}

#[test]
fn sensitive_event_is_skipped_by_default() {
    let (_dir, mut store) = store();
    let raw = RawClip {
        unicode_text: Some("secret".encode_utf16().collect()),
        exclude_flag: true,
        ..Default::default()
    };
    let ev = build_event(raw).unwrap();
    assert!(store.insert_event(&ev, &Config::default()).unwrap().is_none());
    assert!(store.list("", Filter::All, 10).unwrap().is_empty());
}
