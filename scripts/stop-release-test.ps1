param([Parameter(Mandatory=$true)][string]$Record)
$ErrorActionPreference='Stop'
$owned=Get-Content -LiteralPath $Record -Raw|ConvertFrom-Json
$process=Get-Process -Id $owned.pid -ErrorAction SilentlyContinue
if (!$process) { Write-Output 'Owned test process already exited.'; exit 0 }
$expected=(Resolve-Path -LiteralPath $owned.binary).Path
if (![string]::Equals($process.Path,$expected,[StringComparison]::OrdinalIgnoreCase) -or $process.StartTime.ToUniversalTime() -ne ([DateTime]$owned.startedUtc).ToUniversalTime()) { throw 'Test process identity changed; refusing to stop it.' }
Stop-Process -Id $process.Id
Write-Output 'Stopped the verified owned test process.'
