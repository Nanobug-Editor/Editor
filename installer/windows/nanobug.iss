; Invoke ISCC directly with PayloadDir, InstallerDir and AppVersion; no helper scripts are called.
#ifndef PayloadDir
  #error PayloadDir must point to a prepared Nanobug distribution
#endif
#ifndef InstallerDir
  #error InstallerDir must name the installer output directory
#endif
#ifndef AppVersion
  #error AppVersion must match Cargo.toml
#endif

[Setup]
; Keep this identity stable so subsequent versions upgrade the same per-user application.
AppId=Nanobug.Editor
AppName=Nanobug
AppVersion={#AppVersion}
AppPublisher=Nanobug contributors
AppPublisherURL=https://github.com/Nanobug-Editor/Editor
DefaultDirName={localappdata}\Programs\Nanobug
DefaultGroupName=Nanobug
PrivilegesRequired=lowest
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
OutputDir={#InstallerDir}
OutputBaseFilename=Nanobug-Setup-{#AppVersion}-x64
SetupIconFile={#PayloadDir}\Nanobug.ico
UninstallDisplayIcon={app}\Nanobug.exe
UninstallDisplayName=Nanobug
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
DisableProgramGroupPage=yes
; Reinstall and upgrade must retain the user's previous integration choice.
UsePreviousTasks=yes
; The GUI holds this mutex. Never force-close a document or restart the editor during an upgrade.
AppMutex=Local\Nanobug.Installer.Running
CloseApplications=no
RestartApplications=no
SetupLogging=yes

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "chinesesimplified"; MessagesFile: "languages\ChineseSimplified.isl"

[Tasks]
; Desktop integration is optional; no file associations or PATH changes are applied silently.
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
; Checked on a first install; existing installations retain their explicit choice.
Name: "contextmenu"; Description: "{cm:AddProjectMenus}"; GroupDescription: "{cm:ProjectIntegration}"

[CustomMessages]
english.ProjectIntegration=File Explorer integration:
english.AddProjectMenus=Add "Open with Nanobug" to folder and folder background menus
english.OpenWithNanobug=Open with Nanobug
english.MenuConflict=The Nanobug menu registration belongs to another program. Uncheck File Explorer integration to continue without changing it.
english.MenuCleanupFailed=The Nanobug menu registration could not be removed. Close other tools using it and try again.
chinesesimplified.ProjectIntegration=文件资源管理器集成：
chinesesimplified.AddProjectMenus=在文件夹及文件夹空白处添加“使用 Nanobug 打开”
chinesesimplified.OpenWithNanobug=使用 Nanobug 打开
chinesesimplified.MenuConflict=Nanobug 菜单注册项由其他程序占用。请取消资源管理器集成选项，再继续安装；已有注册项将保留。
chinesesimplified.MenuCleanupFailed=无法移除 Nanobug 菜单注册项。请关闭正在使用它的其他工具后重试。

[Registry]
; Own only these two private HKCU verb keys. Uninstall uses checked ownership and preserves unknown data.
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Nanobug"; ValueType: string; ValueData: "{cm:OpenWithNanobug}"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Nanobug"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\Nanobug.exe"",0"; Tasks: contextmenu
; One last-deleted marker preserves both identity and installation path through any partial cleanup failure.
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Nanobug"; ValueType: string; ValueName: "NanobugOwner"; ValueData: "Nanobug.Editor|{app}"; Tasks: contextmenu
; A trailing dot keeps drive-root backslashes from escaping the final command-line quote.
Root: HKCU; Subkey: "Software\Classes\Directory\shell\Nanobug\command"; ValueType: string; ValueData: """{app}\Nanobug.exe"" -- ""%1\."""; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\Nanobug"; ValueType: string; ValueData: "{cm:OpenWithNanobug}"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\Nanobug"; ValueType: string; ValueName: "Icon"; ValueData: """{app}\Nanobug.exe"",0"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\Nanobug"; ValueType: string; ValueName: "NanobugOwner"; ValueData: "Nanobug.Editor|{app}"; Tasks: contextmenu
Root: HKCU; Subkey: "Software\Classes\Directory\Background\shell\Nanobug\command"; ValueType: string; ValueData: """{app}\Nanobug.exe"" -- ""%V\."""; Tasks: contextmenu

[Files]
Source: "{#PayloadDir}\Nanobug.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\Nanobug.ico"; DestDir: "{app}"; Flags: ignoreversion
; Plugins live in the private user store and are acquired explicitly from the reviewed marketplace.
Source: "{#PayloadDir}\licenses\*"; DestDir: "{app}\licenses"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{userprograms}\Nanobug"; Filename: "{app}\Nanobug.exe"; WorkingDir: "{userdocs}"
Name: "{userdesktop}\Nanobug"; Filename: "{app}\Nanobug.exe"; WorkingDir: "{userdocs}"; Tasks: desktopicon

; User settings, installed plugins, history and caches remain outside {app} and survive uninstall.

[Code]
const
  DirectoryMenu = 'Software\Classes\Directory\shell\Nanobug';
  BackgroundMenu = 'Software\Classes\Directory\Background\shell\Nanobug';

// Inno Setup 6 runs a 32-bit installer even in x64 install mode; these pointer arguments are always null.
// Broadcast only association changes; there is no Explorer restart or external helper process.
procedure SHChangeNotify(EventID: Integer; Flags: Cardinal; Item1, Item2: Integer);
  external 'SHChangeNotify@shell32.dll stdcall';

// Compare the exact registered executable and argument boundary before claiming menu ownership.
function MenuCommand(InstallDir: String; Background: Boolean): String;
var
  Target: String;
begin
  if Background then Target := '%V\.' else Target := '%1\.';
  Result := '"' + InstallDir + '\Nanobug.exe" -- "' + Target + '"';
end;

function OwnsMenu(Key: String; Background: Boolean; var InstallDir: String): Boolean;
var
  Owner, Command, Prefix: String;
begin
  Result := False;
  Prefix := 'Nanobug.Editor|';
  if not RegQueryStringValue(HKCU, Key, 'NanobugOwner', Owner) or
    (Pos(Prefix, Owner) <> 1) then Exit;
  InstallDir := Copy(Owner, Length(Prefix) + 1, Length(Owner));
  if InstallDir = '' then Exit;
  // A missing command with our intact marker is an interrupted cleanup, and must remain retryable.
  // An existing foreign or non-string command is never claimed even if a marker survived.
  Result := not RegValueExists(HKCU, Key + '\command', '') or
    (RegQueryStringValue(HKCU, Key + '\command', '', Command) and
    (CompareText(Command, MenuCommand(InstallDir, Background)) = 0));
end;

// Enumeration failures are errors, not proof that an unknown key is safe to modify.
function MenuKeyIsEmpty(Key: String): Boolean;
var
  Values, Subkeys: TArrayOfString;
begin
  Result := True;
  if not RegKeyExists(HKCU, Key) then Exit;
  if not RegGetValueNames(HKCU, Key, Values) or
    not RegGetSubkeyNames(HKCU, Key, Subkeys) then
    RaiseException(CustomMessage('MenuCleanupFailed'));
  Result := (GetArrayLength(Values) = 0) and (GetArrayLength(Subkeys) = 0);
end;

// Unknown keys must never be overwritten; opting out still allows an ordinary application install.
procedure CheckMenu(Key: String; Background: Boolean);
var
  InstallDir: String;
begin
  if RegKeyExists(HKCU, Key) and not OwnsMenu(Key, Background, InstallDir) and
    not MenuKeyIsEmpty(Key) then begin
    Log('Preserving conflicting menu registration: ' + Key);
    RaiseException(CustomMessage('MenuConflict'));
  end;
end;

// Remove only values written by this product, leaving unknown sibling values and subkeys intact.
procedure RemoveValue(Key, Name: String);
begin
  if RegValueExists(HKCU, Key, Name) and not RegDeleteValue(HKCU, Key, Name) then
    RaiseException(CustomMessage('MenuCleanupFailed'));
end;

// Retain unknown values and children intentionally, but report failure to delete an actually empty key.
procedure RemoveEmptyMenuKey(Key: String);
begin
  if RegKeyExists(HKCU, Key) and MenuKeyIsEmpty(Key) and
    not RegDeleteKeyIfEmpty(HKCU, Key) then
    RaiseException(CustomMessage('MenuCleanupFailed'));
end;

procedure RemoveMenu(Key: String; Background, Uninstalling: Boolean);
var
  InstallDir: String;
begin
  if not OwnsMenu(Key, Background, InstallDir) then Exit;
  // An older uninstaller must not remove a later installation's current verb.
  if Uninstalling and (CompareText(InstallDir, ExpandConstant('{app}')) <> 0) then Exit;
  RemoveValue(Key + '\command', '');
  RemoveEmptyMenuKey(Key + '\command');
  RemoveValue(Key, '');
  RemoveValue(Key, 'Icon');
  // Delete the sole ownership marker last: any earlier failure can be safely retried.
  RemoveValue(Key, 'NanobugOwner');
  try
    RemoveEmptyMenuKey(Key);
  except
    // KEY_SET_VALUE and DELETE permissions differ. Restore ownership if deleting the empty key failed.
    if not RegWriteStringValue(HKCU, Key, 'NanobugOwner', 'Nanobug.Editor|' + InstallDir) then
      Log('Failed to restore menu cleanup ownership: ' + Key);
    RaiseException(CustomMessage('MenuCleanupFailed'));
  end;
end;

// A deselected upgrade cleans its old verbs; selecting the task updates both commands to this install path.
procedure CurStepChanged(CurStep: TSetupStep);
begin
  if CurStep = ssInstall then begin
    if WizardIsTaskSelected('contextmenu') then begin
      CheckMenu(DirectoryMenu, False);
      CheckMenu(BackgroundMenu, True);
    end else begin
      RemoveMenu(DirectoryMenu, False, False);
      RemoveMenu(BackgroundMenu, True, False);
    end;
  end;
  if CurStep = ssPostInstall then SHChangeNotify($08000000, 0, 0, 0);
end;

// Installer-owned files and these checked values are the entire uninstall scope; profiles and projects are untouched.
procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then begin
    RemoveMenu(DirectoryMenu, False, True);
    RemoveMenu(BackgroundMenu, True, True);
    SHChangeNotify($08000000, 0, 0, 0);
  end;
end;
