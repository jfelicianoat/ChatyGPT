param(
    [Parameter(Mandatory = $true)]
    [string]$BrokerBaseUrl,

    [Parameter(Mandatory = $true)]
    [string]$BrokerToken,

    [string]$AthenaBaseUrl = "http://127.0.0.1:8770",
    [string]$AthenaRoot = ""
)

$ErrorActionPreference = "Stop"
$env:CHATYGPT_MANAGED_ATHENA_PID = $null

function Test-AthenaHealth {
    # Solo dice si Athena vive. Es publico a proposito, asi que un 200 no autoriza a
    # nadie a concluir que la credencial guardada sirve: para eso esta Test-AthenaCredential.
    try {
        $health = Invoke-RestMethod -UseBasicParsing -Uri "$AthenaBaseUrl/v1/health" -TimeoutSec 2
        return $health.wire_version -eq 1
    }
    catch {
        return $false
    }
}

function Read-StoredToken {
    param([string]$Path)

    if (-not (Test-Path -LiteralPath $Path)) {
        return $null
    }
    try {
        Add-Type -AssemblyName System.Security
        $protected = [IO.File]::ReadAllBytes($Path)
        $plain = [Security.Cryptography.ProtectedData]::Unprotect(
            $protected,
            $null,
            [Security.Cryptography.DataProtectionScope]::CurrentUser
        )
        try {
            return [Text.Encoding]::UTF8.GetString($plain)
        }
        finally {
            [Array]::Clear($plain, 0, $plain.Length)
        }
    }
    catch {
        return $null
    }
}

