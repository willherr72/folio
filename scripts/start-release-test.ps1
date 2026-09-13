param([Parameter(Mandatory=$true)][string]$Binary,[Parameter(Mandatory=$true)][string]$RunRoot,[int]$Port=9257)
$ErrorActionPreference='Stop'
$exe=(Resolve-Path -LiteralPath $Binary).Path
if(Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue){throw 'Release test port is occupied.'}
[IO.Directory]::CreateDirectory($RunRoot)|Out-Null
$RunRoot=(Resolve-Path -LiteralPath $RunRoot).Path
$env:FOLIO_DATA_DIR=Join-Path $RunRoot 'data'
$env:WEBVIEW2_USER_DATA_FOLDER=Join-Path $RunRoot 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS="--remote-debugging-port=$Port --remote-debugging-address=127.0.0.1"
$owned=Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -WindowStyle Hidden -PassThru
$deadline=[DateTime]::UtcNow.AddSeconds(25)
do {
 $owned.Refresh()
 if($owned.HasExited){throw "Release test app exited: $($owned.ExitCode)"}
 try { $version=Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 1
  if($version.webSocketDebuggerUrl){$record=@{pid=$owned.Id;port=$Port;binary=$exe;runRoot=$RunRoot;sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant()};$record|ConvertTo-Json|Set-Content -LiteralPath (Join-Path $RunRoot 'process.json') -Encoding utf8;$record|ConvertTo-Json;exit 0}
 }catch{}
 Start-Sleep -Milliseconds 200
}while([DateTime]::UtcNow -lt $deadline)
Stop-Process -Id $owned.Id
throw 'Owned release app did not expose WebView within 25 seconds.'
