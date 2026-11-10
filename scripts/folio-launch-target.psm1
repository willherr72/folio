function Get-FolioLaunchHash {
    param([Parameter(Mandatory=$true)][string]$Path)
    $stream = [IO.File]::OpenRead($Path)
    $algorithm = [Security.Cryptography.SHA256]::Create()
    try { return [BitConverter]::ToString($algorithm.ComputeHash($stream)).Replace('-','').ToLowerInvariant() }
    finally { $algorithm.Dispose(); $stream.Dispose() }
}
function Resolve-FolioLaunchTarget {
    param([Parameter(Mandatory=$true)][string]$RootDirectory)
    $root = [IO.Path]::GetFullPath($RootDirectory)
    $targetFile = Join-Path $root 'artifacts/folio-launch-target.json'
    $target = $null
    if (Test-Path -LiteralPath $targetFile -PathType Leaf) {
        $target = Get-Content -LiteralPath $targetFile -Raw | ConvertFrom-Json
        $version = $target.version
    } else {
        # Fresh checkouts retain the existing explicit-build launch behavior.
        $version = (Get-Content -LiteralPath (Join-Path $root 'package.json') -Raw | ConvertFrom-Json).version
    }
    if ($version -notmatch '^\d+\.\d+\.\d+(?:-[0-9A-Za-z.-]+)?$') { throw 'Invalid Folio launch version.' }
    $portableDir = Join-Path $root ('artifacts/Folio-v' + $version)
    $exe = Join-Path $portableDir 'Folio.exe'
    $dll = Join-Path $portableDir 'resources/pdfium/pdfium.dll'
    if (!(Test-Path -LiteralPath $exe -PathType Leaf)) {
        throw "Folio $version has not been built. Run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-portable.ps1"
    }
    if (!(Test-Path -LiteralPath $dll -PathType Leaf)) { throw 'The bundled PDF engine is missing. Restore the complete portable package.' }
    if ($null -ne $target) {
        if (!$target.checkpoint -or [IO.Path]::IsPathRooted($target.checkpoint)) { throw 'Invalid launch checkpoint path.' }
        $checkpointPath = [IO.Path]::GetFullPath((Join-Path $root $target.checkpoint))
        $artifactRoot = [IO.Path]::GetFullPath((Join-Path $root 'artifacts')).TrimEnd('\','/') + [IO.Path]::DirectorySeparatorChar
        if (!$checkpointPath.StartsWith($artifactRoot, [StringComparison]::OrdinalIgnoreCase)) { throw 'Launch checkpoint escaped artifacts.' }
        if ((Get-FolioLaunchHash $checkpointPath) -ne $target.checkpointSha256) { throw 'The accepted launch checkpoint changed.' }
        $checkpoint = Get-Content -LiteralPath $checkpointPath -Raw | ConvertFrom-Json
        $expected = $checkpoint.integration.package
        if ($checkpoint.status -ne 'reviewed-development-portable-checkpoint' -or $expected.version -ne $version -or $expected.executable_sha256 -ne $target.executableSha256) { throw 'The launch target is not bound to its accepted package.' }
        if ((Get-FolioLaunchHash $exe) -ne $expected.executable_sha256) { throw 'The accepted Folio executable changed.' }
        if ((Get-FolioLaunchHash $dll) -ne $target.pdfiumSha256) { throw 'The accepted PDF engine changed.' }
        $sourcePath = Join-Path $portableDir 'build-source-fingerprint.json'
        $sourceHash = Get-FolioLaunchHash $sourcePath
        $known = @($checkpoint.pins.PSObject.Properties | Where-Object { $_.Name -match 'build-source-fingerprint\.json$' -and $_.Value -eq $sourceHash })
        if (!$known.Count) { throw 'The launch source manifest is not in the accepted checkpoint.' }
        $source = Get-Content -LiteralPath $sourcePath -Raw | ConvertFrom-Json
        if ($source.fingerprint -ne $expected.source_fingerprint -or $source.files.'src-tauri/resources/pdfium/pdfium.dll' -ne $target.pdfiumSha256) { throw 'The launch renderer does not match the accepted source.' }
    }
    return $exe
}
Export-ModuleMember -Function Resolve-FolioLaunchTarget
