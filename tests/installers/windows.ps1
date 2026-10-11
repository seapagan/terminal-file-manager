# Deterministic installer tests for install.ps1. All release assets come
# from a local mirror directory (TFM_BASE_URL); nothing contacts
# GitHub.
#
# Failure-path cases run on any OS with PowerShell 7+. Success-path cases
# need a real Windows tfm.exe and run only on Windows.

param([string]$RealArchive, [string]$Target, [string]$Version, [string]$Binary)

$ErrorActionPreference = 'Stop'
if ($RealArchive) {
    & (Join-Path $PSScriptRoot 'windows-real.ps1') -Archive $RealArchive -Target $Target -Version $Version -Binary $Binary
    exit 0
}

Set-StrictMode -Version 2.0

$nativeWindows = [Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT
$script:Passes = 0
$script:Skipped = 0

function Fail([string]$Message) {
    Write-Host "FAILED: $Message" -ForegroundColor Red
    exit 1
}

function Pass { $script:Passes++ }

function Skip([string]$Reason) {
    $script:Skipped++
    Write-Host "SKIP: $Reason"
}

$root = Join-Path ([System.IO.Path]::GetTempPath()) `
    ("tfm-ps1-tests-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $root | Out-Null

$installer = Join-Path $PSScriptRoot '..\..\install.ps1'
$resolvedInstaller = (Resolve-Path $installer).Path

# Portable mapping tests override only OS architecture discovery, not parsing
# or installation. Native success tests invoke the unmodified installer.
$portableWrapper = Join-Path $root 'portable.ps1'
$escapedInstaller = $resolvedInstaller.Replace("'", "''")
[IO.File]::WriteAllText($portableWrapper, @"
. '$escapedInstaller'
function Get-NativeArchitecture { return `$env:PROCESSOR_ARCHITECTURE }
Invoke-Install
"@)
$hostExecutable = if ($PSVersionTable.PSVersion.Major -lt 6) {
    Join-Path $PSHOME 'powershell.exe'
} else { Join-Path $PSHOME $(if ($nativeWindows) { 'pwsh.exe' } else { 'pwsh' }) }

# Synthetic versions deliberately unrelated to real releases.
$TestVersion = '99.88.77'
$TestTag = "v$TestVersion"

function New-Mirror {
    $mirror = Join-Path $root ("mirror-" + [System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path (Join-Path $mirror "download\$TestTag") -Force | Out-Null
    return $mirror
}

function Publish-Asset([string]$Mirror, [string]$Target, [byte[]]$Content) {
    $name = "terminal-file-manager-$TestTag-$Target.zip"
    $path = Join-Path $Mirror "download\$TestTag\$name"
    [System.IO.File]::WriteAllBytes($path, $Content)
    $hash = [System.BitConverter]::ToString(
        [System.Security.Cryptography.SHA256]::Create().ComputeHash($Content)
    ).Replace('-', '').ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $Mirror "download\$TestTag\$name.sha256"), "$hash  $name", [Text.Encoding]::ASCII)
}

