# Execute the actual rehearsal PS7 step bodies with the Actions wrapper.
# Fixture failures prove control flow; real suites retain their native limits.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$repo = (Resolve-Path -Path (Join-Path -Path $PSScriptRoot -ChildPath '../..')).Path
$workflow = [IO.File]::ReadAllText((Join-Path -Path $repo -ChildPath '.github/workflows/release-rehearsal.yml'))
$section = [regex]::Match($workflow, '(?ms)^      - name: Run Windows PE.*?(?=^      - name: Run Windows security regressions \(PowerShell 5\.1\))').Value
$steps = @([regex]::Matches($section, '(?ms)^      - name: ([^\r\n]+)\r?\n(.*?)(?=^      - name: |\z)'))
if ($steps.Count -eq 0) { throw 'missing production PS7 regression steps' }
$root = Join-Path -Path ([IO.Path]::GetTempPath()) -ChildPath ('tfm-step-' + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path (Join-Path -Path $root -ChildPath 'tests/release') -Force | Out-Null
New-Item -ItemType Directory -Path (Join-Path -Path $root -ChildPath 'tests/installers') -Force | Out-Null
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
    $path = Join-Path -Path $root -ChildPath ([IO.Path]::GetRandomFileName() + '.ps1')
    # GitHub's pwsh wrapper: terminating script errors and final native status.
    $wrapped = '$ErrorActionPreference = ''stop''' + "`n" + $Body + "`n" +
        'if ((Test-Path -LiteralPath variable:\LASTEXITCODE)) { exit $LASTEXITCODE }'
    [IO.File]::WriteAllText($path, $wrapped)
    $psi = New-Object -TypeName Diagnostics.ProcessStartInfo
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
        $result = Invoke-Step -Body $body -Directory $Directory
        $output += $result.Output
        if ($result.Code -ne 0) { return @{ Code = $result.Code; Output = $output } }
    }
    return @{ Code = 0; Output = $output }
}

function Write-Fixture([string]$Suite, [string]$Content) {
    $relative = if ($Suite -eq 'PE') { 'tests/release/verify.ps1' } else { 'tests/installers/windows-security.ps1' }
    [IO.File]::WriteAllText((Join-Path -Path $root -ChildPath $relative), $Content)
}

try {
    $success = 'if (Test-Path -LiteralPath variable:\LASTEXITCODE) { throw ''LASTEXITCODE unexpectedly initialized'' }; Write-Information -MessageData ''{0} executed with unset LASTEXITCODE'' -InformationAction Continue'
    Write-Fixture -Suite PE -Content ($success.Replace('{0}', 'PE'))
    Write-Fixture -Suite security -Content ($success.Replace('{0}', 'security'))
    $result = Invoke-Job -Directory $root
    if ($result.Code -ne 0 -or $result.Output -notmatch 'PE executed with unset LASTEXITCODE' -or
        $result.Output -notmatch 'security executed with unset LASTEXITCODE') {
        throw "production success path skipped a suite: $($result.Output)"
    }
    if ($bodies.Count -ne 2) { throw 'PE and security must have independent steps' }
    foreach ($suite in @('PE', 'security')) {
        foreach ($failure in @('exit 23', "throw 'injected script failure'")) {
            Write-Fixture -Suite PE -Content 'Write-Information -MessageData ''PE reached'' -InformationAction Continue'
            Write-Fixture -Suite security -Content 'Write-Information -MessageData ''security reached'' -InformationAction Continue'
            Write-Fixture -Suite $suite -Content $failure
            $index = if ($suite -eq 'PE') { 0 } else { 1 }
            $step = Invoke-Step -Body $bodies[$index] -Directory $root
            $job = Invoke-Job -Directory $root
            if ($step.Code -eq 0 -or $job.Code -eq 0) { throw "$suite failure masked: $failure" }
            if ($failure -eq 'exit 23' -and ($step.Code -ne 23 -or $job.Code -ne 23)) {
                throw "$suite nonzero exit was altered"
            }
            if ($suite -eq 'PE' -and $job.Output -match 'security reached') { throw 'job continued after PE failure' }
            if ($suite -eq 'security' -and $job.Output -notmatch 'PE reached') { throw 'PE success path bypassed' }
        }
    }
    $real = Invoke-Job -Directory $repo
    if ($real.Code -ne 0 -or $real.Output -notmatch 'PE verification tests passed: 12 cases' -or
        $real.Output -notmatch 'security.*passed') { throw "real suites failed or were skipped: $($real.Output)" }
    Write-Information -MessageData $real.Output -InformationAction Continue
    Write-Information -MessageData 'Actual PS7 workflow steps: unset-status success; both exit/throw failures propagated; later steps blocked after PE failure' -InformationAction Continue
} finally { Remove-Item -LiteralPath $root -Recurse -Force }
