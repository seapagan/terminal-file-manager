# tfm installer for Windows (PowerShell 5.1 and 7+).
#
# Downloads the correct release archive for this machine's architecture,
# verifies its SHA-256, runs the candidate binary's --version as a smoke
# test, and only then replaces any existing installation. A failure at
# any point leaves an existing installation untouched.
#
# Environment variables:
#   TFM_VERSION     install a specific release tag (e.g. v0.1.0)
#   TFM_INSTALL_DIR override the installation directory
#   TFM_BASE_URL    override the release base: an https:// URL, or
#                         a local directory acting as a release mirror
#                         (used by the installer tests and offline setups)
[CmdletBinding()]
param()

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version 2.0

function Die([string]$Message) {
    Write-Error $Message -ErrorAction Stop
    exit 1
}

if ($PSVersionTable.PSVersion.Major -lt 6) {
    # Windows PowerShell: make sure TLS 1.2 is enabled for GitHub.
    try {
        [Net.ServicePointManager]::SecurityProtocol = `
            [Net.ServicePointManager]::SecurityProtocol -bor `
            [Net.SecurityProtocolType]::Tls12
    } catch { }
}

$script:ReleasesBase = if ($env:TFM_BASE_URL) {
    $env:TFM_BASE_URL
} else {
    'https://github.com/seapagan/terminal-file-manager/releases'
}

function Test-IsHttp([string]$Url) {
    if ($Url -match '^https://') { return $true }
    if ($Url -match '^[A-Za-z][A-Za-z0-9+.-]*://') { Die 'remote sources must use HTTPS' }
    return $false
}

