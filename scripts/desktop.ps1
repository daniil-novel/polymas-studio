param([switch]$Build)
$ErrorActionPreference = 'Stop'
$desktopRepoRoot = Split-Path $PSScriptRoot -Parent
& (Join-Path $PSScriptRoot 'stage-desktop.ps1') -Release:$Build
Push-Location (Join-Path $desktopRepoRoot 'apps/desktop/src-tauri')
try {
    $tauriCli = Join-Path $desktopRepoRoot 'node_modules/.bin/tauri.cmd'
    if ($Build) { & $tauriCli build } else { & $tauriCli dev }
    if ($LASTEXITCODE -ne 0) { throw 'Desktop command failed' }
} finally { Pop-Location }
