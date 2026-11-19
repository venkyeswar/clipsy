# Clipsy Windows: acceptance checklist

Run these checks on Windows 10 or 11. Go through them in order. For each item, write
**pass** or **fail** and, for a fail, what you saw instead.

Clipsy is not finished until you have run this list and told us the results. Do not
start the website work until then.

## Before you start

1. Find your Windows version: press the **Windows** key, type `winver`, press **Enter**.
   Write down the version shown (for example, "Windows 11, version 24H2").
2. Build Clipsy, or get `clipsy-windows.exe` from the person who built it. The README
   section "Windows (experimental)" explains the build. The program is at
   `target\release\clipsy-windows.exe`.
3. Open **PowerShell** (press the **Windows** key, type `powershell`, press **Enter**).
4. Go to the folder that holds the Clipsy project:

   ```powershell
   cd C:\Users\<your-name>\clipsy
   ```

5. Make sure no Clipsy is already running. If one is, quit it:

   ```powershell
   .\target\release\clipsy-windows.exe quit
   ```

6. Optional, for more detail: if a check says "logs", run this line in the same
   PowerShell window **before** starting Clipsy. Logs then appear in that window:

   ```powershell
   $env:RUST_LOG = "debug"
   ```

7. Start Clipsy normally:

   ```powershell
   .\target\release\clipsy-windows.exe
   ```

   A Clipsy icon appears in the system tray, near the clock. If you do not see it,
   click the `^` arrow to show hidden icons.

Unless a check says otherwise, keep Clipsy running for the whole list.

---

## Checks

### 1. Copy text, an image and a file: all three are saved

Steps:
1. Open **Notepad**. Type `Clipsy test one`, select the words, press **Ctrl+C**.
2. Press **Windows+Shift+S**. Drag a box over any part of the screen. The box is copied as an image.
3. Open **File Explorer**. Click any file once and press **Ctrl+C**.
4. Press **Ctrl+Shift+V** to open the Clipsy popup.

Expected: the popup lists the three new items. One row shows `Clipsy test one` as text.
One row shows a picture (labelled `IMG`). One row shows the file name (labelled `FILE`).

### 2. Ctrl+Shift+V opens the popup near the mouse pointer

Steps:
1. Move the mouse pointer to a corner of the screen, away from the middle.
2. Press **Ctrl+Shift+V**.
3. Press **Esc** to close the popup.

Expected: the popup opens next to the mouse pointer, not in the middle of the screen.

### 3. Enter pastes into Notepad and into Windows Terminal

Steps for Notepad:
1. Open **Notepad**. Type `Clipsy paste test`, select it, press **Ctrl+C**.
2. Click in an empty line of Notepad.
3. Press **Ctrl+Shift+V**. The newest item is at the top. Press **Enter**.

Expected: `Clipsy paste test` appears in Notepad at the cursor. The popup closes.
There are no extra symbols such as `^V`.

Steps for Windows Terminal:
1. Keep the text from the Notepad steps copied (or copy it again).
2. Press the **Windows** key, type `terminal`, press **Enter** to open Windows Terminal.
3. Click inside the terminal window.
4. Press **Ctrl+Shift+V**, select the newest item, press **Enter**.

Expected: the text appears at the terminal prompt. It is pasted but not run: the command
does not start by itself. (Clipsy uses Ctrl+Shift+V for terminals.)

### 4. Pin and search work

Steps:
1. Copy at least three different short texts in Notepad, for example `apple`, `banana`, `cherry`.
2. Press **Ctrl+Shift+V**. Click the star (**☆**) at the right end of `banana`.
3. Check the star: it should change to **★**. Then click the **Pinned** tab.
4. Click the **All** tab. Click in the search box and type `ban`.
5. Delete the text in the search box.

Expected:
- After step 2, the star is filled (**★**) and the pinned item moves to the top.
- The **Pinned** tab shows only pinned items.
- Typing `ban` shows only `banana`. Other items are hidden.
- Clearing the search shows all items again.
- Typing text that matches nothing (for example `zzz`) shows the message "No matches".

### 5. History survives a restart

Steps:
1. Press **Ctrl+Shift+V**. Write down the first three items (in order) and which ones are pinned.
2. Press **Escape**. In PowerShell, quit Clipsy:

   ```powershell
   .\target\release\clipsy-windows.exe quit
   ```

3. Wait 3 seconds. Start Clipsy again:

   ```powershell
   .\target\release\clipsy-windows.exe
   ```

4. Press **Ctrl+Shift+V**.

Expected: the same items are there, in the same order, and the pinned item is still pinned.

### 6. Tray menu works, and Pause stops recording

Steps:
1. Right-click the Clipsy tray icon. The menu shows: **Open**, **Pause capture**, **Settings**,
   **Clear history**, **Quit**. Do **not** click **Clear history** (it empties your history).
