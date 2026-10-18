[CmdletBinding()]
param(
    [string]$Executable = "apps\desktop\src-tauri\target\release\chatygpt.exe",
    [switch]$Record
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$executablePath = Join-Path $projectRoot $Executable
$stampPath = "$executablePath.inputs.sha256"

# Todo lo que Rust, Tauri o Vite pueden incorporar al ejecutable. Los
# directorios se recorren completos y los nombres también forman parte de la
# huella, de modo que añadir, borrar o renombrar un fichero invalida el build.
$buildInputs = @(
    "apps\desktop\src",
    "apps\desktop\index.html",
    "apps\desktop\src-tauri\src",
    "apps\desktop\src-tauri\migrations",
    "apps\desktop\src-tauri\queries",
    "apps\desktop\src-tauri\capabilities",
    "apps\desktop\src-tauri\icons",
    "apps\desktop\src-tauri\resources",
    "apps\desktop\src-tauri\Cargo.toml",
    "apps\desktop\src-tauri\Cargo.lock",
    "apps\desktop\src-tauri\build.rs",
    "apps\desktop\src-tauri\tauri.conf.json",
    "package.json",
    "pnpm-lock.yaml",
    "pnpm-workspace.yaml",
    ".npmrc",
    "tsconfig.json",
    "tsconfig.app.json",
    "tsconfig.node.json",
    "vite.config.ts"
)

$entries = [System.Collections.Generic.List[string]]::new()
# Fecha de la fuente más reciente. La huella sola no basta: el 3-oct se registró
# sobre un ejecutable que no se había reconstruido (auditoría 14-oct, INC-01) y
# el lanzador dio por bueno un binario 0.3.0 con las fuentes ya en 0.4.0.
$newestInput = [datetime]::MinValue
foreach ($relativeInput in $buildInputs) {
    $inputPath = Join-Path $projectRoot $relativeInput
    if (-not (Test-Path -LiteralPath $inputPath)) {
        $entries.Add("MISSING`t$relativeInput")
        continue
    }
    $item = Get-Item -LiteralPath $inputPath
    $files = if ($item.PSIsContainer) {
        Get-ChildItem -LiteralPath $inputPath -Recurse -File
    } else {
        @($item)
    }
    foreach ($file in $files) {
        $relative = $file.FullName.Substring($projectRoot.Length).TrimStart('\', '/')
        $hash = (Get-FileHash -LiteralPath $file.FullName -Algorithm SHA256).Hash
        $entries.Add("$relative`t$($file.Length)`t$hash")
        if ($file.LastWriteTimeUtc -gt $newestInput) { $newestInput = $file.LastWriteTimeUtc }
    }
}
$manifest = ($entries | Sort-Object) -join "`n"
$bytes = [System.Text.Encoding]::UTF8.GetBytes($manifest)
$sha256 = [System.Security.Cryptography.SHA256]::Create()
try {
    $fingerprint = -join ($sha256.ComputeHash($bytes) | ForEach-Object { $_.ToString("X2") })
} finally {
    $sha256.Dispose()
}

if ($Record) {
    if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
        throw "No se puede registrar un build que no existe: $executablePath"
    }
    if ((Get-Item -LiteralPath $executablePath).LastWriteTimeUtc -lt $newestInput) {
        throw "El ejecutable es anterior a las fuentes: la compilacion no lo reemplazo y no se registra."
    }
    Set-Content -LiteralPath $stampPath -Value $fingerprint -Encoding ascii -NoNewline
    Write-Output $fingerprint
    exit 0
}

if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf) -or
    -not (Test-Path -LiteralPath $stampPath -PathType Leaf)) {
    Write-Output "1"
    exit 0
}

$recorded = (Get-Content -LiteralPath $stampPath -Raw).Trim()
$executableIsCurrent = (Get-Item -LiteralPath $executablePath).LastWriteTimeUtc -ge $newestInput
Write-Output $(if ($recorded -ceq $fingerprint -and $executableIsCurrent) { "0" } else { "1" })
