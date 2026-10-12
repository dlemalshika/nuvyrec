; Build through tools/build-windows-installer.ps1, using a verified full bundle.
#ifndef BundleDir
  #error BundleDir must name the packaged Windows bundle
#endif
#ifndef ReleaseVersion
  #error ReleaseVersion must be provided
#endif
#ifndef OutputDir
  #error OutputDir must be provided
#endif

[Setup]
; Keep this ID stable across upgrades, including application display-name changes.
AppId={{7CFD2025-7177-4616-AE47-46BD74DA390A}
AppName=Nuvyrec
AppVersion={#ReleaseVersion}
AppPublisher=Nuvyrec contributors
DefaultDirName={localappdata}\Programs\Nuvyrec
DefaultGroupName=Nuvyrec
DisableProgramGroupPage=yes
DisableDirPage=no
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
MinVersion=10.0.18362
OutputDir={#OutputDir}
OutputBaseFilename=Nuvyrec-Windows-x64-v{#ReleaseVersion}-Setup
SetupIconFile=installer_icon.ico
UninstallDisplayIcon={app}\Nuvyrec.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
; Never automatically terminate an active recording during an upgrade.
CloseApplications=no
RestartApplications=no
LicenseFile={#BundleDir}\LICENSE
InfoBeforeFile=installer-info.txt

[Tasks]
Name: "desktopicon"; Description: "Create a &desktop shortcut"; GroupDescription: "Shortcuts:"; Flags: unchecked

[Files]
Source: "{#BundleDir}\*"; DestDir: "{app}"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\Nuvyrec"; Filename: "{app}\Nuvyrec.exe"; WorkingDir: "{app}"
Name: "{autodesktop}\Nuvyrec"; Filename: "{app}\Nuvyrec.exe"; WorkingDir: "{app}"; Tasks: desktopicon

[InstallDelete]
; Remove only the retired executable, after the in-use check below.
Type: files; Name: "{app}\Frame.exe"

[Run]
Filename: "{app}\Nuvyrec.exe"; Description: "Open Nuvyrec"; WorkingDir: "{app}"; Flags: nowait postinstall skipifsilent

[Code]
// Exclusive write access fails while Windows has this executable mapped.
// This also prevents uninstall from interrupting a running recorder.
function CreateFileW(Name: String; Access, Share: Cardinal; Security: Integer;
  Creation, Attributes: Cardinal; Template: Integer): Integer;
  external 'CreateFileW@kernel32.dll stdcall';
function CloseHandle(Handle: Integer): Boolean;
  external 'CloseHandle@kernel32.dll stdcall';

function ExecutableInUse(AppFile: String): Boolean;
var
  Handle: Integer;
begin
  Result := False;
  if not FileExists(AppFile) then exit;
  Handle := CreateFileW(AppFile, $40000000, 0, 0, 3, $80, 0);
  Result := Handle = -1;
  if Handle <> -1 then CloseHandle(Handle);
end;

function AppFileInUse(): Boolean;
begin
  Result := ExecutableInUse(ExpandConstant('{app}\Nuvyrec.exe')) or
    ExecutableInUse(ExpandConstant('{app}\Frame.exe'));
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if AppFileInUse() then
    Result := 'Save and stop your recording, then exit Nuvyrec before installing. If Nuvyrec is already closed, check that this folder is writable.';
end;

function InitializeUninstall(): Boolean;
begin
  Result := not AppFileInUse();
  if not Result then
    MsgBox('Save and stop your recording, then exit Nuvyrec before uninstalling. If Nuvyrec is already closed, check folder permissions.', mbError, MB_OK);
end;
