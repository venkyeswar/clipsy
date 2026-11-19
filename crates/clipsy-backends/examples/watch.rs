//! Manual test for the X11 backend.
//!
//! Usage: cargo run -p clipsy-backends --example watch -- [ACCEL]
//! Prints every clipboard change (mimes + sizes) and, if ACCEL is given (e.g. "<Ctrl><Alt>j"),
//! registers it as TogglePopup and prints presses.

use clipsy_backends::HotkeyAction;

#[cfg(not(target_os = "linux"))]
fn main() {
    eprintln!("the watch example only runs on Linux");
}

#[cfg(target_os = "linux")]
fn main() -> anyhow::Result<()> {
    use std::sync::mpsc::channel;
    use std::thread;
    use std::time::Duration;

    let accel = std::env::args().nth(1);
    let mut backend = clipsy_backends::detect()?;
    println!("backend: {}", backend.name());

    if let Some(accel) = &accel {
        backend.register_hotkey(HotkeyAction::TogglePopup, accel)?;
        println!("hotkey registered: {accel}");
    }

    let (clip_tx, clip_rx) = channel();
    let (hk_tx, hk_rx) = channel();
    backend.start(clip_tx, hk_tx)?;
    println!("watching clipboard (Ctrl+C to quit)");

    loop {
        while let Ok(ev) = clip_rx.try_recv() {
            println!("--- clip: source={:?} sensitive={}", ev.source_app, ev.sensitive);
            for f in &ev.formats {
                println!("    {:<32} {} bytes", f.mime, f.data.len());
            }
        }
        while let Ok(action) = hk_rx.try_recv() {
            println!("hotkey fired: {action:?}");
        }
        thread::sleep(Duration::from_millis(30));
    }
}
