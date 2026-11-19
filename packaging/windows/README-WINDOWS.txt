Clipsy for Windows (portable)
=============================

Clipsy keeps a history of what you copy: text, images and files.
Press Ctrl+Shift+V to open the history, pick an item and press Enter to paste it again.

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

Clipsy starts again at login when you turn that on in its settings.
To remove Clipsy, quit it, then delete this folder and the two folders above.

The licence is in LICENSE.txt (GPL-3.0-or-later).
