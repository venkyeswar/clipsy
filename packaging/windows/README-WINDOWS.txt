Clipsy for Windows (portable)
=============================

Clipsy keeps a history of what you copy: text, images and files.

Clipsy has no main window. It lives in the system tray, the row of small icons
next to the clock at the bottom right of the screen. If you do not see the blue
Clipsy icon there, click the ^ arrow next to the clock to show hidden icons.

How to open your history:
  - Press Ctrl+Shift+V, pick an item and press Enter to paste it again.
  - Or click the Clipsy icon in the tray.
  - Or right-click the icon for the menu (Open, Pause, Settings, Clear history, Quit).

When you start Clipsy yourself, the history opens once so you can see it works.
Clipsy starts again by itself when you log in. You can turn that off in Settings.
You do not need to close anything; Clipsy keeps running in the background.

Start Clipsy
------------
1. Unzip this folder anywhere you like (for example to Documents\Clipsy).
2. Double-click clipsy.exe.
3. Windows may show "Windows protected your PC" because this program is not signed
   with a paid certificate yet. Click "More info", then "Run anyway".
   Clipsy only runs on your PC and makes no network connections.

Command line
------------
Open PowerShell or Command Prompt in the folder and run:

    clipsy.exe open        show the history popup
    clipsy.exe toggle      open or close the history popup
    clipsy.exe settings    open the settings window
    clipsy.exe pause       stop recording copies
    clipsy.exe resume      start recording copies again
    clipsy.exe clear       delete the history
    clipsy.exe quit        stop Clipsy

Where your data is kept
-----------------------
  Settings:  %APPDATA%\clipsy\config\config.toml
  History:   %LOCALAPPDATA%\clipsy\data
  Log:       %LOCALAPPDATA%\clipsy\logs\clipsy.log

Clipsy starts again at login by default. To turn that off, open Settings
from the tray icon and untick "Start Clipsy when I sign in".
To remove Clipsy, quit it, then delete this folder and the two folders above.

The licence is in LICENSE.txt (GPL-3.0-or-later).
