# Deterministic installer tests for install.ps1. All release assets come
# from a local mirror directory (TFM_BASE_URL); nothing contacts
# GitHub.
#
# Failure-path cases run on any OS with PowerShell 7+. Success-path cases
# need a real Windows tfm.exe and run only on Windows.

param([string]$RealArchive, [string]$Target, [string]$Version, [string]$Binary)

$ErrorActionPreference = 'Stop'
if ($RealArchive) {
    & (Join-Path -Path $PSScriptRoot -ChildPath 'windows-real.ps1') -Archive $RealArchive -Target $Target -Version $Version -Binary $Binary
    exit 0
}

Set-StrictMode -Version 2.0

$nativeWindows = [Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT
$script:Passes = 0
$script:Skipped = 0

function Fail([string]$Message) {
    Write-Error -Message "FAILED: $Message" -ErrorAction Continue
    exit 1
}

function Pass { $script:Passes++ }

function Skip([string]$Reason) {
    $script:Skipped++
    Write-Information -MessageData "SKIP: $Reason" -InformationAction Continue
}

$root = Join-Path -Path ([System.IO.Path]::GetTempPath()) `
    -ChildPath ("tfm-ps1-tests-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $root | Out-Null

$installer = Join-Path -Path $PSScriptRoot -ChildPath '..\..\install.ps1'
$resolvedInstaller = (Resolve-Path -Path $installer).Path

# Portable mapping tests override only OS architecture discovery, not parsing
# or installation. Native success tests invoke the unmodified installer.
$portableWrapper = Join-Path -Path $root -ChildPath 'portable.ps1'
$escapedInstaller = $resolvedInstaller.Replace("'", "''")
[IO.File]::WriteAllText($portableWrapper, @"
. '$escapedInstaller'
function Get-NativeArchitecture { return `$env:TFM_TEST_ARCHITECTURE }
Invoke-Install
"@)
$hostExecutable = if ($PSVersionTable.PSVersion.Major -lt 6) {
    Join-Path -Path $PSHOME -ChildPath 'powershell.exe'
} else { Join-Path -Path $PSHOME -ChildPath $(if ($nativeWindows) { 'pwsh.exe' } else { 'pwsh' }) }

# Synthetic versions deliberately unrelated to real releases.
$TestVersion = '99.88.77'
$TestTag = "v$TestVersion"

function Build-Mirror {
    # Required isolated fixture construction; skipping it invalidates the test.
    $mirror = Join-Path -Path $root -ChildPath ("mirror-" + [System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path (Join-Path -Path $mirror -ChildPath "download\$TestTag") -Force | Out-Null
    return $mirror
}

function Publish-Asset([string]$Mirror, [string]$Target, [byte[]]$Content) {
    $name = "terminal-file-manager-$TestTag-$Target.zip"
    $path = Join-Path -Path $Mirror -ChildPath "download\$TestTag\$name"
    [System.IO.File]::WriteAllBytes($path, $Content)
    $hash = [System.BitConverter]::ToString(
        [System.Security.Cryptography.SHA256]::Create().ComputeHash($Content)
    ).Replace('-', '').ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path -Path $Mirror -ChildPath "download\$TestTag\$name.sha256"), "$hash  $name", [Text.Encoding]::ASCII)
}

