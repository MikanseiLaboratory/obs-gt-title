$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$dll = Join-Path $root "target\release\obs_gt_title.dll"
if (-not (Test-Path $dll)) {
    throw "Release build not found. Run: cargo build -p obs-gt-title --release"
}
$dest = Join-Path $root "dist\obs-gt-title-windows"
New-Item -ItemType Directory -Force -Path $dest | Out-Null
Copy-Item $dll (Join-Path $dest "obs_gt_title.dll") -Force
Write-Host "Packaged $dest"
