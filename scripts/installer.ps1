$ErrorActionPreference = "Stop"
$root = Resolve-Path (Join-Path $PSScriptRoot "..")
$dll = Join-Path $root "target\release\obs_gt_title.dll"
if (-not (Test-Path $dll)) {
    throw "Release build not found. Run: cargo build -p obs-gt-title --release"
}

$version = if ($env:GT_TITLE_VERSION) { $env:GT_TITLE_VERSION } else { "0.1.0" }
if ($version -notmatch '^\d+\.\d+\.\d+$') {
    throw "GT_TITLE_VERSION must look like X.Y.Z. Got: $version"
}

$iscc = @(
    "${env:ProgramFiles(x86)}\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe",
    "$env:LOCALAPPDATA\Programs\Inno Setup 6\ISCC.exe"
) | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $iscc) {
    throw "Inno Setup 6 (ISCC.exe) was not found."
}

$dllArg = (Resolve-Path $dll).Path.Replace('\', '/')
$iss = Join-Path $root "installer\obs-gt-title.iss"
& $iscc "/DMyAppVersion=$version" "/DDllSource=$dllArg" $iss
if ($LASTEXITCODE -ne 0) {
    throw "ISCC exited with $LASTEXITCODE"
}

$setup = Join-Path $root "dist\obs-gt-title-windows\obs-gt-title-$version-windows-setup.exe"
if (-not (Test-Path $setup)) {
    throw "Installer was not written to $setup"
}
Write-Host "Built $setup"
