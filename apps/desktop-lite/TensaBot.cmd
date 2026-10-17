@echo off
title Tensa Telegram bot
cd /d "%~dp0"

echo.
echo   Tensa Telegram bot (long polling).
echo   Config: %USERPROFILE%\.tensa\config.toml
echo   If your network uses a proxy, set HTTPS_PROXY before running, e.g.:
echo     set HTTPS_PROXY=http://127.0.0.1:10809
echo   Ctrl+C to stop.
echo.

tensa-bot.exe "%USERPROFILE%\.tensa\config.toml" bot