function Test-AthenaCredential {
    # Devuelve $true, $false, o $null cuando no se ha podido saber.
    #
    # Los tres casos son distintos para quien tiene que arreglarlo: una credencial que no
    # vale se rehace, y un servicio que no contesta a la comprobacion no dice nada sobre
    # la credencial. Tratarlos igual mandaria a alguien a revincular por un problema ajeno.
    param([string]$Token)

    if (-not $Token) {
        return $false
    }
    try {
        $headers = @{ Authorization = "Bearer $Token" }
        Invoke-RestMethod -UseBasicParsing -Uri "$AthenaBaseUrl/v1/auth/check" `
            -Headers $headers -TimeoutSec 3 | Out-Null
        return $true
    }
    catch {
        $status = $_.Exception.Response.StatusCode.value__
        if ($status -eq 401 -or $status -eq 403) {
            return $false
        }
        return $null
    }
}

$dataDir = Join-Path $env:LOCALAPPDATA "es.jfeliciano.chatygpt"
$credentialDir = Join-Path $dataDir "credentials"
$credentialPath = Join-Path $credentialDir "athena-token.dpapi"
$managedStatePath = Join-Path $dataDir "athena-managed.json"

if (Test-AthenaHealth) {
    if (Test-Path -LiteralPath $managedStatePath) {
        try {
            $managed = Get-Content -Raw -LiteralPath $managedStatePath | ConvertFrom-Json
            $managedProcess = Get-Process -Id ([int]$managed.pid) -ErrorAction Stop
            $expectedStart = [DateTime]::Parse(
                [string]$managed.startedAt,
                [Globalization.CultureInfo]::InvariantCulture,
                [Globalization.DateTimeStyles]::RoundtripKind
            ).ToUniversalTime()
            if ([Math]::Abs(($managedProcess.StartTime.ToUniversalTime() - $expectedStart).TotalSeconds) -le 2) {
                $env:CHATYGPT_MANAGED_ATHENA_PID = [string]$managedProcess.Id
                if ($managed.baseUrl) {
                    $AthenaBaseUrl = [string]$managed.baseUrl
                }
                Write-Host "Athena administrada recuperada en $AthenaBaseUrl."
            }
            else {
                Remove-Item -LiteralPath $managedStatePath -Force -ErrorAction SilentlyContinue
            }
        }
        catch {
            Remove-Item -LiteralPath $managedStatePath -Force -ErrorAction SilentlyContinue
        }
    }
    # Esta Athena responde. La pregunta que queda es si es la misma que emitió la
    # credencial guardada, y sólo hay una forma de saberlo: usarla.
    $env:ATHENA_BASE_URL = $AthenaBaseUrl
    if (-not (Test-Path -LiteralPath $credentialPath)) {
        Write-Warning "Athena ya está arrancada, pero ChatyGPT no tiene su credencial. Podrás guardarla en la sección Athena."
        exit 0
    }
    $vale = Test-AthenaCredential -Token (Read-StoredToken -Path $credentialPath)
    if ($vale -eq $false) {
        # Deliberadamente no se reacuña nada. La credencial de un proceso está fijada
        # mientras vive, así que generar otra no la cambiaría; y si ese proceso no lo
        # arrancó ChatyGPT, rotarle nada es meterse con la sesión de otro. Lo que
        # corresponde es decirlo y que la persona vuelva a vincular.
        Write-Warning "Athena está arrancada pero rechaza la credencial guardada: es de otra sesión suya. Vuelve a vincularla en la sección Athena."
        exit 0
    }
    if ($null -eq $vale) {
        Write-Warning "Athena responde pero no se pudo comprobar la credencial. Se intentará usar la guardada."
        exit 0
    }
    Write-Host "Athena ya está disponible en $AthenaBaseUrl."
    exit 0
}

if (-not $AthenaRoot) {
    $AthenaRoot = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot "..\..\Athena"))
}
$pythonw = Join-Path $AthenaRoot ".venv\Scripts\pythonw.exe"
$sourceDir = Join-Path $AthenaRoot "src"
if (-not (Test-Path -LiteralPath $pythonw)) {
    throw "No se encontró el entorno Python de Athena en $pythonw."
}
if (-not (Test-Path -LiteralPath (Join-Path $sourceDir "athena_service.py"))) {
    throw "La instalación de Athena no contiene el servicio para ChatyGPT."
}

$random = New-Object byte[] 32
$generator = [Security.Cryptography.RandomNumberGenerator]::Create()
try {
    $generator.GetBytes($random)
}
finally {
    $generator.Dispose()
}
$serviceToken = [Convert]::ToBase64String($random).TrimEnd("=").Replace("+", "-").Replace("/", "_")
[Array]::Clear($random, 0, $random.Length)

New-Item -ItemType Directory -Force -Path $credentialDir | Out-Null
Add-Type -AssemblyName System.Security
$plain = [Text.Encoding]::UTF8.GetBytes($serviceToken)
try {
    $protected = [Security.Cryptography.ProtectedData]::Protect(
        $plain,
        $null,
        [Security.Cryptography.DataProtectionScope]::CurrentUser
    )
    [IO.File]::WriteAllBytes($credentialPath, $protected)
}
finally {
    [Array]::Clear($plain, 0, $plain.Length)
}

$previous = @{
    PYTHONPATH = $env:PYTHONPATH
    ATHENA_BROKER_BASE_URL = $env:ATHENA_BROKER_BASE_URL
    ATHENA_BROKER_TOKEN = $env:ATHENA_BROKER_TOKEN
    ATHENA_SERVICE_TOKEN = $env:ATHENA_SERVICE_TOKEN
    ATHENA_STATE_DIR = $env:ATHENA_STATE_DIR
}

try {
    $env:PYTHONPATH = if ($env:PYTHONPATH) { "$sourceDir;$env:PYTHONPATH" } else { $sourceDir }
    $env:ATHENA_BROKER_BASE_URL = $BrokerBaseUrl
    $env:ATHENA_BROKER_TOKEN = $BrokerToken
    $env:ATHENA_SERVICE_TOKEN = $serviceToken
    $env:ATHENA_STATE_DIR = Join-Path $env:LOCALAPPDATA "Athena\service"
    $process = Start-Process -FilePath $pythonw -ArgumentList "-m", "athena_service" `
        -WorkingDirectory $AthenaRoot -WindowStyle Hidden -PassThru
}
finally {
    $env:PYTHONPATH = $previous.PYTHONPATH
    $env:ATHENA_BROKER_BASE_URL = $previous.ATHENA_BROKER_BASE_URL
    $env:ATHENA_BROKER_TOKEN = $previous.ATHENA_BROKER_TOKEN
    $env:ATHENA_SERVICE_TOKEN = $previous.ATHENA_SERVICE_TOKEN
    $env:ATHENA_STATE_DIR = $previous.ATHENA_STATE_DIR
    $serviceToken = $null
}

$deadline = [DateTime]::UtcNow.AddSeconds(15)
while ([DateTime]::UtcNow -lt $deadline) {
    if ($process.HasExited) {
        throw "Athena terminó durante el arranque (código $($process.ExitCode))."
    }
    if (Test-AthenaHealth) {
        $managedState = @{
            pid = $process.Id
            startedAt = $process.StartTime.ToUniversalTime().ToString("O")
            baseUrl = $AthenaBaseUrl
        } | ConvertTo-Json -Compress
        [IO.File]::WriteAllText(
            $managedStatePath,
            $managedState,
            (New-Object Text.UTF8Encoding($false))
        )
        $env:CHATYGPT_MANAGED_ATHENA_PID = [string]$process.Id
        # La dirección de la instancia que gobernamos gana sobre la que la aplicación
        # trae por defecto: quien arranca el proceso es quien sabe dónde quedó.
        $env:ATHENA_BASE_URL = $AthenaBaseUrl
        Write-Host "Athena lista en $AthenaBaseUrl."
        exit 0
    }
    Start-Sleep -Milliseconds 250
}

if (-not $process.HasExited) {
    Stop-Process -Id $process.Id -Force -ErrorAction SilentlyContinue
}
throw "Athena no respondió en $AthenaBaseUrl después de 15 segundos."
