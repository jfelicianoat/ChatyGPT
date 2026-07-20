@echo off
setlocal
cd /d "%~dp0"
title ChatyGPT

echo.
echo ========================================
echo          Arrancando ChatyGPT
echo ========================================
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
    echo Despues, cierra y vuelve a abrir Windows antes de intentarlo.
    goto :failed
)

rustup show active-toolchain >nul 2>&1
if errorlevel 1 (
    echo Configurando la version estable de Rust por primera vez...
    rustup default stable
    if errorlevel 1 (
        echo.
        echo [ERROR] No se pudo instalar o seleccionar Rust estable.
        goto :failed
    )
    echo.
)

cargo.exe --version >nul 2>&1
if errorlevel 1 (
    echo [ERROR] Cargo sigue sin estar disponible.
    echo Cierra la sesion de Windows, vuelve a entrar y ejecuta de nuevo este archivo.
    goto :failed
)

if not exist "node_modules\.bin\tauri.cmd" (
    echo Instalando dependencias de ChatyGPT por primera vez...
    call pnpm.cmd install
    if errorlevel 1 (
        echo.
        echo [ERROR] No se pudieron instalar las dependencias.
        goto :failed
    )
    echo.
)

rem esbuild necesita generar su binario nativo. El proyecto autoriza
rem exclusivamente este script en pnpm-workspace.yaml.
call pnpm.cmd rebuild esbuild
if errorlevel 1 (
    echo.
    echo [ERROR] No se pudo preparar esbuild para Vite.
    goto :failed
)

echo Broker AI: http://192.168.1.52:8765
echo La primera compilacion puede tardar varios minutos.
echo.

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference = 'Stop';" ^
  "$env:CHATYGPT_BROKER_BASE_URL = 'http://192.168.1.52:8765';" ^
  "if (-not $env:AI_BROKER_ADMIN_TOKEN) {" ^
  "  $secureToken = Read-Host 'Token de Broker AI' -AsSecureString;" ^
  "  $credential = New-Object System.Management.Automation.PSCredential('broker', $secureToken);" ^
  "  $env:AI_BROKER_ADMIN_TOKEN = $credential.GetNetworkCredential().Password;" ^
  "}" ^
  "& pnpm.cmd tauri dev;" ^
  "exit $LASTEXITCODE"

if errorlevel 1 goto :failed
exit /b 0

:failed
echo.
echo ChatyGPT no ha podido arrancar.
echo Copia el error mostrado en esta ventana para poder revisarlo.
echo.
pause
exit /b 1