function Get-NativeArchitecture {
    if ([Environment]::OSVersion.Platform -ne [PlatformID]::Win32NT) {
        Die 'unsupported operating system: use install.sh on Unix'
    }
    # IsWow64Process2 returns the native machine on ARM64 under emulation.
    # GetNativeSystemInfo alone may report the emulated x86/x64 architecture.
    if (-not ('TfmNativeSystem' -as [type])) {
        Add-Type -TypeDefinition @'
using System;
using System.ComponentModel;
using System.Runtime.InteropServices;
public static class TfmNativeSystem {
    [DllImport("kernel32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    private static extern bool IsWow64Process2(IntPtr process, out ushort emulated, out ushort native);
    [DllImport("kernel32.dll")]
    private static extern void GetNativeSystemInfo(IntPtr information);
    public static ushort Architecture() {
        ushort emulated, native;
        try {
            if (!IsWow64Process2(new IntPtr(-1), out emulated, out native))
                throw new Win32Exception(Marshal.GetLastWin32Error());
            return native;
        } catch (EntryPointNotFoundException) {
            // Pre-1709 Windows has no supported ARM64 desktop emulation.
            IntPtr buffer = Marshal.AllocHGlobal(64);
            try {
                GetNativeSystemInfo(buffer);
                return Marshal.ReadInt16(buffer) == 9 ? (ushort)0x8664 : (ushort)0;
            } finally { Marshal.FreeHGlobal(buffer); }
        }
    }
}
'@
    }
    switch ([TfmNativeSystem]::Architecture()) {
        34404 { return 'AMD64' }
        43620 { return 'ARM64' }
        default { Die 'unsupported architecture' }
    }
}

function Get-Target {
    $architecture = Get-NativeArchitecture
    $arch = switch ($architecture) {
        'AMD64' { 'x86_64'; break }
        'ARM64' { 'aarch64'; break }
        default { Die "unsupported architecture: $architecture" }
    }
    "$arch-pc-windows-msvc"
}

function New-HttpClient([bool]$AllowRedirect) {
    Add-Type -AssemblyName System.Net.Http
    $handler = [System.Net.Http.HttpClientHandler]::new()
    $handler.AllowAutoRedirect = $AllowRedirect
    [System.Net.Http.HttpClient]::new($handler)
}

function Assert-StableTag([string]$Tag) {
    if ($Tag -cnotmatch '\Av[0-9]+\.[0-9]+\.[0-9]+\z') {
        Die 'version did not resolve to a usable release tag (stable vX.Y.Z required)'
    }
    return $Tag
}

function Resolve-Version {
    if ($env:TFM_VERSION) { return (Assert-StableTag $env:TFM_VERSION) }
    if (-not (Test-IsHttp $script:ReleasesBase)) {
        $marker = Join-Path $script:ReleasesBase 'latest'
        if (-not (Test-Path -LiteralPath $marker -PathType Leaf)) {
            Die 'no TFM_VERSION set and the local mirror has no "latest" marker'
        }
        $tag = [Text.Encoding]::ASCII.GetString([IO.File]::ReadAllBytes($marker))
        return (Assert-StableTag ($tag -replace '\r?\n$', ''))
    }
    $client = New-HttpClient -AllowRedirect:$false
    $response = $null
    try {
        $response = $client.GetAsync("$script:ReleasesBase/latest").GetAwaiter().GetResult()
        if ([int]$response.StatusCode -notin 301, 302, 303, 307, 308) {
            Die "could not resolve the latest release (HTTP $([int]$response.StatusCode))"
        }
        $location = $response.Headers.Location
        $prefix = "$script:ReleasesBase/tag/"
        if (-not $location -or -not $location.IsAbsoluteUri -or
            -not $location.AbsoluteUri.StartsWith($prefix, [StringComparison]::Ordinal)) {
            Die 'the latest release redirect did not resolve to a usable release tag'
        }
        return (Assert-StableTag $location.AbsoluteUri.Substring($prefix.Length))
    } finally {
        if ($response) { $response.Dispose() }
        $client.Dispose()
    }
}

function Assert-AssetUri([uri]$Uri) {
    if ($Uri.Scheme -ne 'https' -or $Uri.UserInfo -or $Uri.Fragment) {
        Die 'asset transport must use unambiguous HTTPS'
    }
    $origin = ([uri]$script:ReleasesBase).GetLeftPart([UriPartial]::Authority)
    $allowed = @('github.com', 'release-assets.githubusercontent.com',
        'objects.githubusercontent.com', 'github-releases.githubusercontent.com')
    if ($Uri.GetLeftPart([UriPartial]::Authority) -ne $origin -and
        -not ($origin -eq 'https://github.com' -and $Uri.Host -in $allowed -and $Uri.Port -eq 443)) {
        Die 'unexpected asset redirect origin'
    }
}

function Save-RemoteAsset([string]$Url, [string]$Destination) {
    $client = New-HttpClient -AllowRedirect:$false
    $response = $null
    $uri = [uri]$Url
    try {
        for ($redirect = 0; $redirect -le 5; $redirect++) {
            Assert-AssetUri $uri
            $response = $client.GetAsync($uri).GetAwaiter().GetResult()
            if ([int]$response.StatusCode -in 301, 302, 303, 307, 308) {
                if (-not $response.Headers.Location) { Die 'missing asset redirect location' }
                $uri = [uri]::new($uri, $response.Headers.Location)
                $response.Dispose()
                $response = $null
                continue
            }
            $response.EnsureSuccessStatusCode() | Out-Null
            $bytes = $response.Content.ReadAsByteArrayAsync().GetAwaiter().GetResult()
            [IO.File]::WriteAllBytes($Destination, $bytes)
            return
        }
        Die 'too many asset redirects'
    } catch { Die "could not download ${Url}: $($_.Exception.Message)" }
    finally {
        if ($response) { $response.Dispose() }
        $client.Dispose()
    }
}

function Save-Asset([string]$Url, [string]$Destination) {
    if (-not (Test-IsHttp $script:ReleasesBase)) {
        $relative = $Url.Substring($script:ReleasesBase.Length).TrimStart('/', '\')
        $source = Join-Path $script:ReleasesBase $relative
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) {
            Die "could not download ${Url}: asset not found in the local mirror"
        }
        [IO.File]::Copy($source, $Destination, $false)
        return
    }
    Save-RemoteAsset $Url $Destination
}

function Assert-Checksum([string]$Archive, [string]$Sidecar) {
    if (-not (Test-Path -LiteralPath $Sidecar -PathType Leaf)) { Die 'checksum sidecar is missing' }
    $bytes = [IO.File]::ReadAllBytes($Sidecar)
    if (@($bytes | Where-Object { $_ -gt 127 }).Count) { Die 'checksum verification failed' }
    $record = [Text.Encoding]::ASCII.GetString($bytes)
    $basename = [regex]::Escape([IO.Path]::GetFileName($Archive))
    if ($record -cnotmatch "\A([0-9a-fA-F]{64})[ \t]+$basename(?:\r?\n)?\z") {
        Die 'checksum verification failed'
    }
    $expected = $Matches[1]
    $actual = (Get-FileHash -LiteralPath $Archive -Algorithm SHA256).Hash
    if (-not $actual.Equals($expected, [StringComparison]::OrdinalIgnoreCase)) {
        Die 'checksum verification failed'
    }
}

function Assert-ZipMembers([object]$Zip) {
    $allowed = @('tfm.exe', 'LICENSE-MIT', 'LICENSE-APACHE', 'README.md')
    $seen = [Collections.Generic.HashSet[string]]::new([StringComparer]::Ordinal)
    foreach ($entry in $Zip.Entries) {
        if (-not ($allowed -ccontains $entry.FullName) -or -not $seen.Add($entry.FullName)) {
            Die 'unsafe archive members'
        }
        $unixType = ($entry.ExternalAttributes -shr 16) -band 61440
        $windowsType = $entry.ExternalAttributes -band 1104
        if ($unixType -notin 0, 32768 -or $windowsType) { Die 'unsafe archive member types' }
    }
    if (-not $seen.Contains('tfm.exe')) { Die 'release archive is missing tfm.exe' }
    if ($seen.Count -ne 4) { Die 'unsafe archive members' }
}

function Expand-SafeArchive([string]$Archive, [string]$Destination) {
    Add-Type -AssemblyName System.IO.Compression
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $zip = [IO.Compression.ZipFile]::OpenRead($Archive)
    try {
        Assert-ZipMembers $zip
        New-Item -ItemType Directory -Path $Destination | Out-Null
        foreach ($entry in $zip.Entries) {
            $inputStream = $entry.Open()
            $outputStream = $null
            try {
                $outputStream = [IO.File]::Open((Join-Path $Destination $entry.FullName),
                    [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
                $inputStream.CopyTo($outputStream)
            } finally {
                if ($outputStream) { $outputStream.Dispose() }
                $inputStream.Dispose()
            }
        }
    } finally { $zip.Dispose() }
}

function Assert-CandidateArchitecture([string]$Exe, [string]$Target) {
    Assert-RegularFile $Exe
    $stream = [IO.File]::OpenRead($Exe)
    try {
        $header = New-Object byte[] 64
        if ($stream.Read($header, 0, 64) -ne 64 -or $header[0] -ne 77 -or $header[1] -ne 90) {
            Die 'candidate failed --version validation: invalid DOS header'
        }
        $offset = [BitConverter]::ToUInt32($header, 60)
        if ($offset -lt 64 -or $offset -gt $stream.Length - 24) { Die 'invalid candidate PE offset' }
        $stream.Position = $offset
        $pe = New-Object byte[] 24
        if ($stream.Read($pe, 0, 24) -ne 24 -or [BitConverter]::ToUInt32($pe, 0) -ne 17744) {
            Die 'invalid candidate PE header'
        }
        $machine = [BitConverter]::ToUInt16($pe, 4)
        $expected = if ($Target -eq 'x86_64-pc-windows-msvc') { 34404 } else { 43620 }
        if ($machine -ne $expected) { Die 'wrong candidate architecture' }
        $flags = [BitConverter]::ToUInt16($pe, 22)
        if (-not ($flags -band 2) -or ($flags -band 8192)) { Die 'candidate is not a PE executable' }
    } finally { $stream.Dispose() }
}

function Assert-CandidateVersion([string]$Exe, [string]$Version) {
    # Synchronous on PS5.1/7+: no external supervisor or process-tree promise.
    $reported = $null
    $global:LASTEXITCODE = 1
    try { $reported = @((& $Exe --version) 2>$null) } catch { }
    if ($global:LASTEXITCODE -ne 0 -or $null -eq $reported -or $reported.Count -ne 1) {
        Die 'candidate failed --version validation'
    }
    $expected = "tfm $($Version -replace '^v', '')"
    if ($reported[0] -cne $expected) { Die "reported version does not match ${Version}: $reported" }
}

function Get-PathItem([string]$Path) {
    try { return (Get-Item -LiteralPath $Path -Force -ErrorAction Stop) }
    catch [Management.Automation.ItemNotFoundException] { return $null }
}

function Assert-DirectoryPath([string]$Path) {
    $working = (Get-Location).ProviderPath
    $absolute = if ([IO.Path]::IsPathRooted($Path)) { $Path } else { Join-Path $working $Path }
    $full = [IO.Path]::GetFullPath($absolute)
    # Check the original components too: GetFullPath alone hides link/../ paths.
    $prefix = [IO.Path]::GetPathRoot($full)
    $relative = $absolute.Substring([IO.Path]::GetPathRoot($absolute).Length)
    foreach ($part in ($relative -split '[\\/]')) {
        if (-not $part -or $part -eq '.') { continue }
        $prefix = Join-Path $prefix $part
        $item = Get-PathItem $prefix
        if ($item -and (($item.Attributes -band [IO.FileAttributes]::ReparsePoint) -or -not $item.PSIsContainer)) {
            Die "unsafe destination ancestor: $prefix"
        }
        if (-not $item -and (Test-Path -LiteralPath $prefix -ErrorAction Stop)) {
            Die "inaccessible destination ancestor: $prefix"
        }
    }
    return $full
}

function Assert-RegularFile([string]$Path) {
    $item = Get-PathItem $Path
    if (-not $item -or $item.PSIsContainer -or ($item.Attributes -band [IO.FileAttributes]::ReparsePoint)) {
        Die "unsafe final destination: $Path"
    }
}

function Get-FinalIdentity([string]$Path) {
    $item = Get-PathItem $Path
    if (-not $item) { return 'absent' }
    Assert-RegularFile $Path
    return "$($item.CreationTimeUtc.Ticks):$($item.LastWriteTimeUtc.Ticks):$($item.Length):$((Get-FileHash -LiteralPath $Path -Algorithm SHA256).Hash)"
}

function New-OwnedFile([string]$Directory) {
    $path = Join-Path $Directory ('.tfm-' + [IO.Path]::GetRandomFileName())
    $stream = [IO.File]::Open($path, [IO.FileMode]::CreateNew, [IO.FileAccess]::Write, [IO.FileShare]::None)
    $stream.Dispose()
    return $path
}

function Restore-Backup([string]$Backup, [string]$Final, [string]$OldHash) {
    if (-not (Test-Path -LiteralPath $Backup -PathType Leaf) -or
        (Get-FileHash -LiteralPath $Backup -Algorithm SHA256).Hash -ne $OldHash) {
        Die "replacement failed; recovery material retained at $Backup"
    }
    try {
        if (Get-PathItem $Final) {
            Assert-RegularFile $Final
            [IO.File]::Replace($Backup, $Final, [NullString]::Value)
        } else { [IO.File]::Move($Backup, $Final) }
    } catch { Die "recovery failed; last good copy retained at ${Backup}: $($_.Exception.Message)" }
}

function Copy-IntoStage([string]$Candidate, [string]$Stage) {
    $inputStream = [IO.File]::OpenRead($Candidate)
    $outputStream = $null
    try {
        Assert-RegularFile $Stage
        $outputStream = [IO.File]::Open($Stage, [IO.FileMode]::Open, [IO.FileAccess]::Write, [IO.FileShare]::None)
        $inputStream.CopyTo($outputStream)
        $outputStream.Flush()
    } finally {
        if ($outputStream) { $outputStream.Dispose() }
        $inputStream.Dispose()
    }
}

function Replace-File([string]$Stage, [string]$Final, [string]$Backup) {
    [IO.File]::Replace($Stage, $Final, $Backup)
}

function Commit-Candidate([string]$Candidate, [string]$InstallDir) {
    $installDir = Assert-DirectoryPath $InstallDir
    $final = Join-Path $installDir 'tfm.exe'
    $before = Get-FinalIdentity $final
    [IO.Directory]::CreateDirectory($installDir) | Out-Null
    $null = Assert-DirectoryPath $installDir
    $stage = New-OwnedFile $installDir
    $backup = $null
    try {
        Copy-IntoStage $Candidate $stage
        $null = Assert-DirectoryPath $installDir
        if ((Get-FinalIdentity $final) -ne $before) { Die 'final destination changed during installation' }
        Assert-RegularFile $stage
        if ($before -eq 'absent') { [IO.File]::Move($stage, $final) }
        else {
            $oldHash = (Get-FileHash -LiteralPath $final -Algorithm SHA256).Hash
            $backup = New-OwnedFile $installDir
            try { Replace-File $stage $final $backup }
            catch {
                if ((Get-FinalIdentity $final) -ne $before) {
                    Restore-Backup $backup $final $oldHash
                    $backup = $null
                }
                elseif ((Get-Item -LiteralPath $backup).Length -eq 0) {
                    [IO.File]::Delete($backup)
                    $backup = $null
                }
                throw
            }
        }
        Assert-RegularFile $final
        if ((Get-FileHash -LiteralPath $final -Algorithm SHA256).Hash -ne
            (Get-FileHash -LiteralPath $Candidate -Algorithm SHA256).Hash) {
            if ($backup) { Restore-Backup $backup $final $oldHash }
            Die 'replacement did not preserve candidate bytes'
        }
        if ($backup) { [IO.File]::Delete($backup); $backup = $null }
    } finally {
        [IO.File]::Delete($stage)
        # Keep any backup on failure, including a recovery failure. Never delete
        # the last good copy merely to make the directory look clean.
        if ($backup) { Write-Warning "recovery material retained at $backup" }
    }
}

function Invoke-Install {
    if (Test-IsHttp $script:ReleasesBase) {
        $script:ReleasesBase = $script:ReleasesBase.TrimEnd('/')
        $base = [uri]$script:ReleasesBase
        if (-not $base.IsAbsoluteUri -or $base.UserInfo -or $base.Query -or $base.Fragment) {
            Die 'ambiguous HTTPS release base'
        }
    } else {
        $script:ReleasesBase = [IO.Path]::GetFullPath($script:ReleasesBase)
    }
    $target = Get-Target
    $version = Resolve-Version
    $asset = "terminal-file-manager-$version-$target.zip"
    $tmp = Join-Path ([IO.Path]::GetTempPath()) ("tfm-install-" + [IO.Path]::GetRandomFileName())
    New-Item -ItemType Directory -Path $tmp | Out-Null
    try {
        $archive = Join-Path $tmp $asset
        Save-Asset "$script:ReleasesBase/download/$version/$asset" $archive
        Save-Asset "$script:ReleasesBase/download/$version/$asset.sha256" "$archive.sha256"
        Assert-Checksum $archive "$archive.sha256"
        $extracted = Join-Path $tmp 'extracted'
        Expand-SafeArchive $archive $extracted
        $candidate = Join-Path $extracted 'tfm.exe'
        Assert-CandidateArchitecture $candidate $target
        Assert-CandidateVersion $candidate $version
        if ($env:TFM_INSTALL_DIR) { $installDir = $env:TFM_INSTALL_DIR }
        elseif ($env:LOCALAPPDATA) { $installDir = Join-Path $env:LOCALAPPDATA 'Programs\tfm' }
        else { Die 'no install directory could be determined; set TFM_INSTALL_DIR' }
        Commit-Candidate $candidate $installDir
        Write-Host "Installed tfm $version to $installDir."
        if (($env:Path -split ';') -notcontains $installDir) {
            Write-Warning "$installDir is not on PATH; add it to PATH to use tfm."
        }
    } finally { Remove-Item -LiteralPath $tmp -Recurse -Force -ErrorAction SilentlyContinue }
}

# Dot-sourcing exposes functions for isolated tests without running installation.
if ($MyInvocation.InvocationName -ne '.') { Invoke-Install }
