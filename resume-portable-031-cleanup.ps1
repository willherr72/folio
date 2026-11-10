$ErrorActionPreference='Stop'
$root=(Resolve-Path -LiteralPath (Join-Path $PSScriptRoot 'artifacts/portable-031-20261102')).Path
$attempt=Join-Path $root 'cleanup-recheck-1'
if(Test-Path -LiteralPath $attempt){throw 'Preserve previous continuation'}
$failure=Get-Content -LiteralPath (Join-Path $root 'qualification-failure.json') -Raw | ConvertFrom-Json
if($failure.ok -or $failure.stage -ne 'acceptance'){throw 'Unexpected prior failure'}
$previous=Get-Content -LiteralPath (Join-Path $root 'qualification-process.json') -Raw | ConvertFrom-Json
$oldProcess=Get-Process -Id $previous.pid -ErrorAction SilentlyContinue
if($oldProcess -and $oldProcess.Path -eq $previous.binary -and $oldProcess.StartTime.ToUniversalTime().Ticks -eq ([DateTimeOffset]$previous.startUtc).UtcTicks){throw 'Original coordinator is still active'}
$pins=Get-Content -LiteralPath (Join-Path $root 'qualification-helper-pins.json') -Raw | ConvertFrom-Json -AsHashtable
function Verify-Helpers {
 foreach($entry in $pins.GetEnumerator()){
  if((Get-FileHash -LiteralPath $entry.Key).Hash.ToLowerInvariant() -ne $entry.Value){throw "Frozen helper changed: $($entry.Key)"}
 }
}
Verify-Helpers
$steps=@()
foreach($name in @('automation','desktop-text','desktop-paragraph','desktop-ocr','combined-removal')){
 $record=Get-Content -LiteralPath (Join-Path $root ($name+'-pipeline-result.json')) -Raw | ConvertFrom-Json
 if($record.exit -ne 0 -or $record.logSha256 -ne (Get-FileHash -LiteralPath (Join-Path $root ($name+'-pipeline.log'))).Hash.ToLowerInvariant()){throw 'Completed step evidence differs'}
 $steps+=,$record
}
$failedCheck=Get-Content -LiteralPath (Join-Path $root 'owned-process-absence-pipeline-result.json') -Raw | ConvertFrom-Json
if($failedCheck.exit -eq 0){throw 'Expected the retained process-absence failure'}
New-Item -ItemType Directory -Path $attempt -ErrorAction Stop | Out-Null
$owned=Get-Process -Id $PID
@{pid=$PID;binary=$owned.Path;startUtc=$owned.StartTime.ToUniversalTime().ToString('o');script=$PSCommandPath;scriptSha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant()} | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $attempt 'process.json')
$archived=@()
foreach($name in @('owned-process-absence.json','owned-process-absence-pipeline.log','owned-process-absence-pipeline-result.json')){
 $source=(Resolve-Path -LiteralPath (Join-Path $root $name)).Path
 $target=[IO.Path]::GetFullPath((Join-Path $attempt $name))
 foreach($path in @($source,$target)){if(!$path.StartsWith($root+[IO.Path]::DirectorySeparatorChar,[StringComparison]::OrdinalIgnoreCase)){throw 'Archive path escaped milestone root'}}
 if(Test-Path -LiteralPath $target){throw 'Preserve archived evidence'}
 $digest=(Get-FileHash -LiteralPath $source).Hash.ToLowerInvariant()
 Move-Item -LiteralPath $source -Destination $target -ErrorAction Stop
 if((Get-FileHash -LiteralPath $target).Hash.ToLowerInvariant() -ne $digest){throw 'Archived evidence changed'}
 $archived+=@{original=$source;archive=$target;sha256=$digest}
}
@{reason='Final WebView child processes outlived root termination and exited naturally. Recheck strict absence without modifying its predicate or terminating other processes.';priorFailureSha256=(Get-FileHash -LiteralPath (Join-Path $root 'qualification-failure.json')).Hash.ToLowerInvariant();archived=$archived;completedStepsReused=@($steps | ForEach-Object {$_.name})} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $attempt 'continuation.json')
foreach($name in @('owned-process-absence','reconcile-runtime')){
 Verify-Helpers
 $program=if($name -eq 'owned-process-absence'){'pwsh'}else{'python'}
 [string[]]$arguments=if($name -eq 'owned-process-absence'){@('-NoProfile','-File',(Join-Path $root 'check-owned-absence.ps1'))}else{@((Join-Path $root 'reconcile-runtime.py'))}
 $log=Join-Path $root ($name+'-pipeline.log')
 if(Test-Path -LiteralPath $log){throw 'Preserve existing step log'}
 $started=[DateTime]::UtcNow.ToString('o')
 & $program @arguments *> $log
 $code=$LASTEXITCODE
 $record=@{name=$name;program=$program;arguments=$arguments;exit=$code;startedUtc=$started;finishedUtc=[DateTime]::UtcNow.ToString('o');logSha256=(Get-FileHash -LiteralPath $log).Hash.ToLowerInvariant()}
 $record | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $root ($name+'-pipeline-result.json'))
 if($code -ne 0){throw "$name failed; inspect the retained new attempt"}
 $steps+=,$record
 Write-Output "Passed $name"
}
Verify-Helpers
$continuation=@{script=$PSCommandPath;scriptSha256=(Get-FileHash -LiteralPath $PSCommandPath).Hash.ToLowerInvariant();record=(Join-Path $attempt 'continuation.json');recordSha256=(Get-FileHash -LiteralPath (Join-Path $attempt 'continuation.json')).Hash.ToLowerInvariant()}
foreach($name in @('acceptance-result.json','qualification-result.json')){if(Test-Path -LiteralPath (Join-Path $root $name)){throw 'Preserve existing terminal result'}}
@{ok=$true;steps=$steps;continuation=$continuation;finishedUtc=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 7 | Set-Content -LiteralPath (Join-Path $root 'acceptance-result.json')
@{ok=$true;status='runtime-complete-review-pending';helperManifestSha256=(Get-FileHash -LiteralPath (Join-Path $root 'qualification-helper-pins.json')).Hash.ToLowerInvariant();continuation=$continuation;finishedUtc=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json -Depth 6 | Set-Content -LiteralPath (Join-Path $root 'qualification-result.json')
@{stage='runtime-complete-review-pending';updatedUtc=[DateTime]::UtcNow.ToString('o');pid=$PID;continuation=$continuation} | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $root 'qualification-status.json')
Write-Output 'Strict process absence and runtime reconciliation complete; original failed attempt preserved.'
