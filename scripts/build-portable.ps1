param()
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
Push-Location $folioRoot
try {
    & (Join-Path $PSScriptRoot 'fetch-pdfium.ps1')
    if (!(Test-Path -LiteralPath 'node_modules')) {
        & npm.cmd ci
        if ($LASTEXITCODE -ne 0) { throw 'npm ci failed.' }
    }
    & npm.cmd run tauri -- build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'Desktop release build failed.' }
    $portableDir = Join-Path $folioRoot 'artifacts/Folio'
    [IO.Directory]::CreateDirectory($portableDir) | Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $portableDir 'resources')) | Out-Null
    Copy-Item -LiteralPath 'src-tauri/target/release/folio.exe' -Destination (Join-Path $portableDir 'Folio.exe') -Force
    Copy-Item -LiteralPath 'src-tauri/resources/pdfium' -Destination (Join-Path $portableDir 'resources') -Recurse -Force
    Copy-Item -LiteralPath 'examples' -Destination $portableDir -Recurse -Force
    foreach ($name in @('LICENSE','THIRD_PARTY_NOTICES.md','README.md')) {
        Copy-Item -LiteralPath $name -Destination $portableDir -Force
    }
    $zip = Join-Path $folioRoot 'artifacts/Folio-windows-x64.zip'
    Compress-Archive -Path (Join-Path $portableDir '*') -DestinationPath $zip -Force
    Write-Host "Portable app: $portableDir"
    Write-Host "Archive: $zip"
} finally {
    Pop-Location
}
