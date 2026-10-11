# D2-D6 regressions using production functions and isolated local files.
# Non-Windows File.Replace results are simulations, never native Windows proof.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$securityRoot = Join-Path ([IO.Path]::GetTempPath()) ('tfm-security-' + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $securityRoot | Out-Null
$securityPasses = 0
$securitySkipped = 0
. (Join-Path $PSScriptRoot '../../install.ps1')
# A test-only exception boundary permits independent negative assertions.
function Die([string]$Message) { throw $Message }

function Assert-Rejected([scriptblock]$Action, [string]$Reason) {
    $rejected = $false
    try { & $Action } catch { $rejected = $true }
    if (-not $rejected) { throw "accepted unsafe input: $Reason" }
    $script:securityPasses++
}

function Assert-Equal([object]$Actual, [object]$Expected, [string]$Reason) {
    if ($Actual -cne $Expected) { throw "${Reason}: expected $Expected, got $Actual" }
    $script:securityPasses++
}

function New-MemberZip([string[]]$Names, [int]$Attributes = 0) {
    $path = Join-Path $securityRoot ([IO.Path]::GetRandomFileName() + '.zip')
    $zip = [IO.Compression.ZipFile]::Open($path, [IO.Compression.ZipArchiveMode]::Create)
    try {
        foreach ($name in $Names) {
            $entry = $zip.CreateEntry($name)
            $entry.ExternalAttributes = $Attributes
            $stream = $entry.Open()
            try { $stream.WriteByte(65) } finally { $stream.Dispose() }
        }
    } finally { $zip.Dispose() }
    return $path
}

function Write-Sidecar([string]$Archive, [string]$Record) {
    [IO.File]::WriteAllText("$Archive.sha256", $Record, [Text.Encoding]::ASCII)
}

