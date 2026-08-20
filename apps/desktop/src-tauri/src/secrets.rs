//! Custodia local del token administrativo de AI Broker.
//!
//! La Fase 0 exigía sustituir la variable de entorno por el almacén seguro del
//! sistema. Aquí el secreto se cifra con DPAPI en el ámbito `CurrentUser`, de
//! modo que solo la cuenta de Windows que lo guardó puede descifrarlo: ni otra
//! cuenta del mismo equipo ni una copia del archivo a otra máquina sirven.
//!
//! El token nunca se escribe en SQLite, en el registro, en el vault ni en el
//! script de arranque, y jamás viaja por la línea de órdenes —que es pública en
//! la lista de procesos—, sino por el entorno del proceso hijo.

use std::path::{Path, PathBuf};
use std::process::Command;

use serde::Serialize;

use crate::error::AppError;

const CREDENTIALS_DIR: &str = "credentials";
const BROKER_TOKEN_FILE: &str = "broker-token.dpapi";
/// El servicio de Athena regenera su token en cada arranque, así que este
/// archivo se reescribe a menudo; se protege igual que el del Broker.
const ATHENA_TOKEN_FILE: &str = "athena-token.dpapi";
const API_TOKEN_PREFIX: &str = "api-";
const API_TOKEN_SUFFIX: &str = ".dpapi";
/// Longitud máxima admitida; evita guardar por error el contenido de un archivo.
const MAX_TOKEN_CHARS: usize = 512;

/// De dónde procede el token que está usando la aplicación.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum TokenSource {
    /// Guardado por la persona y cifrado con DPAPI.
    Protected,
    /// Heredado del entorno del proceso; vía de transición.
    Environment,
    /// No hay credencial disponible.
    Missing,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerCredentialStatus {
    pub source: TokenSource,
    pub protected: bool,
    pub environment_present: bool,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ApiCredentialStatus {
    pub name: String,
    pub protected: bool,
}

pub fn validate_api_credential_name(name: &str) -> Result<String, AppError> {
    let name = name.trim().to_ascii_lowercase();
    if name.len() < 3
        || name.len() > 40
        || name.starts_with('_')
        || !name.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte == b'_' || (index > 0 && byte.is_ascii_digit())
        })
    {
        return Err(AppError::Validation(
            "el alias de credencial debe usar entre 3 y 40 letras minúsculas, números o guiones bajos"
                .to_owned(),
        ));
    }
    Ok(name)
}

pub fn api_credential_path(data_dir: &Path, name: &str) -> Result<PathBuf, AppError> {
    let name = validate_api_credential_name(name)?;
    Ok(data_dir
        .join(CREDENTIALS_DIR)
        .join(format!("{API_TOKEN_PREFIX}{name}{API_TOKEN_SUFFIX}")))
}

pub fn list_api_credentials(data_dir: &Path) -> Result<Vec<ApiCredentialStatus>, AppError> {
    let directory = data_dir.join(CREDENTIALS_DIR);
    let entries = match std::fs::read_dir(directory) {
        Ok(entries) => entries,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(error) => {
            return Err(AppError::DataDirectory(format!(
                "credenciales no accesibles: {error}"
            )))
        }
    };
    let mut credentials = entries
        .filter_map(Result::ok)
        .filter_map(|entry| entry.file_name().into_string().ok())
        .filter_map(|file_name| {
            file_name
                .strip_prefix(API_TOKEN_PREFIX)?
                .strip_suffix(API_TOKEN_SUFFIX)
                .map(str::to_owned)
        })
        .filter(|name| validate_api_credential_name(name).is_ok())
        .map(|name| ApiCredentialStatus {
            name,
            protected: true,
        })
        .collect::<Vec<_>>();
    credentials.sort_by(|left, right| left.name.cmp(&right.name));
    Ok(credentials)
}

pub fn store_api_credential(data_dir: &Path, name: &str, secret: &str) -> Result<(), AppError> {
    let secret = validated_secret(secret)?;
    let destination = api_credential_path(data_dir, name)?;
    let parent = destination.parent().ok_or_else(|| {
        AppError::DataDirectory("el directorio de credenciales no es válido".to_owned())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| AppError::DataDirectory(format!("credenciales no accesibles: {error}")))?;
    protect_token(&destination, secret)
}

pub fn load_api_credential(data_dir: &Path, name: &str) -> Option<String> {
    let source = api_credential_path(data_dir, name).ok()?;
    unprotect_token(&source)
}

pub fn clear_api_credential(data_dir: &Path, name: &str) -> Result<(), AppError> {
    let destination = api_credential_path(data_dir, name)?;
    match std::fs::remove_file(destination) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::DataDirectory(format!(
            "no se pudo retirar la credencial: {error}"
        ))),
    }
}