function Build-ZipBytes([hashtable]$Files) {
    # Required isolated fixture construction, not an independently optional action.
    if ($Files.ContainsKey('tfm.exe')) {
        foreach ($name in @('LICENSE-MIT', 'LICENSE-APACHE', 'README.md')) {
            if (-not $Files.ContainsKey($name)) { $Files[$name] = [Text.Encoding]::ASCII.GetBytes($name) }
        }
    }
    $stage = Join-Path -Path $root -ChildPath ("zip-" + [System.IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $stage | Out-Null
    foreach ($name in $Files.Keys) {
        $destination = Join-Path -Path $stage -ChildPath $name
        New-Item -ItemType Directory -Path (Split-Path -Path $destination) -Force | Out-Null
        [System.IO.File]::WriteAllBytes($destination, $Files[$name])
    }
    $archive = "$stage.zip"
    Compress-Archive -Path "$stage\*" -DestinationPath $archive
    return [System.IO.File]::ReadAllBytes($archive)
}

function Invoke-Installer([string]$Mirror, [string]$InstallDir, [string]$Version = $TestTag, [string]$Architecture = 'AMD64', [bool]$Native = $false, [string]$HostArchitecture) {
    $envVars = @{
        TFM_BASE_URL     = $Mirror
        TFM_INSTALL_DIR  = $InstallDir
        TFM_VERSION      = $Version
        TFM_TEST_ARCHITECTURE = $Architecture
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
    if ($HostArchitecture) {
        $psi.EnvironmentVariables['PROCESSOR_ARCHITECTURE'] = $HostArchitecture
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
    if ($Result.Code -eq 0) { Fail -Message "$Why succeeded" }
    if ($Needle -and ($Result.Output -notmatch [regex]::Escape($Needle))) {
        Fail -Message "${Why}: expected output containing `"$Needle`", got: $($Result.Output)"
    }
    Pass
}

function Assert-Succeeds([object]$Result, [string]$Why) {
    if ($Result.Code -ne 0) { Fail -Message "${Why}: $($Result.Output)" }
    Pass
}

# The candidate used by failure tests: not a real executable.
$junk = [System.Text.Encoding]::UTF8.GetBytes('this is not an executable')

# --- Architecture mapping -------------------------------------------------

foreach ($case in @(
    @{ Arch = 'AMD64'; Target = 'x86_64-pc-windows-msvc' },
    @{ Arch = 'ARM64'; Target = 'aarch64-pc-windows-msvc' }
)) {
    $mirror = Build-Mirror   # empty: any asset request fails
    $result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install') -Architecture $case.Arch
    Assert-Fails -Result $result -Why "architecture $($case.Arch) mapping" -Needle $case.Target
}

# Synthetic selection must not depend on the host's architecture variable.
foreach ($case in @(
    @{ Arch = 'AMD64'; HostArch = 'ARM64'; Target = 'x86_64-pc-windows-msvc' },
    @{ Arch = 'ARM64'; HostArch = 'AMD64'; Target = 'aarch64-pc-windows-msvc' }
)) {
    $mirror = Build-Mirror
    $result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'conflict') -Architecture $case.Arch -HostArchitecture $case.HostArch
    Assert-Fails -Result $result -Why "architecture $($case.Arch) with conflicting host $($case.HostArch)" -Needle $case.Target
}

$mirror = Build-Mirror
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install') -Architecture 'MIPS'
Assert-Fails -Result $result -Why 'unsupported architecture' -Needle 'unsupported architecture'

# --- Checksum verification ------------------------------------------------

$mirror = Build-Mirror
Publish-Asset -Mirror $mirror -Target 'x86_64-pc-windows-msvc' -Content $junk
# Corrupt the sidecar.
$sidecar = Get-ChildItem -Path $mirror -Recurse -Filter '*.sha256' | Select-Object -First 1
[IO.File]::WriteAllText($sidecar.FullName, ('0' * 64) + '  ' + $sidecar.Name, [Text.Encoding]::ASCII)
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install')
Assert-Fails -Result $result -Why 'bad checksum' -Needle 'checksum verification failed'

$mirror = Build-Mirror
Publish-Asset -Mirror $mirror -Target 'x86_64-pc-windows-msvc' -Content $junk
# Malformed sidecar (not a hex digest).
$sidecar = Get-ChildItem -Path $mirror -Recurse -Filter '*.sha256' | Select-Object -First 1
[IO.File]::WriteAllText($sidecar.FullName, 'not-a-checksum  x', [Text.Encoding]::ASCII)
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install')
Assert-Fails -Result $result -Why 'malformed checksum' -Needle 'checksum verification failed'

# --- Missing assets -------------------------------------------------------

$mirror = Build-Mirror
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install')
Assert-Fails -Result $result -Why 'missing archive' -Needle 'asset not found in the local mirror'

$mirror = Build-Mirror
$name = "terminal-file-manager-$TestTag-x86_64-pc-windows-msvc.zip"
[System.IO.File]::WriteAllBytes((Join-Path -Path $mirror -ChildPath "download\$TestTag\$name"), $junk)
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install')
Assert-Fails -Result $result -Why 'missing checksum sidecar' -Needle 'terminal-file-manager-v99.88.77-x86_64-pc-windows-msvc.zip.sha256'

# --- Candidate validation -------------------------------------------------

$mirror = Build-Mirror
Publish-Asset -Mirror $mirror -Target 'x86_64-pc-windows-msvc' -Content (Build-ZipBytes -Files @{ 'tfm.exe' = $junk })
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install')
Assert-Fails -Result $result -Why 'corrupt candidate' -Needle 'candidate failed --version validation'

$mirror = Build-Mirror
$zipWithoutExe = Build-ZipBytes -Files @{ 'README.md' = [System.Text.Encoding]::UTF8.GetBytes('readme') }
Publish-Asset -Mirror $mirror -Target 'x86_64-pc-windows-msvc' -Content $zipWithoutExe
$result = Invoke-Installer -Mirror $mirror -InstallDir (Join-Path -Path $root -ChildPath 'install')
Assert-Fails -Result $result -Why 'archive without tfm.exe' -Needle 'release archive is missing tfm.exe'

# --- Failure must preserve an existing installation -----------------------

$preserveDir = Join-Path -Path $root -ChildPath 'preserve'
New-Item -ItemType Directory -Path $preserveDir -Force | Out-Null
[System.IO.File]::WriteAllText((Join-Path -Path $preserveDir -ChildPath 'tfm.exe'), 'old tfm')
$mirror = Build-Mirror
$sidecarName = "terminal-file-manager-$TestTag-x86_64-pc-windows-msvc.zip.sha256"
Publish-Asset -Mirror $mirror -Target 'x86_64-pc-windows-msvc' -Content $junk
$sidecar = Join-Path -Path $mirror -ChildPath "download\$TestTag\$sidecarName"
[IO.File]::WriteAllText($sidecar, ('0' * 64) + "  $sidecarName", [Text.Encoding]::ASCII)
$result = Invoke-Installer -Mirror $mirror -InstallDir $preserveDir
Assert-Fails -Result $result -Why 'failed install' -Needle 'checksum verification failed'
if ((Get-Content -Path (Join-Path -Path $preserveDir -ChildPath 'tfm.exe') -Raw) -ne 'old tfm') {
    Fail -Message 'failed installation changed the existing binary'
}
if (Get-ChildItem -Path $preserveDir -Force -Filter '.tfm-*' -ErrorAction SilentlyContinue) {
    Fail -Message 'failed installation left staging files'
}
Pass

# --- Latest via local marker ---------------------------------------------

$mirror = Build-Mirror
Publish-Asset -Mirror $mirror -Target 'x86_64-pc-windows-msvc' -Content (Build-ZipBytes -Files @{ 'tfm.exe' = $junk })
[IO.File]::WriteAllText((Join-Path -Path $mirror -ChildPath 'latest'), $TestTag, [Text.Encoding]::ASCII)
# No TFM_VERSION: the marker provides the tag. The corrupt
# candidate then fails version validation, proving the marker was used.
$psi = [System.Diagnostics.ProcessStartInfo]::new()
$psi.FileName = $hostExecutable
$psi.Arguments = "-NoProfile -File `"$portableWrapper`""
$psi.UseShellExecute = $false
$psi.RedirectStandardOutput = $true
$psi.RedirectStandardError = $true
$psi.EnvironmentVariables['TFM_BASE_URL'] = $mirror
$psi.EnvironmentVariables['TFM_INSTALL_DIR'] = (Join-Path -Path $root -ChildPath 'install')
$psi.EnvironmentVariables['TFM_TEST_ARCHITECTURE'] = 'AMD64'
$psi.EnvironmentVariables.Remove('TFM_VERSION')
$process = [System.Diagnostics.Process]::Start($psi)
$markerOutput = $process.StandardOutput.ReadToEnd() + $process.StandardError.ReadToEnd()
$process.WaitForExit()
if ($process.ExitCode -eq 0 -or $markerOutput -notmatch 'candidate failed') {
    Fail -Message "local latest marker was not used: $markerOutput"
}
Pass

$mirror = Build-Mirror
[IO.File]::WriteAllText((Join-Path -Path $mirror -ChildPath 'latest'), 'not-a-version', [Text.Encoding]::ASCII)
$psi2 = [System.Diagnostics.ProcessStartInfo]::new()
$psi2.FileName = $psi.FileName
$psi2.Arguments = $psi.Arguments
$psi2.UseShellExecute = $false
$psi2.RedirectStandardOutput = $true
$psi2.RedirectStandardError = $true
$psi2.EnvironmentVariables['TFM_BASE_URL'] = $mirror
$psi2.EnvironmentVariables['TFM_INSTALL_DIR'] = (Join-Path -Path $root -ChildPath 'install')
$psi2.EnvironmentVariables['TFM_TEST_ARCHITECTURE'] = 'AMD64'
$psi2.EnvironmentVariables['TFM_BASE_URL'] = $mirror
$psi2.EnvironmentVariables.Remove('TFM_VERSION')
$process = [System.Diagnostics.Process]::Start($psi2)
$badMarkerOutput = $process.StandardOutput.ReadToEnd() + $process.StandardError.ReadToEnd()
$process.WaitForExit()
if ($process.ExitCode -eq 0 -or $badMarkerOutput -notmatch 'usable release tag') {
    Fail -Message "malformed local latest marker accepted: $badMarkerOutput"
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
            Fail -Message 'native --version did not terminate within ten seconds'
        }
        if ($process.ExitCode -ne 0) { Fail -Message 'native --version failed' }
        return $stdout.GetAwaiter().GetResult().TrimEnd("`r", "`n")
    } finally { $process.Dispose() }
}

# --- Success path: needs a real Windows binary ----------------------------

$builtExe = if ($Binary) { (Resolve-Path -Path $Binary).Path } else {
    Join-Path -Path $PSScriptRoot -ChildPath '..\..\target\release\tfm.exe'
}
if (-not $nativeWindows) {
    Skip -Reason 'success-path cases require Windows (real tfm.exe)'
} elseif (-not (Test-Path -Path $builtExe)) {
    Fail -Message "success-path cases require a built binary at $builtExe (run cargo make release first)"
} else {
    $realVersion = (Get-PromptNativeVersion -Exe $builtExe) -replace '^tfm ', ''
    $realTag = "v$realVersion"
    . $resolvedInstaller
    $runnerArch = Get-NativeArchitecture
    Pass

    $mirror = Build-Mirror
    $zip = Build-ZipBytes -Files @{ 'tfm.exe' = [System.IO.File]::ReadAllBytes($builtExe) }
    $name = "terminal-file-manager-$realTag-$runnerArch-pc-windows-msvc.zip" -replace '-AMD64-', '-x86_64-' -replace '-ARM64-', '-aarch64-'
    $dir = Join-Path -Path $mirror -ChildPath "download\$realTag"
    New-Item -ItemType Directory -Path $dir -Force | Out-Null
    [System.IO.File]::WriteAllBytes((Join-Path -Path $dir -ChildPath $name), $zip)
    $hash = (Get-FileHash -Path (Join-Path -Path $dir -ChildPath $name) -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path -Path $dir -ChildPath "$name.sha256"), "$hash  $name", [Text.Encoding]::ASCII)

    $installDir = Join-Path -Path $root -ChildPath 'success'
    $result = Invoke-Installer -Mirror $mirror -InstallDir $installDir -Version $realTag -Architecture $runnerArch -Native $true
    Assert-Succeeds -Result $result -Why 'successful installation'
    $installed = Join-Path -Path $installDir -ChildPath 'tfm.exe'
    if (-not (Test-Path -Path $installed)) { Fail -Message 'installed binary missing' }
    if ((Get-PromptNativeVersion -Exe $installed) -ne "tfm $realVersion") { Fail -Message 'installed binary reports the wrong version' }
    Pass

    # Replacement of an existing installation.
    $result = Invoke-Installer -Mirror $mirror -InstallDir $installDir -Version $realTag -Architecture $runnerArch -Native $true
    Assert-Succeeds -Result $result -Why 'replacement of existing installation'
    if ((Get-PromptNativeVersion -Exe $installed) -ne "tfm $realVersion") { Fail -Message 'replaced binary is wrong' }
    Pass

    # Wrong advertised version must be rejected and change nothing.
    $before = (Get-FileHash -Path $installed -Algorithm SHA256).Hash
    $wrongDir = Join-Path -Path $mirror -ChildPath 'download/v0.0.0'
    New-Item -ItemType Directory -Path $wrongDir | Out-Null
    $wrongName = $name.Replace($realTag, 'v0.0.0')
    [IO.File]::WriteAllBytes((Join-Path -Path $wrongDir -ChildPath $wrongName), $zip)
    [IO.File]::WriteAllText((Join-Path -Path $wrongDir -ChildPath "$wrongName.sha256"), "$hash  $wrongName", [Text.Encoding]::ASCII)
    $result = Invoke-Installer -Mirror $mirror -InstallDir $installDir -Version 'v0.0.0' -Architecture $runnerArch -Native $true
    Assert-Fails -Result $result -Why 'wrong advertised version' -Needle 'reported version does not match'
    if ((Get-FileHash -Path $installed -Algorithm SHA256).Hash -ne $before) { Fail -Message 'wrong-version attempt changed the installation' }
    Pass
}

Write-Information -MessageData "PowerShell installer tests passed: $($script:Passes) cases ($($script:Skipped) skipped)." -InformationAction Continue
Remove-Item -Path $root -Recurse -Force -ErrorAction SilentlyContinue
