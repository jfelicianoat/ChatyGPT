//! Copia de seguridad y restauración de los datos locales (H16).
//!
//! SQLite es la fuente de verdad: exportar chats a Markdown no conserva GPTs,
//! versiones, permisos, programaciones ni relaciones. Una copia aquí es la base
//! entera —capturada con `VACUUM INTO`, coherente aunque haya WAL pendiente—
//! más el almacén de adjuntos y un manifiesto con la huella de cada fichero.
//!
//! Decisiones que conviene no perder:
//!
//! - **El manifiesto se escribe el último.** Una carpeta sin manifiesto es una
//!   copia a medias y nunca se ofrece para restaurar.
//! - **Verificar es restaurar en un directorio temporal.** Se abre la base
//!   copiada con las mismas migraciones que la aplicación y se pasa
//!   `integrity_check`: una copia que no abre no se da por buena.
//! - **Restaurar ocurre al arrancar.** Sustituir la base con la aplicación
//!   usándola dejaría conexiones abiertas sobre ficheros movidos. Se programa
//!   y se aplica antes de abrir la base; lo anterior se aparta, no se borra.
//! - **Los permisos no viajan.** Las credenciales están cifradas con DPAPI para
//!   la cuenta de Windows y no forman parte de la copia; las carpetas
//!   autorizadas se revocan al restaurar, porque en otro equipo o cuenta la
//!   misma ruta no tiene por qué ser la misma carpeta.

use std::fs;
use std::io::Read;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::db::{Database, SCHEMA_VERSION};
use crate::error::AppError;

const MANIFEST: &str = "manifiesto.json";
const DATABASE_FILE: &str = "chatygpt.db";
const ATTACHMENTS_DIR: &str = "attachments";
const PENDING_RESTORE: &str = "restauracion-pendiente.json";
const FORMAT: u32 = 1;

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase")]
pub struct BackupFile {
    /// Ruta relativa a la carpeta de la copia, con `/`.
    pub path: String,
    pub size_bytes: u64,
    pub sha256: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupManifest {
    pub format: u32,
    pub app_version: String,
    pub schema_version: i64,
    pub created_at: String,
    pub database: BackupFile,
    pub attachments: Vec<BackupFile>,
}

/// Lo que la interfaz enseña de una copia antes de decidir nada.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupReport {
    pub folder: String,
    pub created_at: String,
    pub app_version: String,
    pub schema_version: i64,
    pub conversation_count: i64,
    pub attachment_count: usize,
    pub total_bytes: u64,
    /// Lo que no viaja en la copia y habrá que volver a configurar.
    pub not_included: Vec<String>,
}

fn io_error(error: impl std::fmt::Display) -> AppError {
    AppError::DataDirectory(error.to_string())
}

fn file_digest(path: &Path) -> Result<(u64, String), AppError> {
    let mut file = fs::File::open(path).map_err(io_error)?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 64 * 1024];
    let mut size = 0_u64;
    loop {
        let read = file.read(&mut buffer).map_err(io_error)?;
        if read == 0 {
            break;
        }
        size += read as u64;
        hasher.update(&buffer[..read]);
    }
    Ok((size, format!("{:x}", hasher.finalize())))
}

fn relative(root: &Path, path: &Path) -> String {
    path.strip_prefix(root)
        .unwrap_or(path)
        .components()
        .map(|component| component.as_os_str().to_string_lossy().into_owned())
        .collect::<Vec<_>>()
        .join("/")
}

/// Copia un árbol de ficheros y devuelve la huella de cada uno.
fn copy_tree(source: &Path, destination: &Path, root: &Path) -> Result<Vec<BackupFile>, AppError> {
    let mut files = Vec::new();
    if !source.exists() {
        return Ok(files);
    }
    fs::create_dir_all(destination).map_err(io_error)?;
    let mut entries = fs::read_dir(source)
        .map_err(io_error)?
        .collect::<Result<Vec<_>, _>>()
        .map_err(io_error)?;
    entries.sort_by_key(|entry| entry.file_name());
    for entry in entries {
        let path = entry.path();
        let target = destination.join(entry.file_name());
        let kind = entry.file_type().map_err(io_error)?;
        if kind.is_dir() {
            files.extend(copy_tree(&path, &target, root)?);
        } else if kind.is_file() {
            // Los temporales de una escritura a medias no son adjuntos.
            if entry.file_name().to_string_lossy().ends_with(".tmp") {
                continue;
            }
            fs::copy(&path, &target).map_err(io_error)?;
            let (size_bytes, sha256) = file_digest(&target)?;
            files.push(BackupFile {
                path: relative(root, &target),
                size_bytes,
                sha256,
            });
        }
    }
    Ok(files)
}

