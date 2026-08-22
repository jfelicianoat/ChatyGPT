<#
.SYNOPSIS
    Arranca ChatyGPT: comprueba la credencial de Broker AI, levanta Athena y abre la app.

.DESCRIPTION
    Vivia dentro de "Arrancar ChatyGPT.bat" como un bloque de PowerShell escapado con
    acentos circunflejos, donde no se podia leer ni probar. Aqui hace lo mismo y ademas
    comprueba la credencial de verdad.

    Sobre la comprobacion, que es el motivo de este fichero: antes se validaba el token
    contra `/api/v1/capabilities`, que **no exige credencial** — se comprobo mandandole un
    token inventado y respondio 200. Una credencial caducada pasaba el control y el fallo
    aparecia mucho despues, dentro de un run, como un 403 que nadie relacionaba con el
    arranque. Ahora se pregunta a un endpoint que si la exige, y un rechazo se trata como
    lo que es: hay que dar el token actual.

    Un token tecleado a mano que resulta valido se guarda cifrado con DPAPI, en el mismo
    fichero que lee la aplicacion. Sin eso, quedaba una credencial buena en esta sesion y
    una caducada en el disco, que es exactamente como se llego a un Athena arrancado con
    un token que el broker ya no aceptaba.
#>
param(
    [string]$BrokerBaseUrl = "http://192.168.1.52:8765",

    # Modelo con el que Athena habla al broker. Athena necesita que el modelo devuelva su
    # decision como JSON estructurado; el que elige el broker por su cuenta puede no
    # hacerlo, y entonces el run muere en el primer turno sin haber tocado nada. Sin esta
    # preferencia el broker enruto a `gemma4:31b-cloud`, que contesto con una tabla en
    # markdown comparando Electron, PyQt y .NET.
    #
    # Comprobados contra este broker con el esquema de decision real de Athena:
    # `qwen3-coder:30b` y `qwen3-coder-next:latest` devuelven la decision. Tambien la
    # devuelve `nemotron-3.5-lightning:30b`, pero tarda mas de los 600 s de timeout del
    # broker en cargarse en frio y el run muere esperandolo, asi que no es el de partida.
    # Es preferencia, no imposicion: el broker sigue siendo quien enruta.
    [string]$PreferredModel = "qwen3-coder:30b",

    # Comprueba la credencial y sale. No levanta Athena ni abre la aplicacion.
    [switch]$ValidateOnly
)

$ErrorActionPreference = "Stop"

$dataDir = Join-Path $env:LOCALAPPDATA "es.jfeliciano.chatygpt"
$credentialDir = Join-Path $dataDir "credentials"
$credentialPath = Join-Path $credentialDir "broker-token.dpapi"

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

function Save-StoredToken {
    param([string]$Path, [string]$Token)

    try {
        Add-Type -AssemblyName System.Security
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $Path) | Out-Null
        $plain = [Text.Encoding]::UTF8.GetBytes($Token)
        try {
            $protected = [Security.Cryptography.ProtectedData]::Protect(
                $plain,
                $null,
                [Security.Cryptography.DataProtectionScope]::CurrentUser
            )
            [IO.File]::WriteAllBytes($Path, $protected)
        }
        finally {
            [Array]::Clear($plain, 0, $plain.Length)
        }
        return $true
    }
    catch {
        Write-Warning "No se pudo guardar la credencial: $($_.Exception.Message)"
        return $false
    }
}

