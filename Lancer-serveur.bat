@echo off
rem Starts the Cogneurs server at home and puts it online for the page on
rem GitHub Pages. Close this window to take the server offline.
chcp 65001 >nul
cd /d "%~dp0"
title Cogneurs - serveur

rem Gist where the current server address is published (see web/config.js).
set GIST=a06e4d37f026c071419b6dcbec01d11a

where cloudflared >nul 2>nul && goto :have_cloudflared
if exist "%ProgramFiles(x86)%\cloudflared\cloudflared.exe" goto :have_cloudflared
if exist "%ProgramFiles%\cloudflared\cloudflared.exe" goto :have_cloudflared
echo cloudflared n'est pas installe : installation avec winget...
winget install --id Cloudflare.cloudflared -e
if errorlevel 1 goto :error
:have_cloudflared

echo Compilation...
cargo build --release -p cogneurs-server || goto :error
cargo build --release -p cogneurs-engine --target wasm32-unknown-unknown || goto :error
copy /y target\wasm32-unknown-unknown\release\cogneurs_engine.wasm web\engine.wasm >nul

target\release\cogneurs-server.exe --tunnel --gist %GIST%
goto :end

:error
echo.
echo Echec du lancement.
:end
pause
