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

pub async fn import_custom_gpt_attachment(
    database: Database,
    broker: BrokerClient,
    attachments_dir: PathBuf,
    custom_gpt_id: String,
    source_path: String,
) -> Result<AttachmentView, AppError> {
    let imported = tauri::async_runtime::spawn_blocking(move || {
        copy_into_managed_storage(&attachments_dir, Path::new(&source_path))
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))??;
    let view = database.register_custom_gpt_attachment(
        &custom_gpt_id,
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
    let terminal = state.status == "ready" || state.status == "failed";
    if state.status == "ready" {
        if let Some(markdown_url) = state.markdown_url.as_deref() {
            database.mark_attachment_context_preparing(&record.id)?;
            match broker.download_text(markdown_url).await {
                Ok(markdown) => {
                    database.replace_attachment_chunks(&record.id, &chunk_markdown(&markdown))?;
                    let _ = crate::task_runtime::start_attachment_semantic_index(
                        database.clone(),
                        broker.clone(),
                        &record.id,
                        false,
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
mod tests {
    use super::{
        chunk_markdown, copy_into_managed_storage, import_attachment, recover_at_start,
        retry_attachment, store_captured_image, ATTACHMENT_CHUNK_CHARACTERS,
    };
    use crate::broker::simulated::{
        accepted_file, accepted_task, file_state, task_state, ScriptedResponse, SimulatedBroker,
    };
    use crate::broker::BrokerClient;
    use crate::db::{AttachmentView, Database};
    use serde_json::json;
    use sha2::{Digest, Sha256};
    use std::io::Write;
    use std::path::PathBuf;
    use std::time::Duration;
    use uuid::Uuid;

    /// Margen para que la ingesta asíncrona se asiente sin colgar la suite.
    const SETTLE_TIMEOUT: Duration = Duration::from_secs(20);

    struct IngestionFixture {
        database: Database,
        attachments_dir: PathBuf,
        source: PathBuf,
        sha256: String,
        size_bytes: u64,
    }

    impl IngestionFixture {
        /// Prepara base, almacenamiento gestionado y un archivo de origen real.
        fn new(contents: &[u8]) -> Self {
            let unique = Uuid::new_v4().simple().to_string();
            let database = Database::open(
                std::env::temp_dir().join(format!("chatygpt-ingestion-{unique}.sqlite")),
            )
            .expect("la base de pruebas debe abrirse");
            let attachments_dir = std::env::temp_dir().join(format!("chatygpt-managed-{unique}"));
            let source = std::env::temp_dir().join(format!("chatygpt-origen-{unique}.pdf"));
            std::fs::write(&source, contents).expect("el origen debe escribirse");
            Self {
                database,
                attachments_dir,
                sha256: format!("{:x}", Sha256::digest(contents)),
                size_bytes: contents.len() as u64,
                source,
            }
        }

        fn import(&self, broker: &BrokerClient, conversation_id: &str) -> AttachmentView {
            tauri::async_runtime::block_on(import_attachment(
                self.database.clone(),
                broker.clone(),
                self.attachments_dir.clone(),
                conversation_id.to_owned(),
                self.source.to_string_lossy().into_owned(),
            ))
            .expect("el adjunto debe registrarse localmente")
        }

        fn view(&self, attachment_id: &str) -> AttachmentView {
            self.database
                .attachment_view(attachment_id)
                .expect("el adjunto debe poder consultarse")
        }
    }

    impl Drop for IngestionFixture {
        fn drop(&mut self) {
            let path = self.database.path().to_path_buf();
            for candidate in [
                path.clone(),
                path.with_extension("sqlite-wal"),
                path.with_extension("sqlite-shm"),
            ] {
                let _ = std::fs::remove_file(candidate);
            }
            let _ = std::fs::remove_file(&self.source);
            let _ = std::fs::remove_dir_all(&self.attachments_dir);
        }
    }

    /// Simulador con la ingesta completa programada, hasta el Markdown.
    fn simulated_ingestion(sha256: &str, size_bytes: u64, markdown: &str) -> SimulatedBroker {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "POST /api/v1/files",
            ScriptedResponse::ok(accepted_file(
                "file-ok",
                "informe.pdf",
                size_bytes as i64,
                sha256,
            )),
        );
        let mut ready = file_state("file-ok", "ready", Some("/api/v1/files/file-ok/markdown"));
        ready["sha256"] = json!(sha256);
        simulated.always("GET /api/v1/files/{id}", ScriptedResponse::ok(ready));
        simulated.always(
            "GET /api/v1/files/file-ok/markdown",
            ScriptedResponse::text(markdown),
        );
        // La indexación semántica que dispara la conversión también necesita
        // respuesta, o dejaría ruido de tareas huérfanas en la prueba.
        simulated.always(
            "POST /api/v1/tasks",
            ScriptedResponse::accepted(accepted_task("remote-embedding")),
        );
        simulated.always(
            "GET /api/v1/tasks/{id}",
            ScriptedResponse::ok(task_state("remote-embedding", "failed", None)),
        );
        simulated
    }

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

    #[test]
    fn screen_capture_is_validated_named_and_deduplicated_in_managed_storage() {
        let root = std::env::temp_dir().join(format!("chatygpt-capture-{}", Uuid::new_v4()));
        let jpeg = [0xFF, 0xD8, 0xFF, 0xE0, 1, 2, 3, 4];
        let first =
            store_captured_image(&root, r#"..\captura: escritorio.png"#, &jpeg).expect("valid");
        let second = store_captured_image(&root, "otro-nombre.jpeg", &jpeg).expect("deduplicated");

        assert_eq!(first.display_name, "captura_ escritorio.jpg");
        assert_eq!(first.sha256, second.sha256);
        assert_eq!(first.size_bytes, jpeg.len() as u64);
        assert_eq!(first.media_type.as_deref(), Some("image/jpeg"));
        assert!(first.path.exists());
        assert!(store_captured_image(&root, "captura.jpg", b"not an image").is_err());

        let _ = std::fs::remove_dir_all(root);
    }

    #[test]
    fn converted_markdown_is_split_into_bounded_chunks_without_losing_content() {
        let markdown = "precio apertura cierre volumen ".repeat(700);
        let chunks = chunk_markdown(&markdown);
        assert!(chunks.len() > 1);
        assert!(chunks
            .iter()
            .all(|chunk| chunk.chars().count() <= ATTACHMENT_CHUNK_CHARACTERS));
        assert_eq!(
            chunks.join("").replace(' ', ""),
            markdown.trim().replace(' ', "")
        );
    }

    #[test]
    fn converted_markdown_prefers_document_boundaries() {
        let first_section = "A".repeat(3_000);
        let second_section = "B".repeat(3_000);
        let markdown = format!("{first_section}\n\n{second_section}");
        let chunks = chunk_markdown(&markdown);

        assert_eq!(chunks, vec![first_section, second_section]);
    }

    /// La ingesta completa: subir, sondear, convertir y fragmentar.
    #[test]
    fn ingestion_uploads_polls_and_stores_the_converted_context() {
        let fixture = IngestionFixture::new(b"contenido real del informe");
        let markdown = format!("# Informe\n\n{}", "Contenido convertido. ".repeat(300));
        let simulated = simulated_ingestion(&fixture.sha256, fixture.size_bytes, &markdown);
        let broker =
            BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
        let conversation = fixture
            .database
            .create_conversation("Adjuntos", None)
            .expect("la conversación debe crearse");

        let view = fixture.import(&broker, &conversation.id);
        // Antes de hablar con el Broker, el adjunto ya existe localmente.
        assert_eq!(view.sha256, fixture.sha256);
        assert_eq!(view.ingestion_status, "local");
        assert!(view.broker_file_id.is_none());

        assert!(
            SimulatedBroker::wait_until(SETTLE_TIMEOUT, || fixture.view(&view.id).ingestion_status
                == "ready"),
            "la ingesta debía terminar en ready"
        );

        let ready = fixture.view(&view.id);
        assert_eq!(ready.broker_file_id.as_deref(), Some("file-ok"));
        assert!(ready.ingestion_error.is_none());
        // El Markdown convertido queda fragmentado y disponible como contexto.
        assert_eq!(ready.context_status, "ready");
        assert!(ready.chunk_count > 1);
        assert!(ready.indexed_characters > 0);

        // El archivo se subió una sola vez, con su contenido real.
        let uploads = simulated.requests_to("POST", "/api/v1/files");
        assert_eq!(uploads.len(), 1);
        assert!(uploads[0].raw_body.contains("contenido real del informe"));
    }

    /// Una huella distinta a la local no se acepta: sería otro archivo.
    ///
    /// Es la garantía de que el contexto que se envía al modelo procede del
    /// documento que la persona adjuntó, no de otro que el Broker asoció.
    #[test]
    fn a_fingerprint_mismatch_fails_the_attachment_instead_of_trusting_the_broker() {
        let fixture = IngestionFixture::new(b"documento de la persona");
        let simulated = SimulatedBroker::start();
        simulated.always(
            "POST /api/v1/files",
            ScriptedResponse::ok(accepted_file(
                "file-otro",
                "informe.pdf",
                fixture.size_bytes as i64,
                &"b".repeat(64),
            )),
        );
        let broker =
            BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
        let conversation = fixture
            .database
            .create_conversation("Huella distinta", None)
            .expect("la conversación debe crearse");

        let view = fixture.import(&broker, &conversation.id);
        assert!(
            SimulatedBroker::wait_until(SETTLE_TIMEOUT, || fixture.view(&view.id).ingestion_status
                == "failed"),
            "una huella que no coincide debía marcar el adjunto como fallido"
        );

        let failed = fixture.view(&view.id);
        assert!(
            failed.broker_file_id.is_none(),
            "no se adopta el archivo ajeno"
        );
        assert!(failed
            .ingestion_error
            .as_ref()
            .and_then(|error| error["message"].as_str())
            .is_some_and(|message| message.contains("huella")));
        // No se reintenta: un contenido distinto no mejora repitiendo.
        std::thread::sleep(Duration::from_millis(1_200));
        assert_eq!(simulated.requests_to("POST", "/api/v1/files").len(), 1);
    }

    /// Si la conversión no se puede descargar, el adjunto sigue siendo usable.
    ///
    /// Perder el contexto local degrada la experiencia; perder el adjunto
    /// entero sería una regresión. Los dos estados se registran por separado.
    #[test]
    fn a_failed_conversion_download_degrades_context_without_losing_the_attachment() {
        let fixture = IngestionFixture::new(b"informe con conversion rota");
        let simulated = SimulatedBroker::start();
        simulated.always(
            "POST /api/v1/files",
            ScriptedResponse::ok(accepted_file(
                "file-sin-md",
                "informe.pdf",
                fixture.size_bytes as i64,
                &fixture.sha256,
            )),
        );
        let mut ready = file_state(
            "file-sin-md",
            "ready",
            Some("/api/v1/files/file-sin-md/markdown"),
        );
        ready["sha256"] = json!(fixture.sha256);
        simulated.always("GET /api/v1/files/{id}", ScriptedResponse::ok(ready));
        simulated.always(
            "GET /api/v1/files/file-sin-md/markdown",
            ScriptedResponse::status(500),
        );

        let broker =
            BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
        let conversation = fixture
            .database
            .create_conversation("Conversión rota", None)
            .expect("la conversación debe crearse");
        let view = fixture.import(&broker, &conversation.id);

        assert!(
            SimulatedBroker::wait_until(SETTLE_TIMEOUT, || fixture.view(&view.id).ingestion_status
                == "ready"),
            "el adjunto debía quedar disponible pese al fallo de conversión"
        );

        let degraded = fixture.view(&view.id);
        assert_eq!(degraded.broker_file_id.as_deref(), Some("file-sin-md"));
        assert!(
            degraded.ingestion_error.is_none(),
            "el adjunto no ha fallado"
        );
        // El fallo se registra donde ocurrió: en el contexto, no en la ingesta.
        assert_eq!(degraded.context_status, "failed");
        assert!(degraded.context_error.is_some());
        assert_eq!(degraded.chunk_count, 0);
    }

    /// Sin Markdown publicado, el contexto se declara no disponible sin fingir.
    #[test]
    fn an_attachment_without_conversion_declares_its_context_unavailable() {
        let fixture = IngestionFixture::new(b"imagen sin texto");
        let simulated = SimulatedBroker::start();
        simulated.always(
            "POST /api/v1/files",
            ScriptedResponse::ok(accepted_file(
                "file-imagen",
                "captura.jpg",
                fixture.size_bytes as i64,
                &fixture.sha256,
            )),
        );
        let mut ready = file_state("file-imagen", "ready", None);
        ready["sha256"] = json!(fixture.sha256);
        simulated.always("GET /api/v1/files/{id}", ScriptedResponse::ok(ready));

        let broker =
            BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
        let conversation = fixture
            .database
            .create_conversation("Sin conversión", None)
            .expect("la conversación debe crearse");
        let view = fixture.import(&broker, &conversation.id);

        assert!(
            SimulatedBroker::wait_until(SETTLE_TIMEOUT, || fixture.view(&view.id).ingestion_status
                == "ready"),
            "el adjunto debía quedar disponible"
        );
        let ready_view = fixture.view(&view.id);
        assert_eq!(ready_view.context_status, "unavailable");
        assert_eq!(ready_view.chunk_count, 0);
        assert!(
            ready_view.context_error.is_none(),
            "no es un error, es una ausencia"
        );
    }

    /// Reintentar y recuperar tras reinicio reanudan la ingesta pendiente.
    #[test]
    fn retry_and_restart_resume_a_failed_ingestion() {
        let fixture = IngestionFixture::new(b"documento que primero falla");
        let simulated = SimulatedBroker::start();
        // Primer intento: el Broker rechaza el archivo por contrato.
        simulated.script("POST /api/v1/files", ScriptedResponse::permanent());
        simulated.always(
            "POST /api/v1/files",
            ScriptedResponse::ok(accepted_file(
                "file-reintento",
                "informe.pdf",
                fixture.size_bytes as i64,
                &fixture.sha256,
            )),
        );
        let mut ready = file_state("file-reintento", "ready", None);
        ready["sha256"] = json!(fixture.sha256);
        simulated.always("GET /api/v1/files/{id}", ScriptedResponse::ok(ready));

        let broker =
            BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse");
        let conversation = fixture
            .database
            .create_conversation("Reintento de adjunto", None)
            .expect("la conversación debe crearse");
        let view = fixture.import(&broker, &conversation.id);

        assert!(
            SimulatedBroker::wait_until(SETTLE_TIMEOUT, || fixture.view(&view.id).ingestion_status
                == "failed"),
            "el primer intento debía fallar"
        );

        // Reintentar limpia el error y vuelve a subir.
        let retried = retry_attachment(fixture.database.clone(), broker.clone(), &view.id)
            .expect("el reintento debe aceptarse");
        assert_ne!(retried.ingestion_status, "failed");
        assert!(
            SimulatedBroker::wait_until(SETTLE_TIMEOUT, || fixture.view(&view.id).ingestion_status
                == "ready"),
            "el reintento debía completar la ingesta"
        );
        assert_eq!(simulated.requests_to("POST", "/api/v1/files").len(), 2);

        // Recuperar al arrancar no reabre lo ya terminado ni duplica la subida.
        let recovered = recover_at_start(fixture.database.clone(), broker.clone())
            .expect("la recuperación debe ejecutarse");
        std::thread::sleep(Duration::from_millis(1_200));
        assert_eq!(
            simulated.requests_to("POST", "/api/v1/files").len(),
            2,
            "recuperar no debe volver a subir un adjunto ya ingerido"
        );
        assert_eq!(fixture.view(&view.id).ingestion_status, "ready");
        // Lo recuperado, si acaso, es la preparación de contexto pendiente.
        assert!(recovered <= 1);
    }
}
