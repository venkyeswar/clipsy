# Windows parity with Ubuntu: shortcut, popup, settings, logo (short design)

Status: approved by the user on 2026-11-19. Release is NOT part of this work: build, test on the EC2 Windows box first, release only after the user confirms.

## Intent and why
Windows 0.4.3 works but feels like a first draft next to the Ubuntu app: different shortcut, plain popup, settings and clear-all hidden in the tray menu, and the Clipsy logo is missing in places. The user wants parity with Ubuntu.

## Goals
1. Default shortcut Win+Shift+V (Ubuntu uses Super+Shift+V). If RegisterHotKey fails because Windows owns the combo, fall back to Ctrl+Shift+V and say so in the log and tray tooltip. Existing configs that still hold the old default "<Ctrl><Shift>v" migrate to the new default; a shortcut the user chose themselves is kept.
2. Popup restyle to match the Linux look: header, pill filter row, accent edge on pinned rows, hover buttons for pin and delete, footer hint bar ("Enter paste, Shift+Enter plain, Delete remove"). Keys: Delete removes, Ctrl+P pins, Space previews (same as Linux).
3. Popup header gets a gear button (opens Settings) and a trash button (confirm dialog with "also remove pinned" option, then clear), same behaviour as Linux.
4. Settings window gains the missing Linux options (total size cap, max item size, rich text) and a "Clear history" button under Privacy with the same confirm dialog.
5. Logo: show the Clipsy icon in the popup header, tray, settings window, taskbar and the exe/installer (check which are missing and fix).
6. Windows-only changes; avoid touching shared crates so Linux CI does not run (core additions only if unavoidable, and then say so).

## Non-goals
Code signing, new features beyond the list, Linux UI changes, releasing.

## Approaches
A. Extend the existing Slint popup and settings in place (chosen): lowest risk, reuses Config, Store::clear, Store::delete, Store::count.
B. Rewrite the UI with another toolkit: rejected, too large.

## Constraints
Slint UI in crates/clipsy-windows/ui; cargo clippy -- -D warnings and cargo test must pass; cannot run Windows UI locally, so verification is on the EC2 instance.

## Left to the builder
Exact colours (take them from the Linux style and the website palette), the fallback wording, and how the preview (Space) is shown.
