# Clipsy GNOME Shell extension

On GNOME Wayland a normal app cannot watch the clipboard, grab global shortcuts,
send key presses or position its own windows. This extension does those jobs for
the Clipsy app: it reports clipboard changes, registers the shortcuts Clipsy asks
for, sets the clipboard, sends Ctrl+V (or Ctrl+Shift+V in terminals) to the
previously focused window, and places the popup at the pointer or screen centre.

It only talks to a local Unix socket created by Clipsy
(`$XDG_RUNTIME_DIR/clipsy-shell.sock`, or the snap equivalent). It makes no
network connections. The protocol is described in `docs/gnome-protocol.md`.

## Manual install

    cp -r clipsy@venkyeswar.github.io ~/.local/share/gnome-shell/extensions/
    gnome-extensions enable clipsy@venkyeswar.github.io

On Wayland, log out and back in first so GNOME Shell notices the new extension.