pub fn broker_token_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CREDENTIALS_DIR).join(BROKER_TOKEN_FILE)
}

/// Resuelve el token vigente dando prioridad al almacén protegido.
///
/// La variable de entorno sigue admitiéndose para no romper el arranque con
/// Windows ni los flujos anteriores, pero deja de ser la vía principal.
pub fn resolve_broker_token(data_dir: &Path) -> Option<String> {
    if let Some(token) = load_broker_token(data_dir) {
        return Some(token);
    }
    environment_token()
}

pub fn environment_token() -> Option<String> {
    std::env::var("AI_BROKER_ADMIN_TOKEN")
        .ok()
        .filter(|value| !value.is_empty())
}

pub fn credential_status(data_dir: &Path) -> BrokerCredentialStatus {
    let protected = broker_token_path(data_dir).is_file();
    let environment_present = environment_token().is_some();
    let (source, message) = match (protected, environment_present) {
        (true, _) => (
            TokenSource::Protected,
            "La credencial está guardada y cifrada para tu cuenta de Windows.".to_owned(),
        ),
        (false, true) => (
            TokenSource::Environment,
            "Se está usando el token de la variable de entorno. Guárdalo aquí para \
             no depender de cómo se abra la aplicación."
                .to_owned(),
        ),
        (false, false) => (
            TokenSource::Missing,
            "No hay credencial guardada. Broker AI rechazará las peticiones que la exijan."
                .to_owned(),
        ),
    };
    BrokerCredentialStatus {
        source,
        protected,
        environment_present,
        message,
    }
}

