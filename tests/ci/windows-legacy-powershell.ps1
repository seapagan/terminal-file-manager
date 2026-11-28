# Exercise the real Cargo Make tasks without building or running the installer.
$ErrorActionPreference = 'Stop'
$repo = (Resolve-Path -LiteralPath (Join-Path -Path $PSScriptRoot -ChildPath '../..')).Path
$root = Join-Path -Path ([IO.Path]::GetTempPath()) -ChildPath ('tfm legacy ' + [IO.Path]::GetRandomFileName())
$fixtures = Join-Path -Path $root -ChildPath 'tests/installers'
$cargo = Get-Command -Name cargo -CommandType Application | Select-Object -First 1
$inherited = $env:PSModulePath
$contaminated = Join-Path -Path $root -ChildPath 'empty modules'

function Invoke-Probe {
    $launch = [Diagnostics.ProcessStartInfo]::new($cargo.Source)
    $launch.UseShellExecute = $false
    $launch.WorkingDirectory = $root
    $launch.RedirectStandardOutput = $true
    $launch.RedirectStandardError = $true
    $launch.Environment['PSModulePath'] = $contaminated
    $launch.Environment['TFM_TEST_MODULE_PATH'] = $contaminated
    $launch.ArgumentList.Add('make')
    $launch.ArgumentList.Add('probe')
    $process = [Diagnostics.Process]::Start($launch)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        return @{ Code = $process.ExitCode; Output = $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult() }
    } finally { $process.Dispose() }
}

try {
    [void][IO.Directory]::CreateDirectory($fixtures)
    [void][IO.Directory]::CreateDirectory($contaminated)
    [IO.File]::Copy((Join-Path -Path $repo -ChildPath 'Makefile.toml'), (Join-Path -Path $root -ChildPath 'production.toml'))
    [IO.File]::WriteAllText((Join-Path -Path $root -ChildPath 'Makefile.toml'), @'
extend = "production.toml"

[tasks.release-build]
command = "pwsh"
args = ["-NoProfile", "-Command", "exit 0"]

[tasks.probe]
dependencies = ["_windows-powershell-legacy-version", "windows-installer-test-legacy", "probe-parent"]

[tasks.probe-parent]
command = "python"
args = ["-c", "import os; assert os.environ['PSModulePath'] == os.environ['TFM_TEST_MODULE_PATH']; print('parent module path preserved')"]
'@)
    [IO.File]::WriteAllText((Join-Path -Path $fixtures -ChildPath 'probe.txt'), 'hello')
    $fixture = @'
$ErrorActionPreference = 'Stop'
if ($PSVersionTable.PSVersion.Major -ne 5 -or $PSVersionTable.PSVersion.Minor -ne 1) {
    throw 'Expected Windows PowerShell 5.1'
}
$hash = Get-FileHash -LiteralPath (Join-Path -Path $PSScriptRoot -ChildPath 'probe.txt')
if ($hash.Hash -ne '2CF24DBA5FB0A30E26E83B2AC5B9E29E1B161E5C1FA7425E73043362938B9824') {
    throw 'Get-FileHash returned the wrong digest'
}
Write-Output "hashed by $([IO.Path]::GetFileName($PSCommandPath))"
'@
    $security = Join-Path -Path $fixtures -ChildPath 'windows-security.ps1'
    $installer = Join-Path -Path $fixtures -ChildPath 'windows.ps1'
    [IO.File]::WriteAllText($security, $fixture)
    [IO.File]::WriteAllText($installer, $fixture)
    $result = Invoke-Probe
    if ($result.Code -ne 0 -or $result.Output -notmatch 'hashed by windows-security[.]ps1' -or
        $result.Output -notmatch 'hashed by windows[.]ps1' -or $result.Output -notmatch 'parent module path preserved') {
        throw "Legacy task isolation failed: $($result.Output)"
    }

    foreach ($path in @($security, $installer)) {
        [IO.File]::WriteAllText($path, 'exit 23')
        $result = Invoke-Probe
        if ($result.Code -eq 0 -or $result.Output -notmatch 'exit code: 23' -or
            $result.Output -match 'parent module path preserved') {
            throw "Legacy task masked failure from ${path}: $($result.Output)"
        }
        [IO.File]::WriteAllText($path, $fixture)
    }
    if ($env:PSModulePath -cne $inherited) { throw 'Regression changed its parent module path' }
    Write-Output 'Legacy version, security and installer tasks passed with a contaminated parent module path. Exit 23 propagated.'
} finally { Remove-Item -LiteralPath $root -Recurse -Force }
