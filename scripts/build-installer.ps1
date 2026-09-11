$ErrorActionPreference = 'Stop'

$manifest = Get-Content Cargo.toml -Raw
$match = [regex]::Match($manifest, '(?m)^version\s*=\s*"([0-9]+\.[0-9]+\.[0-9]+)"')
if (-not $match.Success) { throw 'Could not read Cargo package version.' }
$version = $match.Groups[1].Value

cargo build --locked --release --bin process-optimizer --bin process-optimizer-updater
if ($LASTEXITCODE -ne 0) { throw 'Release build failed.' }

$candidates = @(
    (Get-Command ISCC.exe -ErrorAction SilentlyContinue | Select-Object -ExpandProperty Source -ErrorAction SilentlyContinue),
    "$env:ProgramFiles(x86)\Inno Setup 6\ISCC.exe",
    "$env:ProgramFiles\Inno Setup 6\ISCC.exe"
) | Where-Object { $_ -and (Test-Path $_) }
$iscc = $candidates | Select-Object -First 1
if (-not $iscc) {
    throw 'Inno Setup 6 was not found. Install it with: choco install innosetup -y'
}
New-Item -ItemType Directory -Force dist | Out-Null
& $iscc "/DMyAppVersion=$version" installer\process-optimizer.iss
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup compilation failed.' }
Write-Host "Built dist\ProcessOptimizerSetup.exe for version $version"
