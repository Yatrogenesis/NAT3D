; NAT3D Installer Script for Inno Setup
;
; Independent of GitHub - run locally on Windows:
;   1. Install Inno Setup: https://jrsoftware.org/isinfo.php
;   2. cargo build --release -p nat3d-app
;   3. Open this file in Inno Setup Compiler
;   4. Click "Compile" -> produces NAT3D-Setup.exe
;
; SPDX-License-Identifier: AGPL-3.0-or-later
; Copyright (C) 2026 Francisco Molina-Burgos, Avermex Research Division

#define MyAppName "NAT3D"
#define MyAppVersion "0.1.0"
#define MyAppPublisher "Avermex Research Division"
#define MyAppURL "https://github.com/Yatrogenesis/NAT3D"
#define MyAppExeName "nat3d-app.exe"
#define MyAppDescription "Professional 3D Modeling, CAD, Physics Simulation and Rendering Suite"

[Setup]
; NOTE: AppId uniquely identifies this app. Do not use the same AppId in other installers.
AppId={{A7B3C4D5-E6F7-8901-2345-6789ABCDEF01}
AppName={#MyAppName}
AppVersion={#MyAppVersion}
AppVerName={#MyAppName} {#MyAppVersion}
AppPublisher={#MyAppPublisher}
AppPublisherURL={#MyAppURL}
AppSupportURL={#MyAppURL}
AppUpdatesURL={#MyAppURL}
DefaultDirName={autopf}\{#MyAppName}
DefaultGroupName={#MyAppName}
AllowNoIcons=yes
; Output directory relative to this .iss file
OutputDir=..\..\target\installer
OutputBaseFilename=NAT3D-{#MyAppVersion}-Setup
; Compression settings
Compression=lzma2/ultra64
SolidCompression=yes
LZMAUseSeparateProcess=yes
; Modern installer look
WizardStyle=modern
; Require admin for Program Files installation
PrivilegesRequired=admin
PrivilegesRequiredOverridesAllowed=dialog
; Windows version requirements (Windows 10+)
MinVersion=10.0
; Uninstaller settings
UninstallDisplayIcon={app}\{#MyAppExeName}
UninstallDisplayName={#MyAppName}
; License file (optional - uncomment if you have one)
; LicenseFile=..\..\LICENSE

[Languages]
Name: "english"; MessagesFile: "compiler:Default.isl"
Name: "spanish"; MessagesFile: "compiler:Languages\Spanish.isl"

[Tasks]
Name: "desktopicon"; Description: "{cm:CreateDesktopIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked
Name: "quicklaunchicon"; Description: "{cm:CreateQuickLaunchIcon}"; GroupDescription: "{cm:AdditionalIcons}"; Flags: unchecked; OnlyBelowVersion: 6.1; Check: not IsAdminInstallMode

[Files]
; Main executable
Source: "..\..\target\release\nat3d-app.exe"; DestDir: "{app}"; Flags: ignoreversion

; Shader files (if any exist in assets)
; Source: "..\..\assets\shaders\*"; DestDir: "{app}\shaders"; Flags: ignoreversion recursesubdirs createallsubdirs; Check: DirExists(ExpandConstant('..\..\assets\shaders'))

; Example scenes (optional)
; Source: "..\..\examples\*"; DestDir: "{app}\examples"; Flags: ignoreversion recursesubdirs createallsubdirs

[Icons]
Name: "{group}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Comment: "{#MyAppDescription}"
Name: "{group}\{cm:UninstallProgram,{#MyAppName}}"; Filename: "{uninstallexe}"
Name: "{autodesktop}\{#MyAppName}"; Filename: "{app}\{#MyAppExeName}"; Tasks: desktopicon; Comment: "{#MyAppDescription}"

[Run]
Filename: "{app}\{#MyAppExeName}"; Description: "{cm:LaunchProgram,{#StringChange(MyAppName, '&', '&&')}}"; Flags: nowait postinstall skipifsilent

[Registry]
; File associations for .nat files (NAT3D native format)
Root: HKA; Subkey: "Software\Classes\.nat"; ValueType: string; ValueName: ""; ValueData: "NAT3D.Project"; Flags: uninsdeletevalue
Root: HKA; Subkey: "Software\Classes\NAT3D.Project"; ValueType: string; ValueName: ""; ValueData: "NAT3D Project File"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Classes\NAT3D.Project\DefaultIcon"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName},0"
Root: HKA; Subkey: "Software\Classes\NAT3D.Project\shell\open\command"; ValueType: string; ValueName: ""; ValueData: """{app}\{#MyAppExeName}"" ""%1"""

; Register application path for easier command-line access
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\nat3d-app.exe"; ValueType: string; ValueName: ""; ValueData: "{app}\{#MyAppExeName}"; Flags: uninsdeletekey
Root: HKA; Subkey: "Software\Microsoft\Windows\CurrentVersion\App Paths\nat3d-app.exe"; ValueType: string; ValueName: "Path"; ValueData: "{app}"

[Code]
// Check if Visual C++ Redistributable is installed (required for Rust binaries)
function VCRedistInstalled: Boolean;
var
  Version: String;
begin
  Result := RegQueryStringValue(HKLM, 'SOFTWARE\Microsoft\VisualStudio\14.0\VC\Runtimes\x64', 'Version', Version) or
            RegQueryStringValue(HKLM, 'SOFTWARE\WOW6432Node\Microsoft\VisualStudio\14.0\VC\Runtimes\x64', 'Version', Version);
end;

function InitializeSetup: Boolean;
var
  ResultCode: Integer;
begin
  Result := True;

  if not VCRedistInstalled then
  begin
    if MsgBox('NAT3D requires the Microsoft Visual C++ Redistributable.'#13#10#13#10 +
              'Would you like to download it now?'#13#10#13#10 +
              '(You can also install NAT3D first and install the redistributable later)',
              mbConfirmation, MB_YESNO) = IDYES then
    begin
      ShellExec('open', 'https://aka.ms/vs/17/release/vc_redist.x64.exe', '', '', SW_SHOW, ewNoWait, ResultCode);
    end;
  end;
end;
