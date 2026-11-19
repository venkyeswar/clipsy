//! The clipboard history popup (Windows only).
//!
//! `Popup` owns the Slint window. It turns the `PopupState` and the store's rows into the Slint
//! model, and handles key presses, clicks, paste and hide-on-focus-loss. Everything runs on the
//! UI thread, so the shared handles are plain `Rc` / `RefCell`.

use std::cell::{Cell, RefCell};
use std::collections::HashMap;
use std::ffi::c_void;
use std::path::{Path, PathBuf};
use std::rc::{Rc, Weak};
use std::time::Duration;

use anyhow::Result;
use clipsy_backends::{Backend, PasteMode};
use clipsy_core::{ClipItem, Config, Filter, Format, Kind, Store};
use slint::{ComponentHandle, LogicalSize, ModelRc, PhysicalPosition, SharedString, Timer, TimerMode, VecModel};
use windows::core::w;
use windows::Win32::Foundation::{HWND, POINT, RECT};
use windows::Win32::Graphics::Gdi::{GetMonitorInfoW, MonitorFromPoint, MONITORINFO, MONITOR_DEFAULTTONEAREST};
use windows::Win32::UI::WindowsAndMessaging::{
    FindWindowW, GetCursorPos, GetForegroundWindow, GetWindowLongPtrW, SetForegroundWindow, SetWindowLongPtrW,
    ShowWindow, GWL_EXSTYLE, SW_HIDE, SW_SHOWNA, WS_EX_APPWINDOW, WS_EX_TOOLWINDOW,
};
use winreg::enums::HKEY_CURRENT_USER;
use winreg::RegKey;

use crate::backend::WindowsBackend;
use crate::popup_state::{clamp_to_area, PopupState};
use crate::ui::{ClipPopup, RowData};

/// Logical size of the popup. Keep in step with `width` / `height` in ui/popup.slint.
const POPUP_WIDTH: f32 = 420.0;
const POPUP_HEIGHT: f32 = 520.0;
/// How often the watcher checks for focus loss while the popup is open.
const WATCH_EVERY: Duration = Duration::from_millis(150);
/// Delay before pasting, so the popup has gone and focus is back in the target window.
const PASTE_DELAY: Duration = Duration::from_millis(50);
/// Most rows the popup shows.
const ROW_LIMIT: usize = 200;
/// Longest preview shown on one row, in characters.
const PREVIEW_CHARS: usize = 160;
/// Gap between attempts to find the popup's window handle right after it is first shown.
const RETRY_EVERY: Duration = Duration::from_millis(10);
/// Attempts to find the popup's window handle right after it is first shown.
const RETRIES: u32 = 10;
/// Decoded thumbnails kept in memory. The cache is cleared when it grows past this.
const THUMB_CACHE_MAX: usize = 400;

/// The clipboard history popup. Cheap to call from the UI thread; all methods take `&self`.
pub struct Popup {
    inner: Rc<Inner>,
}

struct Inner {
    /// Weak handle to this `Inner`, for the timer and Slint callbacks (no reference cycle).
    me: Weak<Inner>,
    window: ClipPopup,
    state: RefCell<PopupState>,
    store: Rc<RefCell<Store>>,
    config: Rc<RefCell<Config>>,
    backend: Rc<WindowsBackend>,
    /// Runs `watch_tick` every `WATCH_EVERY` while the popup is open.
    watch: Timer,
    /// Popup window handle as an integer (0 = not found yet).
    hwnd: Cell<isize>,
    /// True once the popup has been the foreground window since it was shown.
    focused: Cell<bool>,
    /// Set on each show; the popup is then made foreground once its handle is known.
    focus_pending: Cell<bool>,
    /// True once the popup was hidden from the taskbar. The style stays on the window.
    taskbar_hidden: Cell<bool>,
    /// Decoded thumbnails by file path.
    thumbs: RefCell<HashMap<PathBuf, slint::Image>>,
}

