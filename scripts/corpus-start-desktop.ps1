param(
    [string]$Binary = 'artifacts/Folio-test-0.6/Folio.exe',
    [ValidateRange(1024,65535)][int]$Port = 9239
)
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
$exe = (Resolve-Path -LiteralPath (Join-Path $folioRoot $Binary)).Path
if (Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue) { throw "Test port $Port is already occupied." }
$runRoot = Join-Path $folioRoot ('artifacts/corpus-ui/profile-' + [DateTime]::UtcNow.ToString('yyyyMMdd-HHmmss-fff'))
if (Test-Path -LiteralPath $runRoot) { throw 'Expected a fresh corpus test profile directory.' }
[IO.Directory]::CreateDirectory($runRoot) | Out-Null
$env:FOLIO_DATA_DIR = Join-Path $runRoot 'recovery'
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $runRoot 'webview'
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port --remote-debugging-address=127.0.0.1"
$testProcess = Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -WindowStyle Hidden -PassThru
$deadline = [DateTime]::UtcNow.AddSeconds(30)
do {
    $testProcess.Refresh()
    if ($testProcess.HasExited) { throw "Corpus test app exited with code $($testProcess.ExitCode)." }
    try {
        $endpoint = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 1
        if ($endpoint.webSocketDebuggerUrl) {
            $result = [ordered]@{pid=$testProcess.Id;port=$Port;binary=$exe;sha256=(Get-FileHash -LiteralPath $exe -Algorithm SHA256).Hash.ToLowerInvariant();dataDirectory=$env:FOLIO_DATA_DIR;webviewDirectory=$env:WEBVIEW2_USER_DATA_FOLDER}
            $json = $result | ConvertTo-Json
            $json | Set-Content -LiteralPath (Join-Path $folioRoot 'artifacts/corpus-ui/process.json') -Encoding utf8
            Write-Output $json
            exit 0
        }
    } catch { }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
$testProcess.Refresh()
if (!$testProcess.HasExited) { Stop-Process -Id $testProcess.Id }
throw 'The owned corpus test app did not expose its local WebView endpoint.'
