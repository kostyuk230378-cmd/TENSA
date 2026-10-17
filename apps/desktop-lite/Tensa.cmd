@echo off
title Tensa
cd /d "%~dp0"

set "TENSA_DIR=%USERPROFILE%\.tensa"
set "TENSA_CONFIG=%TENSA_DIR%\config.toml"

if not exist "%TENSA_DIR%" mkdir "%TENSA_DIR%"

if not exist "%TENSA_CONFIG%" (
    echo First run: creating default config...
    tensa-cli.exe -c "%TENSA_CONFIG%" init --owner-name User --secretary-name Tensa --timezone UTC >nul 2>&1
)

echo.
echo   Tensa core: http://127.0.0.1:8765/app
echo   Close this window to stop the server.
echo.
start "" "http://127.0.0.1:8765/app"
tensa-server.exe "%TENSA_CONFIG%"