2. Click **Pause capture**.
3. In Notepad, type `should not be saved` and copy it (**Ctrl+C**).
4. Press **Ctrl+Shift+V**. Check the list.
5. Press **Escape**. Right-click the tray icon again. The menu now shows **Resume capture**.
6. Click **Resume capture**.
7. In Notepad, copy the text `should be saved`. Press **Ctrl+Shift+V**.

Expected: `should not be saved` is **not** in the list (step 4). `should be saved` **is**
in the list (step 7). The menu label changes between "Pause capture" and "Resume capture".

### 7. Launch at login (autostart) adds and removes the Run entry

Steps:
1. Right-click the tray icon, click **Settings**.
2. Turn **on** the switch for **launch at login** (or autostart).
3. In PowerShell, run:

   ```powershell
   Get-ItemProperty -Path "HKCU:\Software\Microsoft\Windows\CurrentVersion\Run" -Name Clipsy
   ```

4. Go back to Settings and turn the switch **off**.
5. Run the same command again.

Expected: after step 3, the output shows a `Clipsy` line with a path to `clipsy-windows.exe`.
After step 5, PowerShell shows an error saying the property `Clipsy` does not exist.

If Settings has no such switch, write that down and skip to the next check.

### 8. A password from a password manager is not saved

Use a **test** entry in the password manager. Do not paste real passwords into Notepad.
This check works with KeePassXC, 1Password or Bitwarden. These apps mark the copied
password so Clipsy skips it.

Steps:
1. Press **Ctrl+Shift+V** and write down the newest item (or take a screenshot of the list).
2. Open your password manager. Select a test entry. Copy its password
   (in KeePassXC: right-click the entry, then **Copy password**).
3. Press **Ctrl+Shift+V** again.

Expected: the password is **not** in the list. The newest item is the same as before.
(Password managers clear the clipboard themselves after a short time. That is normal.)

### 9. Starting Clipsy twice leaves one tray icon

Steps:
1. Quit Clipsy if it is running:

   ```powershell
   .\target\release\clipsy-windows.exe quit
   ```

2. Start it two times, quickly, in one line:

   ```powershell
   Start-Process .\target\release\clipsy-windows.exe; Start-Process .\target\release\clipsy-windows.exe
   ```

3. Look at the tray. Also click the `^` arrow to check the hidden icons.
4. Press **Ctrl+Shift+V**.

Expected: there is **one** Clipsy icon in the tray, not two. The popup still opens with Ctrl+Shift+V.
The second copy closes by itself.

### 10. No network connections

Steps:
1. Start Clipsy. Use it for about 2 minutes: copy a few things, open the popup, paste one item.
2. Press the **Windows** key, type `resmon`, press **Enter** (this opens Resource Monitor).
3. Click the **Network** tab.
4. Look at the table **Processes with Network Activity**.

Expected: `clipsy-windows.exe` is **not** listed in that table. The table is empty or does
not show Clipsy.

### 11. Pasting from history does not create a duplicate or change the order

Steps:
1. In Notepad, copy these three texts in this order: `first`, then `second`, then `third`.
2. Press **Ctrl+Shift+V** and write down the order of the items (for example: third, second, first).
3. Click the item `second` (or select it and press **Enter**). It is pasted into Notepad.
4. Press **Ctrl+Shift+V** again.

Expected: each text appears **once** in the list. There is no second `second`. The order
is the same as in step 2.

### 12. Copying from an app that holds the clipboard open (Excel) does not crash or freeze Clipsy

Steps:
1. Open **Microsoft Excel**. Type `1`, `2`, `3`, `4`, `5` into cells A1 to A5.
2. Select A1:A5 and press **Ctrl+C**. Press **Esc**.
3. Do step 2 five times quickly: select cells, copy, press **Esc**.
4. Press **Ctrl+Shift+V** within a few seconds.

Expected: Excel does not freeze. Clipsy does not close and does not show a "Not responding"
message. The popup opens. Items copied from Excel appear in the list (they may be missing
if Excel was very busy; that is acceptable, but Clipsy must keep running).
If you run Clipsy with logs on, a line such as `reading clipboard failed` may appear. That is fine.

### 13. Pasting into an administrator window does not crash Clipsy

Steps:
1. Press the **Windows** key, type `notepad`. Right-click **Notepad** and choose
   **Run as administrator**. Click **Yes** if asked.
2. In that Notepad, click inside the text area.
3. In a **normal** Notepad, copy the text `admin test`.
4. Switch to the admin Notepad, press **Ctrl+Shift+V**, select the newest item, press **Enter**.
5. Press **Ctrl+Shift+V** again in the admin Notepad to check the popup still opens.
6. Close the admin Notepad.

Expected: Clipsy does **not** crash and its tray icon stays. The text may **not** paste into the
administrator window. This is a known limit (see the README). Write down whether the text pasted.
If logs are on, copy any line that mentions "paste" or "SendInput" into your report.

