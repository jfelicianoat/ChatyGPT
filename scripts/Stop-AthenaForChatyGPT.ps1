param(
    [string]$DataDir = (Join-Path $env:LOCALAPPDATA "es.jfeliciano.chatygpt")
)

$ErrorActionPreference = "Stop"
$markerPath = Join-Path $DataDir "athena-managed.json"

if (-not (Test-Path -LiteralPath $markerPath)) {
    exit 0
}

try {
    $managed = Get-Content -Raw -LiteralPath $markerPath | ConvertFrom-Json
    $managedPid = [int]$managed.pid
    $expectedStart = [DateTime]::Parse(
        [string]$managed.startedAt,
        [Globalization.CultureInfo]::InvariantCulture,
        [Globalization.DateTimeStyles]::RoundtripKind
    ).ToUniversalTime()
    $process = Get-Process -Id $managedPid -ErrorAction Stop
    $actualStart = $process.StartTime.ToUniversalTime()

    # El PID puede ser reutilizado por Windows. Solo se detiene el proceso cuya
    # hora de inicio coincide con la que ChatyGPT guardó al crearlo.
    if ([Math]::Abs(($actualStart - $expectedStart).TotalSeconds) -gt 2) {
        Write-Warning "El PID guardado de Athena pertenece ahora a otro proceso; no se detendrá."
        exit 0
    }

    Stop-Process -Id $managedPid -Force -ErrorAction Stop
    try {
        Wait-Process -Id $managedPid -Timeout 5 -ErrorAction SilentlyContinue
    }
    catch {
        # Stop-Process ya envió el cierre; no se oculta el cierre de ChatyGPT si
        # Windows tarda en retirar la entrada de la tabla de procesos.
    }
    Write-Host "Servicio administrado de Athena detenido."
}
catch [Microsoft.PowerShell.Commands.ProcessCommandException] {
    # El proceso ya terminó; el marcador solo había quedado obsoleto.
}
catch {
    Write-Warning "No se pudo cerrar la instancia administrada de Athena: $($_.Exception.Message)"
}
finally {
    Remove-Item -LiteralPath $markerPath -Force -ErrorAction SilentlyContinue
    $env:CHATYGPT_MANAGED_ATHENA_PID = $null
}
