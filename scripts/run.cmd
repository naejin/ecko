@echo off
setlocal enabledelayedexpansion
set "DIR=%~dp0.."
set "BIN=%DIR%\target\release\ecko.exe"

:: 1. Binary exists -- run it
if exist "%BIN%" (
    "%BIN%" %*
    exit /b %errorlevel%
)

:: 2. Source checkout with Rust -- build from source
where cargo >nul 2>&1
if %errorlevel% equ 0 (
    if exist "%DIR%\Cargo.toml" (
        cargo build --release --manifest-path "%DIR%\Cargo.toml" 1>&2
        "%BIN%" %*
        exit /b %errorlevel%
    )
)

:: 3. Download pre-built binary from GitHub Releases
set "REPO=naejin/ecko"
set "ARTIFACT=ecko-windows-x86_64.zip"
set "VERSION="

:: Get version from plugin.json if available
if exist "%DIR%\.claude-plugin\plugin.json" (
    where python3 >nul 2>&1
    if !errorlevel! equ 0 (
        for /f "delims=" %%v in ('python3 -c "import json; print('v'+json.load(open(r'%DIR%\.claude-plugin\plugin.json'))['version'])" 2^>nul') do set "VERSION=%%v"
    )
)

:: Fall back to latest release
if "%VERSION%"=="" (
    where curl >nul 2>&1
    if !errorlevel! equ 0 (
        for /f "tokens=2 delims=:" %%a in ('curl -fsSL "https://api.github.com/repos/%REPO%/releases/latest" 2^>nul ^| findstr "tag_name"') do (
            set "RAW=%%a"
            set "RAW=!RAW: =!"
            set "RAW=!RAW:"=!"
            set "RAW=!RAW:,=!"
            set "VERSION=!RAW!"
        )
    )
)

if "%VERSION%"=="" (
    echo Error: could not determine ecko version to download. >&2
    echo Install Rust (https://rustup.rs) and build from source, or download manually from https://github.com/%REPO%/releases >&2
    exit /b 1
)

echo Downloading ecko %VERSION% (windows-x86_64)... >&2
set "TMPDIR=%TEMP%\ecko-dl-%RANDOM%"
mkdir "%TMPDIR%" 2>nul

set "URL=https://github.com/%REPO%/releases/download/%VERSION%/%ARTIFACT%"
curl -fsSL -o "%TMPDIR%\%ARTIFACT%" "%URL%" 2>nul
if %errorlevel% neq 0 (
    echo Error: failed to download ecko binary from %URL% >&2
    rd /s /q "%TMPDIR%" 2>nul
    exit /b 1
)

:: Verify checksum using PowerShell
set "CHECKSUM_URL=https://github.com/%REPO%/releases/download/%VERSION%/checksums.txt"
curl -fsSL -o "%TMPDIR%\checksums.txt" "%CHECKSUM_URL%" 2>nul
if exist "%TMPDIR%\checksums.txt" (
    for /f "tokens=1" %%h in ('powershell -Command "(Get-FileHash '%TMPDIR%\%ARTIFACT%' -Algorithm SHA256).Hash.ToLower()"') do set "ACTUAL=%%h"
    for /f "tokens=1" %%h in ('findstr "%ARTIFACT%" "%TMPDIR%\checksums.txt"') do set "EXPECTED=%%h"
    if defined EXPECTED (
        if not "!ACTUAL!"=="!EXPECTED!" (
            echo Error: checksum mismatch for %ARTIFACT% >&2
            echo   expected: !EXPECTED! >&2
            echo   actual:   !ACTUAL! >&2
            rd /s /q "%TMPDIR%" 2>nul
            exit /b 1
        )
    )
)

:: Extract and install
cd /d "%TMPDIR%"
tar xf "%ARTIFACT%" 2>nul || (
    powershell -Command "Expand-Archive -Path '%ARTIFACT%' -DestinationPath '.' -Force" 2>nul
)
if not exist "%DIR%\target\release" mkdir "%DIR%\target\release"
copy /y "ecko\target\release\ecko.exe" "%BIN%" >nul
cd /d "%DIR%"
rd /s /q "%TMPDIR%" 2>nul
echo Downloaded ecko %VERSION% successfully. >&2

"%BIN%" %*
exit /b %errorlevel%
