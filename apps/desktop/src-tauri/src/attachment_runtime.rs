use std::fs::{self, File};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};
use std::time::Duration;

use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::broker::BrokerClient;
use crate::db::{AttachmentRecord, AttachmentView, Database};
use crate::error::AppError;

const MAX_LOCAL_FILE_BYTES: u64 = 512 * 1024 * 1024;

pub async fn import_attachment(
    database: Database,
    broker: BrokerClient,
    attachments_dir: PathBuf,
    conversation_id: String,
    source_path: String,
) -> Result<AttachmentView, AppError> {
    let imported = tauri::async_runtime::spawn_blocking(move || {
        copy_into_managed_storage(&attachments_dir, Path::new(&source_path))
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))??;
    let view = database.register_attachment(
        &conversation_id,
        &imported.path.to_string_lossy(),
        &imported.display_name,
        imported.media_type.as_deref(),
        imported.size_bytes as i64,
        &imported.sha256,
    )?;
    if matches!(view.ingestion_status.as_str(), "local" | "failed") {
        spawn_ingestion(database, broker, view.id.clone());
    }
    Ok(view)
}

pub fn retry_attachment(
    database: Database,
    broker: BrokerClient,
    attachment_id: &str,
) -> Result<AttachmentView, AppError> {
    database.reset_failed_attachment_for_retry(attachment_id)?;
    spawn_ingestion(database.clone(), broker, attachment_id.to_owned());
    database.attachment_view(attachment_id)
}

pub fn recover_at_start(database: Database, broker: BrokerClient) -> Result<usize, AppError> {
    let records = database.recoverable_attachments()?;
    let count = records.len();
    for record in records {
        spawn_ingestion(database.clone(), broker.clone(), record.id);
    }
    Ok(count)
}

fn spawn_ingestion(database: Database, broker: BrokerClient, attachment_id: String) {
    tauri::async_runtime::spawn(async move {
        let mut transport_errors = 0_u32;
        loop {
            let record = match database.attachment_record(&attachment_id) {
                Ok(record) => record,
                Err(_) => return,
            };
            let outcome = if let Some(file_id) = record.broker_file_id.as_deref() {
                poll_remote_file(&database, &broker, &record, file_id).await
            } else {
                upload_local_file(&database, &broker, &record).await
            };
            match outcome {
                Ok(true) => return,
                Ok(false) => {
                    transport_errors = 0;
                    tokio::time::sleep(Duration::from_millis(900)).await;
                }
                Err(error) if is_permanent(&error) => {
                    let value = json!({"message": error.to_string()});
                    let _ = database.update_attachment_ingestion(
                        &attachment_id,
                        "failed",
                        None,
                        None,
                        None,
                        None,
                        Some(&value),
                    );
                    return;
                }
                Err(error) => {
                    transport_errors = transport_errors.saturating_add(1);
                    let value = json!({
                        "message": error.to_string(),
                        "retrying": true,
                        "attempt": transport_errors
                    });
                    let _ = database.update_attachment_ingestion(
                        &attachment_id,
                        "uploading",
                        None,
                        None,
                        None,
                        None,
                        Some(&value),
                    );
                    let seconds = 2_u64.saturating_pow(transport_errors.min(5)).min(30);
                    tokio::time::sleep(Duration::from_secs(seconds)).await;
                }
            }
        }
    });
}

async fn upload_local_file(
    database: &Database,
    broker: &BrokerClient,
    record: &AttachmentRecord,
) -> Result<bool, AppError> {
    database.mark_attachment_uploading(&record.id)?;
    let accepted = broker
        .upload_file(
            Path::new(&record.local_path),
            &record.display_name,
            record.media_type.as_deref(),
            record.size_bytes as u64,
        )
        .await?;
    if !accepted.sha256.eq_ignore_ascii_case(&record.sha256) {
        return Err(AppError::BrokerContract(
            "la huella devuelta por el Broker no coincide con el archivo local".to_owned(),
        ));
    }
    database.update_attachment_ingestion(
        &record.id,
        &accepted.status,
        Some(&accepted.file_id),
        None,
        None,
        None,
        None,
    )?;
    Ok(accepted.status == "ready")
}

