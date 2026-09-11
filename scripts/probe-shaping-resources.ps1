param(
    [Parameter(Mandatory=$true)][string]$TestExecutable,
    [string]$Destination = ''
)
$ErrorActionPreference = 'Stop'
$probeRoot = Split-Path -Parent $PSScriptRoot
if (!$Destination) { $Destination = Join-Path $probeRoot 'artifacts/shaped-text/resources' }
[IO.Directory]::CreateDirectory($Destination) | Out-Null
$probeExecutable = (Resolve-Path -LiteralPath $TestExecutable).Path
$probeOutput = Join-Path $Destination 'test-stdout.txt'
$probeError = Join-Path $Destination 'test-stderr.txt'
$probeTimer = [Diagnostics.Stopwatch]::StartNew()
$probeProcess = Start-Process -FilePath $probeExecutable -ArgumentList @(
    'shaping_exhaustion_returns_an_explicit_error', '--exact', '--nocapture', '--test-threads=1'
) -WindowStyle Hidden -RedirectStandardOutput $probeOutput -RedirectStandardError $probeError -PassThru
$probePeakWorkingSet = [long]0
$probePeakPaged = [long]0
while (!$probeProcess.WaitForExit(10)) {
    $probeProcess.Refresh()
    $probePeakWorkingSet = [Math]::Max($probePeakWorkingSet, $probeProcess.PeakWorkingSet64)
    $probePeakPaged = [Math]::Max($probePeakPaged, $probeProcess.PeakPagedMemorySize64)
    if ($probeTimer.Elapsed.TotalSeconds -gt 10) {
        $probeProcess.Kill()
        throw 'The owned shaping test process exceeded the ten-second probe deadline.'
    }
}
$probeTimer.Stop()
$probeReport = [ordered]@{
    executableSha256 = (Get-FileHash -LiteralPath $probeExecutable -Algorithm SHA256).Hash.ToLowerInvariant()
    elapsedMilliseconds = $probeTimer.Elapsed.TotalMilliseconds
    sampledPeakWorkingSetBytes = $probePeakWorkingSet
    sampledPeakPagedBytes = $probePeakPaged
    exitCode = $probeProcess.ExitCode
    note = 'Development test-process observation including runtime/DLL/font overhead; not a hard production memory cap.'
}
$probeReport | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $Destination 'process-observation.json') -Encoding utf8
$probeReport | ConvertTo-Json
if ($probeProcess.ExitCode -ne 0) { throw 'The shaping resource regression test failed.' }