impl Popup {
    /// Create the popup window. The backend must already be started and its hotkeys registered.
    ///
    /// Panics if Slint cannot create a window, which means the GUI cannot run at all.
    pub fn new(store: Rc<RefCell<Store>>, config: Rc<RefCell<Config>>, backend: Rc<WindowsBackend>) -> Self {
        let window = ClipPopup::new().unwrap_or_else(|e| {
            tracing::error!("could not create the popup window: {e}");
            panic!("could not create the popup window: {e}");
        });
        window.window().set_size(LogicalSize::new(POPUP_WIDTH, POPUP_HEIGHT));
        let inner = Rc::new_cyclic(|me| Inner {
            me: me.clone(),
            window,
            state: RefCell::new(PopupState::new()),
            store,
            config,
            backend,
            watch: Timer::default(),
            hwnd: Cell::new(0),
            focused: Cell::new(false),
            focus_pending: Cell::new(false),
            taskbar_hidden: Cell::new(false),
            thumbs: RefCell::new(HashMap::new()),
        });
        inner.connect();
        Self { inner }
    }

    /// Show the popup if it is hidden, hide it if it is shown.
    pub fn toggle(&self) {
        if self.inner.window.window().is_visible() {
            self.inner.hide();
        } else {
            self.inner.show();
        }
    }

    /// Re-query the store and redraw the rows (for example after a new clip was captured).
    pub fn refresh(&self) {
        self.inner.refresh();
    }
}

/// Run `f` on the popup state if the popup still exists.
fn with_inner(weak: &Weak<Inner>, f: impl FnOnce(&Inner)) {
    if let Some(inner) = weak.upgrade() {
        f(&inner);
    }
}

impl Inner {
    /// Route the window's callbacks to this `Inner`.
    fn connect(&self) {
        let me = &self.me;

        let w = me.clone();
        self.window.on_query_edited(move |text: SharedString| {
            with_inner(&w, |i| i.set_query(text.as_str()));
        });

        let w = me.clone();
        self.window.on_filter_picked(move |index: i32| {
            with_inner(&w, |i| i.set_filter(index));
        });

        let w = me.clone();
        self.window.on_move_selection(move |delta: i32| {
            with_inner(&w, |i| i.move_selection(delta));
        });

        let w = me.clone();
        self.window.on_activate(move |index: i32, plain: bool| {
            if let Ok(index) = usize::try_from(index) {
                with_inner(&w, |i| i.activate(index, plain));
            }
        });

        let w = me.clone();
        self.window.on_pin_toggled(move |index: i32| {
            if let Ok(index) = usize::try_from(index) {
                with_inner(&w, |i| i.toggle_pin(index));
            }
        });

        let w = me.clone();
        self.window.on_hide_requested(move || {
            with_inner(&w, |i| i.hide());
        });
    }

    /// Show the popup at its position with fresh contents.
    fn show(&self) {
        // Must run before the popup is shown, while the previous window is still focused.
        self.backend.remember_active_window();

        let (theme, position, compact) = {
            let config = self.config.borrow();
            (
                config.appearance.theme.clone(),
                config.appearance.popup_position.clone(),
                config.appearance.compact,
            )
        };
        self.backend.popup_will_show(&position);

        // Each opening starts with an empty search on the All tab.
        {
            let mut state = self.state.borrow_mut();
            state.query.clear();
            state.set_filter(Filter::All);
        }
        self.window.set_query(SharedString::new());
        self.window.set_filter_index(0);
        self.window.set_dark(use_dark(&theme));
        self.window.set_compact(compact);
        self.refresh();

        let centered = position == "center";
        self.place(centered);
        if let Err(e) = self.window.show() {
            tracing::warn!("could not show the popup: {e}");
            return;
        }
        // The scale factor and size are final only once the window exists, so place it again.
        self.place(centered);
        self.focus_pending.set(true);
        self.settle(RETRIES);

        self.focused.set(false);
        let me = self.me.clone();
        self.watch.start(TimerMode::Repeated, WATCH_EVERY, move || {
            with_inner(&me, |i| i.watch_tick());
        });
    }

    fn hide(&self) {
        self.watch.stop();
        if let Err(e) = self.window.hide() {
            tracing::warn!("could not hide the popup: {e}");
        }
    }

    /// Re-query the store with the current search and filter, and redraw the rows.
    fn refresh(&self) {
        let found = {
            let state = self.state.borrow();
            self.store.borrow().list(&state.query, state.filter, ROW_LIMIT)
        };
        match found {
            Ok(items) => self.state.borrow_mut().set_items(items),
            Err(e) => {
                // Drop the old items too, so a stale row id cannot be activated.
                tracing::warn!("could not load clipboard history: {e:#}");
                self.state.borrow_mut().set_items(Vec::new());
            }
        }
        self.push_rows();
    }