async fn poll_remote_file(
    database: &Database,
    broker: &BrokerClient,
    record: &AttachmentRecord,
    file_id: &str,
) -> Result<bool, AppError> {
    let state = broker.get_file(file_id).await?;
    if !state.sha256.eq_ignore_ascii_case(&record.sha256) {
        return Err(AppError::BrokerContract(
            "el Broker asociÃ³ un contenido distinto al adjunto local".to_owned(),
        ));
    }
    let terminal = state.status == "ready" || state.status == "failed";
    database.update_attachment_ingestion(
        &record.id,
        &state.status,
        Some(&state.file_id),
        state.kind.as_deref(),
        state.engine.as_deref(),
        Some(&state.meta),
        state.error.as_ref(),
    )?;
    Ok(terminal)
}

fn is_permanent(error: &AppError) -> bool {
    matches!(
        error,
        AppError::BrokerResponse { status, .. }
            if (400..500).contains(status) && !matches!(*status, 408 | 429)
    ) || matches!(error, AppError::BrokerContract(_) | AppError::Validation(_))
}

struct ImportedFile {
    path: PathBuf,
    display_name: String,
    media_type: Option<String>,
    size_bytes: u64,
    sha256: String,
}

fn copy_into_managed_storage(root: &Path, source: &Path) -> Result<ImportedFile, AppError> {
    let canonical = source
        .canonicalize()
        .map_err(|error| AppError::Validation(format!("no se puede abrir el archivo: {error}")))?;
    let metadata = canonical
        .metadata()
        .map_err(|error| AppError::Validation(format!("no se puede leer el archivo: {error}")))?;
    if !metadata.is_file() {
        return Err(AppError::Validation(
            "la ruta seleccionada no es un archivo".to_owned(),
        ));
    }
    if metadata.len() == 0 {
        return Err(AppError::Validation("el archivo esta vacio".to_owned()));
    }
    if metadata.len() > MAX_LOCAL_FILE_BYTES {
        return Err(AppError::Validation(
            "el archivo supera el limite local de 512 MB".to_owned(),
        ));
    }
    let display_name = canonical
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| AppError::Validation("el nombre del archivo no es vÃ¡lido".to_owned()))?
        .to_owned();
    fs::create_dir_all(root).map_err(|error| AppError::DataDirectory(error.to_string()))?;
    let temporary = root.join(format!(".import-{}.tmp", Uuid::new_v4().simple()));
    let mut input = File::open(&canonical)
        .map_err(|error| AppError::Validation(format!("no se puede abrir el archivo: {error}")))?;
    let mut output =
        File::create(&temporary).map_err(|error| AppError::DataDirectory(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = input
            .read(&mut buffer)
            .map_err(|error| AppError::Validation(format!("fallÃ³ la lectura: {error}")))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        output
            .write_all(&buffer[..read])
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
    }
    output
        .sync_all()
        .map_err(|error| AppError::DataDirectory(error.to_string()))?;
    let sha256 = format!("{:x}", hasher.finalize());
    let target_dir = root.join(&sha256);
    fs::create_dir_all(&target_dir).map_err(|error| AppError::DataDirectory(error.to_string()))?;
    let safe_name: String = display_name
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            other => other,
        })
        .collect();
    let target = target_dir.join(safe_name);
    if target.exists() {
        let _ = fs::remove_file(&temporary);
    } else {
        fs::rename(&temporary, &target)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
    }
    Ok(ImportedFile {
        path: target,
        display_name,
        media_type: mime_guess::from_path(&canonical)
            .first_raw()
            .map(str::to_owned),
        size_bytes: metadata.len(),
        sha256,
    })
}

#[cfg(test)]
mod tests {
    use super::copy_into_managed_storage;
    use std::io::Write;
    use uuid::Uuid;

    #[test]
    fn import_hashes_and_deduplicates_managed_copy() {
        let root = std::env::temp_dir().join(format!("chatygpt-attachment-{}", Uuid::new_v4()));
        let source = root.with_extension("txt");
        let mut file = std::fs::File::create(&source).expect("source should be created");
        file.write_all(b"same content")
            .expect("source should be written");
        let first = copy_into_managed_storage(&root, &source).expect("first import should work");
        let second = copy_into_managed_storage(&root, &source).expect("second import should work");
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.path, second.path);
        assert_eq!(first.size_bytes, 12);
        let _ = std::fs::remove_file(source);
        let _ = std::fs::remove_dir_all(root);
    }
}
