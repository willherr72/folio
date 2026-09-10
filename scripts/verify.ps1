param()
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
Push-Location $folioRoot
try {
    & npm.cmd test
    if ($LASTEXITCODE -ne 0) { throw 'Frontend tests failed.' }
    & npm.cmd run build
    if ($LASTEXITCODE -ne 0) { throw 'Frontend typecheck/build failed.' }
    & cargo fmt --manifest-path src-tauri/Cargo.toml -- --check
    if ($LASTEXITCODE -ne 0) { throw 'Rust formatting check failed.' }
    & cargo test --manifest-path src-tauri/Cargo.toml
    if ($LASTEXITCODE -ne 0) { throw 'Native tests failed.' }
    Write-Host 'Folio frontend and native checks passed.'
} finally {
    Pop-Location
}
