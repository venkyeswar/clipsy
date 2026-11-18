import Gio from 'gi://Gio';
import GLib from 'gi://GLib';
import Meta from 'gi://Meta';
import Shell from 'gi://Shell';
import St from 'gi://St';
import Clutter from 'gi://Clutter';

import {Extension} from 'resource:///org/gnome/shell/extensions/extension.js';
import {PACKAGE_VERSION} from 'resource:///org/gnome/shell/misc/config.js';
import * as Main from 'resource:///org/gnome/shell/ui/main.js';

const PROTOCOL_VERSION = 1;
const SOCKET_NAME = 'clipsy-shell.sock';
const RETRY_SECONDS = 2;
const DEBOUNCE_MS = 100;
const ECHO_GUARD_MS = 500;
const POPUP_WINDOW_MS = 2000;
const PASTE_DELAY_MS = 150;
const KEY_GAP_MS = 20;
const MAX_FORMAT_BYTES = 32 * 1024 * 1024;
const POPUP_CLASS = 'io.github.clipsy.Clipsy';
const PLACE_FALLBACK_MS = 120;
const POPUP_TITLE = 'Clipboard history';
const PASSWORD_HINT = 'x-kde-passwordManagerHint';
const PASSWORD_MANAGERS = ['keepassxc', 'bitwarden', '1password', 'org.gnome.world.secrets'];
const TERMINALS = ['gnome-terminal', 'kgx', 'org.gnome.console', 'ptyxis', 'org.gnome.ptyxis',
    'tilix', 'alacritty', 'kitty', 'wezterm', 'xterm', 'konsole', 'terminator', 'foot'];
const ALL_MODES = Shell.ActionMode.NORMAL | Shell.ActionMode.OVERVIEW | Shell.ActionMode.POPUP;
const TEXT_MIMES = ['text/plain;charset=utf-8', 'UTF8_STRING', 'text/plain'];
const SET_ORDER_EXACT = ['x-special/gnome-copied-files', 'image/png'];

function nowMs() {
    return GLib.get_monotonic_time() / 1000;
}

function isClipsyWindow(win) {
    try {
        return win.get_wm_class() === POPUP_CLASS ||
            win.get_wm_class_instance() === POPUP_CLASS ||
            win.get_gtk_application_id() === POPUP_CLASS;
    } catch (e) {
        return false;
    }
}

export default class ClipsyExtension extends Extension {
    enable() {
        this._enabled = true;
        this._cancellable = new Gio.Cancellable();
        this._connection = null;
        this._input = null;
        this._output = null;
        this._writeQueue = [];
        this._writing = false;
        this._retryId = 0;
        this._debounceId = 0;
        this._pasteId = 0;
        this._releaseIds = new Set();
        this._ignoreUntil = 0;
        this._reading = false;
        this._readAgain = false;
        this._binds = new Map(); // action -> {id, name}
        this._remembered = null;
        this._popup = null; // {position, deadline}
        this._popupWatch = new Map(); // win -> [[obj, id], ...]
        this._kbd = null;

        const selection = global.display.get_selection();
        this._selection = selection;
        this._ownerChangedId = selection.connect('owner-changed', (sel, type) => {
            if (type === Meta.SelectionType.SELECTION_CLIPBOARD)
                this._onOwnerChanged();
        });
        this._accelId = global.display.connect('accelerator-activated',
            (display, action) => this._onAccelerator(action));
        this._windowCreatedId = global.display.connect('window-created',
            (display, win) => this._onWindowCreated(win));

        this._connect();
    }

    disable() {
        this._enabled = false;
        this._cancellable?.cancel();
        this._cancellable = null;

        if (this._ownerChangedId) {
            this._selection.disconnect(this._ownerChangedId);
            this._ownerChangedId = 0;
        }
        this._selection = null;
        if (this._accelId) {
            global.display.disconnect(this._accelId);
            this._accelId = 0;
        }
        if (this._windowCreatedId) {
            global.display.disconnect(this._windowCreatedId);
            this._windowCreatedId = 0;
        }

        for (const id of [this._retryId, this._debounceId, this._pasteId]) {
            if (id)
                GLib.source_remove(id);
        }
        this._retryId = this._debounceId = this._pasteId = 0;
        for (const id of this._releaseIds)
            GLib.source_remove(id);
        this._releaseIds.clear();

        this._clearPopupWatch();
        this._popup = null;
        this._remembered = null;

        this._releaseAllBinds();
        this._closeConnection();

        if (this._kbd) {
            try {
                this._kbd.run_dispose();
            } catch (e) {
                // ignore
            }
            this._kbd = null;
        }
        this._binds = null;
        this._writeQueue = null;
    }