function New-ZipBytes([hashtable]$Files) {
    if ($Files.ContainsKey('tfm.exe')) {
        foreach ($name in @('LICENSE-MIT', 'LICENSE-APACHE', 'README.md')) {
            if (-not $Files.ContainsKey($name)) { $Files[$name] = [Text.Encoding]::ASCII.GetBytes($name) }
        }
    }
    $stage = Join-Path $root ("zip-" + [System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $stage | Out-Null
    foreach ($name in $Files.Keys) {
        $destination = Join-Path $stage $name
        New-Item -ItemType Directory -Path (Split-Path $destination) -Force | Out-Null
        [System.IO.File]::WriteAllBytes($destination, $Files[$name])
    }
    $archive = "$stage.zip"
    Compress-Archive -Path "$stage\*" -DestinationPath $archive
    return [System.IO.File]::ReadAllBytes($archive)
}

function Invoke-Installer([string]$Mirror, [string]$InstallDir, [string]$Version = $TestTag, [string]$Architecture = 'AMD64', [bool]$Native = $false) {
    $envVars = @{
        TFM_BASE_URL     = $Mirror
        TFM_INSTALL_DIR  = $InstallDir
        TFM_VERSION      = $Version
        PROCESSOR_ARCHITECTURE = $Architecture
    }
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = $hostExecutable
    $entry = if ($Native) { $resolvedInstaller } else { $portableWrapper }
    $psi.Arguments = "-NoProfile -File `"$entry`""
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    foreach ($name in $envVars.Keys) {
        $psi.EnvironmentVariables[$name] = $envVars[$name]
    }
    $process = [System.Diagnostics.Process]::Start($psi)
    $stdout = $process.StandardOutput.ReadToEnd()
    $stderr = $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    [pscustomobject]@{
        Code = $process.ExitCode
        Output = $stdout + $stderr
    }
}

function Assert-Fails([object]$Result, [string]$Why, [string]$Needle) {
    if ($Result.Code -eq 0) { Fail "$Why succeeded" }
    if ($Needle -and ($Result.Output -notmatch [regex]::Escape($Needle))) {
        Fail "${Why}: expected output containing `"$Needle`", got: $($Result.Output)"
    }
    Pass
}

function Assert-Succeeds([object]$Result, [string]$Why) {
    if ($Result.Code -ne 0) { Fail "${Why}: $($Result.Output)" }
    Pass
}

# The candidate used by failure tests: not a real executable.
$junk = [System.Text.Encoding]::UTF8.GetBytes('this is not an executable')

# --- Architecture mapping -------------------------------------------------

foreach ($case in @(
    @{ Arch = 'AMD64'; Target = 'x86_64-pc-windows-msvc' },
    @{ Arch = 'ARM64'; Target = 'aarch64-pc-windows-msvc' }
)) {
    $mirror = New-Mirror   # empty: any asset request fails
    $result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install') -Architecture $case.Arch
    Assert-Fails $result "architecture $($case.Arch) mapping" $case.Target
}

$mirror = New-Mirror
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install') -Architecture 'MIPS'
Assert-Fails $result 'unsupported architecture' 'unsupported architecture'

# --- Checksum verification ------------------------------------------------

$mirror = New-Mirror
Publish-Asset $mirror 'x86_64-pc-windows-msvc' $junk
# Corrupt the sidecar.
$sidecar = Get-ChildItem $mirror -Recurse -Filter '*.sha256' | Select-Object -First 1
[IO.File]::WriteAllText($sidecar.FullName, ('0' * 64) + '  ' + $sidecar.Name, [Text.Encoding]::ASCII)
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install')
Assert-Fails $result 'bad checksum' 'checksum verification failed'

$mirror = New-Mirror
Publish-Asset $mirror 'x86_64-pc-windows-msvc' $junk
# Malformed sidecar (not a hex digest).
$sidecar = Get-ChildItem $mirror -Recurse -Filter '*.sha256' | Select-Object -First 1
[IO.File]::WriteAllText($sidecar.FullName, 'not-a-checksum  x', [Text.Encoding]::ASCII)
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install')
Assert-Fails $result 'malformed checksum' 'checksum verification failed'

# --- Missing assets -------------------------------------------------------

$mirror = New-Mirror
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install')
Assert-Fails $result 'missing archive' 'asset not found in the local mirror'

$mirror = New-Mirror
$name = "terminal-file-manager-$TestTag-x86_64-pc-windows-msvc.zip"
[System.IO.File]::WriteAllBytes((Join-Path $mirror "download\$TestTag\$name"), $junk)
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install')
Assert-Fails $result 'missing checksum sidecar' 'terminal-file-manager-v99.88.77-x86_64-pc-windows-msvc.zip.sha256'

# --- Candidate validation -------------------------------------------------

$mirror = New-Mirror
Publish-Asset $mirror 'x86_64-pc-windows-msvc' (New-ZipBytes @{ 'tfm.exe' = $junk })
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install')
Assert-Fails $result 'corrupt candidate' 'candidate failed --version validation'

$mirror = New-Mirror
$zipWithoutExe = New-ZipBytes @{ 'README.md' = [System.Text.Encoding]::UTF8.GetBytes('readme') }
Publish-Asset $mirror 'x86_64-pc-windows-msvc' $zipWithoutExe
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path $root 'install')
Assert-Fails $result 'archive without tfm.exe' 'release archive is missing tfm.exe'

