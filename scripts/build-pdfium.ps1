param(
    [Parameter(Mandatory=$true)][string]$OutputDirectory,
    [ValidatePattern('^[D-Z]$')][string]$BuildDrive = 'R',
    [string]$VisualStudio = ''
)
$ErrorActionPreference = 'Stop'
$folioRoot = Split-Path -Parent $PSScriptRoot
$cache = Join-Path $folioRoot 'artifacts/dependencies/pdfium-build-8044'
$drive = $BuildDrive + ':'
$shortRoot = $drive + '\'
$output = [IO.Path]::GetFullPath($OutputDirectory)
if (Test-Path -LiteralPath $output) { throw "Preserving existing candidate: $output" }
if (!$VisualStudio) {
    $vswhere = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
    if (!(Test-Path -LiteralPath $vswhere)) { throw 'Install Visual Studio 2022 C++ build tools first.' }
    $VisualStudio = & $vswhere -latest -version '[17.0,18.0)' -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
    if ($LASTEXITCODE -ne 0 -or !$VisualStudio) { throw 'Visual Studio 2022 C++ build tools not found.' }
}
[IO.Directory]::CreateDirectory($cache) | Out-Null
$mapping = @(& subst.exe) | Where-Object { $_.StartsWith($drive + '\:') }
$created = $false
if ($mapping) {
    $target = ($mapping -split ' => ', 2)[1]
    if (!$target -or [IO.Path]::GetFullPath($target) -ne [IO.Path]::GetFullPath($cache)) {
        throw "Build drive $drive belongs to another directory; choose an unused -BuildDrive."
    }
} elseif (Test-Path -LiteralPath $shortRoot) {
    throw "Build drive $drive already exists; choose an unused -BuildDrive."
} else {
    & subst.exe $drive $cache
    if ($LASTEXITCODE -ne 0) { throw 'Could not create short build path.' }
    $created = $true
}
try {
    & python (Join-Path $PSScriptRoot 'build-pdfium-source.py') --root $shortRoot --output $output --vs $VisualStudio
    if ($LASTEXITCODE -ne 0) { throw 'PDFium source build failed; logs/cache and previous dependency preserved.' }
} finally {
    if ($created) {
        $current = @(& subst.exe) | Where-Object { $_.StartsWith($drive + '\:') }
        if ($current -and (($current -split ' => ', 2)[1] -eq $cache)) {
            & subst.exe $drive /D
        }
    }
}
