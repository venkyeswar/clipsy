//! The Clipsy logo used for the tray icon. Pure pixel code, so it builds and tests on any OS.

use anyhow::Context;
use image::imageops::FilterType;

/// Size of the tray icon in pixels (Windows scales it for the tray).
pub const TRAY_ICON_SIZE: u32 = 32;

/// The 64 px logo, embedded in the binary. Also used by the Slint popup and settings headers.
pub const LOGO_PNG: &[u8] = include_bytes!("../assets/clipsy-64.png");

/// Decodes the embedded logo and returns the RGBA pixels of the tray icon,
/// `TRAY_ICON_SIZE` x `TRAY_ICON_SIZE`.
pub fn tray_icon_rgba() -> anyhow::Result<Vec<u8>> {
    let logo = image::load_from_memory_with_format(LOGO_PNG, image::ImageFormat::Png)
        .context("could not decode the embedded Clipsy logo")?
        .into_rgba8();
    let small =
        image::imageops::resize(&logo, TRAY_ICON_SIZE, TRAY_ICON_SIZE, FilterType::Lanczos3);
    Ok(small.into_raw())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn tray_icon_rgba_is_32x32_and_not_flat() {
        let rgba = tray_icon_rgba().expect("logo should decode");
        assert_eq!(rgba.len(), (TRAY_ICON_SIZE * TRAY_ICON_SIZE * 4) as usize);
        let colours: std::collections::HashSet<[u8; 3]> = rgba
            .chunks(4)
            .filter(|p| p[3] > 0)
            .map(|p| [p[0], p[1], p[2]])
            .collect();
        assert!(colours.len() > 1, "tray icon is a single flat colour");
    }
}
