param()
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
$resourceDir = Join-Path $folioRoot 'src-tauri/resources/pdfium'
$archiveDir = Join-Path $folioRoot 'artifacts/dependencies'
$archive = Join-Path $archiveDir 'pdfium-win-x64-8044.tgz'
$extractDir = Join-Path $archiveDir 'pdfium-8044'
$url = 'https://github.com/bblanchon/pdfium-binaries/releases/download/chromium/8044/pdfium-win-x64.tgz'
$expectedHash = '78a17d9a5f14467631c26a3ac8741b27a0471ecc05bd6a119b523598160a0537'
[IO.Directory]::CreateDirectory($archiveDir) | Out-Null
[IO.Directory]::CreateDirectory($extractDir) | Out-Null
[IO.Directory]::CreateDirectory($resourceDir) | Out-Null
if (!(Test-Path -LiteralPath $archive)) {
    Write-Host 'Downloading pinned PDFium chromium/8044 (Windows x64)...'
    Invoke-WebRequest -Uri $url -OutFile $archive
}
$actualHash = (Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash.ToLowerInvariant()
if ($actualHash -ne $expectedHash) {
    throw "PDFium archive checksum mismatch. Expected $expectedHash; got $actualHash. Remove the archive and retry."
}
& tar -xzf $archive -C $extractDir
if ($LASTEXITCODE -ne 0) { throw 'PDFium extraction failed.' }
$dll = Join-Path $extractDir 'bin/pdfium.dll'
if (!(Test-Path -LiteralPath $dll)) { throw 'PDFium archive did not contain bin/pdfium.dll.' }
Copy-Item -LiteralPath $dll -Destination (Join-Path $resourceDir 'pdfium.dll') -Force
$licenses = Join-Path $extractDir 'licenses'
if (Test-Path -LiteralPath $licenses) {
    Copy-Item -LiteralPath $licenses -Destination $resourceDir -Recurse -Force
}
foreach ($name in @('LICENSE','VERSION','README')) {
    $source = Join-Path $extractDir $name
    if (Test-Path -LiteralPath $source) { Copy-Item -LiteralPath $source -Destination $resourceDir -Force }
}
$provenance = [ordered]@{
    project = 'PDFium'
    distributor = 'bblanchon/pdfium-binaries'
    release = 'chromium/8044'
    url = $url
    archiveSha256 = $actualHash
    dllSha256 = (Get-FileHash -LiteralPath (Join-Path $resourceDir 'pdfium.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
}
$provenance | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $resourceDir 'provenance.json') -Encoding utf8
Write-Host "PDFium ready: $resourceDir"
