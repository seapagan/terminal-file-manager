# Execute the actual rehearsal PS7 step bodies with the Actions wrapper.
# Fixture failures prove control flow; real suites retain their native limits.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$repo = (Resolve-Path (Join-Path $PSScriptRoot '../..')).Path
$workflow = [IO.File]::ReadAllText((Join-Path $repo '.github/workflows/release-rehearsal.yml'))
$section = [regex]::Match($workflow, '(?ms)^      - name: Run Windows PE.*?(?=^      - name: Run Windows security regressions \(PowerShell 5\.1\))').Value
$steps = @([regex]::Matches($section, '(?ms)^      - name: ([^\r\n]+)\r?\n(.*?)(?=^      - name: |\z)'))
if ($steps.Count -eq 0) { throw 'missing production PS7 regression steps' }
$root = Join-Path ([IO.Path]::GetTempPath()) ('tfm-step-' + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path (Join-Path $root 'tests/release') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path $root 'tests/installers') -Force | Out-Null
$hostExecutable = (Get-Process -Id $PID).Path
$bodies = @()
foreach ($step in $steps) {
    if ($step.Value -notmatch "(?m)^        if: matrix.kind == 'windows'\r?$" -or
        $step.Value -notmatch '(?m)^        shell: pwsh\r?$') { throw 'incorrect native PS7 routing' }
    $body = [regex]::Match($step.Value, '(?ms)^        run: \|\r?\n(.*)').Groups[1].Value
    if (-not $body.Trim()) { throw 'missing production step body' }
    $bodies += [regex]::Replace($body, '(?m)^          ', '')
}

function Invoke-Step([string]$Body, [string]$Directory) {
    $path = Join-Path $root ([IO.Path]::GetRandomFileName() + '.ps1')
    # GitHub's pwsh wrapper: terminating script errors and final native status.
    $wrapped = '$ErrorActionPreference = ''stop''' + "`n" + $Body + "`n" +
        'if ((Test-Path -LiteralPath variable:\LASTEXITCODE)) { exit $LASTEXITCODE }'
    [IO.File]::WriteAllText($path, $wrapped)
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $hostExecutable
    $psi.Arguments = "-NoProfile -File `"$path`""
    $psi.WorkingDirectory = $Directory
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($psi)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        return @{ Code = $process.ExitCode; Output = $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult() }
    } finally { $process.Dispose() }
}

function Invoke-Job([string]$Directory) {
    $output = ''
    foreach ($body in $bodies) {
        $result = Invoke-Step $body $Directory
        $output += $result.Output
        if ($result.Code -ne 0) { return @{ Code = $result.Code; Output = $output } }
    }
    return @{ Code = 0; Output = $output }
}

function Write-Fixture([string]$Suite, [string]$Content) {
    $relative = if ($Suite -eq 'PE') { 'tests/release/verify.ps1' } else { 'tests/installers/windows-security.ps1' }
    [IO.File]::WriteAllText((Join-Path $root $relative), $Content)
}

try {
    $success = 'if (Test-Path variable:\LASTEXITCODE) { throw ''LASTEXITCODE unexpectedly initialized'' }; Write-Host ''{0} executed with unset LASTEXITCODE'''
    Write-Fixture PE ($success.Replace('{0}', 'PE'))
    Write-Fixture security ($success.Replace('{0}', 'security'))
    $result = Invoke-Job $root
    if ($result.Code -ne 0 -or $result.Output -notmatch 'PE executed with unset LASTEXITCODE' -or
        $result.Output -notmatch 'security executed with unset LASTEXITCODE') {
        throw "production success path skipped a suite: $($result.Output)"
    }
    if ($bodies.Count -ne 2) { throw 'PE and security must have independent steps' }
    foreach ($suite in @('PE', 'security')) {
        foreach ($failure in @('exit 23', "throw 'injected script failure'")) {
            Write-Fixture PE 'Write-Host ''PE reached'''
            Write-Fixture security 'Write-Host ''security reached'''
            Write-Fixture $suite $failure
            $index = if ($suite -eq 'PE') { 0 } else { 1 }
            $step = Invoke-Step $bodies[$index] $root
            $job = Invoke-Job $root
            if ($step.Code -eq 0 -or $job.Code -eq 0) { throw "$suite failure masked: $failure" }
            if ($failure -eq 'exit 23' -and ($step.Code -ne 23 -or $job.Code -ne 23)) {
                throw "$suite nonzero exit was altered"
            }
            if ($suite -eq 'PE' -and $job.Output -match 'security reached') { throw 'job continued after PE failure' }
            if ($suite -eq 'security' -and $job.Output -notmatch 'PE reached') { throw 'PE success path bypassed' }
        }
    }
    $real = Invoke-Job $repo
    if ($real.Code -ne 0 -or $real.Output -notmatch 'PE verification tests passed: 12 cases' -or
        $real.Output -notmatch 'security.*passed') { throw "real suites failed or were skipped: $($real.Output)" }
    Write-Host $real.Output
    Write-Host 'Actual PS7 workflow steps: unset-status success; both exit/throw failures propagated; later steps blocked after PE failure'
} finally { Remove-Item -LiteralPath $root -Recurse -Force }