# --- Failure must preserve an existing installation -----------------------

$preserveDir = Join-Path $root 'preserve'
New-Item -ItemType Directory -Path $preserveDir -Force | Out-Null
[System.IO.File]::WriteAllText((Join-Path $preserveDir 'tfm.exe'), 'old tfm')
$mirror = New-Mirror
$sidecarName = "terminal-file-manager-$TestTag-x86_64-pc-windows-msvc.zip.sha256"
Publish-Asset $mirror 'x86_64-pc-windows-msvc' $junk
$sidecar = Join-Path $mirror "download\$TestTag\$sidecarName"
[IO.File]::WriteAllText($sidecar, ('0' * 64) + "  $sidecarName", [Text.Encoding]::ASCII)
$result = Invoke-Installer -Mirror $mirror -InstallDir $preserveDir
Assert-Fails $result 'failed install' 'checksum verification failed'
if ((Get-Content (Join-Path $preserveDir 'tfm.exe') -Raw) -ne 'old tfm') {
    Fail 'failed installation changed the existing binary'
}
if (Get-ChildItem $preserveDir -Force -Filter '.tfm-*' -ErrorAction SilentlyContinue) {
    Fail 'failed installation left staging files'
}
Pass

# --- Latest via local marker ---------------------------------------------

$mirror = New-Mirror
Publish-Asset $mirror 'x86_64-pc-windows-msvc' (New-ZipBytes @{ 'tfm.exe' = $junk })
[IO.File]::WriteAllText((Join-Path $mirror 'latest'), $TestTag, [Text.Encoding]::ASCII)
# No TFM_VERSION: the marker provides the tag. The corrupt
# candidate then fails version validation, proving the marker was used.
$psi = [System.Diagnostics.ProcessStartInfo]::new()
$psi.FileName = $hostExecutable
$psi.Arguments = "-NoProfile -File `"$portableWrapper`""
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.EnvironmentVariables['TFM_BASE_URL'] = $mirror
$psi.EnvironmentVariables['TFM_INSTALL_DIR'] = (Join-Path $root 'install')
$psi.EnvironmentVariables['PROCESSOR_ARCHITECTURE'] = 'AMD64'
$psi.EnvironmentVariables.Remove('TFM_VERSION')
$process = [System.Diagnostics.Process]::Start($psi)
$markerOutput = $process.StandardOutput.ReadToEnd() + $process.StandardError.ReadToEnd()
$process.WaitForExit()
if ($process.ExitCode -eq 0 -or $markerOutput -notmatch 'candidate failed') {
    Fail "local latest marker was not used: $markerOutput"
}
Pass

$mirror = New-Mirror
[IO.File]::WriteAllText((Join-Path $mirror 'latest'), 'not-a-version', [Text.Encoding]::ASCII)
$psi2 = [System.Diagnostics.ProcessStartInfo]::new()
$psi2.FileName = $psi.FileName
$psi2.Arguments = $psi.Arguments
$psi2.UseShellExecute = $false
$psi2.RedirectStandardOutput = $true
$psi2.RedirectStandardError = $true
$psi2.EnvironmentVariables['TFM_BASE_URL'] = $mirror
$psi2.EnvironmentVariables['TFM_INSTALL_DIR'] = (Join-Path $root 'install')
$psi2.EnvironmentVariables['PROCESSOR_ARCHITECTURE'] = 'AMD64'
$psi2.EnvironmentVariables['TFM_BASE_URL'] = $mirror
$psi2.EnvironmentVariables.Remove('TFM_VERSION')
$process = [System.Diagnostics.Process]::Start($psi2)
$badMarkerOutput = $process.StandardOutput.ReadToEnd() + $process.StandardError.ReadToEnd()
$process.WaitForExit()
if ($process.ExitCode -eq 0 -or $badMarkerOutput -notmatch 'usable release tag') {
    Fail "malformed local latest marker accepted: $badMarkerOutput"
}
Pass

function Get-PromptNativeVersion([string]$Exe) {
    $psi = [Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = (Resolve-Path -LiteralPath $Exe).Path
    $psi.Arguments = '--version'
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $process = [Diagnostics.Process]::Start($psi)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(10000)) {
            $process.Kill()
            $process.WaitForExit()
            Fail 'native --version did not terminate within ten seconds'
        }
        if ($process.ExitCode -ne 0) { Fail 'native --version failed' }
        return $stdout.GetAwaiter().GetResult().TrimEnd("`r", "`n")
    } finally { $process.Dispose() }
}

# --- Success path: needs a real Windows binary ----------------------------

$builtExe = if ($Binary) { (Resolve-Path $Binary).Path } else {
    Join-Path $PSScriptRoot '..\..\target\release\tfm.exe'
}
if (-not $nativeWindows) {
    Skip 'success-path cases require Windows (real tfm.exe)'
} elseif (-not (Test-Path $builtExe)) {
    Fail "success-path cases require a built binary at $builtExe (run cargo make release first)"
} else {
    $realVersion = (Get-PromptNativeVersion $builtExe) -replace '^tfm ', ''
    $realTag = "v$realVersion"
    . $resolvedInstaller
    $runnerArch = Get-NativeArchitecture
    Pass

    $mirror = New-Mirror
    $zip = New-ZipBytes @{ 'tfm.exe' = [System.IO.File]::ReadAllBytes($builtExe) }
    $name = "terminal-file-manager-$realTag-$runnerArch-pc-windows-msvc.zip" -replace '-AMD64-', '-x86_64-' -replace '-ARM64-', '-aarch64-'
    $dir = Join-Path $mirror "download\$realTag"
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    [System.IO.File]::WriteAllBytes((Join-Path $dir $name), $zip)
    $hash = (Get-FileHash (Join-Path $dir $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $dir "$name.sha256"), "$hash  $name", [Text.Encoding]::ASCII)

    $installDir = Join-Path $root 'success'
    $result = Invoke-Installer -Mirror $mirror -InstallDir $installDir -Version $realTag -Architecture $runnerArch -Native $true
    Assert-Succeeds $result 'successful installation'
    $installed = Join-Path $installDir 'tfm.exe'
    if (-not (Test-Path $installed)) { Fail 'installed binary missing' }
    if ((Get-PromptNativeVersion $installed) -ne "tfm $realVersion") { Fail 'installed binary reports the wrong version' }
    Pass

    # Replacement of an existing installation.
    $result = Invoke-Installer -Mirror $mirror -InstallDir $installDir -Version $realTag -Architecture $runnerArch -Native $true
    Assert-Succeeds $result 'replacement of existing installation'
    if ((Get-PromptNativeVersion $installed) -ne "tfm $realVersion") { Fail 'replaced binary is wrong' }
    Pass

    # Wrong advertised version must be rejected and change nothing.
    $before = (Get-FileHash $installed -Algorithm SHA256).Hash
    $wrongDir = Join-Path $mirror 'download/v0.0.0'
    New-Item -ItemType Directory -Path $wrongDir | Out-Null
    $wrongName = $name.Replace($realTag, 'v0.0.0')
    [IO.File]::WriteAllBytes((Join-Path $wrongDir $wrongName), $zip)
    [IO.File]::WriteAllText((Join-Path $wrongDir "$wrongName.sha256"), "$hash  $wrongName", [Text.Encoding]::ASCII)
    $result = Invoke-Installer -Mirror $mirror -InstallDir $installDir -Version 'v0.0.0' -Architecture $runnerArch -Native $true
    Assert-Fails $result 'wrong advertised version' 'reported version does not match'
    if ((Get-FileHash $installed -Algorithm SHA256).Hash -ne $before) { Fail 'wrong-version attempt changed the installation' }
    Pass
}

Write-Host "PowerShell installer tests passed: $($script:Passes) cases ($($script:Skipped) skipped)."
Remove-Item $root -Recurse -Force -ErrorAction SilentlyContinue
