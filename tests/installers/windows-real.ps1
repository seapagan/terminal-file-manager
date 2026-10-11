# Matching-native, actual archive acceptance. No architecture or candidate mocks.
param(
    [Parameter(Mandatory=$true)][string]$Archive,
    [Parameter(Mandatory=$true)][string]$Target,
    [Parameter(Mandatory=$true)][string]$Version,
    [Parameter(Mandatory=$true)][string]$Binary
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
    throw 'real ZIP acceptance requires native Windows'
}
$archivePath = (Resolve-Path $Archive).Path
$binaryPath = (Resolve-Path $Binary).Path
$name = "terminal-file-manager-v$Version-$Target.zip"
if ([IO.Path]::GetFileName($archivePath) -ne $name) { throw 'incorrect real archive name' }
$installer = (Resolve-Path (Join-Path $PSScriptRoot '..\..\install.ps1')).Path
$hostExecutable = Join-Path $PSHOME $(if ($PSVersionTable.PSVersion.Major -lt 6) { 'powershell.exe' } else { 'pwsh.exe' })
$root = Join-Path ([IO.Path]::GetTempPath()) ('tfm-real-' + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $root | Out-Null

function Invoke-RealInstaller([string]$Mirror, [string]$Destination) {
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $hostExecutable
    $psi.Arguments = "-NoProfile -File `"$installer`""
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.EnvironmentVariables['TFM_BASE_URL'] = $Mirror
    $psi.EnvironmentVariables['TFM_INSTALL_DIR'] = $Destination
    $psi.EnvironmentVariables['TFM_VERSION'] = "v$Version"
    $process = [Diagnostics.Process]::Start($psi)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        if (-not $process.WaitForExit(30000)) {
            $process.Kill()
            $process.WaitForExit()
            throw 'real installer did not complete promptly'
        }
        return @{ Code = $process.ExitCode; Output = $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult() }
    } finally { $process.Dispose() }
}

function Assert-PromptVersion([string]$Path) {
    $psi = New-Object Diagnostics.ProcessStartInfo
    $psi.FileName = $Path
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
            throw 'real tfm --version did not terminate within ten seconds'
        }
        if ($process.ExitCode -ne 0 -or $stdout.GetAwaiter().GetResult().TrimEnd("`r", "`n") -ne "tfm $Version" -or $stderr.GetAwaiter().GetResult()) {
            throw 'incorrect real tfm version or exit status'
        }
    } finally { $process.Dispose() }
}

try {
    $mirror = Join-Path $root 'mirror'
    $download = Join-Path $mirror "download\v$Version"
    $destination = Join-Path $root 'installed'
    New-Item -ItemType Directory -Path $download -Force | Out-Null
    Copy-Item -LiteralPath $archivePath -Destination (Join-Path $download $name)
    Copy-Item -LiteralPath "$archivePath.sha256" -Destination (Join-Path $download "$name.sha256")
    $expected = (Get-FileHash $binaryPath -Algorithm SHA256).Hash
    $final = Join-Path $destination 'tfm.exe'
    Assert-PromptVersion $binaryPath
    foreach ($operation in @('fresh', 'reinstall')) {
        $result = Invoke-RealInstaller $mirror $destination
        if ($result.Code -ne 0) { throw $result.Output }
        if ((Get-FileHash $final -Algorithm SHA256).Hash -ne $expected) { throw 'installed bytes differ from verified build' }
        Assert-PromptVersion $final
        Write-Host "Real archive ${Target}: $operation passed"
    }
    [IO.File]::AppendAllText((Join-Path $download $name), 'corrupt')
    $result = Invoke-RealInstaller $mirror $destination
    if ($result.Code -eq 0) { throw 'corrupted real archive was installed' }
    if ((Get-FileHash $final -Algorithm SHA256).Hash -ne $expected) { throw 'failed installation changed old bytes' }
    if (Get-ChildItem $destination -Filter '.tfm-*' -Force) { throw 'owned stage leaked' }
    Write-Host "Real archive ${Target}: corruption rejected; old bytes preserved"
} finally { Remove-Item -LiteralPath $root -Recurse -Force }