function Test-BrokerCredential {
    <#
        Devuelve $true, $false o $null cuando no se ha podido saber.

        Los tres casos mandan a sitios distintos: una credencial rechazada se renueva, un
        broker apagado se arranca, y confundirlos hace que alguien busque un token nuevo
        para un problema de red. El endpoint tiene que ser uno **protegido**: los de
        lectura abierta —`/health`, `/api/v1/capabilities`, `/api/v1/queue`,
        `/api/v1/models`— contestan 200 a cualquiera y no prueban nada.
    #>
    param([string]$BaseUrl, [string]$Token)

    if (-not $Token) {
        return $false
    }
    try {
        $headers = @{ "x-admin-token" = $Token }
        Invoke-RestMethod -UseBasicParsing -Uri "$BaseUrl/api/v1/dashboard/tasks?limit=1" `
            -Headers $headers -TimeoutSec 15 | Out-Null
        return $true
    }
    catch {
        $status = $null
        if ($_.Exception.Response) {
            $status = $_.Exception.Response.StatusCode.value__
        }
        if ($status -eq 401 -or $status -eq 403) {
            return $false
        }
        return $null
    }
}

# -- credencial ---------------------------------------------------------------

$token = $env:AI_BROKER_ADMIN_TOKEN
$fromDisk = $false
if (-not $token) {
    $token = Read-StoredToken -Path $credentialPath
    if ($token) {
        $fromDisk = $true
        Write-Host "Usando la credencial guardada en ChatyGPT."
    }
}

$validated = $false
for ($intento = 0; $intento -lt 3; $intento++) {
    if ($token) {
        $veredicto = Test-BrokerCredential -BaseUrl $BrokerBaseUrl -Token $token
        if ($veredicto -eq $true) {
            $validated = $true
            break
        }
        if ($null -eq $veredicto) {
            throw ("No se pudo hablar con Broker AI en " + $BrokerBaseUrl +
                ". Comprueba que esta arrancado; la credencial no se ha podido comprobar.")
        }
        if ($fromDisk) {
            Write-Host "La credencial guardada ya no vale: el broker la rechaza."
        }
        else {
            Write-Host "Broker AI rechaza ese token."
        }
    }
    $secureToken = Read-Host "Token actual de Broker AI" -AsSecureString
    $credential = New-Object System.Management.Automation.PSCredential("broker", $secureToken)
    $token = $credential.GetNetworkCredential().Password
    $fromDisk = $false
}

if (-not $validated) {
    throw "Broker AI rechazo la credencial tres veces. Copia el token que muestra el broker al arrancar."
}

if ((Read-StoredToken -Path $credentialPath) -ne $token) {
    # Se guarda despues de comprobarla, nunca antes: escribir una credencial sin
    # comprobar sustituiria una que quiza funcionaba por otra que no. Y se guarda venga
    # de donde venga —del entorno o del teclado— porque lo que rompio un dia fue
    # justamente que la sesion usara una buena y el disco conservara la caducada.
    if (Save-StoredToken -Path $credentialPath -Token $token) {
        Write-Host "Credencial comprobada y guardada."
    }
}

$env:AI_BROKER_ADMIN_TOKEN = $token
$env:CHATYGPT_BROKER_BASE_URL = $BrokerBaseUrl

try {
    $capabilities = Invoke-RestMethod -UseBasicParsing `
        -Uri "$BrokerBaseUrl/api/v1/capabilities" -TimeoutSec 10
    Write-Host ("Broker AI listo. Contrato " + $capabilities.contract_version)
}
catch {
    # No es la puerta: es informacion. Que no se pueda leer la version del contrato no
    # dice nada sobre la credencial, que ya se comprobo arriba.
    Write-Host "Broker AI listo."
}

if ($ValidateOnly) {
    exit 0
}

# -- Athena y aplicacion ------------------------------------------------------

$raiz = Split-Path -Parent $PSScriptRoot
& (Join-Path $PSScriptRoot "Start-AthenaForChatyGPT.ps1") `
    -BrokerBaseUrl $BrokerBaseUrl -BrokerToken $token -PreferredModel $PreferredModel

$releaseExe = Join-Path $raiz "apps\desktop\src-tauri\target\release\chatygpt.exe"
$appExit = 1
try {
    & $releaseExe
    $appExit = $LASTEXITCODE
}
finally {
    & (Join-Path $PSScriptRoot "Stop-AthenaForChatyGPT.ps1")
}
exit $appExit