/// Crea una copia completa dentro de `destination_parent` y la verifica.
pub fn create_backup(
    database: &Database,
    data_dir: &Path,
    destination_parent: &Path,
) -> Result<BackupReport, AppError> {
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let folder = destination_parent.join(format!("ChatyGPT-copia-{stamp}"));
    if folder.exists() {
        return Err(AppError::Conflict(
            "ya existe una copia con esa hora; espera un segundo y vuelve a intentarlo".to_owned(),
        ));
    }
    fs::create_dir_all(&folder).map_err(io_error)?;
    let database_copy = folder.join(DATABASE_FILE);
    database.vacuum_into(&database_copy)?;
    let (size_bytes, sha256) = file_digest(&database_copy)?;
    let attachments = copy_tree(
        &data_dir.join(ATTACHMENTS_DIR),
        &folder.join(ATTACHMENTS_DIR),
        &folder,
    )?;
    let manifest = BackupManifest {
        format: FORMAT,
        app_version: env!("CARGO_PKG_VERSION").to_owned(),
        schema_version: database.schema_version()?,
        created_at: chrono::Utc::now().to_rfc3339(),
        database: BackupFile {
            path: DATABASE_FILE.to_owned(),
            size_bytes,
            sha256,
        },
        attachments,
    };
    // El manifiesto es la marca de copia completa: se escribe el último.
    let serialized = serde_json::to_vec_pretty(&manifest).map_err(io_error)?;
    fs::write(folder.join(MANIFEST), serialized).map_err(io_error)?;
    verify_backup(&folder)
}

fn read_manifest(folder: &Path) -> Result<BackupManifest, AppError> {
    let raw = fs::read(folder.join(MANIFEST)).map_err(|_| {
        AppError::Validation(
            "la carpeta no contiene una copia completa de ChatyGPT (falta el manifiesto)"
                .to_owned(),
        )
    })?;
    let manifest: BackupManifest = serde_json::from_slice(&raw)
        .map_err(|error| AppError::Validation(format!("el manifiesto no es válido: {error}")))?;
    if manifest.format != FORMAT {
        return Err(AppError::Validation(format!(
            "formato de copia {} desconocido",
            manifest.format
        )));
    }
    if manifest.schema_version > SCHEMA_VERSION {
        return Err(AppError::Conflict(format!(
            "la copia usa el esquema {} y esta versión solo entiende hasta el {SCHEMA_VERSION}; actualiza ChatyGPT antes de restaurarla",
            manifest.schema_version
        )));
    }
    Ok(manifest)
}

fn check_file(folder: &Path, entry: &BackupFile) -> Result<(), AppError> {
    if entry.path.contains("..") {
        return Err(AppError::Validation(
            "el manifiesto contiene una ruta no válida".to_owned(),
        ));
    }
    let path = folder.join(&entry.path);
    let (size, sha256) = file_digest(&path).map_err(|_| {
        AppError::Validation(format!("falta el fichero {} de la copia", entry.path))
    })?;
    if size != entry.size_bytes || sha256 != entry.sha256 {
        return Err(AppError::Validation(format!(
            "el fichero {} no coincide con su huella: la copia está dañada",
            entry.path
        )));
    }
    Ok(())
}

