param([switch]$ResolveOnly)
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
Import-Module (Join-Path $PSScriptRoot 'folio-launch-target.psm1') -Force
$exe = Resolve-FolioLaunchTarget -RootDirectory $folioRoot
$portableDir = Split-Path -Parent $exe
if ($ResolveOnly) { $exe; return }
Start-Process -FilePath $exe -WorkingDirectory $portableDir
