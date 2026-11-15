# Builds the OFX bundle into dist/

param(
    [string]$Profile = "release",
    [string]$Configuration = ""
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if (-not $Configuration) {
    $Configuration = if ($Profile -eq "release") { "release" } else { "debug" }
}

Write-Host "building wsmb-ofx ($Configuration)..." -ForegroundColor Cyan
cargo build -p wsmb-ofx --profile $Profile
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$dll = Join-Path $root "target\$Configuration\wsmb_ofx.dll"
if (-not (Test-Path $dll)) { throw "missing $dll" }

$bundle = Join-Path $root "dist\WSMB.ofx.bundle\Contents"
$win64 = Join-Path $bundle "Win64"
New-Item -ItemType Directory -Force -Path $win64 | Out-Null

Copy-Item (Join-Path $root "bundle\Contents\Info.plist") (Join-Path $bundle "Info.plist") -Force
Copy-Item $dll (Join-Path $win64 "WSMB.ofx") -Force

$ofx = Join-Path $win64 "WSMB.ofx"
$size = [math]::Round((Get-Item $ofx).Length / 1KB, 1)
Write-Host "bundle ready: $ofx ($size KB)" -ForegroundColor Green

# Report the entry points so a broken export is obvious right away.
$dumpbin = Get-ChildItem "C:\Program Files*\Microsoft Visual Studio\2022\*\VC\Tools\MSVC\*\bin\Hostx64\x64\dumpbin.exe" -ErrorAction SilentlyContinue | Select-Object -First 1
if ($dumpbin) {
    & $dumpbin.FullName /EXPORTS $ofx | Select-String "Ofx"
} else {
    Write-Host "dumpbin not found, skipping export check" -ForegroundColor Yellow
}
