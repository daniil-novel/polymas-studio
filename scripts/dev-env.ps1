# Dot-source in a development PowerShell; does not change machine/user settings.
$ErrorActionPreference = 'Stop'
if ($IsWindows -or $env:OS -eq 'Windows_NT') {
    $vsWherePath = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (Test-Path -LiteralPath $vsWherePath) {
        $vsDevPath = & $vsWherePath -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
        if ($vsDevPath) {
            Import-Module (Join-Path $vsDevPath 'Common7/Tools/Microsoft.VisualStudio.DevShell.dll')
            Enter-VsDevShell -VsInstallPath $vsDevPath -SkipAutomaticLocation -DevCmdArguments '-arch=x64 -host_arch=x64' | Out-Null
        }
    }
    # Optional per-project official NuGet SDK, for hosts missing their installed SDK.
    $sdkProjectRoot = Join-Path $PSScriptRoot '../tmp/toolchain'
    if (Test-Path -LiteralPath $sdkProjectRoot) {
        $sdkKernel = Get-ChildItem -LiteralPath $sdkProjectRoot -Filter kernel32.lib -Recurse | Where-Object { $_.Directory.Name -eq 'x64' } | Select-Object -First 1
        $sdkCrt = Get-ChildItem -LiteralPath $sdkProjectRoot -Filter ucrt.lib -Recurse | Where-Object { $_.Directory.Name -eq 'x64' } | Select-Object -First 1
        if ($sdkKernel -and $sdkCrt) { $env:LIB = "$($sdkKernel.DirectoryName);$($sdkCrt.DirectoryName);$env:LIB" }
        $sdkWindows = Get-ChildItem -LiteralPath $sdkProjectRoot -Filter Windows.h -Recurse | Select-Object -First 1
        if ($sdkWindows) {
            $sdkIncludeRoot = $sdkWindows.Directory.Parent.FullName
            $env:INCLUDE = "$sdkIncludeRoot/um;$sdkIncludeRoot/shared;$sdkIncludeRoot/ucrt;$sdkIncludeRoot/winrt;$env:INCLUDE"
        }
        $sdkRc = Get-ChildItem -LiteralPath $sdkProjectRoot -Filter rc.exe -Recurse | Where-Object { $_.Directory.Name -eq 'x64' } | Select-Object -First 1
        if ($sdkRc) { $env:PATH = "$($sdkRc.DirectoryName);$env:PATH" }
    }
}
