param([string]$Destination = '')
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
if (!$Destination) { $Destination = Join-Path $folioRoot 'artifacts/Folio/licenses' }
[IO.Directory]::CreateDirectory($Destination) | Out-Null
$records = [Collections.Generic.List[object]]::new()
function Copy-LicenseFiles {
    param([string]$Directory, [string]$Name, [string]$Version, [string]$License, [string]$Kind)
    $safeName = ($Name + '-' + $Version) -replace '[^a-zA-Z0-9._-]', '_'
    $outputDir = Join-Path (Join-Path $Destination $Kind) $safeName
    $files = Get-ChildItem -LiteralPath $Directory -File | Where-Object {
        $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)' -or ($Name -in @('harfrust','lopdf','jpeg-decoder','pdfium-render','libjpeg-turbo') -and $_.Name -in @('FOLIO_VENDOR.md','FOLIO_PATCH.patch','FOLIO_PROVENANCE.json')) -or ($Name -eq 'libjpeg-turbo' -and $_.Name -eq 'README.ijg') }
    if ($files) {
        [IO.Directory]::CreateDirectory($outputDir) | Out-Null
        foreach ($file in $files) { Copy-Item -LiteralPath $file.FullName -Destination $outputDir -Force }
    }
    $records.Add([ordered]@{name=$Name;version=$Version;license=$License;ecosystem=$Kind;licenseFiles=@($files | ForEach-Object Name)})
}
Push-Location $folioRoot
try {
    $cargoJson = & cargo metadata --locked --offline --format-version 1 --manifest-path src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'Could not read Cargo dependency metadata.' }
    $metadata = $cargoJson | ConvertFrom-Json
    foreach ($package in $metadata.packages) {
        if ($package.name -ne 'folio') {
            Copy-LicenseFiles -Directory (Split-Path -Parent $package.manifest_path) -Name $package.name -Version $package.version -License $package.license -Kind 'rust'
        }
    }
    Copy-LicenseFiles -Directory (Join-Path $folioRoot 'src-tauri/vendor/libjpeg-turbo-3.2.1') -Name 'libjpeg-turbo' -Version '3.2.1' -License 'IJG AND BSD-3-Clause' -Kind 'native'
    # npm misclassifies packages when node_modules is a Windows worktree junction.
    # Resolve its actual installation root, and verify the declared production set.
    $npmRoot = & node -e "console.log(require('node:path').dirname(require('node:fs').realpathSync('node_modules')))"
    if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the frontend installation.' }
    $currentPackage = Get-Content -LiteralPath 'package.json' -Raw | ConvertFrom-Json
    $installedPackage = Get-Content -LiteralPath (Join-Path $npmRoot 'package.json') -Raw | ConvertFrom-Json
    if (($currentPackage.dependencies | ConvertTo-Json -Compress) -ne ($installedPackage.dependencies | ConvertTo-Json -Compress)) { throw 'Linked frontend installation has different production dependencies.' }
    $npmDirs = & npm.cmd ls --prefix $npmRoot --omit=dev --all --parseable
    if ($LASTEXITCODE -ne 0) { throw 'Could not read frontend dependency metadata.' }
    foreach ($directory in $npmDirs) {
        if ($directory -eq $npmRoot) { continue }
        $manifest = Join-Path $directory 'package.json'
        if (Test-Path -LiteralPath $manifest) {
            $package = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
            Copy-LicenseFiles -Directory $directory -Name $package.name -Version $package.version -License $package.license -Kind 'javascript'
        }
    }
    & node scripts/setup-ocr.mjs --offline
    if ($LASTEXITCODE -ne 0) { throw 'OCR assets must be verified before collecting their licenses.' }
    $ocrLicenseDirectory = Join-Path $Destination 'ocr'
    [IO.Directory]::CreateDirectory($ocrLicenseDirectory) | Out-Null
    Copy-Item -LiteralPath 'public/ocr/licenses/tessdata_best' -Destination $ocrLicenseDirectory -Recurse -Force
    Copy-Item -LiteralPath 'public/ocr/manifest.json' -Destination (Join-Path $ocrLicenseDirectory 'asset-provenance.json') -Force
    $records.Add([ordered]@{name='tessdata_best';version='4.1.0';license='Apache-2.0';ecosystem='ocr';licenseFiles=@('tessdata_best/LICENSE','asset-provenance.json')})
    $records | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Destination 'dependencies.json') -Encoding utf8
    Write-Host "Collected license files for $($records.Count) dependency packages."
} finally {
    Pop-Location
}
