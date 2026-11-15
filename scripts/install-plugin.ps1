# Copies the built bundle into the machine wide OFX plugin folder.
# Needs an elevated shell.

param(
    [string]$Source = "",
    [string]$Destination = "C:\Program Files\Common Files\OFX\Plugins"
)

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

if (-not $Source) {
    $Source = Join-Path $root "dist\WSMB.ofx.bundle"
}
if (-not (Test-Path $Source)) {
    throw "bundle not found: $Source (run scripts/build-plugin.ps1 first)"
}

$target = Join-Path $Destination "WSMB.ofx.bundle"
Write-Host "installing to $target" -ForegroundColor Cyan

if (Test-Path $target) {
    Remove-Item -Recurse -Force $target
}
New-Item -ItemType Directory -Force -Path $Destination | Out-Null
Copy-Item $Source $target -Recurse -Force

Write-Host "done. restart the host so it rescans plugins." -ForegroundColor Green
