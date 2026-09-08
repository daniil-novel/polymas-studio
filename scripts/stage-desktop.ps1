param([switch]$Release)
$ErrorActionPreference = 'Stop'
. (Join-Path $PSScriptRoot 'dev-env.ps1')
$desktopRepoRoot = Split-Path $PSScriptRoot -Parent
Push-Location $desktopRepoRoot
try {
    $cargoArguments = @('build', '-p', 'polymas-daemon', '--locked')
    if ($Release) { $cargoArguments += '--release' }
    & cargo @cargoArguments
    if ($LASTEXITCODE -ne 0) { throw 'Service build failed' }
    $targetTriple = (& rustc -vV | Select-String '^host: ').Line.Substring(6)
    if ($targetTriple -ne 'x86_64-pc-windows-msvc') { throw 'Desktop packaging currently targets Windows x64 MSVC' }
    $buildProfile = if ($Release) { 'release' } else { 'debug' }
    $destination = Join-Path $desktopRepoRoot 'apps/desktop/src-tauri/binaries'
    New-Item -ItemType Directory -Path $destination -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $desktopRepoRoot "target/$buildProfile/polymas-daemon.exe") -Destination (Join-Path $destination "polymas-daemon-$targetTriple.exe")
} finally { Pop-Location }
