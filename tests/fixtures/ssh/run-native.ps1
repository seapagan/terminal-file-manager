# Run the shared interoperability contract against a private native OpenSSH daemon.
$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

function Get-CargoExecutable {
    param([string] $Target, [string[]] $CargoArguments)
    $artifacts = & cargo @CargoArguments --locked --message-format=json
    $status = $LASTEXITCODE
    $messages = $artifacts | ForEach-Object { ConvertFrom-Json -InputObject $_ }
    if ($status -ne 0) {
        $messages | Where-Object reason -eq 'compiler-message' | ForEach-Object { $_.message.rendered }
        throw "Cargo could not build $Target."
    }
    $executable = $messages |
        Where-Object { $_.executable -and $_.target.name -ceq $Target } |
        Select-Object -ExpandProperty executable
    if (@($executable).Count -ne 1) { throw "Expected one Cargo executable for $Target." }
    return $executable
}

function Get-ProcessInfo {
    param([string] $Program, [string[]] $Arguments, [hashtable] $Environment = @{})
    $info = [Diagnostics.ProcessStartInfo]::new($Program)
    foreach ($argument in $Arguments) { $info.ArgumentList.Add($argument) }
    foreach ($name in $Environment.Keys) { $info.Environment[$name] = $Environment[$name] }
    $info.UseShellExecute = $false
    $info.RedirectStandardInput = $true
    return $info
}

function Invoke-Bounded {
    param([Diagnostics.ProcessStartInfo] $Info, [int] $Seconds = 15)
    $process = [Diagnostics.Process]::Start($Info)
    try {
        $process.StandardInput.Close()
        $watch = [Diagnostics.Stopwatch]::StartNew()
        while (-not $process.WaitForExit(100)) {
            if ($watch.Elapsed.TotalSeconds -ge $Seconds) { throw "Timed out running $($Info.FileName)." }
        }
        if ($process.ExitCode -ne 0) { throw "$($Info.FileName) exited with status $($process.ExitCode)." }
    }
    finally {
        Close-FixtureProcess -Process $process
    }
}

function Close-FixtureProcess {
    param([Diagnostics.Process] $Process)
    if (-not $Process) { return }
    try {
        if (-not $Process.HasExited) {
            $Process.Kill($true)
            if (-not $Process.WaitForExit(5000)) { throw "Could not stop $($Process.StartInfo.FileName)." }
        }
    }
    finally { $Process.Dispose() }
}

function Wait-OpenSsh {
    param([int] $Port, [Diagnostics.Process] $Server)
    $watch = [Diagnostics.Stopwatch]::StartNew()
    while ($watch.Elapsed.TotalSeconds -lt 15 -and (-not $Server -or -not $Server.HasExited)) {
        $client = [Net.Sockets.TcpClient]::new()
        try {
            $client.Connect([Net.IPAddress]::Loopback, $Port)
            $client.ReceiveTimeout = 500
            $reader = [IO.StreamReader]::new($client.GetStream())
            if ($reader.ReadLine() -clike 'SSH-2.0-*') { return }
        }
        catch [Net.Sockets.SocketException], [IO.IOException] { Write-Verbose -Message $_.Exception.Message }
        finally { $client.Dispose() }
        Start-Sleep -Milliseconds 100
    }
    throw 'The loopback OpenSSH fixture did not become ready.'
}

$sshKeygen = (Get-Command -Name ssh-keygen -CommandType Application | Select-Object -First 1).Source
$user = [Environment]::UserName
if ($IsWindows) {
    $identity = [Security.Principal.WindowsIdentity]::GetCurrent()
    $principal = [Security.Principal.WindowsPrincipal]::new($identity)
    if (-not $principal.IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)) {
        throw 'Run the native Windows fixture from an elevated PowerShell 7 console.'
    }
    $user = $user.ToLowerInvariant()
    $sshd = Join-Path -Path $env:SystemRoot -ChildPath 'System32/OpenSSH/sshd.exe'
}
else {
    if ($user -ceq 'root') { throw 'Run the Unix fixture as an unprivileged user.' }
    $sshd = '/usr/sbin/sshd'
    $sshAgent = (Get-Command -Name ssh-agent -CommandType Application | Select-Object -First 1).Source
    $sshAdd = (Get-Command -Name ssh-add -CommandType Application | Select-Object -First 1).Source
}
if (-not [IO.File]::Exists($sshd)) { throw "Install the native OpenSSH server at $sshd." }
if ($user -cnotmatch '^[a-zA-Z0-9_.-]+$') { throw 'Use a simple local account name for the OpenSSH fixture.' }
& $sshd -V
if ($LASTEXITCODE -ne 0) { throw 'Could not query the OpenSSH server version.' }

$checkout = [IO.Path]::GetFullPath("$PSScriptRoot/../../..")
Push-Location -LiteralPath $checkout
try {
    $example = Get-CargoExecutable -Target ssh_connect -CargoArguments @('build', '--example', 'ssh_connect')
    $contract = Get-CargoExecutable -Target ssh -CargoArguments @('test', '--no-run', '--test', 'ssh')
}
finally { Pop-Location }