/// Guarda el token cifrado, sustituyendo el anterior si lo hubiera.
pub fn store_broker_token(data_dir: &Path, token: &str) -> Result<(), AppError> {
    let token = validated_secret(token)?;
    let destination = broker_token_path(data_dir);
    let parent = destination.parent().ok_or_else(|| {
        AppError::DataDirectory("el directorio de credenciales no es válido".to_owned())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| AppError::DataDirectory(format!("credenciales no accesibles: {error}")))?;
    protect_token(&destination, token)
}

pub fn athena_token_path(data_dir: &Path) -> PathBuf {
    data_dir.join(CREDENTIALS_DIR).join(ATHENA_TOKEN_FILE)
}

/// Token vigente de Athena, solo desde el almacén protegido.
///
/// A diferencia del Broker no se admite variable de entorno: aquí no hay una
/// vía de transición que mantener, y una credencial menos en el entorno es una
/// credencial menos en la lista de procesos de cualquiera.
pub fn resolve_athena_token(data_dir: &Path) -> Option<String> {
    unprotect_token(&athena_token_path(data_dir))
}

pub fn athena_credential_status(data_dir: &Path) -> BrokerCredentialStatus {
    let protected = athena_token_path(data_dir).is_file();
    let (source, message) = if protected {
        (
            TokenSource::Protected,
            "La credencial de Athena está guardada y cifrada para tu cuenta de Windows.".to_owned(),
        )
    } else {
        (
            TokenSource::Missing,
            "No hay credencial de Athena guardada; el servicio la regenera en cada arranque."
                .to_owned(),
        )
    };
    BrokerCredentialStatus {
        source,
        protected,
        environment_present: false,
        message,
    }
}

pub fn store_athena_token(data_dir: &Path, token: &str) -> Result<(), AppError> {
    let token = validated_secret(token)?;
    let destination = athena_token_path(data_dir);
    let parent = destination.parent().ok_or_else(|| {
        AppError::DataDirectory("el directorio de credenciales no es válido".to_owned())
    })?;
    std::fs::create_dir_all(parent)
        .map_err(|error| AppError::DataDirectory(format!("credenciales no accesibles: {error}")))?;
    protect_token(&destination, token)
}

pub fn clear_athena_token(data_dir: &Path) -> Result<(), AppError> {
    let destination = athena_token_path(data_dir);
    if destination.is_file() {
        std::fs::remove_file(&destination).map_err(|error| {
            AppError::DataDirectory(format!("no se pudo retirar la credencial: {error}"))
        })?;
    }
    Ok(())
}

fn validated_secret(token: &str) -> Result<&str, AppError> {
    let token = token.trim();
    if token.is_empty() {
        return Err(AppError::Validation(
            "el token no puede estar vacío".to_owned(),
        ));
    }
    if token.chars().count() > MAX_TOKEN_CHARS {
        return Err(AppError::Validation(format!(
            "el token supera el límite de {MAX_TOKEN_CHARS} caracteres"
        )));
    }
    if token.chars().any(|character| character.is_control()) {
        return Err(AppError::Validation(
            "el token no puede contener saltos de línea ni caracteres de control".to_owned(),
        ));
    }
    Ok(token)
}

pub fn clear_broker_token(data_dir: &Path) -> Result<(), AppError> {
    let destination = broker_token_path(data_dir);
    match std::fs::remove_file(&destination) {
        Ok(()) => Ok(()),
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Ok(()),
        Err(error) => Err(AppError::DataDirectory(format!(
            "no se pudo retirar la credencial: {error}"
        ))),
    }
}

/// Cifra el token con DPAPI y comprueba que el resultado no lo contiene en claro.
///
/// La comprobación es deliberadamente paranoica: si Windows devolviera los bytes
/// sin cifrar, el archivo se borra en lugar de dejar un secreto legible en disco.
pub fn protect_token(destination: &Path, token: &str) -> Result<(), AppError> {
    #[cfg(target_os = "windows")]
    {
        // Windows PowerShell 5.1 no carga System.Security por defecto: sin este
        // Add-Type, `ProtectedData` no existe y el cifrado falla.
        let script = r#"
            $ErrorActionPreference = 'Stop'
            Add-Type -AssemblyName System.Security
            $plain = [Text.Encoding]::UTF8.GetBytes($env:CHATYGPT_SECRET_VALUE)
            $protected = [Security.Cryptography.ProtectedData]::Protect(
                $plain,
                $null,
                [Security.Cryptography.DataProtectionScope]::CurrentUser
            )
            [IO.File]::WriteAllBytes($env:CHATYGPT_SECRET_PATH, $protected)
            [Array]::Clear($plain, 0, $plain.Length)
        "#;
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("CHATYGPT_SECRET_VALUE", token)
            .env("CHATYGPT_SECRET_PATH", destination)
            .output()
            .map_err(|error| AppError::DataDirectory(format!("DPAPI no disponible: {error}")))?;
        if !output.status.success() {
            return Err(AppError::DataDirectory(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let protected = std::fs::read(destination).map_err(|error| {
            AppError::DataDirectory(format!("credencial no accesible: {error}"))
        })?;
        if !protects_secret(&protected, token) {
            let _ = std::fs::remove_file(destination);
            return Err(AppError::Conflict(
                "Windows no confirmó el cifrado seguro de la credencial".to_owned(),
            ));
        }
        Ok(())
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = (destination, token);
        Err(AppError::Validation(
            "la custodia de credenciales solo está disponible en Windows".to_owned(),
        ))
    }
}

/// Descifra el token guardado. Un archivo ilegible se trata como ausencia.
pub fn load_broker_token(data_dir: &Path) -> Option<String> {
    unprotect_token(&broker_token_path(data_dir))
}

fn unprotect_token(source: &Path) -> Option<String> {
    #[cfg(target_os = "windows")]
    {
        if !source.is_file() {
            return None;
        }
        let script = r#"
            $ErrorActionPreference = 'Stop'
            Add-Type -AssemblyName System.Security
            $protected = [IO.File]::ReadAllBytes($env:CHATYGPT_SECRET_PATH)
            $plain = [Security.Cryptography.ProtectedData]::Unprotect(
                $protected,
                $null,
                [Security.Cryptography.DataProtectionScope]::CurrentUser
            )
            [Console]::OutputEncoding = [Text.Encoding]::UTF8
            [Console]::Write([Text.Encoding]::UTF8.GetString($plain))
            [Array]::Clear($plain, 0, $plain.Length)
        "#;
        let output = Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-Command", script])
            .env("CHATYGPT_SECRET_PATH", source)
            .output()
            .ok()?;
        if !output.status.success() {
            return None;
        }
        let token = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        (!token.is_empty()).then_some(token)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = source;
        None
    }
}

/// Comprueba que los bytes cifrados no contienen el secreto en claro.
///
/// Un blob más corto que el propio token tampoco se acepta: DPAPI siempre añade
/// cabecera y relleno, así que ese tamaño delata que el cifrado no ocurrió.
fn protects_secret(protected: &[u8], token: &str) -> bool {
    let needle = token.as_bytes();
    if protected.is_empty() || needle.is_empty() || protected.len() < needle.len() {
        return false;
    }
    !protected.windows(needle.len()).any(|part| part == needle)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_unencrypted_blob_is_never_accepted_as_protected() {
        let token = "sk-broker-1234567890";
        // El propio token, o cualquier blob que lo contenga, se rechaza.
        assert!(!protects_secret(token.as_bytes(), token));
        assert!(!protects_secret(
            b"prefijo sk-broker-1234567890 sufijo",
            token
        ));
        assert!(!protects_secret(b"", token));
        // Un blob demasiado corto para ser una salida real de DPAPI también.
        assert!(!protects_secret(&[0x01, 0x00, 0xff, 0xd0], token));
        // Un blob con forma de DPAPI y sin rastro del token sí se acepta.
        let mut blob = vec![0x01, 0x00, 0x00, 0x00, 0xd0, 0x8c, 0x9d, 0xdf];
        blob.extend_from_slice(&[0x7a; 64]);
        assert!(protects_secret(&blob, token));
    }

    #[test]
    fn stored_tokens_reject_empty_control_and_oversized_values() {
        let directory =
            std::env::temp_dir().join(format!("chatygpt-secret-{}", uuid::Uuid::new_v4().simple()));
        assert!(matches!(
            store_broker_token(&directory, "   "),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            store_broker_token(&directory, "token\ncon salto"),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            store_broker_token(&directory, &"a".repeat(MAX_TOKEN_CHARS + 1)),
            Err(AppError::Validation(_))
        ));
        assert!(
            !directory.join(CREDENTIALS_DIR).exists(),
            "un token inválido no debe crear el almacén"
        );
    }

    #[test]
    fn api_credential_aliases_cannot_escape_the_protected_directory() {
        for invalid in [
            "../broker-token",
            "_hidden",
            "2starts_with_number",
            "two words",
            "ab",
            "name.with.dot",
        ] {
            assert!(
                validate_api_credential_name(invalid).is_err(),
                "debía rechazarse: {invalid}"
            );
        }
        assert_eq!(
            validate_api_credential_name("  Mi_Servicio_2  ").unwrap(),
            "mi_servicio_2"
        );
    }

    #[test]
    fn api_credential_listing_exposes_only_aliases() {
        let directory = std::env::temp_dir().join(format!(
            "chatygpt-api-secret-list-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let credentials = directory.join(CREDENTIALS_DIR);
        std::fs::create_dir_all(&credentials).expect("credentials directory should exist");
        std::fs::write(credentials.join("api-weather.dpapi"), b"protected-one")
            .expect("test credential should exist");
        std::fs::write(credentials.join("api-stock_prices.dpapi"), b"protected-two")
            .expect("test credential should exist");
        std::fs::write(credentials.join("api-../escape.dpapi"), b"ignored").ok();
        std::fs::write(credentials.join(BROKER_TOKEN_FILE), b"broker-secret")
            .expect("broker credential should exist");

        let listed = list_api_credentials(&directory).expect("aliases should list");
        assert_eq!(
            listed
                .iter()
                .map(|item| item.name.as_str())
                .collect::<Vec<_>>(),
            vec!["stock_prices", "weather"]
        );
        assert!(listed.iter().all(|item| item.protected));
        assert!(!serde_json::to_string(&listed)
            .unwrap()
            .contains("protected-one"));
        let _ = std::fs::remove_dir_all(directory);
    }

    #[test]
    fn credential_status_prefers_the_protected_store_over_the_environment() {
        let directory = std::env::temp_dir().join(format!(
            "chatygpt-secret-status-{}",
            uuid::Uuid::new_v4().simple()
        ));
        let status = credential_status(&directory);
        assert!(!status.protected);
        assert!(matches!(
            status.source,
            TokenSource::Missing | TokenSource::Environment
        ));
        assert!(!status.message.is_empty());

        // Con el archivo presente, la fuente pasa a ser el almacén protegido.
        let path = broker_token_path(&directory);
        std::fs::create_dir_all(path.parent().expect("parent should exist"))
            .expect("credentials directory should be created");
        std::fs::write(&path, [0x01, 0x00, 0xd0, 0x8c]).expect("protected blob should be written");
        let stored = credential_status(&directory);
        assert_eq!(stored.source, TokenSource::Protected);
        assert!(stored.protected);

        clear_broker_token(&directory).expect("clearing should work");
        assert!(!broker_token_path(&directory).exists());
        // Retirar dos veces no es un error: la ausencia ya es el estado deseado.
        clear_broker_token(&directory).expect("clearing twice should be harmless");
        let _ = std::fs::remove_dir_all(directory);
    }
}
