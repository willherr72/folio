param(
    [string]$CorpusDirectory = '',
    [string]$OutputDirectory = '',
    [ValidateSet('release','debug')][string]$Profile = 'release',
    [string[]]$Fixtures = @('dense-300-pages.pdf','scan-40-pages.pdf'),
    [ValidateRange(1,10)][int]$Runs = 1
)
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
if (!$CorpusDirectory) { $CorpusDirectory = Join-Path $folioRoot 'artifacts/corpus' }
if (!$OutputDirectory) { $OutputDirectory = Join-Path $folioRoot 'artifacts/corpus-results' }
$CorpusDirectory = [IO.Path]::GetFullPath($CorpusDirectory)
$OutputDirectory = [IO.Path]::GetFullPath($OutputDirectory)
[IO.Directory]::CreateDirectory($OutputDirectory) | Out-Null
$manifest = Get-Content -Raw -LiteralPath (Join-Path $CorpusDirectory 'manifest.json') | ConvertFrom-Json
$cpu = Get-CimInstance Win32_Processor | Select-Object Name,NumberOfCores,NumberOfLogicalProcessors,MaxClockSpeed
$computer = Get-CimInstance Win32_ComputerSystem
$os = Get-CimInstance Win32_OperatingSystem
$machine = [ordered]@{
    collectedUtc = [DateTime]::UtcNow.ToString('o')
    os = $os.Caption; osVersion = $os.Version; osBuild = $os.BuildNumber
    architecture = $os.OSArchitecture; processors = @($cpu)
    totalPhysicalMemoryBytes = [long]$computer.TotalPhysicalMemory
    freePhysicalMemoryBytesAtStart = [long]$os.FreePhysicalMemory * 1024
    profile = $Profile; renderWidth = 900; sequentialPages = 24
    powerPlan = (powercfg /getactivescheme | Out-String).Trim()
    gitRevision = (git -C $folioRoot rev-parse HEAD | Out-String).Trim()
    rust = (rustc --version | Out-String).Trim()
    note = 'Isolated native test processes; warm OS filesystem cache may apply. No WebView, printer, or UI latency included. Machine may be shared with other release work.'
}
$machine.sourceSha256 = [ordered]@{}
foreach ($relative in @('src-tauri/Cargo.toml','src-tauri/Cargo.lock','src-tauri/src/engine.rs','src-tauri/src/persistence.rs','src-tauri/src/types.rs','src-tauri/tests/corpus.rs')) {
    $path = Join-Path $folioRoot $relative
    if (Test-Path -LiteralPath $path) { $machine.sourceSha256[$relative] = (Get-FileHash -Algorithm SHA256 -LiteralPath $path).Hash.ToLowerInvariant() }
}
$machine | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'machine.json') -Encoding utf8
$arguments = @('test','--manifest-path',(Join-Path $folioRoot 'src-tauri/Cargo.toml'),'--test','corpus','--offline','--no-run','--message-format=json')
if ($Profile -eq 'release') { $arguments += '--release' }
$build = & cargo @arguments
if ($LASTEXITCODE -ne 0) { throw 'Corpus native test build failed.' }
$binary = $build | ForEach-Object { try { $_ | ConvertFrom-Json } catch {} } | Where-Object { $_.reason -eq 'compiler-artifact' -and $_.target.name -eq 'corpus' -and $_.executable } | Select-Object -Last 1 -ExpandProperty executable
if (!$binary) { throw 'Cargo did not report the corpus executable.' }
$machine.testExecutableSha256 = (Get-FileHash -Algorithm SHA256 -LiteralPath $binary).Hash.ToLowerInvariant()
$machine | ConvertTo-Json -Depth 8 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'machine.json') -Encoding utf8
$results = @()
foreach ($name in $Fixtures) {
    $fixture = @($manifest.fixtures | Where-Object { $_.file -eq $name })
    if ($fixture.Count -ne 1) { throw "Unknown fixture: $name" }
    $source = Join-Path $CorpusDirectory $name
    $before = (Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash.ToLowerInvariant()
    if ($before -ne $fixture[0].sha256) { throw "Manifest checksum mismatch: $name" }
    for ($run = 1; $run -le $Runs; $run++) {
        $report = Join-Path $OutputDirectory ($name.Replace('.pdf','') + "-run-$run.json")
        $env:FOLIO_CORPUS_DIR = $CorpusDirectory
        $env:FOLIO_CORPUS_FILE = $name
        $env:FOLIO_CORPUS_REPORT = $report
        & $binary measured_large_corpus --exact --ignored --nocapture
        if ($LASTEXITCODE -ne 0) { throw "Native corpus failed: $name run $run" }
        $results += Get-Content -Raw -LiteralPath $report | ConvertFrom-Json
    }
    if ((Get-FileHash -Algorithm SHA256 -LiteralPath $source).Hash.ToLowerInvariant() -ne $before) { throw "Original source changed: $name" }
}
[ordered]@{machine=$machine; results=$results; manifest=(Join-Path $CorpusDirectory 'manifest.json')} | ConvertTo-Json -Depth 12 | Set-Content -LiteralPath (Join-Path $OutputDirectory 'summary.json') -Encoding utf8
Write-Output "Corpus results: $(Join-Path $OutputDirectory 'summary.json')"