    /// Copy the state's items into the Slint window. Setting the rows resets the list's scroll
    /// position, so only the refresh path calls this; moving the selection does not.
    fn push_rows(&self) {
        let show_source = self.config.borrow().appearance.show_source_app;
        let rows: Vec<RowData> = self
            .state
            .borrow()
            .items
            .iter()
            .map(|item| self.row(item, show_source))
            .collect();
        self.window.set_rows(ModelRc::new(VecModel::from(rows)));
        self.sync_selection();
    }

    /// Copy only the selection into the Slint window. Slint scrolls to keep it visible.
    fn sync_selection(&self) {
        let selected = self.state.borrow().selected;
        self.window.set_selected(i32::try_from(selected).unwrap_or(0));
    }

    fn row(&self, item: &ClipItem, show_source: bool) -> RowData {
        let thumb = item.thumb_path.as_deref().and_then(|path| self.thumbnail(path));
        to_row(item, show_source, thumb)
    }

    /// Decoded thumbnail for `path`. Cached, so refreshes and key presses do not decode it again.
    fn thumbnail(&self, path: &Path) -> Option<slint::Image> {
        let mut cache = self.thumbs.borrow_mut();
        if let Some(image) = cache.get(path) {
            return Some(image.clone());
        }
        let image = slint::Image::load_from_path(path).ok()?;
        if cache.len() >= THUMB_CACHE_MAX {
            cache.clear();
        }
        cache.insert(path.to_path_buf(), image.clone());
        Some(image)
    }

    /// Hide the popup from the taskbar and make it foreground. Taskbar style is applied once. If
    /// the handle is not found yet, retry shortly; the watch tick also calls this until it works.
    fn settle(&self, tries_left: u32) {
        if !self.window.window().is_visible() {
            return;
        }
        let Some(hwnd) = self.popup_hwnd() else {
            if tries_left > 0 {
                let me = self.me.clone();
                Timer::single_shot(RETRY_EVERY, move || {
                    with_inner(&me, |i| i.settle(tries_left - 1));
                });
            } else {
                tracing::debug!("the popup window was not found yet");
            }
            return;
        };
        if !self.taskbar_hidden.get() {
            hide_from_taskbar(hwnd);
            self.taskbar_hidden.set(true);
        }
        if self.focus_pending.replace(false) {
            // SAFETY: `hwnd` is the popup's own window. SetForegroundWindow only asks Windows to
            // activate it; a refusal is harmless, so the result is ignored.
            let _ = unsafe { SetForegroundWindow(hwnd) };
        }
    }

    fn set_query(&self, text: &str) {
        {
            let mut state = self.state.borrow_mut();
            state.query = text.to_owned();
            state.selected = 0;
        }
        self.refresh();
    }

    /// Change the tab. The search text is kept.
    fn set_filter(&self, index: i32) {
        let filter = filter_from_index(index);
        self.state.borrow_mut().set_filter(filter);
        self.window.set_filter_index(filter_index(filter));
        self.refresh();
    }

    fn move_selection(&self, delta: i32) {
        self.state.borrow_mut().move_selection(delta);
        self.sync_selection();
    }

    /// Put the item at `index` on the clipboard, hide the popup and paste it into the window
    /// that was active before the popup opened.
    fn activate(&self, index: usize, plain: bool) {
        let Some(item) = self.state.borrow().items.get(index).cloned() else {
            return;
        };
        let formats = match self.formats_for(item.id, plain) {
            Ok(formats) => formats,
            Err(e) => {
                tracing::warn!("could not read clipboard item {}: {e:#}", item.id);
                return;
            }
        };
        // Copy first. If that fails, the popup stays open and nothing is pasted.
        if let Err(e) = self.backend.set_clipboard(&formats) {
            tracing::warn!("could not copy the item to the clipboard, so nothing was pasted: {e:#}");
            return;
        }
        self.hide();

        let (paste_on_select, terminal_apps) = {
            let config = self.config.borrow();
            (config.general.paste_on_select, config.general.terminal_apps.clone())
        };
        if paste_on_select {
            self.backend.set_terminal_apps(&terminal_apps);
            let backend = Rc::clone(&self.backend);
            Timer::single_shot(PASTE_DELAY, move || {
                if let Err(e) = backend.paste(PasteMode::Normal) {
                    tracing::warn!("paste failed: {e:#}");
                }
            });
        }

        if let Err(e) = self.store.borrow().touch(item.id) {
            tracing::warn!("could not update the item's last use: {e:#}");
        }
    }

