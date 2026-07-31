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

if not exist "%RELEASE_EXE%" (
    echo Preparando ChatyGPT por primera vez...
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

    call pnpm.cmd tauri build --no-bundle
    if errorlevel 1 goto :failed
)

echo Broker AI: http://192.168.1.52:8765
echo.

powershell.exe -NoLogo -NoProfile -ExecutionPolicy Bypass -Command ^
  "$ErrorActionPreference = 'Stop';" ^
  "$env:CHATYGPT_BROKER_BASE_URL = 'http://192.168.1.52:8765';" ^
  "if (-not $env:AI_BROKER_ADMIN_TOKEN) {" ^
  "  $secureToken = Read-Host 'Token actual de Broker AI' -AsSecureString;" ^
  "  $credential = New-Object System.Management.Automation.PSCredential('broker', $secureToken);" ^
  "  $env:AI_BROKER_ADMIN_TOKEN = $credential.GetNetworkCredential().Password;" ^
  "}" ^
  "$headers = @{ 'x-admin-token' = $env:AI_BROKER_ADMIN_TOKEN };" ^
  "$capabilitiesUrl = $env:CHATYGPT_BROKER_BASE_URL + '/api/v1/capabilities';" ^
  "try {" ^
  "  $capabilities = Invoke-RestMethod -UseBasicParsing -Uri $capabilitiesUrl -Headers $headers -TimeoutSec 10;" ^
  "} catch {" ^
  "  throw ('No se pudo validar Broker AI en ' + $capabilitiesUrl + '. Comprueba que esta arrancado y que el token es el actual. ' + $_.Exception.Message);" ^
  "}" ^
  "Write-Host ('Broker AI listo. Contrato ' + $capabilities.contract_version);" ^
  "$releaseExe = Join-Path (Get-Location) 'apps\desktop\src-tauri\target\release\chatygpt.exe';" ^
  "& $releaseExe;" ^
  "exit $LASTEXITCODE"

if errorlevel 1 goto :failed
exit /b 0

:failed
echo.
echo ChatyGPT no ha podido arrancar.
echo La ventana muestra si fallo la conexion, el token o la compilacion.
echo.
pause
exit /b 1
