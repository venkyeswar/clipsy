# Store request: personal-files for Clipsy

Post this in the Snapcraft forum, category **store-requests**:
https://forum.snapcraft.io/c/store-requests/19

**Title:** Request: auto-connect personal-files for clipsy (its own GNOME Shell extension folder)

---

Hello,

I'm requesting **use and auto-connection** of the `personal-files` interface for
the snap **clipsy** (https://snapcraft.io/clipsy, source:
https://github.com/venkyeswar/clipsy).

**What Clipsy is:** a clipboard history manager for Ubuntu (Super+Shift+V opens
a popup listing what you copied: text, images, files).

**Plug:**

```yaml
plugs:
  gnome-shell-extension:
    interface: personal-files
    write:
      - $HOME/.local/share/gnome-shell/extensions/clipsy@venkyeswar.github.io
```

**Why it is needed:**
- On GNOME **Wayland** (Ubuntu's default session), applications cannot watch
  the clipboard, register a global shortcut or paste into another window.
  Clipsy therefore ships a small companion GNOME Shell extension
  (`clipsy@venkyeswar.github.io`, source in `gnome-extension/` of the repo)
  that does those jobs and talks to the app over a Unix socket in the snap's
  own `$XDG_RUNTIME_DIR`.
- Today users must copy a Terminal command to install that extension, which
  is a common point of failure for non-technical users.
- With this plug, Clipsy copies the extension's two files (`metadata.json`
  and `extension.js`) into **only its own extension folder** and enables it.

**Scope:**
- Write access is limited to the single folder named after Clipsy's own
  extension UUID. Clipsy does not read or change any other extension.
- The files are the ones shipped in the snap (also in the public repo). Clipsy
  makes no network connections.
- Enabling uses the already auto-connected `gsettings` interface (adds the UUID
  to `org.gnome.shell enabled-extensions`).
- Without the connection, Clipsy falls back to showing the command, so the
  plug is purely a usability improvement.

Thank you!

---

## After approval
1. Merge the `personal-files` branch into `main` (it adds the plug to
   `snap/snapcraft.yaml`). CI uploads it to edge.
2. Test on a GNOME Wayland login: a fresh install should show "Almost ready:
   log out and log back in once", with no command.
3. Release to stable.

Until then, do **not** merge: an upload with this plug waits for manual review
and blocks later uploads.
