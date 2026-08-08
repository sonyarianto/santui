; Inno Setup script for Santui.
; Build with:
;   ISCC.exe scripts/winget/santui.iss /DMyAppVersion=0.2.39
; The staging directory (containing santui.exe + plugin binaries) must exist
; next to this script under `staging/`.

#ifndef MyAppVersion
  #define MyAppVersion "0.0.0"
#endif
#ifndef SourceDir
  #define SourceDir "staging"
#endif
#ifndef OutputDir
  #define OutputDir "staging-installer"
#endif

#define MyAppName "Santui"
#define MyAppPublisher "Sony Arianto"
#define MyAppURL "https://github.com/sonyarianto/santui"
#define MyAppExeName "santui.exe"

[Setup]
AppId={{4976E2FC-FBE5-432D-91CD-B9EBB436B684}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
DefaultDirName={localappdata}\santui\current
DisableProgramGroupPage=yes
PrivilegesRequired=lowest
OutputDir={#OutputDir}
OutputBaseFilename=santui-setup
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ChangesEnvironment=yes
ArchitecturesInstallIn64BitMode=x64compatible
ArchitecturesAllowed=x64compatible
UninstallDisplayName={#MyAppName}
UninstallDisplayIcon={app}\{#MyAppExeName}

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked

[Files]
Source: "{#SourceDir}\santui.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\santui-*.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#SourceDir}\*.dll"; DestDir: "{app}"; Flags: ignoreversion skipifsourcedoesntexist
Source: "{#SourceDir}\native\*"; DestDir: "{app}\native"; Flags: ignoreversion recursesubdirs skipifsourcedoesntexist

[Registry]
Root: HKCU; Subkey: "Environment"; ValueType: expandsz; ValueName: "Path"; ValueData: "{olddata};{app}"; Check: NeedsAddPath

[Icons]
Name: "{autoprograms}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Code]
function NeedsAddPath: Boolean;
var
  CurrentPath: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', CurrentPath) then
    Result := True
  else
    Result := Pos(';' + ExpandConstant('{app}'), ';' + CurrentPath) = 0;
end;

procedure RemovePath(PathToRemove: string);
var
  CurrentPath, NewPath, Entry, Delimiter: string;
begin
  if not RegQueryStringValue(HKCU, 'Environment', 'Path', CurrentPath) then
    Exit;
  NewPath := '';
  while True do begin
    Delimiter := ';';
    Entry := CurrentPath;
    if Pos(Delimiter, Entry) > 0 then begin
      Entry := Copy(Entry, 1, Pos(Delimiter, Entry) - 1);
      CurrentPath := Copy(CurrentPath, Pos(Delimiter, CurrentPath) + 1, Length(CurrentPath));
    end else begin
      CurrentPath := '';
    end;
    if CompareText(Entry, PathToRemove) <> 0 then begin
      if NewPath <> '' then
        NewPath := NewPath + ';';
      NewPath := NewPath + Entry;
    end;
    if CurrentPath = '' then
      Break;
  end;
  RegWriteExpandStringValue(HKCU, 'Environment', 'Path', NewPath);
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if CurUninstallStep = usUninstall then
    RemovePath(ExpandConstant('{app}'));
end;
