param([Parameter(Mandatory=$true)][string]$BuildRoot, [Parameter(Mandatory=$true)][string]$VisualStudio)
$ErrorActionPreference = 'Stop'
$folioBuild = $BuildRoot
$folioVs = $VisualStudio
if (!(Test-Path -LiteralPath (Join-Path $folioVs 'VC/Tools/MSVC/14.44.35207/include'))) { throw 'Requires MSVC 14.44.35207.' }
$folioToolchain = Join-Path $folioBuild 'toolchain'
function Add-FolioJunction([string]$Path, [string]$Target) {
    if (Test-Path -LiteralPath $Path) {
        $item = Get-Item -LiteralPath $Path
        if ($item.LinkType -ne 'Junction' -or [IO.Path]::GetFullPath($item.Target) -ne [IO.Path]::GetFullPath($Target)) {
            throw "Existing toolchain path is not the expected junction: $Path"
        }
        return
    }
    New-Item -ItemType Directory -Path (Split-Path -Parent $Path) -Force | Out-Null
    New-Item -ItemType Junction -Path $Path -Target $Target | Out-Null
}
Add-FolioJunction (Join-Path $folioToolchain 'VC/Tools') (Join-Path $folioVs 'VC/Tools')
Add-FolioJunction (Join-Path $folioToolchain 'VC/Redist') (Join-Path $folioVs 'VC/Redist')
Add-FolioJunction (Join-Path $folioToolchain 'DIA SDK') (Join-Path $folioVs 'DIA SDK')
Add-FolioJunction (Join-Path $folioToolchain 'Windows Kits/10') (Join-Path $folioBuild 'sdk')
$folioVcvars = @'
@echo off
rem Folio private SDK environment; no registry or installed-toolchain changes.
set "FOLIO_BUILD_ARCH="
if /I "%~1"=="amd64" set "FOLIO_BUILD_ARCH=x64"
if /I "%~1"=="amd64_x86" set "FOLIO_BUILD_ARCH=x86"
if not defined FOLIO_BUILD_ARCH exit /b 2
if not "%~2"=="10.0.28000.0" exit /b 3
set "INCLUDE=R:\toolchain\VC\Tools\MSVC\14.44.35207\include;R:\toolchain\Windows Kits\10\Include\10.0.28000.0\ucrt;R:\toolchain\Windows Kits\10\Include\10.0.28000.0\shared;R:\toolchain\Windows Kits\10\Include\10.0.28000.0\um;R:\toolchain\Windows Kits\10\Include\10.0.28000.0\winrt"
set "LIB=R:\toolchain\VC\Tools\MSVC\14.44.35207\lib\%FOLIO_BUILD_ARCH%;R:\toolchain\Windows Kits\10\Lib\10.0.28000.0\ucrt\%FOLIO_BUILD_ARCH%;R:\toolchain\Windows Kits\10\Lib\10.0.28000.0\um\%FOLIO_BUILD_ARCH%"
set "LIBPATH=R:\toolchain\VC\Tools\MSVC\14.44.35207\lib\%FOLIO_BUILD_ARCH%"
set "PATH=R:\toolchain\VC\Tools\MSVC\14.44.35207\bin\Hostx64\%FOLIO_BUILD_ARCH%;R:\toolchain\Windows Kits\10\bin\10.0.28000.0\x64;%PATH%"
set "FOLIO_BUILD_ARCH="
exit /b 0
'@
$folioVcvars = $folioVcvars.Replace('R:\', ([IO.Path]::GetFullPath($folioBuild).TrimEnd('\') + '\'))
Set-Content -LiteralPath (Join-Path $folioToolchain 'VC/vcvarsall.bat') -Value $folioVcvars -Encoding ascii
Write-Output 'Private SDK and installed MSVC view prepared; installed toolchains unchanged'
