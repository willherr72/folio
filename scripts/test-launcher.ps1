param()
$ErrorActionPreference = 'Stop'
$testRoot = Join-Path ([IO.Path]::GetTempPath()) ('Folio launcher test ' + [Guid]::NewGuid().ToString('N'))
[IO.Directory]::CreateDirectory((Join-Path $testRoot 'scripts')) | Out-Null
try {
    Copy-Item -LiteralPath (Join-Path $PSScriptRoot 'launch-folio.ps1') -Destination (Join-Path $testRoot 'scripts')
    $launcher = Join-Path $testRoot 'scripts/launch-folio.ps1'
    $manifest = Join-Path $testRoot 'package.json'
    [IO.File]::WriteAllText($manifest, '{"version":"0.14.0-dev.1"}')
    $package = Join-Path $testRoot 'artifacts/Folio-v0.14.0-dev.1'
    [IO.Directory]::CreateDirectory((Join-Path $package 'resources/pdfium')) | Out-Null
    function Expect-Failure([string]$Expected) {
        try { & $launcher -ResolveOnly; throw 'Unexpected success' }
        catch { if ($_.Exception.Message -notlike "*$Expected*") { throw } }
    }
    Expect-Failure 'has not been built'
    [IO.File]::WriteAllText((Join-Path $package 'Folio.exe'), 'fixture')
    Expect-Failure 'PDF engine is missing'
    [IO.File]::WriteAllText((Join-Path $package 'resources/pdfium/pdfium.dll'), 'fixture')
    if ((& $launcher -ResolveOnly) -ne (Join-Path $package 'Folio.exe')) { throw 'Wrong version resolved' }
    [IO.File]::WriteAllText($manifest, '{"version":"../outside"}')
    Expect-Failure 'Invalid Folio version'
    'Launcher: missing build, missing engine, versioned path with spaces, and invalid version checks passed.'
} finally {
    # This unique test directory was created above; never remove a supplied path.
    $resolved = [IO.Path]::GetFullPath($testRoot)
    $tempParent = [IO.Path]::GetFullPath([IO.Path]::GetTempPath()).TrimEnd('\') + '\'
    if (!$resolved.StartsWith($tempParent, [StringComparison]::OrdinalIgnoreCase)) { throw 'Test cleanup escaped temp directory' }
    Remove-Item -LiteralPath $resolved -Recurse -Force
}
