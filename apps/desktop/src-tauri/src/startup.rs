use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::error::AppError;

const RUN_KEY: &str = r"HKCU\Software\Microsoft\Windows\CurrentVersion\Run";
const RUN_VALUE: &str = "ChatyGPT";
const STARTUP_DIR: &str = "windows-startup";
const STARTUP_SCRIPT: &str = "start-chatygpt.ps1";
const STARTUP_SECRET: &str = "broker-token.dpapi";

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WindowsStartupStatus {
    pub supported: bool,
    pub enabled: bool,
    pub credential_protected: bool,
    pub message: String,
}

pub fn status(data_dir: &Path) -> Result<WindowsStartupStatus, AppError> {
    #[cfg(target_os = "windows")]
    {
        let paths = startup_paths(data_dir);
        let registered = registry_entry_exists()?;
        let script_exists = paths.script.is_file();
        let secret_exists = paths.secret.is_file();
        let enabled = registered && script_exists && secret_exists;
        let message = if enabled {
            "Inicio automático activo. ChatyGPT esperará a que Broker AI esté listo.".to_owned()
        } else if registered || script_exists || secret_exists {
            "La configuración de inicio está incompleta. Actívala de nuevo para repararla."
                .to_owned()
        } else {
            "ChatyGPT no se inicia automáticamente con Windows.".to_owned()
        };
        Ok(WindowsStartupStatus {
            supported: true,
            enabled,
            credential_protected: secret_exists,
            message,
        })
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = data_dir;
        Ok(WindowsStartupStatus {
            supported: false,
            enabled: false,
            credential_protected: false,
            message: "El inicio automático solo está disponible en Windows.".to_owned(),
        })
    }
}

pub fn set_enabled(
    data_dir: &Path,
    broker_base_url: &str,
    enabled: bool,
    confirmed: bool,
) -> Result<WindowsStartupStatus, AppError> {
    #[cfg(target_os = "windows")]
    {
        if enabled {
            if !confirmed {
                return Err(AppError::Validation(
                    "activar el inicio con Windows requiere confirmación explícita".to_owned(),
                ));
            }
            enable(data_dir, broker_base_url)?;
        } else {
            disable(data_dir)?;
        }
        status(data_dir)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (data_dir, broker_base_url, enabled, confirmed);
        Err(AppError::Validation(
            "el inicio automático solo está disponible en Windows".to_owned(),
        ))
    }
}

pub fn refresh_protected_token_if_enabled(data_dir: &Path) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        let paths = startup_paths(data_dir);
        if registry_entry_exists()? && paths.script.is_file() && paths.secret.is_file() {
            if let Ok(token) = current_token(data_dir) {
                crate::secrets::protect_token(&paths.secret, &token)?;
            }
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = data_dir;
        Ok(())
    }
}