/// Comprueba una copia sin tocar los datos en uso.
///
/// Cada fichero debe coincidir con su huella, y la base se abre —con las
/// migraciones de esta versión— sobre una copia en un directorio temporal.
pub fn verify_backup(folder: &Path) -> Result<BackupReport, AppError> {
    let manifest = read_manifest(folder)?;
    check_file(folder, &manifest.database)?;
    for attachment in &manifest.attachments {
        check_file(folder, attachment)?;
    }
    let scratch = std::env::temp_dir().join(format!(
        "chatygpt-verificacion-{}",
        uuid::Uuid::new_v4().simple()
    ));
    fs::create_dir_all(&scratch).map_err(io_error)?;
    let result = (|| {
        let trial = scratch.join(DATABASE_FILE);
        fs::copy(folder.join(DATABASE_FILE), &trial).map_err(io_error)?;
        let opened = Database::open(&trial)?;
        let integrity = opened.integrity_check()?;
        if integrity != "ok" {
            return Err(AppError::Validation(format!(
                "la base de la copia no supera la comprobación de integridad: {integrity}"
            )));
        }
        opened.conversation_count()
    })();
    let _ = fs::remove_dir_all(&scratch);
    let conversation_count = result?;
    Ok(BackupReport {
        folder: folder.to_string_lossy().into_owned(),
        created_at: manifest.created_at,
        app_version: manifest.app_version,
        schema_version: manifest.schema_version,
        conversation_count,
        attachment_count: manifest.attachments.len(),
        total_bytes: manifest.database.size_bytes
            + manifest
                .attachments
                .iter()
                .map(|item| item.size_bytes)
                .sum::<u64>(),
        not_included: vec![
            "Credenciales de Broker AI, Athena y APIs: están cifradas para tu cuenta de Windows y habrá que volver a introducirlas.".to_owned(),
            "Carpetas autorizadas: se revocan al restaurar y habrá que autorizarlas de nuevo.".to_owned(),
        ],
    })
}

#[derive(Debug, Serialize, Deserialize)]
struct PendingRestore {
    folder: String,
    requested_at: String,
}

/// Programa la restauración para el próximo arranque, tras verificar la copia.
pub fn schedule_restore(data_dir: &Path, folder: &Path) -> Result<BackupReport, AppError> {
    let report = verify_backup(folder)?;
    let pending = PendingRestore {
        folder: folder.to_string_lossy().into_owned(),
        requested_at: chrono::Utc::now().to_rfc3339(),
    };
    fs::write(
        data_dir.join(PENDING_RESTORE),
        serde_json::to_vec_pretty(&pending).map_err(io_error)?,
    )
    .map_err(io_error)?;
    Ok(report)
}

/// Resultado de aplicar una restauración pendiente al arrancar.
#[derive(Debug)]
pub enum RestoreOutcome {
    Nothing,
    Restored { previous_data: PathBuf },
    Rejected(String),
}

