@echo off
setlocal
cd /d "%~dp0"
title ChatyGPT

echo.
echo ========================================
echo          Arrancando ChatyGPT
echo ========================================
echo.

set "RELEASE_EXE=apps\desktop\src-tauri\target\release\chatygpt.exe"
set "STAGED_EXE=apps\desktop\src-tauri\target-next\release\chatygpt.exe"

if exist "%STAGED_EXE%" (
    echo Aplicando la ultima actualizacion de ChatyGPT...
    copy /y "%STAGED_EXE%" "%RELEASE_EXE%" >nul
    if errorlevel 1 (
        echo [ERROR] ChatyGPT sigue abierto y no se puede actualizar.
        echo Cierra todas sus ventanas y vuelve a ejecutar este BAT.
        goto :failed
    )
)

set "NEED_BUILD=1"
for /f "delims=" %%I in ('powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "scripts\Needs-ChatyGPTBuild.ps1" -Executable "%RELEASE_EXE%"') do set "NEED_BUILD=%%I"

if "%NEED_BUILD%"=="1" (
    if exist "%RELEASE_EXE%" (
        echo Se han detectado cambios. Actualizando ChatyGPT...
    ) else (
        echo Preparando ChatyGPT por primera vez...
    )
    echo.

    where node.exe >nul 2>&1
    if errorlevel 1 (
        echo [ERROR] No se encuentra Node.js.
        echo Instala Node.js y vuelve a ejecutar este archivo.
        goto :failed
    )

    where pnpm.cmd >nul 2>&1
    if errorlevel 1 (
        echo [ERROR] No se encuentra pnpm.
        echo Ejecuta: npm install -g pnpm@11.9.0
        goto :failed
    )

    where rustup.exe >nul 2>&1
    if errorlevel 1 (
        echo [ERROR] No se encuentra Rustup.
        echo Instala Rust con: winget install --id Rustlang.Rustup -e
        goto :failed
    )

    rustup show active-toolchain >nul 2>&1
    if errorlevel 1 (
        rustup default stable
        if errorlevel 1 goto :failed
    )

    if not exist "node_modules\.bin\tauri.cmd" (
        call pnpm.cmd install
        if errorlevel 1 goto :failed
    )

    call pnpm.cmd rebuild esbuild
    if errorlevel 1 goto :failed

    call node_modules\.bin\tauri.cmd build --no-bundle
    if errorlevel 1 goto :failed
)

echo Broker AI: http://192.168.1.52:8765
echo.

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -File "scripts\Start-ChatyGPT.ps1" -BrokerBaseUrl "http://192.168.1.52:8765"

if errorlevel 1 goto :failed
exit /b 0

:failed
echo.
echo ChatyGPT no ha podido arrancar.
echo La ventana muestra si fallo la conexion, el token o la compilacion.
echo.
pause
exit /b 1
