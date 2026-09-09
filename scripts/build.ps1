$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot '..')
try {
    if (-not (Get-Command cargo -ErrorAction SilentlyContinue)) {
        throw 'Install Rust (MSVC toolchain) and Visual Studio Build Tools with Desktop development with C++ first.'
    }
    cargo fmt --all -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Formatting check failed.' }
    cargo test --locked --all-targets
    if ($LASTEXITCODE -ne 0) { throw 'Tests failed; no release was produced.' }
    cargo clippy --locked --all-targets -- -D warnings
    if ($LASTEXITCODE -ne 0) { throw 'Static checks failed.' }
    cargo build --locked --release --bin process-optimizer
    if ($LASTEXITCODE -ne 0) { throw 'Build failed.' }
    Write-Host "Portable application: $((Get-Location).Path)\target\release\process-optimizer.exe"
    Write-Host 'Run normally, not as administrator. The build did not start an optimizer session.'
    Write-Host 'The force-termination integration test is ignored unless separately requested; all test mutations target test-owned fixtures only.'
} finally { Pop-Location }
