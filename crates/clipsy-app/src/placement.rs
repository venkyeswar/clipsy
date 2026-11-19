//! Moves the popup window on X11 (GTK4 cannot position toplevels itself).
//!
//! `appearance.popup_position`: "cursor" puts the top-left corner at the mouse
//! pointer, "center" centers the popup on its monitor. The result is always kept
//! inside the monitor. Any failure is logged, and the window stays where the
//! window manager put it.

use gtk::gdk;
use gtk::prelude::*;
use x11rb::connection::Connection;
use x11rb::protocol::xproto::{ConfigureWindowAux, ConnectionExt as _};


/// Popup size used when the window has not been allocated yet, and the popup's default size
/// (see `load_saved_size` in popup.rs).
pub const DEFAULT_SIZE: (i32, i32) = (440, 560);

/// Moves `window` according to `position` ("cursor" or "center"). Other values do nothing.
pub fn place(window: &gtk::Window, position: &str) {
    if let Err(err) = try_place(window, position) {
        tracing::warn!("could not position the popup: {err:#}");
    }
}

fn try_place(window: &gtk::Window, position: &str) -> anyhow::Result<()> {
    if position != "cursor" && position != "center" {
        return Ok(());
    }
    // Not shown yet, or not running on X11: nothing to move.
    let Some(surface) = window.surface() else {
        return Ok(());
    };
    let Ok(x11_surface) = surface.clone().downcast::<gdk_x11::X11Surface>() else {
        return Ok(());
    };

    let (conn, screen_num) = x11rb::connect(None)?;
    let root = conn.setup().roots[screen_num].root;

    let pointer = conn.query_pointer(root)?.reply()?;
    let (px, py) = (i32::from(pointer.root_x), i32::from(pointer.root_y));

    let monitor = monitor_area(&surface, px, py);
    let (width, height) = match (window.width(), window.height()) {
        (w, h) if w > 0 && h > 0 => (w, h),
        _ => DEFAULT_SIZE,
    };

    let (x, y) = if position == "center" {
        (
            monitor.0 + (monitor.2 - width) / 2,
            monitor.1 + (monitor.3 - height) / 2,
        )
    } else {
        (px, py)
    };
    let (x, y) = clamp_origin(x, y, width, height, monitor);

    let win = x11_surface.xid() as u32;
    conn.configure_window(win, &ConfigureWindowAux::new().x(x).y(y))?;
    conn.flush()?;
    Ok(())
}

/// The monitor that contains the point (px, py), or the monitor of the surface
/// when no monitor contains it. Returns (x, y, width, height).
fn monitor_area(surface: &gdk::Surface, px: i32, py: i32) -> (i32, i32, i32, i32) {
    let display = surface.display();
    let monitors = display.monitors();
    let mut found = None;
    for i in 0..monitors.n_items() {
        let Some(monitor) = monitors.item(i).and_then(|m| m.downcast::<gdk::Monitor>().ok()) else {
            continue;
        };
        let g = monitor.geometry();
        if px >= g.x() && px < g.x() + g.width() && py >= g.y() && py < g.y() + g.height() {
            found = Some(g);
            break;
        }
    }
    let g = found
        .or_else(|| display.monitor_at_surface(surface).map(|m| m.geometry()))
        .unwrap_or_else(|| gdk::Rectangle::new(0, 0, 1920, 1080));
    (g.x(), g.y(), g.width(), g.height())
}

/// Keeps a window of `width` x `height` inside `area` (x, y, width, height).
/// If the window is larger than the area, its top-left corner is put on the area's corner.
fn clamp_origin(x: i32, y: i32, width: i32, height: i32, area: (i32, i32, i32, i32)) -> (i32, i32) {
    let (ax, ay, aw, ah) = area;
    let x = x.min(ax + aw - width).max(ax);
    let y = y.min(ay + ah - height).max(ay);
    (x, y)
}

#[cfg(test)]
mod tests {
    use super::clamp_origin;

    const AREA: (i32, i32, i32, i32) = (0, 0, 1920, 1080);

    #[test]
    fn keeps_position_that_fits() {
        assert_eq!(clamp_origin(100, 200, 420, 540, AREA), (100, 200));
    }

    #[test]
    fn moves_back_from_right_and_bottom_edges() {
        assert_eq!(clamp_origin(1900, 1000, 420, 540, AREA), (1500, 540));
    }

    #[test]
    fn moves_back_from_left_and_top_edges() {
        assert_eq!(clamp_origin(-50, -10, 420, 540, AREA), (0, 0));
    }

    #[test]
    fn respects_monitor_offset() {
        // Second monitor to the right of the first one.
        let area = (1920, 0, 1280, 1024);
        assert_eq!(clamp_origin(1000, 500, 420, 540, area), (1920, 484));
    }
}