    // ---- connection -------------------------------------------------

    _socketPaths() {
        const runtime = GLib.get_user_runtime_dir();
        const paths = [`${runtime}/snap.clipsy/${SOCKET_NAME}`, `${runtime}/${SOCKET_NAME}`];
        return paths;
    }

    _scheduleRetry() {
        if (!this._enabled || this._retryId)
            return;
        this._retryId = GLib.timeout_add_seconds(GLib.PRIORITY_DEFAULT, RETRY_SECONDS, () => {
            this._retryId = 0;
            this._connect();
            return GLib.SOURCE_REMOVE;
        });
    }

    _connect() {
        if (!this._enabled || this._connection)
            return;
        const paths = this._socketPaths().filter(p => GLib.file_test(p, GLib.FileTest.EXISTS));
        if (paths.length === 0) {
            this._scheduleRetry();
            return;
        }
        this._tryConnect(paths, 0);
    }

    _tryConnect(paths, index) {
        if (!this._enabled)
            return;
        if (index >= paths.length) {
            this._scheduleRetry();
            return;
        }
        const client = new Gio.SocketClient();
        const address = Gio.UnixSocketAddress.new(paths[index]);
        const cancellable = this._cancellable;
        client.connect_async(address, cancellable, (c, res) => {
            let conn;
            try {
                conn = c.connect_finish(res);
            } catch (e) {
                if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    this._tryConnect(paths, index + 1);
                return;
            }
            if (!this._enabled) {
                try {
                    conn.close(null);
                } catch (e) {
                    // ignore
                }
                return;
            }
            this._onConnected(conn);
        });
    }

    _onConnected(conn) {
        console.debug('Clipsy: connected to app');
        this._connection = conn;
        this._output = conn.get_output_stream();
        this._input = new Gio.DataInputStream({
            base_stream: conn.get_input_stream(),
            close_base_stream: false,
        });
        this._writeQueue = [];
        this._writing = false;
        this._send({type: 'hello', version: PROTOCOL_VERSION, shell: this._shellVersion()});
        this._readLine(conn);
    }

    _shellVersion() {
        return `${PACKAGE_VERSION}`;
    }

    _readLine(conn) {
        this._input.read_line_async(GLib.PRIORITY_DEFAULT, this._cancellable, (stream, res) => {
            if (conn !== this._connection)
                return;
            let bytes;
            try {
                [bytes] = stream.read_line_finish(res);
            } catch (e) {
                if (!e.matches?.(Gio.IOErrorEnum, Gio.IOErrorEnum.CANCELLED))
                    console.debug(`Clipsy: read error: ${e.message}`);
                this._onDisconnected(conn);
                return;
            }
            if (bytes === null) {
                this._onDisconnected(conn);
                return;
            }
            try {
                if (bytes.length > 48 * 1024 * 1024)
                    console.warn('Clipsy: dropping oversized line');
                else if (bytes.length > 0)
                    this._handleMessage(JSON.parse(new TextDecoder().decode(bytes)));
            } catch (e) {
                console.warn(`Clipsy: bad message: ${e.message}`);
            }
            if (this._enabled && conn === this._connection)
                this._readLine(conn);
        });
    }

    _onDisconnected(conn) {
        if (conn !== this._connection)
            return;
        console.debug('Clipsy: disconnected');
        this._releaseAllBinds();
        this._closeConnection();
        this._scheduleRetry();
    }

    _closeConnection() {
        const conn = this._connection;
        this._connection = null;
        this._input = null;
        this._output = null;
        this._writeQueue = [];
        if (conn) {
            try {
                conn.close(null);
            } catch (e) {
                // ignore
            }
        }
    }

    _send(obj) {
        if (!this._output)
            return;
        const bytes = new TextEncoder().encode(`${JSON.stringify(obj)}\n`);
        this._writeQueue.push(bytes);
        this._pump();
    }

