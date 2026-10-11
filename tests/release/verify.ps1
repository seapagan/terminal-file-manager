# Tests for scripts/verify-release-binary.ps1 using synthetic PE headers.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

$script:Passes = 0

function Fail([string]$Message) {
    Write-Error -Message "FAILED: $Message" -ErrorAction Continue
    exit 1
}

function Pass { $script:Passes++ }

$root = Join-Path -Path ([System.IO.Path]::GetTempPath()) `
    -ChildPath ("tfm-pe-tests-" + [System.IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $root | Out-Null
$verifier = Join-Path -Path $PSScriptRoot -ChildPath '..\..\scripts\verify-release-binary.ps1'
$verifier = (Resolve-Path -Path $verifier).Path

function Build-Pe([int]$Machine, [byte[]]$Prefix = @()) {
    # Required isolated fixture construction; skipping it invalidates the test.
    # Minimal MZ/PE structure: DOS header with e_lfanew, PE signature,
    # and the machine field.
    $bytes = New-Object -TypeName byte[] -ArgumentList 256
    $bytes[0] = 0x4D; $bytes[1] = 0x5A
    [BitConverter]::GetBytes([int]0x80).CopyTo($bytes, 0x3C)
    $bytes[0x80] = 0x50; $bytes[0x81] = 0x45; $bytes[0x82] = 0; $bytes[0x83] = 0
    [BitConverter]::GetBytes([uint16]$Machine).CopyTo($bytes, 0x84)
    if ($Prefix.Length -gt 0) { $Prefix.CopyTo($bytes, 0) }
    $path = Join-Path -Path $root -ChildPath ("pe-" + [System.IO.Path]::GetRandomFileName() + '.exe')
    [System.IO.File]::WriteAllBytes($path, $bytes)
    return $path
}

function Invoke-Verifier([string]$Arch, [string]$Binary) {
    $psi = [System.Diagnostics.ProcessStartInfo]::new()
    $psi.FileName = (Get-Command -Name pwsh -ErrorAction SilentlyContinue).Source
    if (-not $psi.FileName) { $psi.FileName = (Get-Command -Name powershell).Source }
    $psi.Arguments = "-NoProfile -File `"$verifier`" $Arch `"$Binary`""
    $psi.UseShellExecute = $false
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $process = [System.Diagnostics.Process]::Start($psi)
    $output = $process.StandardOutput.ReadToEnd() + $process.StandardError.ReadToEnd()
    $process.WaitForExit()
    [pscustomobject]@{ Code = $process.ExitCode; Output = $output }
}

function Assert-Passes([string]$Why, [string]$Arch, [string]$Binary) {
    $result = Invoke-Verifier -Arch $Arch -Binary $Binary
    if ($result.Code -ne 0) { Fail -Message "${Why}: $($result.Output)" }
    Pass
}

function Assert-Fails([string]$Why, [string]$Arch, [string]$Binary, [string]$Needle) {
    $result = Invoke-Verifier -Arch $Arch -Binary $Binary
    if ($result.Code -eq 0) { Fail -Message "$Why unexpectedly succeeded" }
    if ($result.Output -notmatch [regex]::Escape($Needle)) {
        Fail -Message "${Why}: expected `"$Needle`" in: $($result.Output)"
    }
    Pass
}

$x64 = Build-Pe -Machine 0x8664
$arm64 = Build-Pe -Machine 0xAA64

Assert-Passes -Why 'valid x86_64 PE' -Arch 'x86_64' -Binary $x64
Assert-Passes -Why 'valid aarch64 PE' -Arch 'aarch64' -Binary $arm64
Assert-Fails -Why 'wrong architecture rejected' -Arch 'x86_64' -Binary $arm64 -Needle 'expected x86_64'
Assert-Fails -Why 'arm64 mismatch rejected' -Arch 'aarch64' -Binary $x64 -Needle 'expected aarch64'

$notPe = Join-Path -Path $root -ChildPath 'not-pe.exe'
[System.IO.File]::WriteAllBytes($notPe, [System.Text.Encoding]::UTF8.GetBytes(('definitely not a PE file. ' * 10)))
Assert-Fails -Why 'non-PE rejected' -Arch 'x86_64' -Binary $notPe -Needle 'MZ signature'

$short = Join-Path -Path $root -ChildPath 'short.exe'
[System.IO.File]::WriteAllBytes($short, (New-Object -TypeName byte[] -ArgumentList 10))
Assert-Fails -Why 'tiny file rejected' -Arch 'x86_64' -Binary $short -Needle 'too small'

$missing = Join-Path -Path $root -ChildPath 'absent.exe'
Assert-Fails -Why 'missing file rejected' -Arch 'x86_64' -Binary $missing -Needle 'not found'

# D1: signature corruption and DOS-header offsets at invalid boundaries.
$badSignature = Build-Pe -Machine 0x8664
$bytes = [System.IO.File]::ReadAllBytes($badSignature)
$bytes[0x82] = 1
[System.IO.File]::WriteAllBytes($badSignature, $bytes)
Assert-Fails -Why 'corrupt PE signature rejected' -Arch 'x86_64' -Binary $badSignature -Needle 'PE signature'

foreach ($offset in @(-1, 256, 251, [int]::MaxValue)) {
    $badOffset = Build-Pe -Machine 0x8664
    $bytes = [System.IO.File]::ReadAllBytes($badOffset)
    [BitConverter]::GetBytes([int]$offset).CopyTo($bytes, 0x3C)
    [System.IO.File]::WriteAllBytes($badOffset, $bytes)
    Assert-Fails -Why "invalid PE offset $offset rejected" -Arch 'x86_64' -Binary $badOffset -Needle 'out of bounds'
}

Write-Information -MessageData "PE verification tests passed: $script:Passes cases." -InformationAction Continue
Remove-Item -Path $root -Recurse -Force -ErrorAction SilentlyContinue