function Assert-OldBytes([string]$Directory) {
    Assert-Equal ([IO.File]::ReadAllText((Join-Path $Directory 'tfm.exe'))) 'old tfm' 'old bytes changed'
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
try {
    $members = @('tfm.exe', 'LICENSE-MIT', 'LICENSE-APACHE', 'README.md')
    $archive = New-MemberZip $members
    $hash = (Get-FileHash $archive -Algorithm SHA256).Hash
    $basename = [IO.Path]::GetFileName($archive)
    foreach ($record in @("$hash  wrong.zip", "$hash  $basename`n$hash  $basename",
        "$hash  $basename extra", "not-hex  $basename", " $hash  $basename", "$hash  $basename`n`n")) {
        Write-Sidecar $archive $record
        Assert-Rejected { Assert-Checksum $archive "$archive.sha256" } 'sidecar structure'
    }
    [IO.File]::WriteAllBytes("$archive.sha256", [byte[]](239, 187, 191) + [Text.Encoding]::ASCII.GetBytes("$hash  $basename"))
    Assert-Rejected { Assert-Checksum $archive "$archive.sha256" } 'UTF-8 BOM'
    [IO.File]::WriteAllText("$archive.sha256", "$hash  $basename", [Text.Encoding]::Unicode)
    Assert-Rejected { Assert-Checksum $archive "$archive.sha256" } 'UTF-16'
    Write-Sidecar $archive "$hash  $basename"
    Assert-Checksum $archive "$archive.sha256"
    $securityPasses++
    [IO.File]::AppendAllText($archive, 'tampered')
    Assert-Rejected { Assert-Checksum $archive "$archive.sha256" } 'checksum mismatch'

    $outside = Join-Path $securityRoot 'outside'
    [IO.File]::WriteAllText($outside, 'outside sentinel')
    foreach ($bad in @('extra', '../outside', '/outside', 'nested/tfm.exe', './tfm.exe',
        'tfm.exe', 'TFM.EXE', 'tfm.exe:stream', 'tfm.exe.', '\outside', 'C:\outside', '\\server\share')) {
        $archive = New-MemberZip ($members + $bad)
        $extracted = Join-Path $securityRoot ([IO.Path]::GetRandomFileName())
        # Malicious archives have valid checksums; integrity is not structure.
        $hash = (Get-FileHash $archive -Algorithm SHA256).Hash
        Write-Sidecar $archive "$hash  $([IO.Path]::GetFileName($archive))"
        Assert-Checksum $archive "$archive.sha256"
        Assert-Rejected { Expand-SafeArchive $archive $extracted } "archive member $bad"
        Assert-Equal (Test-Path $extracted) $false 'unsafe archive extracted'
        Assert-Equal ([IO.File]::ReadAllText($outside)) 'outside sentinel' 'outside bytes changed'
    }
    foreach ($attributes in @(268435456, 536870912, 1610612736, -1610612736, -1073741824, 16, 64, 1024)) {
        $archive = New-MemberZip $members $attributes
        Assert-Rejected { Expand-SafeArchive $archive (Join-Path $securityRoot 'types') } 'archive type'
    }
    foreach ($omitted in $members) {
        $archive = New-MemberZip @($members | Where-Object { $_ -ne $omitted })
        Assert-Rejected { Expand-SafeArchive $archive (Join-Path $securityRoot 'missing') } 'missing member'
    }
    $archive = New-MemberZip $members -2119958528
    # 0x81A40000 denotes a regular Unix file with mode 0644.
    Expand-SafeArchive $archive (Join-Path $securityRoot 'regular')
    $securityPasses++

    $exe = Join-Path $securityRoot 'candidate.exe'
    $bytes = New-Object byte[] 128
    $bytes[0] = 77; $bytes[1] = 90; $bytes[60] = 64
    $bytes[64] = 80; $bytes[65] = 69
    $bytes[68] = 100; $bytes[69] = 134; $bytes[86] = 2
    [IO.File]::WriteAllBytes($exe, $bytes)
    Assert-CandidateArchitecture $exe 'x86_64-pc-windows-msvc'
    $securityPasses++
    Assert-Rejected { Assert-CandidateArchitecture $exe 'aarch64-pc-windows-msvc' } 'wrong machine'
    $bytes[68] = 100; $bytes[69] = 170
    [IO.File]::WriteAllBytes($exe, $bytes)
    Assert-CandidateArchitecture $exe 'aarch64-pc-windows-msvc'
    $securityPasses++
    foreach ($offset in @(0, 64, 86)) {
        $badBytes = $bytes.Clone(); $badBytes[$offset] = 0
        [IO.File]::WriteAllBytes($exe, $badBytes)
        Assert-Rejected { Assert-CandidateArchitecture $exe 'aarch64-pc-windows-msvc' } 'bad PE header'
    }
    [IO.File]::WriteAllBytes($exe, [byte[]](77, 90))
    Assert-Rejected { Assert-CandidateArchitecture $exe 'x86_64-pc-windows-msvc' } 'truncated DOS header'

    foreach ($tag in @('v1.2', '1.2.3', 'v1.2.3-rc.1', 'v1.2.3/extra', "v1.2.3`nmore", "v1.2.3`n")) {
        Assert-Rejected { Assert-StableTag $tag } 'unstable tag'
    }
    $script:ReleasesBase = 'https://github.com/seapagan/terminal-file-manager/releases'
    foreach ($uri in @('http://github.com/file', 'https://evil.example/file', 'https://github.com:444/file',
        'https://user@github.com/file', 'https://github.com/file#fragment')) {
        Assert-Rejected { Assert-AssetUri ([uri]$uri) } 'unsafe transport'
    }
    foreach ($url in @('http://mirror.example', 'ftp://mirror.example', 'file:///tmp/mirror')) {
        Assert-Rejected { Test-IsHttp $url } 'non-HTTPS source'
    }
    Assert-AssetUri ([uri]'https://release-assets.githubusercontent.com/file')
    $securityPasses++

    $destination = Join-Path $securityRoot 'destination'
    New-Item -ItemType Directory -Path $destination | Out-Null
    [IO.File]::WriteAllText($exe, 'new tfm')
    Commit-Candidate $exe $destination
    Assert-Equal ([IO.File]::ReadAllText((Join-Path $destination 'tfm.exe'))) 'new tfm' 'fresh transaction'
    [IO.File]::WriteAllText((Join-Path $destination 'tfm.exe'), 'old tfm')
    Commit-Candidate $exe $destination
    Assert-Equal ([IO.File]::ReadAllText((Join-Path $destination 'tfm.exe'))) 'new tfm' 'replacement transaction'
    Assert-Equal @(Get-ChildItem $destination -Force -Filter '.tfm-*').Count 0 'transaction cleanup'
    [IO.File]::WriteAllText((Join-Path $destination 'tfm.exe'), 'old tfm')
    Assert-Rejected { Commit-Candidate (Join-Path $securityRoot 'missing.exe') $destination } 'missing candidate'
    Assert-OldBytes $destination
    Assert-Equal @(Get-ChildItem $destination -Force -Filter '.tfm-*').Count 0 'copy failure cleanup'
    New-Item -ItemType Directory -Path (Join-Path $securityRoot 'directory-final/tfm.exe') -Force | Out-Null
    Assert-Rejected { Commit-Candidate $exe (Join-Path $securityRoot 'directory-final') } 'final directory'
    [IO.File]::WriteAllText((Join-Path $securityRoot 'file-ancestor'), 'file')
    Assert-Rejected { Commit-Candidate $exe (Join-Path $securityRoot 'file-ancestor/child') } 'non-directory ancestor'

    # Recovery succeeds for absent and existing finals. Failed recovery keeps
    # the last good bytes and includes their exact location in the error.
    foreach ($exists in @($false, $true)) {
        $backup = New-OwnedFile $securityRoot
        [IO.File]::WriteAllText($backup, 'old tfm')
        $oldHash = (Get-FileHash $backup -Algorithm SHA256).Hash
        $recovery = Join-Path $securityRoot ('recover-' + [IO.Path]::GetRandomFileName())
        if ($exists) { [IO.File]::WriteAllText($recovery, 'new tfm') }
        Restore-Backup $backup $recovery $oldHash
        Assert-Equal ([IO.File]::ReadAllText($recovery)) 'old tfm' 'rollback bytes'
    }
    $backup = New-OwnedFile $securityRoot
    [IO.File]::WriteAllText($backup, 'old tfm')
    $oldHash = (Get-FileHash $backup -Algorithm SHA256).Hash
    $recoveryDirectory = Join-Path $securityRoot 'recovery-directory'
    New-Item -ItemType Directory -Path $recoveryDirectory | Out-Null
    $errorMessage = ''
    try { Restore-Backup $backup $recoveryDirectory $oldHash }
    catch { $errorMessage = $_.Exception.Message }
    Assert-Equal ($errorMessage.Contains($backup)) $true 'recovery location missing'
    Assert-Equal ([IO.File]::ReadAllText($backup)) 'old tfm' 'last good copy discarded'

    # R2: a relative destination must inspect current-directory ancestors
    # before creating anything through a linked/reparse working directory.
    $relativeOutside = Join-Path $securityRoot 'relative-outside'
    New-Item -ItemType Directory -Path $relativeOutside | Out-Null
    $outsideSentinel = Join-Path $relativeOutside 'sentinel'
    [IO.File]::WriteAllText($outsideSentinel, 'outside sentinel')
    $workingLink = Join-Path $securityRoot 'working-link'
    $linkType = if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        'Junction'
    } else { 'SymbolicLink' }
    New-Item -ItemType $linkType -Path $workingLink -Target $relativeOutside | Out-Null
    $savedLocation = Get-Location
    try {
        Set-Location -LiteralPath $workingLink
        Assert-Rejected { Commit-Candidate $exe 'child' } 'linked relative working directory'
        Assert-Equal (Test-Path -LiteralPath (Join-Path $relativeOutside 'child')) $false 'relative destination created through link'
        Assert-Equal @(Get-ChildItem -LiteralPath $relativeOutside -Force).Count 1 'outside object created'
        Assert-Equal ([IO.File]::ReadAllText($outsideSentinel)) 'outside sentinel' 'outside sentinel changed'
    } finally { Set-Location -LiteralPath $savedLocation.Path }

    # Inject only a destination mutation after production stage creation.
    $originalNewOwnedFile = (Get-Item Function:\New-OwnedFile).ScriptBlock
    function New-OwnedFile([string]$Directory) {
        $path = & $originalNewOwnedFile $Directory
        [IO.File]::WriteAllText((Join-Path $Directory 'tfm.exe'), 'external replacement')
        return $path
    }
    try {
        Assert-Rejected { Commit-Candidate $exe $destination } 'changed final object'
        Assert-Equal ([IO.File]::ReadAllText((Join-Path $destination 'tfm.exe'))) 'external replacement' 'external object overwritten'
    } finally { Set-Item Function:\New-OwnedFile $originalNewOwnedFile }
    [IO.File]::WriteAllText((Join-Path $destination 'tfm.exe'), 'old tfm')

    # Exercise the production post-replace error/rollback path, not just the
    # recovery helper. The wrapper injects an error after the real OS primitive.
    $originalReplace = (Get-Item Function:\Replace-File).ScriptBlock
    function Replace-File([string]$Stage, [string]$Final, [string]$Backup) {
        & $originalReplace $Stage $Final $Backup
        throw 'injected error after replacement'
    }
    try {
        Assert-Rejected { Commit-Candidate $exe $destination } 'post-replace failure'
        Assert-OldBytes $destination
        Assert-Equal @(Get-ChildItem $destination -Force -Filter '.tfm-*').Count 0 'rollback cleanup'
        $originalRestore = (Get-Item Function:\Restore-Backup).ScriptBlock
        function Restore-Backup([string]$Backup, [string]$Final, [string]$OldHash) {
            throw "injected recovery failure; last good copy retained at $Backup"
        }
        try {
            Assert-Rejected { Commit-Candidate $exe $destination } 'recovery failure'
            $retained = @(Get-ChildItem $destination -Force -Filter '.tfm-*')
            Assert-Equal $retained.Count 1 'recovery material count'
            Assert-Equal ([IO.File]::ReadAllText($retained[0].FullName)) 'old tfm' 'recovery material bytes'
        } finally { Set-Item Function:\Restore-Backup $originalRestore }
    } finally { Set-Item Function:\Replace-File $originalReplace }
    # Reset only fixture material after checking its preservation.
    Get-ChildItem $destination -Force -Filter '.tfm-*' | Remove-Item -Force
    [IO.File]::WriteAllText((Join-Path $destination 'tfm.exe'), 'old tfm')

    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        $runningSource = Join-Path $securityRoot 'running.rs'
        [IO.File]::WriteAllText($runningSource, @'
fn main() {
    if std::env::args().any(|arg| arg == "--version") { println!("tfm 99.88.77"); }
    else { std::thread::sleep(std::time::Duration::from_secs(60)); }
}
'@, [Text.Encoding]::ASCII)
        $runningExe = Join-Path $securityRoot 'running.exe'
        & rustc $runningSource -o $runningExe
        if ($LASTEXITCODE -ne 0) { throw 'native running-executable fixture build failed' }
        $runningDir = Join-Path $securityRoot 'running-install'
        New-Item -ItemType Directory -Path $runningDir | Out-Null
        $runningFinal = Join-Path $runningDir 'tfm.exe'
        [IO.File]::Copy($runningExe, $runningFinal)
        $runningHash = (Get-FileHash $runningFinal -Algorithm SHA256).Hash
        $runningProcess = [Diagnostics.Process]::Start($runningFinal)
        try {
            Assert-Rejected { Commit-Candidate $exe $runningDir } 'running existing executable'
            Assert-Equal (Get-FileHash $runningFinal -Algorithm SHA256).Hash $runningHash 'running executable bytes'
        } finally {
            if (-not $runningProcess.HasExited) { $runningProcess.Kill(); $runningProcess.WaitForExit() }
            $runningProcess.Dispose()
        }
        $nativeTarget = Get-Target
        $savedArchitecture = $env:PROCESSOR_ARCHITECTURE
        try {
            $env:PROCESSOR_ARCHITECTURE = 'x86'
            Assert-Equal (Get-Target) $nativeTarget 'process architecture selected over native host'
        } finally { $env:PROCESSOR_ARCHITECTURE = $savedArchitecture }
        $final = Join-Path $destination 'tfm.exe'
        $lock = [IO.File]::Open($final, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        try { Assert-Rejected { Commit-Candidate $exe $destination } 'locked existing binary' }
        finally { $lock.Dispose() }
        Assert-OldBytes $destination
        [IO.File]::SetAttributes($final, [IO.FileAttributes]::ReadOnly)
        try { Assert-Rejected { Commit-Candidate $exe $destination } 'read-only existing binary' }
        finally { [IO.File]::SetAttributes($final, [IO.FileAttributes]::Normal) }
        Assert-OldBytes $destination
        Assert-Equal @(Get-ChildItem $destination -Force -Filter '.tfm-*').Count 0 'read-only failure cleanup'
        $acl = Get-Acl -LiteralPath $destination
        $denied = Get-Acl -LiteralPath $destination
        $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
        $rule = [Security.AccessControl.FileSystemAccessRule]::new($sid,
            [Security.AccessControl.FileSystemRights]::CreateFiles, [Security.AccessControl.AccessControlType]::Deny)
        $denied.AddAccessRule($rule)
        Set-Acl -LiteralPath $destination -AclObject $denied
        try { Assert-Rejected { Commit-Candidate $exe $destination } 'denied staging directory' }
        finally { Set-Acl -LiteralPath $destination -AclObject $acl }
        Assert-OldBytes $destination
        $linked = Join-Path $securityRoot 'junction'
        New-Item -ItemType Junction -Path $linked -Target $destination | Out-Null
        Assert-Rejected { Commit-Candidate $exe $linked } 'reparse install root'
        Assert-Rejected { Commit-Candidate $exe (Join-Path $linked 'child') } 'reparse ancestor'
        Assert-OldBytes $destination
    } else {
        $securitySkipped = 13
        Write-Host 'SKIP: thirteen native Windows running-file/architecture/lock/read-only/denied-path/junction assertions; transaction successes above are Unix simulations.'
    }
} finally {
    Remove-Item -LiteralPath $securityRoot -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Host "PowerShell security tests passed: $securityPasses assertions ($securitySkipped native assertions skipped)."
