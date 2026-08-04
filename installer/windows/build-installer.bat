@echo off
REM NAT3D Windows Installer Builder
REM
REM Prerequisites:
REM   1. Rust toolchain installed (rustup.rs)
REM   2. Inno Setup installed (https://jrsoftware.org/isinfo.php)
REM
REM Usage: Run this script from the NAT3D root directory
REM        > installer\windows\build-installer.bat
REM
REM Output: target\installer\NAT3D-0.1.0-Setup.exe

echo ============================================
echo NAT3D Windows Installer Builder
echo ============================================
echo.

REM Check if we're in the right directory
if not exist "Cargo.toml" (
    echo ERROR: Please run this script from the NAT3D root directory
    echo        e.g., cd G:\NAT3D ^&^& installer\windows\build-installer.bat
    exit /b 1
)

REM Step 1: Build release binary
echo [1/3] Building release binary...
cargo build --release -p nat3d-app
if %ERRORLEVEL% neq 0 (
    echo ERROR: Cargo build failed
    exit /b 1
)
echo       Done: target\release\nat3d-app.exe

REM Step 2: Create output directory
echo [2/3] Preparing output directory...
if not exist "target\installer" mkdir target\installer

REM Step 3: Find and run Inno Setup compiler
echo [3/3] Compiling installer...

REM Try common Inno Setup locations
set "ISCC="
if exist "C:\Program Files (x86)\Inno Setup 6\ISCC.exe" set "ISCC=C:\Program Files (x86)\Inno Setup 6\ISCC.exe"
if exist "C:\Program Files\Inno Setup 6\ISCC.exe" set "ISCC=C:\Program Files\Inno Setup 6\ISCC.exe"

if not defined ISCC (
    echo.
    echo WARNING: Inno Setup compiler (ISCC.exe) not found!
    echo.
    echo Please install Inno Setup from: https://jrsoftware.org/isinfo.php
    echo Then either:
    echo   A) Run build-installer.bat again, or
    echo   B) Open installer\windows\nat3d.iss in Inno Setup and click Compile
    echo.
    echo The release binary is ready at: target\release\nat3d-app.exe
    exit /b 0
)

"%ISCC%" installer\windows\nat3d.iss
if %ERRORLEVEL% neq 0 (
    echo ERROR: Inno Setup compilation failed
    exit /b 1
)

echo.
echo ============================================
echo SUCCESS!
echo ============================================
echo.
echo Installer created: target\installer\NAT3D-0.1.0-Setup.exe
echo.
echo You can distribute this file - no GitHub required.
echo.
