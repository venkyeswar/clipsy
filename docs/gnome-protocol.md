# Clipsy ↔ GNOME Shell extension protocol (v1)

Used on GNOME **Wayland**, where a normal app cannot watch the clipboard, grab global
shortcuts, send key presses or place its own windows. The extension
(`clipsy@venkyeswar.github.io`, in `gnome-extension/`) does those jobs for the app.

## Transport
- The **app is the server**. It listens on a Unix stream socket at
  `$XDG_RUNTIME_DIR/clipsy-shell.sock`, created with mode 0600.
  - In the snap, the app's `XDG_RUNTIME_DIR` is `/run/user/<uid>/snap.clipsy`.
  - Outside the snap, it is `/run/user/<uid>`.
- The **extension is the client**. It tries `/run/user/<uid>/snap.clipsy/clipsy-shell.sock`
  first, then `/run/user/<uid>/clipsy-shell.sock`. While it is not connected, it retries every
  2 seconds, using GLib timeouts and never blocking.
- Each message is one JSON object on one line (UTF-8, ending in `\n`). Binary payloads are
  base64 (standard alphabet, with padding). Lines can be large (images), so the maximum line
  is 48 MiB. A peer drops bigger lines and logs them.
- Only one extension connection is active at a time. A new connection replaces the old one.
- Unknown `type` values are ignored on both sides (forward compatibility).

## Extension → app
| type | fields | meaning |
|---|---|---|
| `hello` | `version`: 1, `shell`: "46.0" | First message after connecting. The app replies by (re)sending every `bind`. |
| `clip` | `formats`: [{`mime`, `data`(b64)}], `source_app`: string or null, `sensitive`: bool | The clipboard (CLIPBOARD selection, not PRIMARY) changed. |
| `hotkey` | `action`: `toggle_popup` / `clear_history` / `pause_recording` | A bound shortcut was pressed. |
| `bind_result` | `action`, `ok`: bool, `error`: string (optional) | The answer to a `bind`. `ok:false` means the key combination is already taken. |

### `clip` details
- Mimes to read, if offered, in this order of preference:
  - text: `text/plain;charset=utf-8`, else `UTF8_STRING`, else `text/plain`. Always send it as `text/plain;charset=utf-8`.
  - `text/html`
  - one image: `image/png` preferred, else the first `image/*`
  - `text/uri-list`
  - `x-special/gnome-copied-files`
- `sensitive` is true when the offered mimes include `x-kde-passwordManagerHint` (then send no
  formats at all), or when the source is a known password manager (keepassxc, bitwarden,
  1password, org.gnome.World.Secrets).
- `source_app` is the WM class (or the app id on Wayland) of the focused window when the change happened.
- Skip any single format larger than 32 MiB.
- The extension must not echo back content it set itself for `set_clipboard` within the last 500 ms. The app dedups anyway.

## App → extension
| type | fields | meaning |
|---|---|---|
| `bind` | `action`, `accel` (GTK syntax, e.g. `<Super><Shift>v`) | Grab a global shortcut. Replaces an earlier binding for the same action. The extension answers with `bind_result`. |
| `unbind` | `action` | Release that shortcut. |
| `remember_window` | – | Store the currently focused window (the paste target). Ignore it if that window is the Clipsy popup itself. |
| `show_popup` | `position`: `cursor` / `center` | The app is about to present the popup. The next Clipsy window titled `Clipboard history` that appears within 2 s gets the steps below. |
| `set_clipboard` | `formats`: [{`mime`, `data`}] | Become the clipboard owner. GNOME's St.Clipboard offers one mime only, so use the first match in this order: `x-special/gnome-copied-files`, `image/png`, any `image/*`, `text/plain;charset=utf-8`, `text/plain`, `text/html`. |
| `paste` | `mode`: `normal` / `terminal` / `binary`, `terminal_apps`: [class] | Re-activate the remembered window. About 150 ms later, send Ctrl+V. Send Ctrl+Shift+V instead when `mode` is `terminal`, or (for `normal`) when the window's class looks like a terminal (gnome-terminal, kgx/Console, ptyxis, tilix, alacritty, kitty, wezterm, xterm, konsole, terminator, foot, or a class listed in `terminal_apps`). Mode `binary` (used for image items) always sends Ctrl+V, even in a terminal, so tools that read the clipboard image themselves receive it. |

When the popup window appears after `show_popup`, the extension:
1. clamps it inside the work area of the pointer's monitor
2. moves it to the pointer for `cursor`, or to the middle of that monitor for `center`
3. sets it to always-on-top (`make_above`)
4. focuses it with `Main.activateWindow`

## Lifetime
- The app removes stale sockets at startup.
- When the connection closes:
  - the extension releases all of its grabs and starts retrying
  - the app marks the backend "extension not connected" (`setup_needed()` → `GnomeExtension`)
- Bindings exist only while connected. The app re-sends them on every `hello`.
