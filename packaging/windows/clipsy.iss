; Inno Setup script for the Clipsy Windows installer.
;
; Built by .github/workflows/release.yml with:
;   ISCC.exe /DAppVersion=<version> /DSourceExe=<path to clipsy.exe> /O<output dir> clipsy.iss
; The output is clipsy-<version>-setup.exe.
;
; The installer does not need administrator rights. It installs for the current
; user by default, and offers an all-users install if you choose it.
; Clipsy starts at login through its own settings (a Run key it writes itself),
; so this script does not add one.

#ifndef AppVersion
  #define AppVersion "0.0.0-dev"
#endif
#ifndef SourceExe
  #error SourceExe must be passed on the command line: /DSourceExe=path\to\clipsy.exe
#endif

[Setup]
; Fixed identifier for Clipsy's installs. Do not change it, or upgrades will not
; find the old version.
AppId={{0FA2A6F6-9CE7-4FBD-B3D9-B709CB63D0D2}
AppName=Clipsy
AppVersion={#AppVersion}
AppVerName=Clipsy {#AppVersion}
AppPublisher=venkyeswar
AppPublisherURL=https://github.com/venkyeswar/clipsy
AppSupportURL=https://github.com/venkyeswar/clipsy/issues
AppUpdatesURL=https://github.com/venkyeswar/clipsy/releases
DefaultDirName={autopf}\Clipsy
DefaultGroupName=Clipsy
DisableProgramGroupPage=yes
; Per-user install by default and no administrator rights needed. The user can
; choose an all-users install (Program Files) in the dialog.
PrivilegesRequired=lowest
PrivilegesRequiredOverridesAllowed=dialog
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0
LicenseFile=..\..\LICENSE
UninstallDisplayName=Clipsy
UninstallDisplayIcon={app}\clipsy.exe
OutputBaseFilename=clipsy-{#AppVersion}-setup
OutputDir=..\..\dist
Compression=lzma2/max
SolidCompression=yes
WizardStyle=modern
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Files]
Source: "{#SourceExe}"; DestDir: "{app}"; DestName: "clipsy.exe"; Flags: ignoreversion

[Icons]
Name: "{group}\Clipsy"; Filename: "{app}\clipsy.exe"
Name: "{group}\Uninstall Clipsy"; Filename: "{uninstallexe}"

[Run]
Filename: "{app}\clipsy.exe"; Description: "Start Clipsy"; Flags: nowait postinstall skipifsilent

[Code]
// Remove the start-at-login entry that Clipsy's settings may have written,
// so uninstalling does not leave a Run key that points at a deleted file.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usPostUninstall then
  begin
    RegDeleteValue(HKEY_CURRENT_USER, 'Software\Microsoft\Windows\CurrentVersion\Run', 'Clipsy');
  end;
end;
