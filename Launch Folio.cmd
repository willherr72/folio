@echo off
setlocal
cd /d "%~dp0"
if exist "artifacts\Folio\Folio.exe" (
    start "" "artifacts\Folio\Folio.exe"
    exit /b
)
echo Folio has not been built yet.
echo Run: powershell -NoProfile -ExecutionPolicy Bypass -File scripts\build-portable.ps1
pause
