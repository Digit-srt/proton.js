#ifndef PayloadDir
  #define PayloadDir "..\dist\installer-payload"
#endif

[Setup]
#ifdef ProtonPathTest
AppId={{D808AF69-5D12-4B98-8CEF-33166F61D2E6}
AppName=Proton.js Installer Test
DefaultDirName={src}\test-install
CreateUninstallRegKey=no
#else
AppId={{B73DCFEC-836D-4FBA-B1F0-43DA7BC74A58}
AppName=Proton.js
DefaultDirName={localappdata}\Programs\Proton.js
ChangesEnvironment=yes
#endif
AppVersion=0.2.0
AppPublisher=Digit-srt
AppPublisherURL=https://github.com/Digit-srt/proton.js
ArchitecturesAllowed=x64compatible
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
WizardStyle=modern
Compression=lzma2/normal
SolidCompression=yes
OutputDir=..\dist\installer
OutputBaseFilename=Proton-0.2.0-windows-x64-setup
UninstallDisplayIcon={app}\bin\Proton.exe

[Tasks]
Name: addtopath; Description: "Add Proton Batcher (pb) to my PATH"

[Files]
Source: "{#PayloadDir}\Proton.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#PayloadDir}\pb.exe"; DestDir: "{app}\bin"; Flags: ignoreversion
Source: "{#PayloadDir}\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion
Source: "{#PayloadDir}\notices\*"; DestDir: "{app}\notices"; Flags: ignoreversion recursesubdirs

#ifndef ProtonPathTest
[Icons]
Name: "{autoprograms}\Proton.js"; Filename: "{app}\bin\Proton.exe"
#endif

[Code]
const
#ifdef ProtonPathTest
  EnvironmentKey = 'Software\Proton.js\InstallerTest\Environment';
  SetupKey = 'Software\Proton.js\InstallerTest\Setup';
#else
  EnvironmentKey = 'Environment';
  SetupKey = 'Software\Proton.js\Setup';
#endif
  PathValue = 'Path';
  OwnedPathValue = 'PathEntry';
  CurrentUserHandle = $80000001;
  StringTypesWithoutExpansion = $10000006;
  RegistryString = 1;

function RegGetValue(Root: Cardinal; Key, Name: String; Flags: Cardinal;
  var ValueType: Cardinal; Data: Cardinal; var DataSize: Cardinal): Integer;
  external 'RegGetValueW@advapi32.dll stdcall';

function ReadPath(var Value: String): Boolean;
begin
  Value := '';
  Result := not RegValueExists(HKCU, EnvironmentKey, PathValue) or
    RegQueryStringValue(HKCU, EnvironmentKey, PathValue, Value);
end;

function WritePath(Value: String): Boolean;
var
  ValueType, DataSize: Cardinal;
begin
  DataSize := 0;
  if (RegGetValue(CurrentUserHandle, EnvironmentKey, PathValue,
    StringTypesWithoutExpansion, ValueType, 0, DataSize) = 0) and
    (ValueType = RegistryString) then
    Result := RegWriteStringValue(HKCU, EnvironmentKey, PathValue, Value)
  else
    Result := RegWriteExpandStringValue(HKCU, EnvironmentKey, PathValue, Value);
end;

function ContainsPath(Value, Directory: String): Boolean;
var
  Entries: TArrayOfString;
  Entry: String;
  Index: Integer;
begin
  Result := True;
  Entries := StringSplit(Value, [';'], stAll);
  for Index := 0 to GetArrayLength(Entries) - 1 do
  begin
    Entry := RemoveBackslashUnlessRoot(RemoveQuotes(Trim(Entries[Index])));
    if CompareText(Entry, Directory) = 0 then
      Exit;
  end;
  Result := False;
end;

function RemoveOwnedPath: Boolean;
var
  Owned, Value, Padded: String;
  Position: Integer;
begin
  Result := True;
  if not RegQueryStringValue(HKCU, SetupKey, OwnedPathValue, Owned) or (Owned = '') then
    Exit;

  Result := False;
  if not ReadPath(Value) then
    Exit;
  Padded := ';' + Value + ';';
  Position := Pos(';' + Owned + ';', Padded);
  if Position > 0 then
  begin
    Delete(Padded, Position, Length(Owned) + 1);
    if not WritePath(Copy(Padded, 2, Length(Padded) - 2)) then
      Exit;
  end;
  Result := RegDeleteValue(HKCU, SetupKey, OwnedPathValue);
end;

function AddPath(Directory: String): Boolean;
var
  Value, Owned: String;
begin
  Result := False;
  if RegQueryStringValue(HKCU, SetupKey, OwnedPathValue, Owned) and
    (Owned <> Directory) and not RemoveOwnedPath then
    Exit;
  if not ReadPath(Value) then
    Exit;
  Result := True;
  if ContainsPath(Value, Directory) then
    Exit;
  if Value <> '' then
    Value := Value + ';';

  Result := RegWriteStringValue(HKCU, SetupKey, OwnedPathValue, Directory);
  if Result then
  begin
    Result := WritePath(Value + Directory);
    if not Result then
      RegDeleteValue(HKCU, SetupKey, OwnedPathValue);
  end;
end;

function PrepareToInstall(var NeedsRestart: Boolean): String;
begin
  Result := '';
  if WizardIsTaskSelected('addtopath') and (Pos(';', ExpandConstant('{app}')) > 0) then
    Result := 'Choose an installation folder without semicolons to add Proton Batcher to PATH.';
end;

procedure CurStepChanged(CurStep: TSetupStep);
var
  Updated: Boolean;
begin
  if CurStep <> ssPostInstall then
    Exit;
  if WizardIsTaskSelected('addtopath') then
    Updated := AddPath(ExpandConstant('{app}\bin'))
  else
    Updated := RemoveOwnedPath;
  if not Updated then
  begin
    Log('Could not update the current user PATH.');
    SuppressibleMsgBox('Setup could not update your PATH.', mbError, MB_OK, IDOK);
  end;
end;

procedure CurUninstallStepChanged(CurUninstallStep: TUninstallStep);
begin
  if (CurUninstallStep = usUninstall) and not RemoveOwnedPath then
  begin
    Log('Could not remove the Proton Batcher entry from the current user PATH.');
    SuppressibleMsgBox('Uninstall could not remove Proton Batcher from your PATH.', mbError, MB_OK, IDOK);
  end;
end;
