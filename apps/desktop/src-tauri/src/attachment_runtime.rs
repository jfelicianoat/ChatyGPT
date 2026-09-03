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
const MAX_CAPTURE_BYTES: usize = 20 * 1024 * 1024;
const ATTACHMENT_CHUNK_CHARACTERS: usize = 4_000;

pub async fn import_attachment(
    database: Database,
    broker: BrokerClient,
    attachments_dir: PathBuf,
    conversation_id: String,
    source_path: String,
    describe_images: bool,
) -> Result<AttachmentView, AppError> {
    let imported = tauri::async_runtime::spawn_blocking(move || {
        copy_into_managed_storage(&attachments_dir, Path::new(&source_path))
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))??;
    let describe_images = direct_image_policy(imported.media_type.as_deref(), describe_images);
    let view = database.register_attachment_with_image_policy(
        &conversation_id,
        &imported.path.to_string_lossy(),
        &imported.display_name,
        imported.media_type.as_deref(),
        imported.size_bytes as i64,
        &imported.sha256,
        Some(describe_images),
    )?;
    if matches!(view.ingestion_status.as_str(), "local" | "failed") {
        spawn_ingestion(database, broker, view.id.clone());
    }
    Ok(view)
}

pub async fn import_captured_image(
    database: Database,
    broker: BrokerClient,
    attachments_dir: PathBuf,
    conversation_id: String,
    display_name: String,
    bytes: Vec<u8>,
) -> Result<AttachmentView, AppError> {
    let imported = tauri::async_runtime::spawn_blocking(move || {
        store_captured_image(&attachments_dir, &display_name, &bytes)
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))??;
    let view = database.register_attachment_with_image_policy(
        &conversation_id,
        &imported.path.to_string_lossy(),
        &imported.display_name,
        imported.media_type.as_deref(),
        imported.size_bytes as i64,
        &imported.sha256,
        Some(true),
    )?;
    if matches!(view.ingestion_status.as_str(), "local" | "failed") {
        spawn_ingestion(database, broker, view.id.clone());
    }
    Ok(view)
}

pub async fn import_custom_gpt_attachment(
    database: Database,
    broker: BrokerClient,
    attachments_dir: PathBuf,
    custom_gpt_id: String,
    source_path: String,
    describe_images: bool,
) -> Result<AttachmentView, AppError> {
    let imported = tauri::async_runtime::spawn_blocking(move || {
        copy_into_managed_storage(&attachments_dir, Path::new(&source_path))
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))??;
    let describe_images = direct_image_policy(imported.media_type.as_deref(), describe_images);
    let view = database.register_custom_gpt_attachment_with_image_policy(
        &custom_gpt_id,
        &imported.path.to_string_lossy(),
        &imported.display_name,
        imported.media_type.as_deref(),
        imported.size_bytes as i64,
        &imported.sha256,
        Some(describe_images),
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

pub fn retry_attachment_context(
    database: Database,
    broker: BrokerClient,
    attachment_id: &str,
) -> Result<AttachmentView, AppError> {
    database.reset_attachment_context_for_retry(attachment_id)?;
    spawn_ingestion(database.clone(), broker, attachment_id.to_owned());
    database.attachment_view(attachment_id)
}

pub fn recover_at_start(database: Database, broker: BrokerClient) -> Result<usize, AppError> {
    let mut records = database.recoverable_attachments()?;
    records.extend(database.ready_attachments_without_chunks()?);
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
            record.describe_images,
        )
        .await?;
    if !accepted.sha256.eq_ignore_ascii_case(&record.sha256) {
        return Err(AppError::BrokerContract(
            "la huella devuelta por el Broker no coincide con el archivo local".to_owned(),
        ));
    }
    if let Some(describe_images) = accepted.describe_images {
        database.set_attachment_describe_images(&record.id, describe_images)?;
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
    // Even when upload answers `ready`, query the final file state once: that
    // contract is where Broker exposes the converted Markdown URL.
    Ok(accepted.status == "failed")
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
            "el Broker asoció un contenido distinto al adjunto local".to_owned(),
        ));
    }
    if let Some(describe_images) = state.describe_images {
        database.set_attachment_describe_images(&record.id, describe_images)?;
    }
    let terminal = state.status == "ready" || state.status == "failed";
    if state.status == "ready" {
        if let Some(markdown_url) = state.markdown_url.as_deref() {
            database.mark_attachment_context_preparing(&record.id)?;
            match broker.download_text(markdown_url).await {
                Ok(markdown) => {
                    database.replace_attachment_chunks(&record.id, &chunk_markdown(&markdown))?;
                    let dependencies_enabled = broker
                        .capabilities()
                        .await
                        .is_ok_and(|capabilities| capabilities.task_dependencies);
                    let _ = crate::task_runtime::start_attachment_semantic_index(
                        database.clone(),
                        broker.clone(),
                        &record.id,
                        false,
                        dependencies_enabled,
                    );
                }
                Err(error) => {
                    let context_error = json!({"message": error.to_string()});
                    database.record_attachment_context_failure(&record.id, &context_error)?;
                    eprintln!(
                        "No se pudo preparar el contexto local del adjunto {}: {}",
                        record.id, error
                    );
                }
            }
        } else {
            database.mark_attachment_context_unavailable(&record.id)?;
        }
    }
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