### 14. Target window closed before pressing Enter: no crash, no typing into the popup

Steps:
1. Open **Notepad**. Click inside it.
2. Press **Ctrl+Shift+V** to open the popup.
3. While the popup is open, right-click the Notepad button on the taskbar and click **Close window**.
   If the popup closes by itself, go to step 4 anyway.
4. Press **Enter** (on the popup if it is still open).
5. Press **Ctrl+Shift+V** to open the popup again.

Expected: Clipsy does not crash. The search box of the popup stays empty. No stray text
appears in the popup. Ctrl+Shift+V still opens the popup afterwards.

### 15. An already-used hotkey shows a clear error, and the old hotkey still works

Steps:
1. Right-click the tray icon, click **Settings**.
2. Try to set the shortcut to **Win+V** (or to any shortcut another running app already uses).
   If Settings has no shortcut field, do this instead:
   - Quit Clipsy (`.\target\release\clipsy-windows.exe quit`).
   - Open Notepad, choose **File > Open**, type `%APPDATA%\clipsy\config.toml` in the file name box and press **Enter**. In the `[shortcuts]` section, change `toggle_popup` to `"<Super>v"`. Save with **Ctrl+S**.
   - Start Clipsy again.
3. Read the message that Clipsy shows.
4. Close Settings. Press **Ctrl+Shift+V**.

Expected: a clear message says the shortcut cannot be used or is already taken. Clipsy keeps
running and the tray works. **Ctrl+Shift+V** still opens the popup.
If the Win+V shortcut appears to work, write that down.
Afterwards, set `toggle_popup` back to `"<Ctrl><Shift>v"` in `config.toml` and restart Clipsy.

### 16. The `toggle` command opens and closes the popup

Steps:
1. Keep Clipsy running. Open a **second** PowerShell window (normal, not "Run as administrator").
2. Go to the Clipsy folder and run:

   ```powershell
   .\target\release\clipsy-windows.exe toggle
   ```

3. Run the same command again.

Expected: the first `toggle` opens the popup. The second `toggle` closes it.

### 17. Pause label is right after a restart when paused is set in config.toml

Steps:
1. Quit Clipsy:

   ```powershell
   .\target\release\clipsy-windows.exe quit
   ```

2. Open `%APPDATA%\clipsy\config.toml` in Notepad. If the file does not exist, create it:
   - Open Notepad, type the lines below, then choose **File > Save as**.
   - In the address bar type `%APPDATA%\clipsy` and press **Enter**. Create the `clipsy` folder if it asks.
   - File name: `config.toml`. Encoding: **UTF-8**. Click **Save**.

   ```toml
   [privacy]
   paused = true
   ```

3. Save the file and start Clipsy again.
4. Right-click the tray icon.
5. Copy the text `paused test` in Notepad, then press **Ctrl+Shift+V**.
6. Quit Clipsy, set `paused = false` in `config.toml`, and start Clipsy again.

Expected: after the restart with `paused = true`, the tray menu shows **Resume capture** (not
"Pause capture"). The text `paused test` is **not** in the list.

### 18. Text copied from Word or Excel has no garbage characters at the end

Steps:
1. Open **Microsoft Word** (or WordPad). Type `Clipsy NUL test`, select it, press **Ctrl+C**.
2. In Excel, type `Clipsy cell test` in one cell, select it, press **Ctrl+C**.
3. Press **Ctrl+Shift+V**. Select the `Clipsy NUL test` item and press **Enter** into Notepad.
4. In Notepad, press **Ctrl+End**.

Expected: the pasted text ends right after the word `test`. There are no boxes, no odd
symbols and no spaces-and-symbols after the last letter. The cursor after **Ctrl+End**
sits directly after `test`. Repeat with the `Clipsy cell test` item.

### 19. The popup closes when you click another window, and is not in the taskbar

Steps:
1. Press **Ctrl+Shift+V** to open the popup.
2. Look at the taskbar. Clipsy's popup must not have its own button there.
3. Click on a part of another window (for example, the Notepad window) outside the popup.

Expected: the popup disappears as soon as you click the other window. No Clipsy button
appears in the taskbar for the popup.

---

## Report back

Send us the following in one message:

1. Your **Windows version** (from `winver`, for example "Windows 11, version 24H2").
2. For each check from **1 to 19**: **pass** or **fail**. For a fail, say what you saw.
3. For any error message: a screenshot, or copy the text of the message.
4. If you ran Clipsy with logs on (`$env:RUST_LOG = "debug"`): copy the lines from the
   PowerShell window that mention `warn`, `error` or `paste`. Tell us if the log lines
   were shown in the PowerShell window, or where you saw them.
5. If something crashed or froze: say which check you were doing, and whether Clipsy
   closed or the tray icon disappeared.
