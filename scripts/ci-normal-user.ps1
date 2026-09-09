# Disposable GitHub-hosted Windows CI only. Never run against user applications.
# Creates a new standard test account; does not modify any existing account.
$ErrorActionPreference = 'Stop'
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_OS -ne 'Windows') {
    throw 'Use an isolated GitHub Actions Windows runner, not your PC.'
}
$name = 'po' + [guid]::NewGuid().ToString('N').Substring(0, 12)
$password = 'Po!9' + [guid]::NewGuid().ToString('N') + 'zA7!'
$secret = ConvertTo-SecureString $password -AsPlainText -Force
$stage = Join-Path $env:PUBLIC ('ProcessOptimizerCI-' + [guid]::NewGuid().ToString('N'))
$created = $false
try {
    New-Item -ItemType Directory -Path $stage | Out-Null
    New-LocalUser -Name $name -Password $secret -AccountNeverExpires -PasswordNeverExpires | Out-Null
    $created = $true
    $users = Get-LocalGroup -SID 'S-1-5-32-545'
    Add-LocalGroupMember -Group $users -Member $name
    & icacls.exe $stage /grant "${name}:(OI)(CI)M" | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'Could not grant access to the isolated test directory.' }
    Copy-Item target/debug/process-optimizer.exe,target/debug/optimizer-test-fixture.exe $stage
    $test = Get-ChildItem target/debug/deps/worker_e2e-*.exe | Select-Object -First 1
    if (-not $test) { throw 'The worker test executable was not built.' }
    Copy-Item $test.FullName (Join-Path $stage 'worker-e2e.exe')
    $runner = @'
$ErrorActionPreference = 'Stop'
$env:OPTIMIZER_ALLOW_WORKER_E2E = '1'
$env:OPTIMIZER_E2E_BIN_DIR = $PSScriptRoot
$env:TEMP = Join-Path $PSScriptRoot 'temp'
$env:TMP = $env:TEMP
New-Item -ItemType Directory -Force $env:TEMP | Out-Null
Set-Location $PSScriptRoot
& .\worker-e2e.exe --ignored --nocapture --test-threads=1 *> .\result.log
$code = $LASTEXITCODE
Set-Content -Path .\exit-code.txt -Value $code
exit $code
'@
    $runner | Set-Content (Join-Path $stage 'run.ps1') -Encoding utf8
    $credential = [pscredential]::new("$env:COMPUTERNAME\$name", $secret)
    $process = Start-Process -FilePath "$env:SystemRoot\System32\WindowsPowerShell\v1.0\powershell.exe" `
        -Credential $credential -LoadUserProfile -WorkingDirectory $stage `
        -ArgumentList @('-NoProfile','-NonInteractive','-File',"`"$(Join-Path $stage 'run.ps1')`"") -PassThru
    if (-not $process.WaitForExit(180000)) { $process.Kill(); throw 'Isolated worker tests timed out.' }
    $log = Join-Path $stage 'result.log'
    # These logs contain only owned test fixtures, never a real user inventory.
    if (Test-Path $log) {
        Get-Content $log | ForEach-Object { $_.Replace($stage, '<isolated-fixture>') } | Write-Host
    }
    $codeFile = Join-Path $stage 'exit-code.txt'
    if (-not (Test-Path $codeFile) -or (Get-Content $codeFile -Raw).Trim() -ne '0') {
        throw 'Non-administrator worker lifecycle tests failed.'
    }
} finally {
    if ($created) { Remove-LocalUser -Name $name -ErrorAction Continue }
    if (Test-Path $stage) { Remove-Item $stage -Recurse -Force -ErrorAction Continue }
    $password = $null
    $secret = $null
}
