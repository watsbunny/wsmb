# Points the machine-wide OFX bundle at the repo's dist folder, so every rebuild
# is picked up without copying. Needs an elevated shell.

$ErrorActionPreference = "Stop"
$root = Split-Path -Parent $PSScriptRoot

$target = "C:\Program Files\Common Files\OFX\Plugins\WSMB.ofx.bundle"
$source = Join-Path $root "dist\WSMB.ofx.bundle"

if (-not (Test-Path $source)) {
    throw "build the bundle first: scripts/build-plugin.ps1"
}
$source = (Resolve-Path $source).Path

if (Test-Path $target) {
    $item = Get-Item $target -Force
    if ($item.LinkType -eq "Junction") {
        Write-Host "junction already points at $($item.Target)"
        exit 0
    }
    Remove-Item -Recurse -Force $target
}

New-Item -ItemType Junction -Path $target -Target $source | Out-Null
Write-Host "linked $target -> $source"
