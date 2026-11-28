$ErrorActionPreference = 'Stop'
$PSNativeCommandUseErrorActionPreference = $false

if ($args.Count -gt 1) {
    [Console]::Error.WriteLine('Usage: run.ps1 [smoke|full|SERVER]')
    exit 2
}
$selection = if ($args.Count -eq 0 -or $args[0] -eq '') { 'smoke' } else { $args[0] }
$rows = @(Import-Csv -LiteralPath (Join-Path $PSScriptRoot 'images.csv') | Where-Object {
    $selection -ceq 'full' -or $_.server -ceq $selection -or
        ($selection -ceq 'smoke' -and $_.server -cin @('debian', 'asyncssh', 'sftpgo'))
})
if ($rows.Count -eq 0) {
    [Console]::Error.WriteLine("Unknown SSH fixture: $selection (see images.csv)")
    exit 2
}

$docker = Get-Command docker -CommandType Application -ErrorAction SilentlyContinue | Select-Object -First 1
if (-not $docker) {
    [Console]::Error.WriteLine('The SSH matrix requires Docker with BuildKit (buildx).')
    exit 2
}
$docker = $docker.Source
& $docker buildx version | Out-Null
if ($LASTEXITCODE -ne 0) {
    [Console]::Error.WriteLine('The SSH matrix requires Docker with BuildKit (buildx).')
    exit 2
}
$engine = & $docker info --format '{{.OSType}}'
if ($LASTEXITCODE -ne 0 -or $engine -cne 'linux') {
    [Console]::Error.WriteLine('Start Docker with a Linux container engine before running the SSH matrix.')
    exit 2
}

function Invoke-Docker {
    & $docker @args
    if ($LASTEXITCODE -ne 0) { throw "docker $($args[0]) exited with status $LASTEXITCODE" }
}

function Remove-RunContainer($Name) {
    $id = Invoke-Docker container ls -aq --filter "name=^/$Name$"
    if ($id) { Invoke-Docker rm -fv $id | Out-Null }
}

function Get-ServerLog {
    # Windows PowerShell can convert redirected native stderr into error records.
    $ErrorActionPreference = 'Continue'
    & $docker logs @args $server 2>&1 | ForEach-Object { $_.ToString() }
}

function Start-RunServer($Row) {
    $image = $Row.image
    $platform = if ($Row.platform) { @('--platform', $Row.platform) }
    if ($Row.server -cne 'sftpgo') {
        $iid = Join-Path $state 'server'
        Invoke-Docker build --load --pull @platform --iidfile $iid --build-arg "BASE=$image" --build-arg "SERVER=$($Row.server)" $PSScriptRoot
        $image = (Get-Content -LiteralPath $iid -Raw).Trim()
        Remove-Item -LiteralPath $iid
    }
    $command = @(switch -CaseSensitive ($Row.server) {
        'sftpgo' { '/fixture/sftpgo.sh' }
        'asyncssh' { '-c'; 'sh /fixture/layout.sh /home/tfm && exec python /usr/local/lib/tfm-ssh/asyncssh_server.py' }
        default { '/usr/local/lib/tfm-ssh/openssh.sh' }
    })
    Invoke-Docker run -d --init --name $server @platform --network $prefix --network-alias server --mount "type=volume,src=$prefix,dst=/fixture,readonly" --entrypoint /bin/sh $image @command | Out-Null
    Invoke-Docker inspect --format 'Container image {{.Image}}' $server
}

$prefix = 'tfm-ssh-interop-' + [Guid]::NewGuid().ToString('N')
$client = "$prefix-client"
$server = "$prefix-server"
$state = Join-Path ([IO.Path]::GetTempPath()) $prefix
$root = (Resolve-Path -LiteralPath (Join-Path $PSScriptRoot '../../..')).Path
# Ctrl+C can stop the pipeline before the catch block. Keep 130 until a result is known.
$result = 130
try {
    [IO.Directory]::CreateDirectory($state) | Out-Null
    $iid = Join-Path $state 'client'
    Invoke-Docker build --load --pull --iidfile $iid -f (Join-Path $PSScriptRoot 'Dockerfile.client') $root
    $image = (Get-Content -LiteralPath $iid -Raw).Trim()
    Invoke-Docker network create --internal $prefix | Out-Null
    Invoke-Docker volume create $prefix | Out-Null
    Invoke-Docker run -d --init --name $client --network $prefix --mount "type=volume,src=$prefix,dst=/fixture" $image sleep infinity | Out-Null
    Invoke-Docker exec $client rustc --version
    Invoke-Docker exec $client timeout -k 5s 60s bash /opt/tfm-ssh/client.sh prepare

    $failed = @()
    foreach ($row in $rows) {
        Write-Host "=== $($row.server) ==="
        Write-Host "Base image $($row.image)"
        $timer = [Diagnostics.Stopwatch]::StartNew()
        try {
            Start-RunServer $row
            Invoke-Docker exec $client bash /opt/tfm-ssh/client.sh run $row.server
            Write-Host "PASS $($row.server) ($([int]$timer.Elapsed.TotalSeconds)s)"
        } catch {
            $failed += $row.server
            [Console]::Error.WriteLine("FAIL $($row.server) ($([int]$timer.Elapsed.TotalSeconds)s): $_")
            Get-ServerLog --tail 40 | & $docker exec -i $client bash /opt/tfm-ssh/client.sh redact | ForEach-Object {
                [Console]::Error.WriteLine($_)
            }
        }
        Get-ServerLog | Where-Object { $_ -cmatch '^(OpenSSH_|SFTPGo |INFO:__main__:AsyncSSH )' } |
            ForEach-Object { $_ -creplace '^INFO:__main__:', '' }
        Remove-RunContainer $server
    }
    if ($failed.Count) { [Console]::Error.WriteLine("Failed fixtures: $($failed -join ' ')") }
    $result = [int]($failed.Count -gt 0)
} catch {
    [Console]::Error.WriteLine($_)
    $result = 1
} finally {
    foreach ($cleanup in @(
        { Remove-RunContainer $server }
        { Remove-RunContainer $client }
        { if (Invoke-Docker network ls -q --filter "name=^$prefix$") { Invoke-Docker network rm $prefix | Out-Null } }
        { if (Invoke-Docker volume ls -q --filter "name=^$prefix$") { Invoke-Docker volume rm $prefix | Out-Null } }
        { if ([IO.Directory]::Exists($state)) { [IO.Directory]::Delete($state, $true) } }
    )) {
        try { & $cleanup } catch {
            [Console]::Error.WriteLine($_)
            if ($result -eq 0) { $result = 1 }
        }
    }
    exit $result
}
