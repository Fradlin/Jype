; Inno Setup script. Built by GitHub Actions (or run ISCC.exe installer\Jype.iss after build.bat).
#define AppVer "0.3.0"

[Setup]
AppId={{6B1F3C2A-5D0E-4C7B-9A41-0A0B0C0D0E0F}
AppName=Jype
AppVersion={#AppVer}
AppVerName=Jype {#AppVer}
AppPublisher=Frad
AppPublisherURL=https://github.com/fradlin
DefaultDirName={localappdata}\Programs\Jype
DefaultGroupName=Jype
PrivilegesRequired=lowest
DisableProgramGroupPage=yes
OutputDir=..\dist
OutputBaseFilename=Jype-Setup-{#AppVer}
SetupIconFile=..\crates\app\assets\jype.ico
UninstallDisplayIcon={app}\Jype.exe
Compression=lzma2
SolidCompression=yes
WizardStyle=modern
ArchitecturesInstallIn64BitMode=x64compatible
VersionInfoCompany=Frad
VersionInfoDescription=Jype setup
VersionInfoVersion={#AppVer}

[Tasks]
Name: "autostart"; Description: "Start Jype with Windows"; Flags: checkedonce
Name: "desktopicon"; Description: "Create a desktop shortcut"; Flags: unchecked

[Files]
Source: "..\dist\Jype.exe"; DestDir: "{app}"; Flags: ignoreversion
Source: "..\dist\LICENSES\*"; DestDir: "{app}\LICENSES"; Flags: ignoreversion recursesubdirs
Source: "..\THIRD_PARTY_NOTICES.md"; DestDir: "{app}"; Flags: ignoreversion

[Icons]
Name: "{autoprograms}\Jype"; Filename: "{app}\Jype.exe"
Name: "{autodesktop}\Jype"; Filename: "{app}\Jype.exe"; Tasks: desktopicon

[Registry]
Root: HKCU; Subkey: "Software\Microsoft\Windows\CurrentVersion\Run"; ValueType: string; ValueName: "Jype"; ValueData: """{app}\Jype.exe"""; Tasks: autostart; Flags: uninsdeletevalue

[Run]
Filename: "{app}\Jype.exe"; Description: "Start Jype now"; Flags: nowait postinstall skipifsilent

[UninstallRun]
Filename: "{sys}\taskkill.exe"; Parameters: "/F /IM Jype.exe"; Flags: runhidden; RunOnceId: "StopJype"
