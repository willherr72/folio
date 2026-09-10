param([string]$Binary = 'artifacts/Folio-v0.6.0/Folio.exe', [int]$Port = 9228)
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
$exe = (Resolve-Path -LiteralPath (Join-Path $folioRoot $Binary)).Path
$existing = Get-NetTCPConnection -LocalPort $Port -State Listen -ErrorAction SilentlyContinue
if ($existing) { throw "Local automation port $Port is already in use." }
$env:WEBVIEW2_ADDITIONAL_BROWSER_ARGUMENTS = "--remote-debugging-port=$Port --remote-debugging-address=127.0.0.1"
$env:WEBVIEW2_USER_DATA_FOLDER = Join-Path $folioRoot 'artifacts/desktop-test-profile'
$folioProcess = Start-Process -FilePath $exe -WorkingDirectory (Split-Path -Parent $exe) -WindowStyle Hidden -PassThru
$deadline = [DateTime]::UtcNow.AddSeconds(25)
do {
    $folioProcess.Refresh()
    if ($folioProcess.HasExited) { throw "Folio exited before its desktop interface started (exit $($folioProcess.ExitCode))." }
    try {
        $version = Invoke-RestMethod -Uri "http://127.0.0.1:$Port/json/version" -TimeoutSec 1
        if ($version.webSocketDebuggerUrl) {
            $result = @{pid=$folioProcess.Id;port=$Port;binary=$exe}
            $result | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $folioRoot 'artifacts/desktop-process.json')
            $result | ConvertTo-Json
            exit 0
        }
    } catch { }
    Start-Sleep -Milliseconds 200
} while ([DateTime]::UtcNow -lt $deadline)
Stop-Process -Id $folioProcess.Id
throw 'Folio did not expose its test WebView within 25 seconds.'
