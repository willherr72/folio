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
        $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE|COPYRIGHT)' }
    if ($files) {
        [IO.Directory]::CreateDirectory($outputDir) | Out-Null
        foreach ($file in $files) { Copy-Item -LiteralPath $file.FullName -Destination $outputDir -Force }
    }
    $records.Add([ordered]@{name=$Name;version=$Version;license=$License;ecosystem=$Kind;licenseFiles=@($files | ForEach-Object Name)})
}
Push-Location $folioRoot
try {
    $cargoJson = & cargo metadata --locked --format-version 1 --manifest-path src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'Could not read Cargo dependency metadata.' }
    $metadata = $cargoJson | ConvertFrom-Json
    foreach ($package in $metadata.packages) {
        if ($package.name -ne 'folio') {
            Copy-LicenseFiles -Directory (Split-Path -Parent $package.manifest_path) -Name $package.name -Version $package.version -License $package.license -Kind 'rust'
        }
    }
    $npmDirs = & npm.cmd ls --omit=dev --all --parseable
    if ($LASTEXITCODE -ne 0) { throw 'Could not read frontend dependency metadata.' }
    foreach ($directory in $npmDirs) {
        if ($directory -eq $folioRoot) { continue }
        $manifest = Join-Path $directory 'package.json'
        if (Test-Path -LiteralPath $manifest) {
            $package = Get-Content -LiteralPath $manifest -Raw | ConvertFrom-Json
            Copy-LicenseFiles -Directory $directory -Name $package.name -Version $package.version -License $package.license -Kind 'javascript'
        }
    }
    $records | ConvertTo-Json -Depth 5 | Set-Content -LiteralPath (Join-Path $Destination 'dependencies.json') -Encoding utf8
    Write-Host "Collected license files for $($records.Count) dependency packages."
} finally {
    Pop-Location
}