    _pump() {
        if (this._writing || !this._output || this._writeQueue.length === 0)
            return;
        this._writing = true;
        const conn = this._connection;
        const bytes = this._writeQueue.shift();
        this._output.write_all_async(bytes, GLib.PRIORITY_DEFAULT, this._cancellable, (out, res) => {
            try {
                out.write_all_finish(res);
            } catch (e) {
                if (conn === this._connection) {
                    console.debug(`Clipsy: write error: ${e.message}`);
                    this._onDisconnected(conn);
                }
                return;
            }
            if (conn !== this._connection)
                return;
            this._writing = false;
            this._pump();
        });
    }

    // ---- incoming messages ------------------------------------------

    _handleMessage(msg) {
        switch (msg?.type) {
        case 'bind':
            this._bind(msg.action, msg.accel);
            break;
        case 'unbind':
            this._unbind(msg.action);
            break;
        case 'remember_window':
            this._rememberWindow();
            break;
        case 'show_popup':
            this._popup = {
                position: msg.position === 'center' ? 'center' : 'cursor',
                deadline: nowMs() + POPUP_WINDOW_MS,
            };
            // The popup window can be mapped before this message is read, so
            // also handle one that already exists.
            this._placeExistingPopup();
            break;
        case 'set_clipboard':
            this._setClipboard(msg.formats);
            break;
        case 'paste':
            this._paste(msg.mode === 'terminal');
            break;
        default:
            break;
        }
    }

    // ---- shortcuts --------------------------------------------------

    _bind(action, accel) {
        if (typeof action !== 'string' || typeof accel !== 'string')
            return;
        this._unbind(action);
        let ok = false;
        let error;
        try {
            const id = global.display.grab_accelerator(accel, Meta.KeyBindingFlags.NONE);
            if (id === Meta.KeyBindingAction.NONE) {
                error = 'key combination already taken';
            } else {
                const name = Meta.external_binding_name_for_action(id);
                Main.wm.allowKeybinding(name, ALL_MODES);
                this._binds.set(action, {id, name});
                ok = true;
            }
        } catch (e) {
            error = e.message;
        }
        const reply = {type: 'bind_result', action, ok};
        if (error)
            reply.error = error;
        this._send(reply);
    }

    _unbind(action) {
        const b = this._binds?.get(action);
        if (!b)
            return;
        this._binds.delete(action);
        try {
            global.display.ungrab_accelerator(b.id);
            Main.wm.allowKeybinding(b.name, Shell.ActionMode.NONE);
        } catch (e) {
            console.warn(`Clipsy: unbind failed: ${e.message}`);
        }
    }

    _releaseAllBinds() {
        if (!this._binds)
            return;
        for (const action of [...this._binds.keys()])
            this._unbind(action);
    }

    _onAccelerator(id) {
        for (const [action, b] of this._binds) {
            if (b.id === id) {
                this._send({type: 'hotkey', action});
                return;
            }
        }
    }

    // ---- clipboard watching -----------------------------------------

