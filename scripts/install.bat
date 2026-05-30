@echo off
chcp 65001 >nul
title Vlkxn Windows Installer

echo ╔══════════════════════════════════╗
echo ║   🌋 Vlkxn Windows Installer     ║
echo ╚══════════════════════════════════╝
echo.

:: Check --portable flag
if "%1"=="--portable" goto portable
if "%1"=="-p" goto portable

:: Admin check
net session >nul 2>&1
if %errorLevel% neq 0 (
    echo ⚠  Administrator rights required!
    echo.
    echo Right-click install.bat -^> "Run as administrator"
    echo.
    echo Or use portable: install.bat --portable
    echo.
    pause
    exit /b 1
)

set INSTALL_DIR=%ProgramFiles%\Vlkxn

echo Installing to %INSTALL_DIR% ...
if not exist "%INSTALL_DIR%" mkdir "%INSTALL_DIR%"

copy /Y "vlkxn-cli.exe" "%INSTALL_DIR%\" 2>nul
copy /Y "vlkxn-gui.exe" "%INSTALL_DIR%\" 2>nul
copy /Y "README.md" "%INSTALL_DIR%\" 2>nul
copy /Y "LICENSE" "%INSTALL_DIR%\" 2>nul

echo Downloading wintun driver...
powershell -ExecutionPolicy Bypass -Command "& {
    $url = 'https://www.wintun.net/builds/wintun-0.14.1.zip'
    $zip = \"$env:TEMP\wintun.zip\"
    try {
        Invoke-WebRequest -Uri $url -OutFile $zip -UseBasicParsing
        Expand-Archive -Path $zip -DestinationPath \"$env:TEMP\wintun\" -Force
        Copy-Item \"$env:TEMP\wintun\wintun\bin\amd64\wintun.dll\" \"%INSTALL_DIR%\" -Force
        echo wintun installed
    } catch { echo 'Warning: wintun download failed' }
    Remove-Item $zip -Force -ErrorAction SilentlyContinue
    Remove-Item \"$env:TEMP\wintun\" -Recurse -Force -ErrorAction SilentlyContinue
}" >nul 2>&1

:: Add to PATH
for /f "skip=2 tokens=3*" %%A in ('reg query "HKLM\SYSTEM\CurrentControlSet\Control\Session Manager\Environment" /v Path 2^>nul') do (
    setx /M PATH "%%A %%B;%INSTALL_DIR%" >nul 2>&1
)

echo.
echo ✅ Vlkxn installed!
echo Launch from Start Menu or run: vlkxn-cli --help
echo.
pause
exit /b 0

:portable
set PORTABLE_DIR=%CD%\vlkxn-portable
echo Creating portable build in %PORTABLE_DIR% ...
if not exist "%PORTABLE_DIR%" mkdir "%PORTABLE_DIR%"
copy /Y "vlkxn-cli.exe" "%PORTABLE_DIR%\" 2>nul
copy /Y "vlkxn-gui.exe" "%PORTABLE_DIR%\" 2>nul
copy /Y "README.md" "%PORTABLE_DIR%\" 2>nul
copy /Y "LICENSE" "%PORTABLE_DIR%\" 2>nul
echo.
echo ✅ Portable build ready: %PORTABLE_DIR%
echo.
pause
exit /b 0
