param([string]$TargetDirectory = '')
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
$previousCargoTarget = $env:CARGO_TARGET_DIR
$folioVersion = (Get-Content -LiteralPath (Join-Path $folioRoot 'package.json') -Raw | ConvertFrom-Json).version
if ($folioVersion -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') { throw 'Invalid Folio version.' }
$tauriVersion = (Get-Content -LiteralPath (Join-Path $folioRoot 'src-tauri/tauri.conf.json') -Raw | ConvertFrom-Json).version
$cargoManifest = Get-Content -LiteralPath (Join-Path $folioRoot 'src-tauri/Cargo.toml') -Raw
if ($cargoManifest.Contains("`r")) {
    throw 'Cargo.toml must use LF line endings before building: Tauri normalizes this file and would invalidate the source fingerprint.'
}
if ($tauriVersion -ne $folioVersion -or $cargoManifest -notmatch ('(?m)^version = "' + [regex]::Escape($folioVersion) + '"\r?$')) {
    throw 'Package, Tauri and Cargo versions must match.'
}
$portableName = 'Folio-v' + $folioVersion
$portableDir = Join-Path $folioRoot ('artifacts/' + $portableName)
$zip = Join-Path $folioRoot ('artifacts/' + $portableName + '-windows-x64.zip')
foreach ($output in @($portableDir, $zip, ($zip + '.sha256'))) {
    if (Test-Path -LiteralPath $output) { throw "Preserving existing package: $output. Move it aside or increment the version before rebuilding." }
}
# Assemble out of sight of the launcher. An interrupted build cannot look complete.
$stagingDir = Join-Path $folioRoot ('artifacts/portable-staging-' + [Guid]::NewGuid().ToString('N'))
if (!$TargetDirectory) { $TargetDirectory = Join-Path $folioRoot ('artifacts/build-target-v' + $folioVersion) }
$env:CARGO_TARGET_DIR = [IO.Path]::GetFullPath($TargetDirectory)
Push-Location $folioRoot
try {
    & (Join-Path $PSScriptRoot 'fetch-pdfium.ps1')
    [IO.Directory]::CreateDirectory($stagingDir) | Out-Null
    & python (Join-Path $PSScriptRoot 'build-fingerprint.py') write $stagingDir
    if ($LASTEXITCODE -ne 0) { throw 'Could not fingerprint the product and renderer.' }
    if (!(Test-Path -LiteralPath 'node_modules')) {
        & npm.cmd ci
        if ($LASTEXITCODE -ne 0) { throw 'npm ci failed.' }
    }
    & npm.cmd run tauri -- build --no-bundle --ci -- --locked --offline
    if ($LASTEXITCODE -ne 0) { throw 'Desktop release build failed.' }
    & cargo build --locked --offline --release --manifest-path src-tauri/Cargo.toml --bin folio-cli
    if ($LASTEXITCODE -ne 0) { throw 'CLI release build failed.' }
    & python (Join-Path $PSScriptRoot 'build-fingerprint.py') check $stagingDir
    if ($LASTEXITCODE -ne 0) { throw 'Product or renderer changed during build.' }
    $finalDir = $portableDir
    $portableDir = $stagingDir
    [IO.Directory]::CreateDirectory($portableDir) | Out-Null
    [IO.Directory]::CreateDirectory((Join-Path $portableDir 'resources')) | Out-Null
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/folio.exe') -Destination (Join-Path $portableDir 'Folio.exe') -Force
    Copy-Item -LiteralPath (Join-Path $env:CARGO_TARGET_DIR 'release/folio-cli.exe') -Destination (Join-Path $portableDir 'folio-cli.exe') -Force
    Copy-Item -LiteralPath 'src-tauri/resources/pdfium' -Destination (Join-Path $portableDir 'resources') -Recurse -Force
    Copy-Item -LiteralPath 'patches/pdfium/form-text-state-inheritance.patch' -Destination (Join-Path $portableDir 'resources/pdfium/form-text-state-inheritance.patch')
    Copy-Item -LiteralPath 'patches/pdfium/fractional-horizontal-advances.patch' -Destination (Join-Path $portableDir 'resources/pdfium/fractional-horizontal-advances.patch')
    Copy-Item -LiteralPath 'patches/pdfium/ordered-default-colors.patch' -Destination (Join-Path $portableDir 'resources/pdfium/ordered-default-colors.patch')
    Copy-Item -LiteralPath 'patches/pdfium/embedded-form-font-preference.patch' -Destination (Join-Path $portableDir 'resources/pdfium/embedded-form-font-preference.patch')
    Copy-Item -LiteralPath 'patches/pdfium/fractional-form-borders.patch' -Destination (Join-Path $portableDir 'resources/pdfium/fractional-form-borders.patch')
    Copy-Item -LiteralPath 'patches/pdfium/form-separator-coordinates.patch' -Destination (Join-Path $portableDir 'resources/pdfium/form-separator-coordinates.patch')
    Copy-Item -LiteralPath 'scripts/pdfium-source-lock.json' -Destination (Join-Path $portableDir 'resources/pdfium/source-lock.json')
    $dependencySnapshot = (Get-Content (Join-Path $portableDir 'build-source-fingerprint.json') -Raw | ConvertFrom-Json).files
    $packagedDependencies = @{
        'pdfium.dll' = 'src-tauri/resources/pdfium/pdfium.dll'
        'provenance.json' = 'src-tauri/resources/pdfium/provenance.json'
        'form-text-state-inheritance.patch' = 'patches/pdfium/form-text-state-inheritance.patch'
        'fractional-horizontal-advances.patch' = 'patches/pdfium/fractional-horizontal-advances.patch'
        'ordered-default-colors.patch' = 'patches/pdfium/ordered-default-colors.patch'
        'embedded-form-font-preference.patch' = 'patches/pdfium/embedded-form-font-preference.patch'
        'fractional-form-borders.patch' = 'patches/pdfium/fractional-form-borders.patch'
        'form-separator-coordinates.patch' = 'patches/pdfium/form-separator-coordinates.patch'
        'source-lock.json' = 'scripts/pdfium-source-lock.json'
    }
    foreach ($entry in $packagedDependencies.GetEnumerator()) {
        $packagedHash = (Get-FileHash (Join-Path $portableDir ('resources/pdfium/' + $entry.Key)) -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($packagedHash -ne $dependencySnapshot.($entry.Value)) { throw 'Packaged renderer differs from the build fingerprint.' }
    }
    Copy-Item -LiteralPath 'examples' -Destination $portableDir -Recurse -Force
    Copy-Item -LiteralPath 'docs' -Destination $portableDir -Recurse -Force
    foreach ($name in @('LICENSE','THIRD_PARTY_NOTICES.md','README.md','CHANGELOG.md')) {
        Copy-Item -LiteralPath $name -Destination $portableDir -Force
    }
    & (Join-Path $PSScriptRoot 'collect-licenses.ps1') -Destination (Join-Path $portableDir 'licenses')
    [ordered]@{
        version = $folioVersion
        builtUtc = [DateTime]::UtcNow.ToString('o')
        configuration = 'release'
        executableSha256 = (Get-FileHash -LiteralPath (Join-Path $portableDir 'Folio.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
        cliExecutableSha256 = (Get-FileHash -LiteralPath (Join-Path $portableDir 'folio-cli.exe') -Algorithm SHA256).Hash.ToLowerInvariant()
        ocrAssetManifestSha256 = (Get-FileHash -LiteralPath 'public/ocr/manifest.json' -Algorithm SHA256).Hash.ToLowerInvariant()
        pdfiumSha256 = (Get-FileHash -LiteralPath (Join-Path $portableDir 'resources/pdfium/pdfium.dll') -Algorithm SHA256).Hash.ToLowerInvariant()
        pdfiumVariant = (Get-Content 'src-tauri/resources/pdfium/provenance.json' -Raw | ConvertFrom-Json).variant
        pdfiumPatchSha256 = (Get-Content 'src-tauri/resources/pdfium/provenance.json' -Raw | ConvertFrom-Json).patchSha256
        pdfiumAdvancePatchSha256 = (Get-Content 'src-tauri/resources/pdfium/provenance.json' -Raw | ConvertFrom-Json).advancePatchSha256
        pdfiumColorPatchSha256 = (Get-Content 'src-tauri/resources/pdfium/provenance.json' -Raw | ConvertFrom-Json).colorPatchSha256
        pdfiumFormFontPatchSha256 = (Get-Content 'src-tauri/resources/pdfium/provenance.json' -Raw | ConvertFrom-Json).formFontPatchSha256
        pdfiumBorderPatchSha256 = (Get-Content 'src-tauri/resources/pdfium/provenance.json' -Raw | ConvertFrom-Json).borderPatchSha256
        pdfiumSeparatorPatchSha256 = (Get-Content 'src-tauri/resources/pdfium/provenance.json' -Raw | ConvertFrom-Json).separatorPatchSha256
        sourceFingerprint = (Get-Content (Join-Path $portableDir 'build-source-fingerprint.json') -Raw | ConvertFrom-Json).fingerprint
    } | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $portableDir 'build-info.json') -Encoding utf8
    # Some upstream license files carry 1970 timestamps, outside ZIP's range.
    # Normalize only copied distribution entries, leaving upstream sources intact.
    $zipMinimumDate = [DateTime]::new(1980, 2, 1)
    Get-ChildItem -LiteralPath $portableDir -Recurse -Force | ForEach-Object {
        if ($_.LastWriteTime.Year -lt 1980 -or $_.LastWriteTime.Year -gt 2107) {
            $_.LastWriteTime = $zipMinimumDate
        }
    }
    # .NET emits portable forward-slash entry names on Windows too.
    Add-Type -AssemblyName System.IO.Compression.FileSystem
    $stagedZip = $stagingDir + '.zip'
    [IO.Compression.ZipFile]::CreateFromDirectory($portableDir, $stagedZip, [IO.Compression.CompressionLevel]::Optimal, $false)
    $digest = (Get-FileHash -LiteralPath $stagedZip -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText($stagedZip + '.sha256', $digest + '  ' + [IO.Path]::GetFileName($zip) + [Environment]::NewLine)
    [IO.File]::Move($stagedZip, $zip)
    [IO.File]::Move($stagedZip + '.sha256', $zip + '.sha256')
    # Rename within artifacts only after the app, resources, licenses and ZIP exist.
    $artifactRoot = [IO.Path]::GetFullPath((Join-Path $folioRoot 'artifacts')).TrimEnd('\') + '\'
    foreach ($directory in @($portableDir, $finalDir)) {
        if (![IO.Path]::GetFullPath($directory).StartsWith($artifactRoot, [StringComparison]::OrdinalIgnoreCase)) {
            throw 'Portable directory escaped the artifacts workspace.'
        }
    }
    [IO.Directory]::Move($portableDir, $finalDir)
    Write-Host "Portable app: $finalDir"
    Write-Host "Archive: $zip"
} finally {
    $env:CARGO_TARGET_DIR = $previousCargoTarget
    Pop-Location
}
