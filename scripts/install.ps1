$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$dll = Join-Path $root "target\release\obs_gt_title.dll"
if (-not (Test-Path $dll)) {
    throw "Release build not found. Run: cargo build -p obs-gt-title --release"
}
$destDir = Join-Path $env:ProgramFiles "obs-studio\obs-plugins\64bit"
New-Item -ItemType Directory -Force -Path $destDir | Out-Null
Copy-Item $dll (Join-Path $destDir "obs_gt_title.dll") -Force
Write-Host "Installed obs_gt_title.dll to $destDir"
