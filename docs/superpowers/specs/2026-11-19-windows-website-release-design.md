# Clipsy: Windows app, website updates, release automation: design

Date: 2026-11-19. Status: draft, awaiting user review.

## Goal
Make Clipsy available on Windows (native UI, `.exe` installer and portable build), update the website so it is responsive and presents Windows alongside Linux, and automate releases. Build order is fixed by the user: **Phase 1 Windows app, Phase 2 website, Phase 3 release automation.**

Note: `docs/PLAN.md` lists Windows as a non-goal for v1; this spec supersedes that for the Windows work and PLAN.md should be updated when Phase 1 starts.

## Decisions (confirmed by user)
- Windows UI is a native rewrite using **Slint** (not GTK on Windows).
- Windows deliverables: **Setup installer .exe + portable .exe (zip)**, both with SHA256 sums.
- Release order: Windows pieces first, then website, then release workflow.

## Phase 1: Windows app
- New crate `crates/clipsy-windows` (Slint UI), compiled only for Windows (`cfg(windows)`); added to the workspace without breaking Linux builds (`cargo build`/`clippy`/`test` on Linux must keep passing; Linux CI must not try to build Slint/windows-rs code).
- `clipsy-core` (SQLite store, config, images) is reused unchanged except for path handling, which must resolve to `%APPDATA%\clipsy` (config) and `%LOCALAPPDATA%\clipsy` (data) on Windows via the `directories` crate.
- Platform pieces, implemented with `windows-rs`:
  - Clipboard capture: `AddClipboardFormatListener` (text, images, file lists); skip content flagged by password managers (`ExcludeClipboardContentFromMonitorProcessing`, `CanIncludeInClipboardHistory=0`).
  - Global hotkey: `RegisterHotKey`, default `Ctrl+Shift+V` (Win+V is reserved by Windows), configurable in config.toml.
  - Auto-paste: restore focus to the previous foreground window, write item to clipboard, `SendInput` Ctrl+V (Ctrl+Shift+V for terminal apps, using the existing `terminal_apps` setting matched on process name).
  - System tray icon with menu: open, pause/resume, settings, clear, quit.
  - Autostart via `HKCU\Software\Microsoft\Windows\CurrentVersion\Run`.
  - Single instance via a named mutex; CLI args (`toggle`, `settings`, `pause`, `resume`, `clear`, `quit`) forwarded to the running instance over a named pipe.
- Slint UI: popup (search, pins, text/image/file previews, keyboard navigation, positioned near cursor / screen center, closes on focus loss) and settings window; light/dark/system theme following Windows.
- Refactor: move Linux-specific code in `clipsy-backends` behind `cfg(target_os = "linux")` and keep the `Backend` trait the shared interface; Windows implements it in `clipsy-windows`.
- Verification: on Linux `cargo check -p clipsy-windows --target x86_64-pc-windows-gnu` (install mingw target) plus unit tests of pure logic (hotkey parsing, terminal detection, path resolution); real behaviour is verified by the user on a Windows machine/VM against acceptance checks listed in the plan. Dev machine is Ubuntu 24.04 and cannot run the Windows app.
- Acceptance (user runs on Windows 10/11): copy text/image/file shows in history; hotkey opens popup; Enter pastes into Notepad and a terminal; pins and search work; history survives restart; tray menu works; autostart toggle works; no network activity.

## Phase 2: Website (website/, React + Vite + Tailwind v4)
1. Install + Downloads panels (`App.tsx`, `install-section.tsx`, `downloads-section.tsx`): at `lg` and up, two equal-height panels (about 70vh max) each with its own internal scrollbar (thin themed scrollbar, bottom fade as scroll cue); below `lg` they stack and scroll only if taller than the viewport.
2. Responsive behaviour: replace the fixed `max-w-6xl` container with a fluid width (up to about 90rem) and `clamp()`-based padding, fluid type/spacing, an extra breakpoint for very wide screens; verify widths 320 px to 2560 px and browser zoom 50 to 200 percent with no horizontal scroll and no large empty side margins at zoomed-out views.
3. Windows content: Downloads card gets a Windows button (installer and portable) beside `.deb`, with its SHA256 info; previous-versions table gains a Windows column; Install section gets a Windows tab (installer, portable, verify checksum with `Get-FileHash`/`certutil`, SmartScreen "unknown publisher" note since binaries are unsigned); OS detection highlights the visitor's platform; hero/features copy and the shortcut shown (Ctrl+Shift+V on Windows) are updated. Asset matching in `src/lib/releases.ts` extended to Windows assets by filename pattern (`clipsy-<version>-setup.exe`, `clipsy-<version>-windows-x64.zip`).
4. Tests: vitest for asset matching, OS detection and install-commands; manual responsive checks.

## Phase 3: Release automation
- New `.github/workflows/release.yml` on `v*` tags: Ubuntu job builds the `.deb` (existing `packaging/build-deb.sh`); `windows-latest` job builds `clipsy.exe`, the portable zip and the Inno Setup installer; a final job generates a combined `SHA256SUMS` and publishes a GitHub Release with all assets plus `install.sh`.
- Add `.github/workflows/deploy-website.yml` (GitHub Pages) because it is referenced by earlier plans but missing. Hosting of clipsy.thalvik.in is unconfirmed and must be checked with the user before this step.
- Extend `ci.yml` with a Windows build/check job and a website lint/test/build job.
- Out of scope: code signing certificate (document SmartScreen warning instead), macOS, auto-update.

## Risks
- Windows code cannot be run on the Ubuntu dev machine; correctness depends on the user's Windows testing and CI.
- Slint plus windows-rs popup behaviours (focus stealing, foreground window restore) are the most likely trouble spots; budget iteration time.
- Unsigned binaries trigger SmartScreen warnings.
