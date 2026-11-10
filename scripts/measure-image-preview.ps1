param([Parameter(Mandatory=$true)][string]$Binary,[Parameter(Mandatory=$true)][string]$FixtureRoot,[Parameter(Mandatory=$true)][string]$Report)
$ErrorActionPreference='Stop'
$exe=(Resolve-Path -LiteralPath $Binary).Path
$fixture=(Resolve-Path -LiteralPath $FixtureRoot).Path
$reportPath=[IO.Path]::GetFullPath($Report)
[IO.Directory]::CreateDirectory((Split-Path -Parent $reportPath))|Out-Null
$inputs=@('alpha-small.png','alpha-large.png','photo-large.jpg')|ForEach-Object{(Resolve-Path -LiteralPath (Join-Path $fixture $_)).Path}
$stdout=$reportPath+'.stdout';$stderr=$reportPath+'.stderr'
$process=Start-Process -FilePath $exe -ArgumentList ($inputs|ForEach-Object{'"'+$_+'"'}) -WindowStyle Hidden -PassThru -RedirectStandardOutput $stdout -RedirectStandardError $stderr
$started=$process.StartTime
$deadline=[DateTime]::UtcNow.AddMinutes(5)
$observedPeak=0L
try {
 while(-not $process.WaitForExit(250)){
  $process.Refresh()
  if(-not $process.HasExited){$observedPeak=[Math]::Max($observedPeak,$process.PeakWorkingSet64)}
  if([DateTime]::UtcNow -gt $deadline){throw 'Image preview benchmark exceeded five minutes.'}
 }
 $process.WaitForExit()
 if($process.ExitCode -ne 0){throw "Image preview benchmark failed: $(Get-Content -Raw -LiteralPath $stderr)"}
 $result=Get-Content -Raw -LiteralPath $stdout|ConvertFrom-Json
 $result|Add-Member -NotePropertyName observedPeakWorkingSetBytes -NotePropertyValue $observedPeak
 $result|Add-Member -NotePropertyName memorySamplingNote -NotePropertyValue 'Maximum OS peak working set observed at 250ms intervals; not a whole-process allocation ceiling.'
 $result|ConvertTo-Json -Depth 12|Set-Content -LiteralPath $reportPath -Encoding utf8
 $result.cases|ForEach-Object{[pscustomobject]@{Image=$_.file;PrepareMs=($_.prepareMilliseconds|Measure-Object -Average).Average;Render1200Ms=(($_.renders|Where-Object targetWidth -eq 1200).milliseconds|Measure-Object -Average).Average}}
 Write-Output "Observed peak working set: $([Math]::Round($observedPeak/1MB,1)) MiB"
} finally {
 $process.Refresh()
 if(-not $process.HasExited){
  if($process.StartTime -eq $started -and [string]::Equals($process.Path,$exe,[StringComparison]::OrdinalIgnoreCase)){Stop-Process -Id $process.Id}
  else{throw 'Benchmark process identity changed; refusing to stop it.'}
 }
 $process.Dispose()
}
