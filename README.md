# Clipsy

A Win+V style clipboard manager for Ubuntu and GNOME.

Clipboard history for Linux. Clipsy remembers what you copy (text, rich text,
images and files), lets you search it, and pastes any earlier item again with
a keystroke.

![Clipsy popup](docs/store/1-history-dark.png)

[![Get it from the Snap Store](https://snapcraft.io/static/images/badges/en/snap-store-black.svg)](https://snapcraft.io/clipsy)

```
sudo snap install clipsy
```

## Install

Download `clipsy_<version>_amd64.deb` from the releases page and install it:

```sh
sudo apt install ./clipsy_<version>_amd64.deb
```

This pulls in GTK 4 and libadwaita from Ubuntu's repositories. Clipsy starts
in the background at login (you can turn this off in Settings).

## Ubuntu on Wayland (default)

Ubuntu uses Wayland by default. There, Clipsy needs its small GNOME extension
to see what you copy and to use the shortcut. Set it up once:

1. Open Clipsy from the app menu. It shows a setup card with a command and a
   **Copy** button.
2. Paste the command in a Terminal (Ctrl+Alt+T) and press Enter. For the snap,
   the command is:

   ```
   gnome-extensions install --force /snap/clipsy/current/share/clipsy/clipsy@venkyeswar.github.io.zip && gsettings set org.gnome.shell disable-user-extensions false && L="$(gsettings get org.gnome.shell enabled-extensions)" && case "$L" in *clipsy@venkyeswar.github.io*) ;; *) gsettings set org.gnome.shell enabled-extensions "$(echo "$L" | sed -E "s/^@as \[\]$/[]/; s/\]$/, 'clipsy@venkyeswar.github.io']/; s/^\[, /[/")" ;; esac
   ```

3. Log out and log back in. GNOME turns on new extensions only at login.
4. Open Clipsy again. The card says **Connected**. Press **Super+Shift+V**.

To check the extension: `gnome-extensions info clipsy@venkyeswar.github.io`
should show `State: ACTIVE`.

On an Xorg session ("Ubuntu on Xorg"), no extension is needed.

## Usage

Press **Super+Shift+V** to open the history popup.

- Click an item, or press Enter, to paste it. Shift+Enter pastes plain text.
- Type to search. Filter by type with the tabs (All, Text, Images, Files, Pinned).
- Pin items you use often, or remove items you do not want to keep.
- Right-click an item for more actions.

From a terminal:

```sh
clipsy toggle      # show or hide the popup
clipsy settings    # open Settings
clipsy pause       # stop recording
clipsy resume      # start recording again
clipsy clear       # clear history (pinned items are kept)
clipsy quit        # stop Clipsy
```

## Privacy

Clipsy works entirely on your machine. History is stored locally, and Clipsy
makes no network connections, needs no account, and sends no telemetry.

## Windows (experimental)

Clipsy also has a Windows version. It is new and still being tested, so expect rough edges.

**What works:** copying text, images and files into history; a popup opened with
**Ctrl+Shift+V**; pick an item and press Enter to paste it; pins and search; history
kept after restart; a tray icon with a menu; autostart at login; one running copy
at a time; no network connections.

**Where data lives:** history in `%LOCALAPPDATA%\clipsy`, settings in
`%APPDATA%\clipsy\config.toml`.

### Build on Windows

You need Windows 10 or 11 (64-bit) and:

1. Install Rust from <https://rustup.rs>. Run the installer and accept the defaults
   (this installs the MSVC toolchain).
2. Install **Build Tools for Visual Studio** from <https://visualstudio.microsoft.com/visual-cpp-build-tools/>.
   In the installer, tick the **Desktop development with C++** workload, then install.
3. Open **PowerShell** (Start menu, type `powershell`). Go to the Clipsy project folder
   (the folder that contains this README), for example:

   ```powershell
   cd C:\Users\<your-name>\clipsy
   ```

4. Build the app:

   ```powershell
   cargo build --release -p clipsy-windows
   ```

   The first build takes a few minutes. When it finishes, the program is at
   `target\release\clipsy-windows.exe`.

### Run on Windows

Start Clipsy from the same PowerShell window:

```powershell
.\target\release\clipsy-windows.exe
```

A Clipsy icon appears in the system tray (the `^` arrow near the clock may hide it;
drag it out to keep it visible). Copy something, then press **Ctrl+Shift+V** to open
the popup. Press **Enter** to paste the selected item.

The default hotkey is **Ctrl+Shift+V** because Windows reserves Win+V for its own
clipboard. To change it, edit `%APPDATA%\clipsy\config.toml` (the `toggle_popup` line
under `[shortcuts]`) and restart Clipsy.

Clipsy also understands these commands. Run them from a second PowerShell window while
Clipsy is running:

```powershell
.\target\release\clipsy-windows.exe toggle     # show or hide the popup
.\target\release\clipsy-windows.exe settings   # open Settings
.\target\release\clipsy-windows.exe pause      # stop recording
.\target\release\clipsy-windows.exe resume     # start recording again
.\target\release\clipsy-windows.exe clear      # clear history (pinned items are kept)
.\target\release\clipsy-windows.exe quit       # stop Clipsy
```

### Windows security warning

The Windows program is not signed yet, so Windows SmartScreen may say
**"Windows protected your PC"** with the publisher shown as **unknown**. To run it,
click **More info**, then **Run anyway**.

### Known limits (Windows)

- Clipsy cannot paste into a window that runs as administrator unless Clipsy also runs
  as administrator. **Do not run Clipsy as administrator.** Paste into normal windows only.
- Win+V is reserved by Windows and cannot be used for Clipsy.
- Not yet tested on every Windows setup. The checks in `docs/windows-acceptance.md`
  list what has been verified.

### Developers: check the Windows code from Ubuntu

You can check that the Windows code compiles from Ubuntu. This does not run the app:

```sh
sudo apt install gcc-mingw-w64-x86-64
rustup target add x86_64-pc-windows-gnu
cargo check -p clipsy-windows --target x86_64-pc-windows-gnu
```

## Build from source (Linux)

You need a Rust toolchain (stable) and these packages (Ubuntu 24.04):

```sh
sudo apt install build-essential pkg-config libgtk-4-dev libadwaita-1-dev libx11-dev
```

Then:

```sh
cargo build --release -p clipsy-app
./target/release/clipsy
```

To build the .deb:

```sh
cargo install cargo-deb --locked
packaging/build-deb.sh
```

## Known limits

- Works on Ubuntu's default desktop (GNOME), on both Wayland and Xorg. On
  Wayland, the GNOME extension described above is required.
- Tested on Ubuntu 24.04 with GNOME Shell 46.

## License

GPL-3.0-or-later. See [LICENSE](LICENSE).

## Ubuntu App Center (Snap Store)

The `snap/` folder holds the Snap Store packaging. To build and publish:

```bash
sudo snap install snapcraft --classic
snapcraft                      # builds clipsy_0.2.0_amd64.snap (needs LXD or Multipass)
snapcraft login                # your Snap Store account
snapcraft register clipsy      # reserve the name once
snapcraft upload --release=edge clipsy_0.2.0_amd64.snap
```
Promote from `edge` to `beta`, `candidate` and `stable` with `snapcraft release`.