    /// Formats to put on the clipboard. Plain mode uses only the text when the item has any
    /// (an image has none, so it is copied in full).
    fn formats_for(&self, id: i64, plain: bool) -> Result<Vec<Format>> {
        let store = self.store.borrow();
        if plain {
            if let Some(text) = store.plain_text(id)? {
                return Ok(vec![Format { mime: "text/plain;charset=utf-8".into(), data: text.into_bytes() }]);
            }
        }
        store.formats(id)
    }

    fn toggle_pin(&self, index: usize) {
        let Some(item) = self.state.borrow().items.get(index).cloned() else {
            return;
        };
        if let Err(e) = self.store.borrow().set_pinned(item.id, !item.pinned) {
            tracing::warn!("could not change the pin: {e:#}");
            return;
        }
        self.refresh();
    }

    /// Position the popup: at the cursor (clamped into the monitor's work area), or centred.
    ///
    /// The size is `POPUP_*` logical pixels times the window's scale factor. The code assumes
    /// per-monitor DPI awareness, which winit sets up, so physical pixels from Win32 match. The
    /// scale is only right once the window exists, so `show` calls this again after showing.
    fn place(&self, centered: bool) {
        let scale = self.window.window().scale_factor();
        let size = ((POPUP_WIDTH * scale).round() as i32, (POPUP_HEIGHT * scale).round() as i32);

        let mut cursor = POINT::default();
        // SAFETY: GetCursorPos writes one POINT through the pointer, and `cursor` is a live local.
        let have_cursor = unsafe { GetCursorPos(&mut cursor) }.is_ok();

        // SAFETY: MonitorFromPoint takes the POINT by value and returns a monitor handle, or null.
        let monitor = unsafe { MonitorFromPoint(cursor, MONITOR_DEFAULTTONEAREST) };
        let mut info = MONITORINFO {
            cbSize: std::mem::size_of::<MONITORINFO>() as u32,
            rcMonitor: RECT::default(),
            rcWork: RECT::default(),
            dwFlags: 0,
        };
        // SAFETY: `info` is a valid MONITORINFO with cbSize set to its size, as the call requires.
        let found = unsafe { GetMonitorInfoW(monitor, &mut info) }.as_bool();
        if !found {
            // Without a work area, leave the position to Slint's default.
            return;
        }
        let work = info.rcWork;
        let area = (work.left, work.top, work.right, work.bottom);

        let wanted = if centered || !have_cursor {
            (area.0 + (area.2 - area.0 - size.0) / 2, area.1 + (area.3 - area.1 - size.1) / 2)
        } else {
            (cursor.x, cursor.y)
        };
        let (x, y) = clamp_to_area(wanted, size, area);
        self.window.window().set_position(PhysicalPosition::new(x, y));
    }

    /// Hide the popup when it loses focus. Runs every `WATCH_EVERY` while shown.
    ///
    /// Only a loss after the popup held focus counts. If Windows never gave it focus, it stays
    /// open, and the shortcut still closes it.
    fn watch_tick(&self) {
        if !self.window.window().is_visible() {
            self.watch.stop();
            return;
        }
        // Finish the taskbar style if the first attempts in `show` did not find the window.
        if !self.taskbar_hidden.get() {
            self.settle(0);
        }
        // Without a handle there is nothing to compare against, so do not hide.
        let Some(hwnd) = self.popup_hwnd() else {
            return;
        };
        // SAFETY: GetForegroundWindow takes no arguments and only reads which window is active.
        let foreground = unsafe { GetForegroundWindow() };
        if foreground == hwnd {
            self.focused.set(true);
        } else if self.focused.get() {
            self.hide();
        }
    }

