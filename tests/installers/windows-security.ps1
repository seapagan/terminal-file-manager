# D2-D6 regressions using production functions and isolated local files.
# Non-Windows File.Replace results are simulations, never native Windows proof.
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0
$securityRoot = Join-Path -Path ([IO.Path]::GetTempPath()) -ChildPath ('tfm-security-' + [IO.Path]::GetRandomFileName())
New-Item -ItemType Directory -Path $securityRoot | Out-Null
$securityPasses = 0
$securitySkipped = 0
. (Join-Path -Path $PSScriptRoot -ChildPath '../../install.ps1')
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

function Build-MemberZip([string[]]$Names, [int]$Attributes = 0) {
    # Required isolated fixture construction; skipping it invalidates the test.
    $path = Join-Path -Path $securityRoot -ChildPath ([IO.Path]::GetRandomFileName() + '.zip')
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
    Assert-Equal -Actual ([IO.File]::ReadAllText((Join-Path -Path $Directory -ChildPath 'tfm.exe'))) -Expected 'old tfm' -Reason 'old bytes changed'
}

function Invoke-ControlledAction([string]$Action, [string]$Answer = 'N') {
    $installer = (Resolve-Path -LiteralPath (Join-Path -Path $PSScriptRoot '../../install.ps1')).Path.Replace("'", "''")
    $path = Join-Path -Path $securityRoot -ChildPath 'control.ps1'
    $transcript = Join-Path -Path $securityRoot -ChildPath 'control-transcript.txt'
    # Windows confirmation prompts bypass redirected stdout; transcription captures host output.
    [IO.File]::WriteAllText($path, ". '$installer'`nfunction Get-NativeArchitecture { 'AMD64' }`nStart-Transcript -LiteralPath `$env:TFM_TEST_TRANSCRIPT -Force | Out-Null`ntry {`n$Action`n} finally { Stop-Transcript | Out-Null }")
    $psi = New-Object -TypeName Diagnostics.ProcessStartInfo
    $psi.FileName = (Get-Process -Id $PID).Path
    $psi.Arguments = "-NoProfile -File `"$path`""
    $psi.UseShellExecute = $false
    $psi.RedirectStandardInput = $true
    $psi.RedirectStandardOutput = $true
    $psi.RedirectStandardError = $true
    $psi.EnvironmentVariables['TFM_INSTALL_DIR'] = $destination
    $psi.EnvironmentVariables['TFM_TEST_CANDIDATE'] = $exe
    $psi.EnvironmentVariables['TFM_TEST_TRANSCRIPT'] = $transcript
    $psi.EnvironmentVariables['TFM_BASE_URL'] = (Join-Path -Path $securityRoot -ChildPath 'missing-mirror')
    $process = [Diagnostics.Process]::Start($psi)
    try {
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.StandardInput.WriteLine($Answer)
        $process.StandardInput.Close()
        $process.WaitForExit()
        return @{ Code = $process.ExitCode; Output = $stdout.GetAwaiter().GetResult() + $stderr.GetAwaiter().GetResult(); Transcript = [IO.File]::ReadAllText($transcript) }
    } finally { $process.Dispose() }
}

Add-Type -AssemblyName System.IO.Compression
Add-Type -AssemblyName System.IO.Compression.FileSystem
try {
    # Inject failures into the real legacy TLS block without changing host TLS.
    Add-Type -TypeDefinition @'
using System;
using System.Net;
public static class TfmTlsFixture {
    public static int Failure;
    public static SecurityProtocolType Protocol;
    public static SecurityProtocolType SecurityProtocol {
        get { return Protocol; }
        set {
            if (Failure == 1) throw new NotSupportedException("legacy TLS");
            if (Failure == 2) throw new InvalidOperationException("unrelated TLS error");
            Protocol = value;
        }
    }
}
'@
    $installerPath = Join-Path -Path $PSScriptRoot -ChildPath '../../install.ps1'
    $installerAst = [Management.Automation.Language.Parser]::ParseFile($installerPath, [ref]$null, [ref]$null)
    $legacy = @($installerAst.EndBlock.Statements | Where-Object -FilterScript {
        $_ -is [Management.Automation.Language.IfStatementAst]
    })[0].Clauses[0].Item2.Statements[0].Extent.Text
    $tlsAction = [scriptblock]::Create($legacy.Replace('[Net.ServicePointManager]', '[TfmTlsFixture]'))
    [TfmTlsFixture]::Protocol = 0
    & $tlsAction
    Assert-Equal -Actual ([int][TfmTlsFixture]::Protocol) -Expected 3072 -Reason 'TLS 1.2 not enabled'
    [TfmTlsFixture]::Failure = 1
    & $tlsAction
    Assert-Equal -Actual ([int][TfmTlsFixture]::Protocol) -Expected 3072 -Reason 'unsupported TLS changed existing protocols'
    [TfmTlsFixture]::Failure = 2
    $errorMessage = ''
    try { & $tlsAction } catch { $errorMessage = $_.Exception.InnerException.Message }
    Assert-Equal -Actual $errorMessage -Expected 'unrelated TLS error' -Reason 'unrelated TLS error swallowed'
    $members = @('tfm.exe', 'LICENSE-MIT', 'LICENSE-APACHE', 'README.md')
    $archive = Build-MemberZip -Names $members
    $hash = (Get-FileHash -Path $archive -Algorithm SHA256).Hash
    $basename = [IO.Path]::GetFileName($archive)
    foreach ($record in @("$hash  wrong.zip", "$hash  $basename`n$hash  $basename",
        "$hash  $basename extra", "not-hex  $basename", " $hash  $basename", "$hash  $basename`n`n")) {
        Write-Sidecar -Archive $archive -Record $record
        Assert-Rejected -Action { Assert-Checksum -Archive $archive -Sidecar "$archive.sha256" } -Reason 'sidecar structure'
    }
    [IO.File]::WriteAllBytes("$archive.sha256", [byte[]](239, 187, 191) + [Text.Encoding]::ASCII.GetBytes("$hash  $basename"))
    Assert-Rejected -Action { Assert-Checksum -Archive $archive -Sidecar "$archive.sha256" } -Reason 'UTF-8 BOM'
    [IO.File]::WriteAllText("$archive.sha256", "$hash  $basename", [Text.Encoding]::Unicode)
    Assert-Rejected -Action { Assert-Checksum -Archive $archive -Sidecar "$archive.sha256" } -Reason 'UTF-16'
    Write-Sidecar -Archive $archive -Record "$hash  $basename"
    Assert-Checksum -Archive $archive -Sidecar "$archive.sha256"
    $securityPasses++
    [IO.File]::AppendAllText($archive, 'tampered')
    Assert-Rejected -Action { Assert-Checksum -Archive $archive -Sidecar "$archive.sha256" } -Reason 'checksum mismatch'

    $outside = Join-Path -Path $securityRoot -ChildPath 'outside'
    [IO.File]::WriteAllText($outside, 'outside sentinel')
    foreach ($bad in @('extra', '../outside', '/outside', 'nested/tfm.exe', './tfm.exe',
        'tfm.exe', 'TFM.EXE', 'tfm.exe:stream', 'tfm.exe.', '\outside', 'C:\outside', '\\server\share')) {
        $archive = Build-MemberZip -Names ($members + $bad)
        $extracted = Join-Path -Path $securityRoot -ChildPath ([IO.Path]::GetRandomFileName())
        # Malicious archives have valid checksums; integrity is not structure.
        $hash = (Get-FileHash -Path $archive -Algorithm SHA256).Hash
        Write-Sidecar -Archive $archive -Record "$hash  $([IO.Path]::GetFileName($archive))"
        Assert-Checksum -Archive $archive -Sidecar "$archive.sha256"
        Assert-Rejected -Action { Expand-SafeArchive -Archive $archive -Destination $extracted } -Reason "archive member $bad"
        Assert-Equal -Actual (Test-Path -Path $extracted) -Expected $false -Reason 'unsafe archive extracted'
        Assert-Equal -Actual ([IO.File]::ReadAllText($outside)) -Expected 'outside sentinel' -Reason 'outside bytes changed'
    }
    foreach ($attributes in @(268435456, 536870912, 1610612736, -1610612736, -1073741824, 16, 64, 1024)) {
        $archive = Build-MemberZip -Names $members -Attributes $attributes
        Assert-Rejected -Action { Expand-SafeArchive -Archive $archive -Destination (Join-Path -Path $securityRoot -ChildPath 'types') } -Reason 'archive type'
    }
    foreach ($omitted in $members) {
        $archive = Build-MemberZip -Names @($members | Where-Object -FilterScript { $_ -ne $omitted })
        Assert-Rejected -Action { Expand-SafeArchive -Archive $archive -Destination (Join-Path -Path $securityRoot -ChildPath 'missing') } -Reason 'missing member'
    }
    $archive = Build-MemberZip -Names $members -Attributes -2119958528
    # 0x81A40000 denotes a regular Unix file with mode 0644.
    Expand-SafeArchive -Archive $archive -Destination (Join-Path -Path $securityRoot -ChildPath 'regular')
    $securityPasses++

    $exe = Join-Path -Path $securityRoot -ChildPath 'candidate.exe'
    $bytes = New-Object -TypeName byte[] -ArgumentList 128
    $bytes[0] = 77; $bytes[1] = 90; $bytes[60] = 64
    $bytes[64] = 80; $bytes[65] = 69
    $bytes[68] = 100; $bytes[69] = 134; $bytes[86] = 2
    [IO.File]::WriteAllBytes($exe, $bytes)
    Assert-CandidateArchitecture -Exe $exe -Target 'x86_64-pc-windows-msvc'
    $securityPasses++
    Assert-Rejected -Action { Assert-CandidateArchitecture -Exe $exe -Target 'aarch64-pc-windows-msvc' } -Reason 'wrong machine'
    $bytes[68] = 100; $bytes[69] = 170
    [IO.File]::WriteAllBytes($exe, $bytes)
    Assert-CandidateArchitecture -Exe $exe -Target 'aarch64-pc-windows-msvc'
    $securityPasses++
    foreach ($offset in @(0, 64, 86)) {
        $badBytes = $bytes.Clone(); $badBytes[$offset] = 0
        [IO.File]::WriteAllBytes($exe, $badBytes)
        Assert-Rejected -Action { Assert-CandidateArchitecture -Exe $exe -Target 'aarch64-pc-windows-msvc' } -Reason 'bad PE header'
    }
    [IO.File]::WriteAllBytes($exe, [byte[]](77, 90))
    Assert-Rejected -Action { Assert-CandidateArchitecture -Exe $exe -Target 'x86_64-pc-windows-msvc' } -Reason 'truncated DOS header'

    # Expected launch failures reject the candidate; unrelated errors propagate.
    $nativeStatus = Get-Variable -Name LASTEXITCODE -Scope Global -ErrorAction SilentlyContinue
    $savedNativeStatus = if ($nativeStatus) { $nativeStatus.Value } else { $null }
    try {
        $rejectedExecutables = @((Join-Path -Path $securityRoot -ChildPath 'absent.exe'))
        # A PE invocation on Unix may launch a desktop association, not Windows.
        if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) { $rejectedExecutables += $exe }
        foreach ($rejectedExe in $rejectedExecutables) {
            $errorMessage = ''
            try { Assert-CandidateVersion -Exe $rejectedExe -Version 'v1.2.3' }
            catch { $errorMessage = $_.Exception.Message }
            Assert-Equal -Actual $errorMessage -Expected 'candidate failed --version validation' -Reason 'unlaunchable candidate rejection'
        }
        $scriptCandidate = Join-Path -Path $securityRoot -ChildPath 'candidate.ps1'
        [IO.File]::WriteAllText($scriptCandidate, "throw [InvalidOperationException]::new('unrelated candidate error')")
        $errorMessage = ''
        try { Assert-CandidateVersion -Exe $scriptCandidate -Version 'v1.2.3' }
        catch { $errorMessage = $_.Exception.Message }
        Assert-Equal -Actual $errorMessage -Expected 'unrelated candidate error' -Reason 'unrelated candidate error swallowed'
    } finally {
        # Negative assertions must not manufacture a failure for the CI wrapper.
        if ($nativeStatus) { $global:LASTEXITCODE = $savedNativeStatus }
        else { Remove-Variable -Name LASTEXITCODE -Scope Global }
    }

    foreach ($tag in @('v1.2', '1.2.3', 'v1.2.3-rc.1', 'v1.2.3/extra', "v1.2.3`nmore", "v1.2.3`n")) {
        Assert-Rejected -Action { Assert-StableTag -Tag $tag } -Reason 'unstable tag'
    }
    $script:ReleasesBase = 'https://github.com/seapagan/terminal-file-manager/releases'
    foreach ($uri in @('http://github.com/file', 'https://evil.example/file', 'https://github.com:444/file',
        'https://user@github.com/file', 'https://github.com/file#fragment')) {
        Assert-Rejected -Action { Assert-AssetUri -Uri ([uri]$uri) } -Reason 'unsafe transport'
    }
    foreach ($url in @('http://mirror.example', 'ftp://mirror.example', 'file:///tmp/mirror')) {
        Assert-Rejected -Action { Test-IsHttp -Url $url } -Reason 'non-HTTPS source'
    }
    Assert-AssetUri -Uri ([uri]'https://release-assets.githubusercontent.com/file')
    $securityPasses++

    $destination = Join-Path -Path $securityRoot -ChildPath 'destination'
    [IO.File]::WriteAllText($exe, 'new tfm')
    # A declined whole transaction creates neither the install root nor a stage.
    Install-Candidate -Candidate $exe -InstallDir $destination -WhatIf
    Assert-Equal -Actual (Test-Path -LiteralPath $destination) -Expected $false -Reason 'WhatIf created destination'
    $result = Invoke-ControlledAction -Action 'Install-Candidate -Candidate $env:TFM_TEST_CANDIDATE -InstallDir $env:TFM_INSTALL_DIR -Confirm'
    Assert-Equal -Actual $result.Code -Expected 0 -Reason 'declined fresh transaction failed'
    Assert-Equal -Actual (Test-Path -LiteralPath $destination) -Expected $false -Reason 'declined transaction created destination'
    foreach ($control in @('-WhatIf', '-Confirm')) {
        $result = Invoke-ControlledAction -Action "Invoke-Install $control"
        Assert-Equal -Actual $result.Code -Expected 0 -Reason 'declined installation accessed missing mirror'
        Assert-Equal -Actual ($result.Output.Contains('Installed tfm')) -Expected $false -Reason 'declined installation reported success'
        Assert-Equal -Actual (Test-Path -LiteralPath $destination) -Expected $false -Reason 'declined installation created destination'
    }
    New-Item -ItemType Directory -Path $destination | Out-Null
    $result = Invoke-ControlledAction -Action 'New-OwnedFile -Directory $env:TFM_INSTALL_DIR -WhatIf'
    Assert-Equal -Actual $result.Code -Expected 0 -Reason 'WhatIf staging failed'
    Assert-Equal -Actual @(Get-ChildItem -LiteralPath $destination -Force).Count -Expected 0 -Reason 'WhatIf created stage'
    Install-Candidate -Candidate $exe -InstallDir $destination
    Assert-Equal -Actual ([IO.File]::ReadAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'))) -Expected 'new tfm' -Reason 'fresh transaction'
    [IO.File]::WriteAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'), 'old tfm')
    Install-Candidate -Candidate $exe -InstallDir $destination
    Assert-Equal -Actual ([IO.File]::ReadAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'))) -Expected 'new tfm' -Reason 'replacement transaction'
    Assert-Equal -Actual @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'transaction cleanup'
    [IO.File]::WriteAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'), 'old tfm')
    $result = Invoke-ControlledAction -Action 'Install-Candidate -Candidate $env:TFM_TEST_CANDIDATE -InstallDir $env:TFM_INSTALL_DIR -Confirm'
    Assert-Equal -Actual $result.Code -Expected 0 -Reason 'declined replacement failed'
    Assert-OldBytes -Directory $destination
    $result = Invoke-ControlledAction -Action 'Remove-BackupFile -Backup (Join-Path -Path $env:TFM_INSTALL_DIR -ChildPath "tfm.exe") -WhatIf'
    Assert-Equal -Actual $result.Code -Expected 0 -Reason 'WhatIf backup deletion failed'
    Assert-OldBytes -Directory $destination
    $result = Invoke-ControlledAction -Action 'Install-Candidate -Candidate $env:TFM_TEST_CANDIDATE -InstallDir $env:TFM_INSTALL_DIR -Confirm' -Answer 'Y'
    Assert-Equal -Actual $result.Code -Expected 0 -Reason 'approved transaction failed'
    Assert-Equal -Actual ([regex]::Matches($result.Transcript, 'Are you sure').Count) -Expected 1 -Reason 'nested transaction confirmation'
    Assert-Equal -Actual ([IO.File]::ReadAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'))) -Expected 'new tfm' -Reason 'approved transaction bytes'
    Assert-Equal -Actual @(Get-ChildItem -LiteralPath $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'approved transaction cleanup'
    [IO.File]::WriteAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'), 'old tfm')
    Assert-Rejected -Action { Install-Candidate -Candidate (Join-Path -Path $securityRoot -ChildPath 'missing.exe') -InstallDir $destination } -Reason 'missing candidate'
    Assert-OldBytes -Directory $destination
    Assert-Equal -Actual @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'copy failure cleanup'
    New-Item -ItemType Directory -Path (Join-Path -Path $securityRoot -ChildPath 'directory-final/tfm.exe') -Force | Out-Null
    Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir (Join-Path -Path $securityRoot -ChildPath 'directory-final') } -Reason 'final directory'
    [IO.File]::WriteAllText((Join-Path -Path $securityRoot -ChildPath 'file-ancestor'), 'file')
    Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir (Join-Path -Path $securityRoot -ChildPath 'file-ancestor/child') } -Reason 'non-directory ancestor'

    # Recovery succeeds for absent and existing finals. Failed recovery keeps
    # the last good bytes and includes their exact location in the error.
    foreach ($exists in @($false, $true)) {
        $backup = New-OwnedFile -Directory $securityRoot
        [IO.File]::WriteAllText($backup, 'old tfm')
        $oldHash = (Get-FileHash -Path $backup -Algorithm SHA256).Hash
        $recovery = Join-Path -Path $securityRoot -ChildPath ('recover-' + [IO.Path]::GetRandomFileName())
        if ($exists) { [IO.File]::WriteAllText($recovery, 'new tfm') }
        Restore-Backup -Backup $backup -Final $recovery -OldHash $oldHash
        Assert-Equal -Actual ([IO.File]::ReadAllText($recovery)) -Expected 'old tfm' -Reason 'rollback bytes'
    }
    $backup = New-OwnedFile -Directory $securityRoot
    [IO.File]::WriteAllText($backup, 'old tfm')
    $oldHash = (Get-FileHash -Path $backup -Algorithm SHA256).Hash
    $recoveryDirectory = Join-Path -Path $securityRoot -ChildPath 'recovery-directory'
    New-Item -ItemType Directory -Path $recoveryDirectory | Out-Null
    $errorMessage = ''
    try { Restore-Backup -Backup $backup -Final $recoveryDirectory -OldHash $oldHash }
    catch { $errorMessage = $_.Exception.Message }
    Assert-Equal -Actual ($errorMessage.Contains($backup)) -Expected $true -Reason 'recovery location missing'
    Assert-Equal -Actual ([IO.File]::ReadAllText($backup)) -Expected 'old tfm' -Reason 'last good copy discarded'

    # R2: a relative destination must inspect current-directory ancestors
    # before creating anything through a linked/reparse working directory.
    $relativeOutside = Join-Path -Path $securityRoot -ChildPath 'relative-outside'
    New-Item -ItemType Directory -Path $relativeOutside | Out-Null
    $outsideSentinel = Join-Path -Path $relativeOutside -ChildPath 'sentinel'
    [IO.File]::WriteAllText($outsideSentinel, 'outside sentinel')
    $workingLink = Join-Path -Path $securityRoot -ChildPath 'working-link'
    $linkType = if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        'Junction'
    } else { 'SymbolicLink' }
    New-Item -ItemType $linkType -Path $workingLink -Target $relativeOutside | Out-Null
    $savedLocation = Get-Location
    try {
        Set-Location -LiteralPath $workingLink
        Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir 'child' } -Reason 'linked relative working directory'
        Assert-Equal -Actual (Test-Path -LiteralPath (Join-Path -Path $relativeOutside -ChildPath 'child')) -Expected $false -Reason 'relative destination created through link'
        Assert-Equal -Actual @(Get-ChildItem -LiteralPath $relativeOutside -Force).Count -Expected 1 -Reason 'outside object created'
        Assert-Equal -Actual ([IO.File]::ReadAllText($outsideSentinel)) -Expected 'outside sentinel' -Reason 'outside sentinel changed'
    } finally { Set-Location -LiteralPath $savedLocation.Path }

    # Inject only a destination mutation after production stage creation.
    $originalNewOwnedFile = (Get-Item -Path Function:\New-OwnedFile).ScriptBlock
    function New-OwnedFile {
        [CmdletBinding(SupportsShouldProcess = $true)]
        param([string]$Directory)
        if (-not $PSCmdlet.ShouldProcess($Directory, 'Create stage and inject a destination mutation')) { return }
        $path = & $originalNewOwnedFile -Directory $Directory -Confirm:$false -WhatIf:$false
        [IO.File]::WriteAllText((Join-Path -Path $Directory -ChildPath 'tfm.exe'), 'external replacement')
        return $path
    }
    try {
        Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $destination } -Reason 'changed final object'
        Assert-Equal -Actual ([IO.File]::ReadAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'))) -Expected 'external replacement' -Reason 'external object overwritten'
    } finally { Set-Item -Path Function:\New-OwnedFile -Value $originalNewOwnedFile }
    [IO.File]::WriteAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'), 'old tfm')

    # Exercise the production post-replace error/rollback path, not just the
    # recovery helper. The wrapper injects an error after the real OS primitive.
    $originalReplace = (Get-Item -Path Function:\Move-StagedFile).ScriptBlock
    function Move-StagedFile([string]$Stage, [string]$Final, [string]$Backup) {
        & $originalReplace -Stage $Stage -Final $Final -Backup $Backup
        throw 'injected error after replacement'
    }
    try {
        Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $destination } -Reason 'post-replace failure'
        Assert-OldBytes -Directory $destination
        Assert-Equal -Actual @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'rollback cleanup'
        $savedWhatIf = $WhatIfPreference
        $savedConfirm = $ConfirmPreference
        try {
            $WhatIfPreference = $true
            $ConfirmPreference = 'Low'
            Assert-Rejected -Action {
                Install-Candidate -Candidate $exe -InstallDir $destination -WhatIf:$false -Confirm:$false
            } -Reason 'approved rollback under inherited preferences'
            Assert-OldBytes -Directory $destination
            Assert-Equal -Actual @(Get-ChildItem -LiteralPath $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'inherited preferences skipped rollback cleanup'
        } finally { $WhatIfPreference = $savedWhatIf; $ConfirmPreference = $savedConfirm }
        $originalRestore = (Get-Item -Path Function:\Restore-Backup).ScriptBlock
        function Restore-Backup([string]$Backup, [string]$Final, [string]$OldHash) {
            throw "injected recovery failure; last good copy retained at $Backup"
        }
        try {
            Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $destination } -Reason 'recovery failure'
            $retained = @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*')
            Assert-Equal -Actual $retained.Count -Expected 1 -Reason 'recovery material count'
            Assert-Equal -Actual ([IO.File]::ReadAllText($retained[0].FullName)) -Expected 'old tfm' -Reason 'recovery material bytes'
        } finally { Set-Item -Path Function:\Restore-Backup -Value $originalRestore }
    } finally { Set-Item -Path Function:\Move-StagedFile -Value $originalReplace }
    # Reset only fixture material after checking its preservation.
    Get-ChildItem -Path $destination -Force -Filter '.tfm-*' | Remove-Item -Force
    [IO.File]::WriteAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'), 'old tfm')

    # A cleanup failure occurs after replacement succeeds. Recover the original
    # or retain its verified bytes with an explicit recovery-required failure.
    $originalCleanup = (Get-Item -Path Function:\Remove-BackupFile).ScriptBlock
    function Remove-BackupFile {
        [CmdletBinding(SupportsShouldProcess = $true)]
        param([string]$Backup)
        if ($PSCmdlet.ShouldProcess($Backup, 'Inject a backup cleanup failure')) {
            throw 'injected backup cleanup failure'
        }
    }
    try {
        $cleanupError = ''
        try { Install-Candidate -Candidate $exe -InstallDir $destination }
        catch { $cleanupError = $_.Exception.Message }
        Assert-Equal -Actual ($cleanupError.Contains('replacement succeeded')) -Expected $true -Reason 'cleanup failure misreported replacement'
        Assert-Equal -Actual ($cleanupError.Contains('original installation restored')) -Expected $true -Reason 'cleanup recovery missing'
        Assert-OldBytes -Directory $destination
        Assert-Equal -Actual @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'cleanup recovery leftovers'
        $originalRestore = (Get-Item -Path Function:\Restore-Backup).ScriptBlock
        function Restore-Backup([string]$Backup, [string]$Final, [string]$OldHash) {
            throw 'injected cleanup recovery failure'
        }
        try {
            $oldHash = (Get-FileHash -Path (Join-Path -Path $destination -ChildPath 'tfm.exe') -Algorithm SHA256).Hash
            $cleanupError = ''
            try { Install-Candidate -Candidate $exe -InstallDir $destination }
            catch { $cleanupError = $_.Exception.Message }
            $retained = @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*')
            Assert-Equal -Actual $retained.Count -Expected 1 -Reason 'cleanup recovery backup count'
            Assert-Equal -Actual ($cleanupError.Contains('replacement succeeded')) -Expected $true -Reason 'partial commit status missing'
            Assert-Equal -Actual ($cleanupError.Contains('recovery required')) -Expected $true -Reason 'recovery requirement missing'
            Assert-Equal -Actual ($cleanupError.Contains($retained[0].FullName)) -Expected $true -Reason 'cleanup recovery location missing'
            Assert-Equal -Actual (Get-FileHash -Path $retained[0].FullName -Algorithm SHA256).Hash -Expected $oldHash -Reason 'cleanup original backup hash'
            Assert-Equal -Actual ([IO.File]::ReadAllText((Join-Path -Path $destination -ChildPath 'tfm.exe'))) -Expected 'new tfm' -Reason 'cleanup candidate state'
        } finally { Set-Item -Path Function:\Restore-Backup -Value $originalRestore }
        Restore-Backup -Backup $retained[0].FullName -Final (Join-Path -Path $destination -ChildPath 'tfm.exe') -OldHash $oldHash
        Assert-OldBytes -Directory $destination
        Assert-Equal -Actual @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'manual recovery leftovers'
    } finally { Set-Item -Path Function:\Remove-BackupFile -Value $originalCleanup }

    if ([Environment]::OSVersion.Platform -eq [PlatformID]::Win32NT) {
        $runningSource = Join-Path -Path $securityRoot -ChildPath 'running.rs'
        [IO.File]::WriteAllText($runningSource, @'
fn main() {
    if std::env::args().any(|arg| arg == "--version") { println!("tfm 99.88.77"); }
    else { std::thread::sleep(std::time::Duration::from_secs(60)); }
}
'@, [Text.Encoding]::ASCII)
        $runningExe = Join-Path -Path $securityRoot -ChildPath 'running.exe'
        & rustc $runningSource -o $runningExe
        if ($LASTEXITCODE -ne 0) { throw 'native running-executable fixture build failed' }
        $runningDir = Join-Path -Path $securityRoot -ChildPath 'running-install'
        New-Item -ItemType Directory -Path $runningDir | Out-Null
        $runningFinal = Join-Path -Path $runningDir -ChildPath 'tfm.exe'
        [IO.File]::Copy($runningExe, $runningFinal)
        $runningHash = (Get-FileHash -Path $runningFinal -Algorithm SHA256).Hash
        $runningProcess = [Diagnostics.Process]::Start($runningFinal)
        try {
            Assert-Equal -Actual $runningProcess.HasExited -Expected $false -Reason 'running fixture exited before preflight'
            Assert-Rejected -Action { Assert-ReplaceableFile -Final $runningFinal } -Reason 'running executable preflight'
            Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $runningDir } -Reason 'running existing executable'
            Assert-Equal -Actual (Get-FileHash -Path $runningFinal -Algorithm SHA256).Hash -Expected $runningHash -Reason 'running executable bytes'
            Assert-Equal -Actual @(Get-ChildItem -Path $runningDir -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'running preflight cleanup'
        } finally {
            if (-not $runningProcess.HasExited) { $runningProcess.Kill(); $runningProcess.WaitForExit() }
            $runningProcess.Dispose()
        }
        $nativeTarget = Get-Target
        $savedArchitecture = $env:PROCESSOR_ARCHITECTURE
        try {
            $env:PROCESSOR_ARCHITECTURE = 'x86'
            Assert-Equal -Actual (Get-Target) -Expected $nativeTarget -Reason 'process architecture selected over native host'
        } finally { $env:PROCESSOR_ARCHITECTURE = $savedArchitecture }
        $final = Join-Path -Path $destination -ChildPath 'tfm.exe'
        $lock = [IO.File]::Open($final, [IO.FileMode]::Open, [IO.FileAccess]::Read, [IO.FileShare]::Read)
        try {
            Assert-Rejected -Action { Assert-ReplaceableFile -Final $final } -Reason 'explicit locked-file preflight'
            Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $destination } -Reason 'locked existing binary'
        }
        finally { $lock.Dispose() }
        Assert-OldBytes -Directory $destination
        [IO.File]::SetAttributes($final, [IO.FileAttributes]::ReadOnly)
        try { Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $destination } -Reason 'read-only existing binary' }
        finally { [IO.File]::SetAttributes($final, [IO.FileAttributes]::Normal) }
        Assert-OldBytes -Directory $destination
        Assert-Equal -Actual @(Get-ChildItem -Path $destination -Force -Filter '.tfm-*').Count -Expected 0 -Reason 'read-only failure cleanup'
        $acl = Get-Acl -LiteralPath $destination
        $denied = Get-Acl -LiteralPath $destination
        $sid = [Security.Principal.WindowsIdentity]::GetCurrent().User
        $rule = [Security.AccessControl.FileSystemAccessRule]::new($sid,
            [Security.AccessControl.FileSystemRights]::CreateFiles, [Security.AccessControl.AccessControlType]::Deny)
        $denied.AddAccessRule($rule)
        Set-Acl -LiteralPath $destination -AclObject $denied
        try { Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $destination } -Reason 'denied staging directory' }
        finally { Set-Acl -LiteralPath $destination -AclObject $acl }
        Assert-OldBytes -Directory $destination
        $linked = Join-Path -Path $securityRoot -ChildPath 'junction'
        New-Item -ItemType Junction -Path $linked -Target $destination | Out-Null
        Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir $linked } -Reason 'reparse install root'
        Assert-Rejected -Action { Install-Candidate -Candidate $exe -InstallDir (Join-Path -Path $linked -ChildPath 'child') } -Reason 'reparse ancestor'
        Assert-OldBytes -Directory $destination
    } else {
        $securitySkipped = 18
        Write-Information -MessageData 'SKIP: eighteen native Windows candidate-launch/running-file/preflight/architecture/lock/read-only/denied-path/junction assertions; transaction successes above are Unix simulations.' -InformationAction Continue
    }
} finally {
    Remove-Item -LiteralPath $securityRoot -Recurse -Force -ErrorAction SilentlyContinue
}
Write-Information -MessageData "PowerShell security tests passed: $securityPasses assertions ($securitySkipped native assertions skipped)." -InformationAction Continue