# A short Unix path also stays within the ssh-agent socket length limit.
if ($IsWindows) {
    $root = Join-Path -Path ([IO.Path]::GetTempPath()) -ChildPath ("tfm-ssh-" + [Guid]::NewGuid().ToString('N'))
}
else {
    $root = & mktemp -d /tmp/tfm-ssh.XXXXXXXX
    if ($LASTEXITCODE -ne 0) { throw 'Could not create a private fixture directory.' }
}

$log = "$root/sshd.log"
$task = $null
$daemon = $null
$agent = $null
$denied = $null
$deniedAcl = $null
# PowerShell can stop the pipeline on Ctrl+C without entering catch.
$result = 130
try {
    $rootPath = $root.Replace('\', '/')
    if ($rootPath -match '["%\r\n]') { throw 'The temporary path cannot contain quotes, percent signs or line breaks.' }
    $remote = "$rootPath/remote"
    $denied = "$remote/suite/denied"
    $fixture = "$rootPath/fixture"
    $local = "$rootPath/local"
    if ($IsWindows) {
        [IO.Directory]::CreateDirectory($root) | Out-Null
        $acl = Get-Acl -LiteralPath $root
        $acl.SetSecurityDescriptorSddlForm("D:P(A;OICI;FA;;;SY)(A;OICI;FA;;;BA)(A;OICI;FA;;;$($identity.User.Value))",
            [Security.AccessControl.AccessControlSections]::Access)
        Set-Acl -LiteralPath $root -AclObject $acl
    }
    [IO.Directory]::CreateDirectory($fixture) | Out-Null
    [IO.Directory]::CreateDirectory($local) | Out-Null
    [IO.File]::WriteAllText("$local/local-marker", '')

    $passphrase = [Guid]::NewGuid().ToString('N')
    foreach ($key in @("$fixture/host", "$rootPath/client", "$rootPath/encrypted", "$rootPath/wrong")) {
        $keyOptions = if ($key.EndsWith('/client')) { @('-t', 'rsa', '-b', '3072') } else { @('-t', 'ed25519') }
        $secret = if ($key.EndsWith('/encrypted')) { $passphrase } else { '' }
        $info = Get-ProcessInfo -Program $sshKeygen -Arguments (@('-q', '-C', 'tfm-native-fixture', '-N', $secret, '-f', $key) + $keyOptions)
        Invoke-Bounded -Info $info
    }
    $keys = @([IO.File]::ReadAllText("$rootPath/client.pub"), [IO.File]::ReadAllText("$rootPath/encrypted.pub"))
    [IO.File]::WriteAllText("$fixture/authorized_keys", ($keys -join ''))

    if ($IsWindows) {
        $suite = "$remote/suite"
        foreach ($name in @('child', 'empty', 'denied')) { [IO.Directory]::CreateDirectory("$suite/$name") | Out-Null }
        [IO.File]::WriteAllText("$suite/child/child-marker", '')
        [IO.File]::WriteAllText("$suite/plain-file", "fixture`n")
        [IO.File]::SetLastWriteTimeUtc("$suite/plain-file", [DateTimeOffset]::FromUnixTimeSeconds(1700000000).UtcDateTime)
        [IO.File]::WriteAllText("$suite/byte-`u{754c}", '')
        [IO.Directory]::CreateSymbolicLink("$suite/linked-dir", 'child') | Out-Null
        [IO.File]::CreateSymbolicLink("$suite/linked-file", 'plain-file') | Out-Null
        [IO.File]::CreateSymbolicLink("$suite/broken-link", 'missing') | Out-Null
        $deniedAcl = Get-Acl -LiteralPath $denied
        $acl = Get-Acl -LiteralPath $denied
        $rule = [Security.AccessControl.FileSystemAccessRule]::new($identity.User, 'ReadAndExecute', 'Deny')
        $acl.AddAccessRule($rule)
        Set-Acl -LiteralPath $denied -AclObject $acl
    }
    else {
        $name = if ($IsLinux) { 'raw' } else { 'unicode' }
        & sh "$PSScriptRoot/layout.sh" $remote $name
        if ($LASTEXITCODE -ne 0) { throw 'Could not create the remote directory fixture.' }
    }

    $listener = [Net.Sockets.TcpListener]::new([Net.IPAddress]::Loopback, 0)
    $listener.Start()
    try { $port = $listener.LocalEndpoint.Port }
    finally { $listener.Stop() }
    $config = "$rootPath/sshd_config"
    # The keys file is in a private temporary directory, outside the user's real home.
    @"
ListenAddress 127.0.0.1
Port $port
PidFile "$rootPath/sshd.pid"
HostKey "$fixture/host"
AuthorizedKeysFile "$fixture/authorized_keys"
StrictModes no
AuthenticationMethods publickey
PasswordAuthentication no
KbdInteractiveAuthentication no
AllowUsers $user
DisableForwarding yes
PermitTTY no
Subsystem sftp internal-sftp
ForceCommand internal-sftp -R -d "$remote"
"@ | Set-Content -LiteralPath $config -Encoding utf8NoBOM
    if (-not $IsWindows) { Add-Content -LiteralPath $config -Value "UsePAM no`nPermitUserRC no" }

    $environment = @{
        TFM_SSH_INTEROP_ROOT = $rootPath
        TFM_SSH_INTEROP_HOST = '127.0.0.1'
        TFM_SSH_INTEROP_PORT = "$port"
        TFM_SSH_INTEROP_USER = $user
        TFM_SSH_INTEROP_EXAMPLE = $example
        TFM_SSH_INTEROP_PASSPHRASE = $passphrase
        TFM_SSH_INTEROP_MISSING_METADATA = '0'
        TFM_SSH_INTEROP_PASSWORD = '0'
        TFM_SSH_INTEROP_AGENT = ('1', '0')[$IsWindows]
        TFM_SSH_INTEROP_RAW_NAME = ('0', '1')[$IsLinux]
        TFM_SSH_INTEROP_POSIX_MODE = ('1', '0')[$IsWindows]
        SSH_AUTH_SOCK = "$rootPath/agent"
        SSH_AGENT_PID = ''
    }
    if ($IsWindows) {
        # Win32 OpenSSH needs SYSTEM for public-key logon. Do not touch its service.
        $hostKey = "$fixture/host"
        $acl = Get-Acl -LiteralPath $hostKey
        $sections = [Security.AccessControl.AccessControlSections]'Owner, Access'
        $acl.SetSecurityDescriptorSddlForm('O:BAD:P(A;;FA;;;SY)(A;;FA;;;BA)', $sections)
        Set-Acl -LiteralPath $hostKey -AclObject $acl
        $taskName = [IO.Path]::GetFileName($root)
        $action = New-ScheduledTaskAction -Execute $sshd -Argument ('-D -f "{0}" -E "{1}"' -f $config, $log) -WorkingDirectory $root
        $system = New-ScheduledTaskPrincipal -UserId 'S-1-5-18' -LogonType ServiceAccount
        $settings = New-ScheduledTaskSettingsSet -ExecutionTimeLimit (New-TimeSpan -Minutes 2) -AllowStartIfOnBatteries -DontStopIfGoingOnBatteries
        $task = Register-ScheduledTask -TaskName $taskName -Action $action -Principal $system -Settings $settings
        Start-ScheduledTask -InputObject $task
    }
    else {
        $agent = [Diagnostics.Process]::Start((Get-ProcessInfo -Program $sshAgent -Arguments @('-D', '-a', $environment.SSH_AUTH_SOCK)))
        $agent.StandardInput.Close()
        $watch = [Diagnostics.Stopwatch]::StartNew()
        while (-not [IO.File]::Exists($environment.SSH_AUTH_SOCK) -and -not $agent.HasExited -and $watch.Elapsed.TotalSeconds -lt 5) {
            Start-Sleep -Milliseconds 20
        }
        $info = Get-ProcessInfo -Program $sshAdd -Arguments @("$rootPath/client") -Environment $environment
        Invoke-Bounded -Info $info
        $daemon = [Diagnostics.Process]::Start((Get-ProcessInfo -Program $sshd -Arguments @('-D', '-f', $config, '-E', $log)))
        $daemon.StandardInput.Close()
    }
    Wait-OpenSsh -Port $port -Server $daemon
    $info = Get-ProcessInfo -Program $contract -Arguments @('interoperability::', '--ignored', '--test-threads=1') -Environment $environment
    Invoke-Bounded -Info $info -Seconds 90
    $result = 0
}
catch {
    $result = 1
    if ([IO.File]::Exists($log)) { Get-Content -LiteralPath $log | Write-Output }
    [Console]::Error.WriteLine($_)
}
finally {
    try {
        if ($task) {
            try { Stop-ScheduledTask -InputObject $task }
            finally { Unregister-ScheduledTask -InputObject $task -Confirm:$false }
        }
        try { Close-FixtureProcess -Process $daemon }
        finally { Close-FixtureProcess -Process $agent }
    }
    finally {
        if ($IsWindows) {
            if ($deniedAcl) { Set-Acl -LiteralPath $denied -AclObject $deniedAcl }
            if ([IO.Directory]::Exists($root)) { Remove-Item -LiteralPath $root -Recurse -Force }
        }
        else {
            if ($denied -and [IO.Directory]::Exists($denied)) { & chmod 700 $denied }
            # Native rm preserves filenames that PowerShell cannot decode.
            & rm -rf -- $root
            if ($LASTEXITCODE -ne 0) { throw 'Could not remove the native fixture directory.' }
        }
    }
    exit $result
}
