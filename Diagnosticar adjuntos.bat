@echo off
setlocal
cd /d "%~dp0"

where python >nul 2>nul
if errorlevel 1 (
  echo [ERROR] No se encuentra Python en PATH.
  echo Instala Python o ejecuta el diagnostico desde una consola con Python disponible.
  pause
  exit /b 1
)

python "%~dp0scripts\diagnose_attachments.py" --output "%~dp0diagnostico-adjuntos.txt"
if errorlevel 1 (
  echo.
  echo No se pudo completar el diagnostico.
  pause
  exit /b 1
)

echo.
echo Diagnostico creado en:
echo %~dp0diagnostico-adjuntos.txt
start "" notepad.exe "%~dp0diagnostico-adjuntos.txt"
pause
