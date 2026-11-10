param()
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
$resourceDir = Join-Path $folioRoot 'src-tauri/resources/pdfium'
$registeredPath = Join-Path $resourceDir 'provenance.json'
if (Test-Path -LiteralPath $registeredPath) {
    $registered = Get-Content -LiteralPath $registeredPath -Raw | ConvertFrom-Json
    if ($registered.variant) {
        $lockPath = Join-Path $folioRoot 'scripts/pdfium-source-lock.json'
        $sourceLock = Get-Content -LiteralPath $lockPath -Raw | ConvertFrom-Json
        $patchPath = Join-Path $folioRoot 'patches/pdfium/form-text-state-inheritance.patch'
        $advancePatchPath = Join-Path $folioRoot 'patches/pdfium/fractional-horizontal-advances.patch'
        $colorPatchPath = Join-Path $folioRoot 'patches/pdfium/ordered-default-colors.patch'
        $formFontPatchPath = Join-Path $folioRoot 'patches/pdfium/embedded-form-font-preference.patch'
        $borderPatchPath = Join-Path $folioRoot 'patches/pdfium/fractional-form-borders.patch'
        $separatorPatchPath = Join-Path $folioRoot 'patches/pdfium/form-separator-coordinates.patch'
        $registeredDll = Join-Path $resourceDir 'pdfium.dll'
        if ($registered.variant -ne 'folio-text-state-advances-colors-form-fonts-borders-separators-v7' -or
            $registered.sourceCommit -ne 'f91ca5a72358bb0b00b4da9481b21fe668157614' -or
            !(Test-Path -LiteralPath $patchPath) -or !(Test-Path -LiteralPath $advancePatchPath) -or !(Test-Path -LiteralPath $registeredDll) -or !(Test-Path -LiteralPath $colorPatchPath) -or !(Test-Path -LiteralPath $formFontPatchPath) -or !(Test-Path -LiteralPath $borderPatchPath) -or !(Test-Path -LiteralPath $separatorPatchPath)) {
            throw 'The registered Folio PDFium build is incomplete or unsupported. Preserve it and rebuild from the pinned source recipe.'
        }
        $patchHash = (Get-FileHash -LiteralPath $patchPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $advancePatchHash = (Get-FileHash -LiteralPath $advancePatchPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $colorPatchHash = (Get-FileHash -LiteralPath $colorPatchPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $formFontPatchHash = (Get-FileHash -LiteralPath $formFontPatchPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $borderPatchHash = (Get-FileHash -LiteralPath $borderPatchPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $separatorPatchHash = (Get-FileHash -LiteralPath $separatorPatchPath -Algorithm SHA256).Hash.ToLowerInvariant()
        $dllHash = (Get-FileHash -LiteralPath $registeredDll -Algorithm SHA256).Hash.ToLowerInvariant()
        if ($patchHash -ne $registered.patchSha256 -or $advancePatchHash -ne $registered.advancePatchSha256 -or $dllHash -ne $registered.dllSha256 -or $colorPatchHash -ne $registered.colorPatchSha256 -or $formFontPatchHash -ne $registered.formFontPatchSha256 -or $borderPatchHash -ne $registered.borderPatchSha256 -or $separatorPatchHash -ne $registered.separatorPatchSha256) {
            throw 'The registered Folio PDFium build failed fingerprint verification. It will not be replaced with the baseline DLL.'
        }
        if ($patchHash -ne $sourceLock.patchSha256 -or $colorPatchHash -ne $sourceLock.colorPatchSha256 -or
            $formFontPatchHash -ne $sourceLock.formFontPatchSha256 -or
            $borderPatchHash -ne $sourceLock.borderPatchSha256 -or
            $separatorPatchHash -ne $sourceLock.separatorPatchSha256 -or
            $registered.separatorProbeSha256 -ne $sourceLock.separatorProbeSha256 -or
            $advancePatchHash -ne $sourceLock.advancePatchSha256 -or $registered.variant -ne $sourceLock.variant -or
            $registered.sourceLockSha256 -ne (Get-FileHash -LiteralPath $lockPath -Algorithm SHA256).Hash.ToLowerInvariant() -or
            $registered.buildCommit -ne $sourceLock.repositories.build.commit -or
            $registered.distributorCommit -ne $sourceLock.repositories.distributor.commit -or
            $registered.depotToolsCommit -ne $sourceLock.repositories.depot_tools.commit) {
            throw 'The PDFium provenance does not match the pinned source recipe. Preserve it and rebuild with scripts/build-pdfium.ps1.'
        }
        Write-Host 'Verified Folio PDFium with corrected text-state inheritance, fractional advances, ordered form colors, original embedded form fonts, fractional borders and Form-space text separators.'
        return
    }
}
throw 'Folio requires the corrected PDFium source build. Run scripts/build-pdfium.ps1 -OutputDirectory artifacts/pdfium-candidate, then follow docs/building-pdfium.md to install it. Existing resources have been preserved.'