    /// The popup's window handle. Found by title on first use, then cached.
    fn popup_hwnd(&self) -> Option<HWND> {
        let cached = self.hwnd.get();
        if cached != 0 {
            return Some(HWND(cached as *mut c_void));
        }
        // The window is found by its title, which must match `title` in ui/popup.slint.
        // SAFETY: FindWindowW with a null class and a constant title only searches the window
        // list. The returned handle is used only for further Win32 calls and is never freed.
        let found = unsafe { FindWindowW(windows::core::PCWSTR::null(), w!("Clipsy popup")) }.ok()?;
        if found.0.is_null() {
            return None;
        }
        self.hwnd.set(found.0 as isize);
        Some(found)
    }
}

/// Keep the popup off the taskbar: add WS_EX_TOOLWINDOW and remove WS_EX_APPWINDOW.
fn hide_from_taskbar(hwnd: HWND) {
    // SAFETY: `hwnd` is the popup's own window. GetWindowLongPtrW only reads its extended style,
    // and the Set call writes that same style back with the two flags changed.
    unsafe {
        let style = GetWindowLongPtrW(hwnd, GWL_EXSTYLE);
        let style = (style | WS_EX_TOOLWINDOW.0 as isize) & !(WS_EX_APPWINDOW.0 as isize);
        SetWindowLongPtrW(hwnd, GWL_EXSTYLE, style);
    }
    // Windows keeps a visible window's taskbar button until the window is hidden and shown again.
    // SAFETY: ShowWindow only changes the visibility of the popup's own window. SW_SHOWNA shows it
    // without activating it; the caller requests focus separately.
    unsafe {
        let _ = ShowWindow(hwnd, SW_HIDE);
        let _ = ShowWindow(hwnd, SW_SHOWNA);
    }
}

/// Dark or light colours. `theme` is "dark" or "light"; anything else follows Windows.
fn use_dark(theme: &str) -> bool {
    match theme {
        "dark" => true,
        "light" => false,
        _ => system_uses_dark(),
    }
}

/// True when Windows is set to dark app mode (`AppsUseLightTheme` = 0). Missing means light.
fn system_uses_dark() -> bool {
    RegKey::predef(HKEY_CURRENT_USER)
        .open_subkey(r"Software\Microsoft\Windows\CurrentVersion\Themes\Personalize")
        .and_then(|key| key.get_value::<u32, _>("AppsUseLightTheme"))
        .map(|light| light == 0)
        .unwrap_or(false)
}

/// Tab index as sent by the Slint window: 0 All, 1 Text, 2 Images, 3 Files, 4 Pinned.
fn filter_from_index(index: i32) -> Filter {
    match index {
        1 => Filter::Text,
        2 => Filter::Images,
        3 => Filter::Files,
        4 => Filter::Pinned,
        _ => Filter::All,
    }
}

fn filter_index(filter: Filter) -> i32 {
    match filter {
        Filter::All => 0,
        Filter::Text => 1,
        Filter::Images => 2,
        Filter::Files => 3,
        Filter::Pinned => 4,
    }
}

/// Build one Slint row from a store item and its already-decoded thumbnail, if any.
fn to_row(item: &ClipItem, show_source: bool, thumb: Option<slint::Image>) -> RowData {
    RowData {
        preview: row_text(item).into(),
        kind: item.kind.as_str().into(),
        pinned: item.pinned,
        source: if show_source { item.source_app.clone().unwrap_or_default() } else { String::new() }.into(),
        has_thumb: thumb.is_some(),
        thumb: thumb.unwrap_or_default(),
    }
}

/// One-line label for a row: the preview with whitespace collapsed and cut short. Items without
/// text (images, files) get a label describing them.
fn row_text(item: &ClipItem) -> String {
    let flat = item.preview.split_whitespace().collect::<Vec<_>>().join(" ");
    if flat.is_empty() {
        return match (item.kind, item.width, item.height) {
            (Kind::Image, Some(w), Some(h)) => format!("Image {w} x {h}"),
            (Kind::Image, _, _) => "Image".into(),
            (Kind::Files, _, _) => "Files".into(),
            _ => "(empty)".into(),
        };
    }
    if flat.chars().count() > PREVIEW_CHARS {
        let cut: String = flat.chars().take(PREVIEW_CHARS).collect();
        format!("{cut}...")
    } else {
        flat
    }
}
