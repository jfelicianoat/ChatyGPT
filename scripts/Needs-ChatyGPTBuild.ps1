[CmdletBinding()]
param(
    [string]$Executable = "apps\desktop\src-tauri\target\release\chatygpt.exe"
)

$ErrorActionPreference = "Stop"
$projectRoot = Split-Path -Parent $PSScriptRoot
$executablePath = Join-Path $projectRoot $Executable

if (-not (Test-Path -LiteralPath $executablePath -PathType Leaf)) {
    Write-Output "1"
    exit 0
}

$sourcePaths = @(
    "apps\desktop\src",
    "apps\desktop\src-tauri\src",
    "apps\desktop\src-tauri\capabilities",
    "apps\desktop\src-tauri\Cargo.toml",
    "apps\desktop\src-tauri\build.rs",
    "apps\desktop\src-tauri\tauri.conf.json",
    "index.html",
    "package.json",
    "pnpm-lock.yaml",
    "tsconfig.json",
    "vite.config.ts"
)

$executableTime = (Get-Item -LiteralPath $executablePath).LastWriteTimeUtc
foreach ($relativePath in $sourcePaths) {
    $path = Join-Path $projectRoot $relativePath
    if (-not (Test-Path -LiteralPath $path)) {
        continue
    }

    $item = Get-Item -LiteralPath $path
    if (-not $item.PSIsContainer) {
        if ($item.LastWriteTimeUtc -gt $executableTime) {
            Write-Output "1"
            exit 0
        }
        continue
    }

    foreach ($sourceFile in Get-ChildItem -LiteralPath $path -Recurse -File) {
        if ($sourceFile.LastWriteTimeUtc -gt $executableTime) {
            Write-Output "1"
            exit 0
        }
    }
}

Write-Output "0"
