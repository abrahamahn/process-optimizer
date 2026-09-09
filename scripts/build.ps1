$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) { throw 'Install Rust (MSVC toolchain) and Visual Studio Build Tools with Desktop development with C++ first.' }
    cargo test --all-targets
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed; no release was produced.' }
    cargo build --release --bin process-optimizer
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    Write-Host "Portable application: $((Get-Location).Path)\target\release\process-optimizer.exe"
    Write-Host 'Run normally, not as administrator. No optimizer session was started by this build script.'
} finally { Pop-Location }