    _onOwnerChanged() {
        if (!this._output)
            return;
        // Capture focus now: it may change by the time the debounce fires.
        this._pendingSource = this._focusedApp();
        if (this._debounceId)
            GLib.source_remove(this._debounceId);
        this._debounceId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, DEBOUNCE_MS, () => {
            this._debounceId = 0;
            this._readClipboard();
            return GLib.SOURCE_REMOVE;
        });
    }

    _focusedApp() {
        try {
            const win = global.display.focus_window;
            return win?.get_wm_class() || win?.get_gtk_application_id() || null;
        } catch (e) {
            return null;
        }
    }

    async _readClipboard() {
        if (!this._enabled || !this._output)
            return;
        if (nowMs() < this._ignoreUntil)
            return;
        if (this._reading) {
            this._readAgain = true;
            return;
        }
        this._reading = true;
        try {
            const source = this._pendingSource ?? null;
            const mimes = this._selection.get_mimetypes(Meta.SelectionType.SELECTION_CLIPBOARD) ?? [];
            const lower = mimes.map(m => m.toLowerCase());
            const sensitive = lower.includes(PASSWORD_HINT) ||
                (source !== null && PASSWORD_MANAGERS.some(p => source.toLowerCase().includes(p)));
            const formats = [];
            if (!lower.includes(PASSWORD_HINT)) {
                for (const [mime, outMime] of this._chooseMimes(mimes)) {
                    if (!this._enabled)
                        return;
                    const bytes = await this._transfer(mime);
                    if (!bytes || bytes.get_size() === 0 || bytes.get_size() > MAX_FORMAT_BYTES)
                        continue;
                    formats.push({mime: outMime, data: GLib.base64_encode(bytes.get_data())});
                }
            }
            if (this._enabled && (formats.length > 0 || sensitive))
                this._send({type: 'clip', formats, source_app: source, sensitive});
        } catch (e) {
            console.warn(`Clipsy: clipboard read failed: ${e.message}`);
        } finally {
            this._reading = false;
            if (this._readAgain && this._enabled) {
                this._readAgain = false;
                this._readClipboard();
            }
        }
    }

    _chooseMimes(mimes) {
        const out = [];
        const text = TEXT_MIMES.find(m => mimes.includes(m));
        if (text)
            out.push([text, 'text/plain;charset=utf-8']);
        if (mimes.includes('text/html'))
            out.push(['text/html', 'text/html']);
        const image = mimes.includes('image/png') ? 'image/png' : mimes.find(m => m.startsWith('image/'));
        if (image)
            out.push([image, image]);
        for (const m of ['text/uri-list', 'x-special/gnome-copied-files']) {
            if (mimes.includes(m))
                out.push([m, m]);
        }
        return out;
    }

    _transfer(mime) {
        return new Promise(resolve => {
            const out = Gio.MemoryOutputStream.new_resizable();
            try {
                this._selection.transfer_async(Meta.SelectionType.SELECTION_CLIPBOARD, mime, -1,
                    out, this._cancellable, (sel, res) => {
                        try {
                            sel.transfer_finish(res);
                            out.close(null);
                            resolve(out.steal_as_bytes());
                        } catch (e) {
                            console.debug(`Clipsy: transfer of ${mime} failed: ${e.message}`);
                            resolve(null);
                        }
                    });
            } catch (e) {
                resolve(null);
            }
        });
    }

    // ---- setting the clipboard --------------------------------------

    _setClipboard(formats) {
        if (!Array.isArray(formats))
            return;
        const byMime = new Map();
        for (const f of formats) {
            if (typeof f?.mime === 'string' && typeof f?.data === 'string')
                byMime.set(f.mime, f.data);
        }
        let mime = SET_ORDER_EXACT.find(m => byMime.has(m));
        mime ??= [...byMime.keys()].find(m => m.startsWith('image/'));
        mime ??= ['text/plain;charset=utf-8', 'text/plain', 'text/html'].find(m => byMime.has(m));
        if (!mime)
            return;
        try {
            const data = GLib.base64_decode(byMime.get(mime));
            this._ignoreUntil = nowMs() + ECHO_GUARD_MS;
            St.Clipboard.get_default().set_content(St.ClipboardType.CLIPBOARD, mime,
                new GLib.Bytes(data));
        } catch (e) {
            console.warn(`Clipsy: set_clipboard failed: ${e.message}`);
        }
    }

    // ---- windows / paste --------------------------------------------

    _rememberWindow() {
        const win = global.display.focus_window;
        if (!win || isClipsyWindow(win))
            return;
        this._remembered = win;
    }

    _paste(terminalMode) {
        const win = this._remembered;
        if (win) {
            try {
                Main.activateWindow(win);
            } catch (e) {
                console.debug(`Clipsy: remembered window is gone: ${e.message}`);
                this._remembered = null;
            }
        }
        let terminal = terminalMode;
        if (!terminal && win) {
            try {
                const cls = (win.get_wm_class() ?? '').toLowerCase();
                terminal = TERMINALS.some(t => cls.includes(t));
            } catch (e) {
                // ignore
            }
        }
        if (this._pasteId)
            GLib.source_remove(this._pasteId);
        this._pasteId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PASTE_DELAY_MS, () => {
            this._pasteId = 0;
            this._sendPasteKeys(terminal);
            return GLib.SOURCE_REMOVE;
        });
    }

    _sendPasteKeys(shift) {
        try {
            if (!this._kbd) {
                const seat = Clutter.get_default_backend().get_default_seat();
                this._kbd = seat.create_virtual_device(Clutter.InputDeviceType.KEYBOARD_DEVICE);
            }
            const kbd = this._kbd;
            const keys = shift
                ? [Clutter.KEY_Control_L, Clutter.KEY_Shift_L, Clutter.KEY_v]
                : [Clutter.KEY_Control_L, Clutter.KEY_v];
            const press = (key, state) =>
                kbd.notify_keyval(Clutter.get_current_event_time() * 1000, key, state);
            for (const key of keys)
                press(key, Clutter.KeyState.PRESSED);
            // Release in reverse order, slightly later so apps see the chord.
            const id = GLib.timeout_add(GLib.PRIORITY_DEFAULT, KEY_GAP_MS, () => {
                this._releaseIds.delete(id);
                if (this._kbd) {
                    for (const key of [...keys].reverse())
                        press(key, Clutter.KeyState.RELEASED);
                }
                return GLib.SOURCE_REMOVE;
            });
            this._releaseIds.add(id);
        } catch (e) {
            console.warn(`Clipsy: paste failed: ${e.message}`);
        }
    }

    // ---- popup placement --------------------------------------------

    _watch(win, obj, signal, cb) {
        const id = obj.connect(signal, cb);
        if (!this._popupWatch.has(win))
            this._popupWatch.set(win, []);
        this._popupWatch.get(win).push([obj, id]);
    }

    _unwatch(win) {
        for (const [obj, id] of this._popupWatch.get(win) ?? []) {
            try {
                obj.disconnect(id);
            } catch (e) {
                // object already gone
            }
        }
        this._popupWatch.delete(win);
    }

    _clearPopupWatch() {
        this._cancelPlaceTimer();
        for (const win of [...this._popupWatch.keys()])
            this._unwatch(win);
    }

    _onWindowCreated(win) {
        if (!this._popup || nowMs() > this._popup.deadline) {
            this._popup = null;
            return;
        }
        // On Wayland the app id and title usually arrive after the window is
        // created, so check now and again whenever either one changes.
        const check = () => {
            const popup = this._popup;
            if (!popup || nowMs() > popup.deadline) {
                this._unwatch(win);
                return;
            }
            if (isClipsyWindow(win) && win.get_title() === POPUP_TITLE) {
                this._unwatch(win);
                this._placeWhenReady(win, popup);
            }
        };
        for (const signal of ['notify::title', 'notify::wm-class', 'notify::gtk-application-id'])
            this._watch(win, win, signal, check);
        this._watch(win, win, 'unmanaged', () => this._unwatch(win));
        check();
    }

    _placeExistingPopup() {
        const popup = this._popup;
        for (const actor of global.get_window_actors()) {
            const win = actor.meta_window;
            if (win && isClipsyWindow(win) && win.get_title() === POPUP_TITLE) {
                this._placeWhenReady(win, popup);
                return;
            }
        }
    }

    _placeWhenReady(win, popup) {
        this._popup = null;
        const actor = win.get_compositor_private();
        if (actor?.mapped) {
            this._place(win, popup);
            return;
        }
        // Place after the first frame is drawn. That signal is not always sent,
        // so a short timer does the same job as a fallback.
        let done = false;
        const placeOnce = () => {
            if (done)
                return;
            done = true;
            this._unwatch(win);
            this._cancelPlaceTimer();
            this._place(win, popup);
        };
        if (actor)
            this._watch(win, actor, 'first-frame', placeOnce);
        this._watch(win, win, 'unmanaged', () => {
            done = true;
            this._unwatch(win);
            this._cancelPlaceTimer();
        });
        this._cancelPlaceTimer();
        this._placeTimerId = GLib.timeout_add(GLib.PRIORITY_DEFAULT, PLACE_FALLBACK_MS, () => {
            this._placeTimerId = 0;
            placeOnce();
            return GLib.SOURCE_REMOVE;
        });
    }

    _cancelPlaceTimer() {
        if (this._placeTimerId) {
            GLib.source_remove(this._placeTimerId);
            this._placeTimerId = 0;
        }
    }

    _place(win, popup) {
        try {
            const monitor = global.display.get_current_monitor();
            const work = Main.layoutManager.getWorkAreaForMonitor(monitor);
            const [px, py] = global.get_pointer();
            const rect = win.get_frame_rect();
            let x;
            let y;
            if (popup.position === 'center') {
                x = work.x + Math.floor((work.width - rect.width) / 2);
                y = work.y + Math.floor((work.height - rect.height) / 2);
            } else {
                x = px;
                y = py;
            }
            x = Math.max(work.x, Math.min(x, work.x + work.width - rect.width));
            y = Math.max(work.y, Math.min(y, work.y + work.height - rect.height));
            win.move_frame(true, x, y);
            win.make_above();
            Main.activateWindow(win);
        } catch (e) {
            console.warn(`Clipsy: popup placement failed: ${e.message}`);
        }
    }
}