#[cfg(target_os = "windows")]
fn enable(data_dir: &Path, broker_base_url: &str) -> Result<(), AppError> {
    let token = current_token(data_dir)?;
    let executable = std::env::current_exe()
        .map_err(|error| AppError::DataDirectory(format!("ejecutable no accesible: {error}")))?;
    if !executable.is_file() {
        return Err(AppError::Validation(
            "no se encontró el ejecutable actual de ChatyGPT".to_owned(),
        ));
    }
    let paths = startup_paths(data_dir);
    std::fs::create_dir_all(&paths.directory)
        .map_err(|error| AppError::DataDirectory(format!("inicio no accesible: {error}")))?;
    crate::secrets::protect_token(&paths.secret, &token)?;
    let script = render_startup_script(&executable, &paths.secret, broker_base_url);
    let mut script_bytes = vec![0xEF, 0xBB, 0xBF];
    script_bytes.extend_from_slice(script.as_bytes());
    if let Err(error) = atomic_write(&paths.script, &script_bytes) {
        let _ = std::fs::remove_file(&paths.secret);
        return Err(error);
    }
    let command = format!(
        "powershell.exe -NoProfile -NonInteractive -ExecutionPolicy Bypass -WindowStyle Hidden -File \"{}\"",
        paths.script.display()
    );
    let output = Command::new("reg.exe")
        .args([
            "add", RUN_KEY, "/v", RUN_VALUE, "/t", "REG_SZ", "/d", &command, "/f",
        ])
        .output()
        .map_err(|error| AppError::DataDirectory(format!("registro no accesible: {error}")))?;
    if !output.status.success() {
        let _ = std::fs::remove_file(&paths.script);
        let _ = std::fs::remove_file(&paths.secret);
        return Err(AppError::DataDirectory(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    Ok(())
}

#[cfg(target_os = "windows")]
fn disable(data_dir: &Path) -> Result<(), AppError> {
    let output = Command::new("reg.exe")
        .args(["delete", RUN_KEY, "/v", RUN_VALUE, "/f"])
        .output()
        .map_err(|error| AppError::DataDirectory(format!("registro no accesible: {error}")))?;
    if !output.status.success() && registry_entry_exists()? {
        return Err(AppError::DataDirectory(
            String::from_utf8_lossy(&output.stderr).trim().to_owned(),
        ));
    }
    let paths = startup_paths(data_dir);
    for path in [&paths.script, &paths.secret] {
        if let Err(error) = std::fs::remove_file(path) {
            if error.kind() != std::io::ErrorKind::NotFound {
                return Err(AppError::DataDirectory(format!(
                    "no se pudo retirar {}: {error}",
                    path.display()
                )));
            }
        }
    }
    let _ = std::fs::remove_dir(paths.directory);
    Ok(())
}

#[cfg(target_os = "windows")]
fn current_token(data_dir: &Path) -> Result<String, AppError> {
    crate::secrets::resolve_broker_token(data_dir).ok_or_else(|| {
        AppError::Validation(
            "guarda antes la credencial del Broker en Inicio para poder activar el inicio con Windows"
                .to_owned(),
        )
    })
}

#[cfg(target_os = "windows")]
fn registry_entry_exists() -> Result<bool, AppError> {
    Command::new("reg.exe")
        .args(["query", RUN_KEY, "/v", RUN_VALUE])
        .output()
        .map(|output| output.status.success())
        .map_err(|error| AppError::DataDirectory(format!("registro no accesible: {error}")))
}

#[derive(Debug)]
struct StartupPaths {
    directory: PathBuf,
    script: PathBuf,
    secret: PathBuf,
}

fn startup_paths(data_dir: &Path) -> StartupPaths {
    let directory = data_dir.join(STARTUP_DIR);
    StartupPaths {
        script: directory.join(STARTUP_SCRIPT),
        secret: directory.join(STARTUP_SECRET),
        directory,
    }
}

fn render_startup_script(executable: &Path, secret: &Path, broker_base_url: &str) -> String {
    format!(
        r#"$ErrorActionPreference = 'SilentlyContinue'
$executable = '{}'
$secretPath = '{}'
$brokerUrl = '{}'.TrimEnd('/')
if (-not (Test-Path -LiteralPath $executable) -or -not (Test-Path -LiteralPath $secretPath)) {{ exit 1 }}
Add-Type -AssemblyName System.Security
$protected = [IO.File]::ReadAllBytes($secretPath)
$plain = [Security.Cryptography.ProtectedData]::Unprotect($protected, $null, [Security.Cryptography.DataProtectionScope]::CurrentUser)
$token = [Text.Encoding]::UTF8.GetString($plain)
[Array]::Clear($plain, 0, $plain.Length)
$headers = @{{ 'x-admin-token' = $token }}
while ($true) {{
    try {{
        $null = Invoke-RestMethod -UseBasicParsing -Uri ($brokerUrl + '/api/v1/capabilities') -Headers $headers -TimeoutSec 10
        break
    }} catch {{
        Start-Sleep -Seconds 15
    }}
}}
if (-not (Get-Process -Name 'chatygpt' -ErrorAction SilentlyContinue)) {{
    $env:CHATYGPT_BROKER_BASE_URL = $brokerUrl
    $env:AI_BROKER_ADMIN_TOKEN = $token
    Start-Process -FilePath $executable
}}
$env:AI_BROKER_ADMIN_TOKEN = $null
$token = $null
"#,
        powershell_literal(&executable.to_string_lossy()),
        powershell_literal(&secret.to_string_lossy()),
        powershell_literal(broker_base_url)
    )
}

fn powershell_literal(value: &str) -> String {
    value.replace('\'', "''").replace(['\r', '\n'], "")
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = destination.parent().ok_or_else(|| {
        AppError::Validation("el inicio automático no tiene un directorio válido".to_owned())
    })?;
    let temporary = parent.join(format!(".{}.tmp", uuid::Uuid::new_v4().simple()));
    std::fs::write(&temporary, bytes)
        .map_err(|error| AppError::DataDirectory(format!("inicio no escribible: {error}")))?;
    if destination.exists() {
        std::fs::remove_file(destination).map_err(|error| {
            AppError::DataDirectory(format!("inicio anterior no reemplazable: {error}"))
        })?;
    }
    std::fs::rename(&temporary, destination)
        .map_err(|error| AppError::DataDirectory(format!("inicio no instalable: {error}")))?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::{powershell_literal, render_startup_script, set_enabled, startup_paths};
    use crate::error::AppError;
    use std::path::Path;

    #[test]
    fn enabling_requires_explicit_confirmation_before_mutating_windows() {
        let error = set_enabled(Path::new(r"C:\Data"), "http://broker.test", true, false)
            .expect_err("un inicio no confirmado debe rechazarse");
        assert!(matches!(error, AppError::Validation(_)));
    }

    #[test]
    fn startup_script_waits_for_authenticated_broker_without_embedding_the_token() {
        let script = render_startup_script(
            Path::new(r"D:\ChatyGPT\chatygpt.exe"),
            Path::new(r"C:\Users\me\broker-token.dpapi"),
            "http://192.168.1.52:8765",
        );
        assert!(script.contains("/api/v1/capabilities"));
        assert!(script.contains("ProtectedData]::Unprotect"));
        // Sin cargar System.Security, PowerShell 5.1 no conoce ProtectedData.
        assert!(script.contains("Add-Type -AssemblyName System.Security"));
        assert!(script.contains("Start-Sleep -Seconds 15"));
        assert!(script.contains("Get-Process -Name 'chatygpt'"));
        assert!(!script.contains("AI_BROKER_ADMIN_TOKEN = '"));
        assert!(!script.contains("dsfdsjk"));
    }

    #[test]
    fn startup_paths_and_powershell_literals_are_stable_and_injection_safe() {
        let paths = startup_paths(Path::new(r"C:\Data"));
        assert_eq!(
            paths.script,
            Path::new(r"C:\Data\windows-startup\start-chatygpt.ps1")
        );
        assert_eq!(
            paths.secret,
            Path::new(r"C:\Data\windows-startup\broker-token.dpapi")
        );
        assert_eq!(powershell_literal("C:\\D'Angelo"), "C:\\D''Angelo");
        assert_eq!(powershell_literal("line\r\nbreak"), "linebreak");
    }
}
