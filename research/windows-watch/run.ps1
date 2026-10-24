param([switch]$Worker, [switch]$CompileOnly, [switch]$Isolation, [string]$Fixture, [string]$Operation,
    [string]$NotifyHelper, [string]$Output = 'probe.json')
$ErrorActionPreference = 'Stop'
Add-Type -Path @("$PSScriptRoot/Native.cs", "$PSScriptRoot/Watchers.cs", "$PSScriptRoot/Probe.cs")
if ($Worker) { [WatchProbe]::Write($Fixture, $Operation); exit 0 }
[WatchProbe]::SelfCheck()
if ($CompileOnly) { 'Compiled; ABI, attribution and capture-bound self-checks passed'; exit 0 }
if (-not $IsWindows) { throw 'Native experiment requires Windows; compilation is not native evidence' }
[WatchProbe]::Run($PSCommandPath, $NotifyHelper, $Output, $Isolation.IsPresent)
$report = Get-Content -Raw -LiteralPath $Output | ConvertFrom-Json
"Recorded $($report.cases.Count) cases on $($report.host.os); shell window: $($report.host.shell_window_present)"