/// Aplica una restauración programada. Se llama antes de abrir la base.
///
/// Nada se borra: la base y los adjuntos actuales se apartan a una carpeta
/// `antes-de-restaurar-*` dentro del directorio de datos.
pub fn apply_pending_restore(data_dir: &Path) -> RestoreOutcome {
    let pending_path = data_dir.join(PENDING_RESTORE);
    let Ok(raw) = fs::read(&pending_path) else {
        return RestoreOutcome::Nothing;
    };
    let _ = fs::remove_file(&pending_path);
    let Ok(pending) = serde_json::from_slice::<PendingRestore>(&raw) else {
        return RestoreOutcome::Rejected("la petición de restauración no es legible".to_owned());
    };
    let folder = PathBuf::from(&pending.folder);
    if let Err(error) = verify_backup(&folder) {
        return RestoreOutcome::Rejected(error.to_string());
    }
    let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
    let previous = data_dir.join(format!("antes-de-restaurar-{stamp}"));
    let outcome = (|| -> Result<(), AppError> {
        fs::create_dir_all(&previous).map_err(io_error)?;
        for name in [
            DATABASE_FILE.to_owned(),
            format!("{DATABASE_FILE}-wal"),
            format!("{DATABASE_FILE}-shm"),
            ATTACHMENTS_DIR.to_owned(),
        ] {
            let current = data_dir.join(&name);
            if current.exists() {
                fs::rename(&current, previous.join(&name)).map_err(io_error)?;
            }
        }
        fs::copy(folder.join(DATABASE_FILE), data_dir.join(DATABASE_FILE)).map_err(io_error)?;
        copy_tree(
            &folder.join(ATTACHMENTS_DIR),
            &data_dir.join(ATTACHMENTS_DIR),
            data_dir,
        )?;
        Ok(())
    })();
    match outcome {
        Ok(()) => RestoreOutcome::Restored {
            previous_data: previous,
        },
        Err(error) => {
            // Si algo falló a mitad, se devuelve lo apartado a su sitio.
            let _ = fs::remove_file(data_dir.join(DATABASE_FILE));
            let _ = fs::remove_dir_all(data_dir.join(ATTACHMENTS_DIR));
            if let Ok(entries) = fs::read_dir(&previous) {
                for entry in entries.flatten() {
                    let _ = fs::rename(entry.path(), data_dir.join(entry.file_name()));
                }
            }
            RestoreOutcome::Rejected(error.to_string())
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn scratch(name: &str) -> PathBuf {
        let path = std::env::temp_dir().join(format!(
            "chatygpt-respaldo-{name}-{}",
            uuid::Uuid::new_v4().simple()
        ));
        fs::create_dir_all(&path).expect("carpeta de prueba");
        path
    }

    #[test]
    fn a_backup_restores_into_a_clean_profile_without_reusing_permissions() {
        let data_dir = scratch("origen");
        let database = Database::open(data_dir.join(DATABASE_FILE)).expect("base");
        let conversation = database
            .create_conversation("Conversación a conservar", None)
            .expect("chat");
        database
            .authorize_folder(&data_dir, "Carpeta", "export")
            .expect("permiso");
        fs::create_dir_all(data_dir.join(ATTACHMENTS_DIR).join("abc")).expect("adjuntos");
        fs::write(
            data_dir.join(ATTACHMENTS_DIR).join("abc").join("nota.txt"),
            "contenido del adjunto",
        )
        .expect("adjunto");

        let backups = scratch("copias");
        let report = create_backup(&database, &data_dir, &backups).expect("copia");
        assert_eq!(report.conversation_count, 1);
        assert_eq!(report.attachment_count, 1);

        // Perfil limpio: otro directorio de datos, con otra base dentro.
        let profile = scratch("destino");
        let other = Database::open(profile.join(DATABASE_FILE)).expect("base destino");
        other.create_conversation("Otra", None).expect("chat");
        drop(other);
        schedule_restore(&profile, Path::new(&report.folder)).expect("programar");
        let outcome = apply_pending_restore(&profile);
        let RestoreOutcome::Restored { previous_data } = outcome else {
            panic!("la restauración debía aplicarse: {outcome:?}");
        };
        assert!(
            previous_data.join(DATABASE_FILE).exists(),
            "lo anterior se aparta"
        );

        let restored = Database::open(profile.join(DATABASE_FILE)).expect("base restaurada");
        restored.revoke_all_authorized_folders().expect("revocar");
        let titles = restored
            .list_conversations()
            .expect("lista")
            .into_iter()
            .map(|item| item.id)
            .collect::<Vec<_>>();
        assert_eq!(titles, vec![conversation.id]);
        let folders = restored.list_authorized_folders().expect("carpetas");
        assert!(!folders.is_empty());
        assert!(
            folders.iter().all(|folder| folder.revoked_at.is_some()),
            "ningún permiso sobrevive a la restauración"
        );
        assert_eq!(
            fs::read_to_string(profile.join(ATTACHMENTS_DIR).join("abc").join("nota.txt"))
                .expect("adjunto restaurado"),
            "contenido del adjunto"
        );
        drop(database);
        drop(restored);
        for path in [data_dir, backups, profile] {
            let _ = fs::remove_dir_all(path);
        }
    }

    #[test]
    fn a_damaged_or_incomplete_backup_is_never_restored() {
        let data_dir = scratch("dañada");
        let database = Database::open(data_dir.join(DATABASE_FILE)).expect("base");
        let backups = scratch("copias-dañadas");
        let report = create_backup(&database, &data_dir, &backups).expect("copia");
        let folder = PathBuf::from(&report.folder);

        // Un byte cambiado en la base invalida la copia.
        let mut bytes = fs::read(folder.join(DATABASE_FILE)).expect("lectura");
        let last = bytes.len() - 1;
        bytes[last] ^= 0xff;
        fs::write(folder.join(DATABASE_FILE), bytes).expect("escritura");
        assert!(verify_backup(&folder).is_err());

        // Sin manifiesto, la copia se considera incompleta.
        fs::remove_file(folder.join(MANIFEST)).expect("manifiesto");
        assert!(verify_backup(&folder)
            .expect_err("sin manifiesto")
            .to_string()
            .contains("manifiesto"));

        let profile = scratch("perfil");
        fs::write(
            profile.join(PENDING_RESTORE),
            serde_json::to_vec(&PendingRestore {
                folder: report.folder.clone(),
                requested_at: String::new(),
            })
            .expect("json"),
        )
        .expect("pendiente");
        assert!(matches!(
            apply_pending_restore(&profile),
            RestoreOutcome::Rejected(_)
        ));
        assert!(!profile.join(DATABASE_FILE).exists());
        drop(database);
        for path in [data_dir, backups, profile] {
            let _ = fs::remove_dir_all(path);
        }
    }
}