pub(crate) fn chunk_markdown(markdown: &str) -> Vec<String> {
    let characters = markdown.chars().collect::<Vec<_>>();
    let mut chunks = Vec::new();
    let mut start = 0;
    while start < characters.len() {
        let hard_end = start
            .saturating_add(ATTACHMENT_CHUNK_CHARACTERS)
            .min(characters.len());
        let end = preferred_chunk_end(&characters, start, hard_end);
        let chunk = characters[start..end]
            .iter()
            .collect::<String>()
            .trim()
            .to_owned();
        if !chunk.is_empty() {
            chunks.push(chunk);
        }
        start = end;
    }
    chunks
}

fn preferred_chunk_end(characters: &[char], start: usize, hard_end: usize) -> usize {
    if hard_end == characters.len() {
        return hard_end;
    }
    let preferred_start = start + ATTACHMENT_CHUNK_CHARACTERS * 7 / 10;
    let search_start = preferred_start.min(hard_end);
    for delimiter in ['\n', '.', '!', '?', ';', ',', ' '] {
        if let Some(offset) = characters[search_start..hard_end]
            .iter()
            .rposition(|character| *character == delimiter)
        {
            return search_start + offset + 1;
        }
    }
    hard_end
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

fn direct_image_policy(media_type: Option<&str>, requested: bool) -> bool {
    media_type.is_some_and(|value| value.starts_with("image/")) || requested
}

fn store_captured_image(
    root: &Path,
    display_name: &str,
    bytes: &[u8],
) -> Result<ImportedFile, AppError> {
    if bytes.is_empty() {
        return Err(AppError::Validation("la captura está vacía".to_owned()));
    }
    if bytes.len() > MAX_CAPTURE_BYTES {
        return Err(AppError::Validation(
            "la captura supera el límite local de 20 MB".to_owned(),
        ));
    }

    let (extension, media_type) = if bytes.starts_with(&[0xFF, 0xD8, 0xFF]) {
        ("jpg", "image/jpeg")
    } else if bytes.starts_with(&[0x89, b'P', b'N', b'G', 0x0D, 0x0A, 0x1A, 0x0A]) {
        ("png", "image/png")
    } else {
        return Err(AppError::Validation(
            "la captura no contiene una imagen JPEG o PNG válida".to_owned(),
        ));
    };

    let requested_name = Path::new(display_name)
        .file_name()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("captura");
    let stem = Path::new(requested_name)
        .file_stem()
        .and_then(|name| name.to_str())
        .filter(|name| !name.trim().is_empty())
        .unwrap_or("captura");
    let safe_stem: String = stem
        .chars()
        .take(80)
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            other => other,
        })
        .collect();
    let display_name = format!("{safe_stem}.{extension}");

    let mut hasher = Sha256::new();
    hasher.update(bytes);
    let sha256 = format!("{:x}", hasher.finalize());
    let target_dir = root.join(&sha256);
    let target = target_dir.join(&display_name);
    fs::create_dir_all(&target_dir).map_err(|error| AppError::DataDirectory(error.to_string()))?;
    if !target.exists() {
        let temporary = root.join(format!(".capture-{}.tmp", Uuid::new_v4().simple()));
        let mut output =
            File::create(&temporary).map_err(|error| AppError::DataDirectory(error.to_string()))?;
        output
            .write_all(bytes)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        output
            .sync_all()
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        fs::rename(&temporary, &target)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
    }

    Ok(ImportedFile {
        path: target,
        display_name,
        media_type: Some(media_type.to_owned()),
        size_bytes: bytes.len() as u64,
        sha256,
    })
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
        .ok_or_else(|| AppError::Validation("el nombre del archivo no es válido".to_owned()))?
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
            .map_err(|error| AppError::Validation(format!("falló la lectura: {error}")))?;
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
mod tests;
