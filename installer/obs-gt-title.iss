; Windows installer for the GT Title OBS plugin.
; Version and DLL path can be overridden: ISCC /DMyAppVersion=1.2.3 /DDllSource=...\obs_gt_title.dll

#ifndef MyAppVersion
  #define MyAppVersion "0.1.0"
#endif

#ifndef DllSource
  #define DllSource "..\target\release\obs_gt_title.dll"
#endif

[Setup]
AppId={{7E4A9C21-6B38-4F05-9D14-2C8E5A7B0F63}
AppName=GT Title for OBS Studio
AppVersion={#MyAppVersion}
AppPublisher=MikanseiLaboratory
AppPublisherURL=https://github.com/MikanseiLaboratory/obs-gt-title
AppSupportURL=https://github.com/MikanseiLaboratory/obs-gt-title/issues
AppUpdatesURL=https://github.com/MikanseiLaboratory/obs-gt-title/releases
VersionInfoVersion={#MyAppVersion}.0
VersionInfoProductName=GT Title for OBS Studio
DefaultDirName={commonappdata}\obs-gt-title
DisableDirPage=yes
DisableProgramGroupPage=yes
UninstallFilesDir={commonappdata}\obs-gt-title
OutputDir=..\dist\obs-gt-title-windows
OutputBaseFilename=obs-gt-title-{#MyAppVersion}-windows-setup
Compression=lzma2
SolidCompression=yes
ArchitecturesAllowed=x64compatible
ArchitecturesInstallIn64BitMode=x64compatible
PrivilegesRequired=admin
MinVersion=10.0
WizardStyle=modern
LicenseFile=..\LICENSE
CloseApplications=yes
RestartApplications=no

[Languages]
Name: "en"; MessagesFile: "compiler:Default.isl"
Name: "ja"; MessagesFile: "compiler:Languages\Japanese.isl"

[Files]
Source: "{#DllSource}"; DestDir: "{code:ObsPluginDir}"; DestName: "obs_gt_title.dll"; Flags: ignoreversion

[Messages]
en.FinishedLabel=GT Title was copied into the OBS 64-bit plugin folder.%nRestart OBS, then add a GT Title source.
ja.FinishedLabel=GT Title を OBS の 64bit プラグインフォルダへコピーしました。%nOBS を再起動すると、ソース一覧に GT Title が出ます。

[Code]
function ObsPluginDir(Param: string): string;
var
  Root: string;
begin
  Root := '';
  if RegQueryStringValue(HKLM64, 'SOFTWARE\OBS Studio', '', Root) and (Root <> '') then
    Result := AddBackslash(Root) + 'obs-plugins\64bit'
  else
    Result := ExpandConstant('{autopf}\obs-studio\obs-plugins\64bit');
end;
