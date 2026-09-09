# Build-time check only. Inspect import metadata; do not execute an optimizer session.
param([string]$Executable = 'target/release/process-optimizer.exe')
$ErrorActionPreference = 'Stop'
$vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (-not (Test-Path $vswhere)) { throw 'Visual Studio Build Tools metadata is unavailable.' }
$vs = & $vswhere -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if (-not $vs) { throw 'MSVC build tools installation was not found.' }
$dumpbin = Get-ChildItem (Join-Path $vs 'VC/Tools/MSVC/*/bin/Hostx64/x64/dumpbin.exe') | Sort-Object FullName -Descending | Select-Object -First 1
if (-not $dumpbin) { throw 'dumpbin.exe was not found.' }
$imports = & $dumpbin.FullName /nologo /dependents $Executable
if ($LASTEXITCODE -ne 0) { throw 'Executable import inspection failed.' }
if ($imports -match '(?i)\b(?:vcruntime|msvcp|msvcr)\d+[^\s]*\.dll') {
    throw 'Portable application still depends on an external Visual C++ runtime DLL.'
}
Write-Host 'Executable import inspection passed: no external Visual C++ runtime DLL is required.'
