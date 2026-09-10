param()
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
$previousCargoTarget = $env:CARGO_TARGET_DIR
$folioVersion = (Get-Content -LiteralPath (Join-Path $folioRoot 'package.json') -Raw | ConvertFrom-Json).version
$env:CARGO_TARGET_DIR = Join-Path $folioRoot ('artifacts/build-target-v' + $folioVersion)
Push-Location $folioRoot
try {
    & (Join-Path $PSScriptRoot 'fetch-pdfium.ps1')
    if (!(Test-Path -LiteralPath 'node_modules')) {
        & npm.cmd ci
        if ($LASTEXITCODE -ne 0) { throw 'npm ci failed.' }
    }
    & npm.cmd run tauri -- build --no-bundle
    if ($LASTEXITCODE -ne 0) { throw 'Desktop release build failed.' }
    $folioVersion = (Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json).version
    $portableName = 'Folio-v' + $folioVersion
    $portableDir = Join-Path $folioRoot ('artifacts/' + $portableName)
    [IO.Directory]::CreateDirectory($portableDir) | Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $portableDir 'resources')) | Out-Null
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/folio.exe') -Destination (Join-Path $portableDir 'Folio.exe') -Force
    Copy-Item -LiteralPath 'src-tauri/resources/pdfium' -Destination (Join-Path $portableDir 'resources') -Recurse -Force
    Copy-Item -LiteralPath 'examples' -Destination $portableDir -Recurse -Force
    Copy-Item -LiteralPath 'docs' -Destination $portableDir -Recurse -Force
    foreach ($name in @('LICENSE','THIRD_PARTY_NOTICES.md','README.md','CHANGELOG.md')) {
        Copy-Item -LiteralPath $name -Destination $portableDir -Force
    }
    & (Join-Path $PSScriptRoot 'collect-licenses.ps1') -Destination (Join-Path $portableDir 'licenses')
    # Some upstream license files carry 1970 timestamps, outside ZIP's range.
    # Normalize only copied distribution entries, leaving upstream sources intact.
    $zipMinimumDate = [DateTime]::new(1980, 2, 1)
    Get-ChildItem -LiteralPath $portableDir -Recurse -Force | ForEach-Object {
        if ($_.LastWriteTime.Year -lt 1980 -or $_.LastWriteTime.Year -gt 2107) {
            $_.LastWriteTime = $zipMinimumDate
        }
    }
    $zip = Join-Path $folioRoot ('artifacts/' + $portableName + '-windows-x64.zip')
    Compress-Archive -Path (Join-Path $portableDir '*') -DestinationPath $zip -Force
    Write-Host "Portable app: $portableDir"
    Write-Host "Archive: $zip"
} finally {
    $env:CARGO_TARGET_DIR = $previousCargoTarget
    Pop-Location
}
