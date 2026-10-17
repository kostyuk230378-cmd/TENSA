@echo off
title Tensa - download models
cd /d "%~dp0"
echo.
echo   Tensa models loader: GGUF local model + optional whisper.
echo   Target: %USERPROFILE%\.tensa\models
echo   (e5-small embedder is downloaded by fastembed automatically.)
echo.
powershell -ExecutionPolicy Bypass -File "%~dp0download_models.ps1" %*
echo.
pause
