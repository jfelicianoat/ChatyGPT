use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::Serialize;
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db::{ConversationMessage, ConversationView, Database};
use crate::error::AppError;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub destination_path: String,
    pub source_hash: String,
    pub destination_hash: String,
    pub overwritten: bool,
}

pub fn export_conversation(
    database: Database,
    conversation_id: &str,
    destination_path: &str,
    overwrite_confirmed: bool,
) -> Result<ExportReport, AppError> {
    let view = database.conversation_view(conversation_id)?;
    let markdown = render_conversation_markdown(&view);
    let source_hash = hash_bytes(markdown.as_bytes());
    let destination = validate_destination(destination_path)?;
    let destination_string = destination.to_string_lossy().into_owned();
    let stable_export_id = format!("conversation:{conversation_id}:markdown:v1");
    let existed = destination.exists();
    let hash_before = if existed {
        Some(hash_file(&destination)?)
    } else {
        None
    };
    let previous_hash =
        database.last_completed_export_hash(&stable_export_id, &destination_string)?;
    let unchanged_previous_export = hash_before.is_some() && hash_before == previous_hash;
    if existed && !overwrite_confirmed && !unchanged_previous_export {
        let error = json!({
            "code": "EXPORT_DESTINATION_CONFLICT",
            "message": "El destino ya existe o fue modificado fuera de ChatyGPT"
        });
        database.record_export(
            conversation_id,
            &stable_export_id,
            &destination_string,
            &source_hash,
            hash_before.as_deref(),
            None,
            "conflict",
            Some(&error),
        )?;
        return Err(AppError::Conflict(
            "el archivo de destino existe o cambió; vuelve a elegirlo y confirma la sobrescritura"
                .to_owned(),
        ));
    }

    database.record_export(
        conversation_id,
        &stable_export_id,
        &destination_string,
        &source_hash,
        hash_before.as_deref(),
        None,
        "pending",
        None,
    )?;
    if let Err(error) = atomic_write(&destination, markdown.as_bytes()) {
        let detail = json!({"message": error.to_string()});
        let _ = database.record_export(
            conversation_id,
            &stable_export_id,
            &destination_string,
            &source_hash,
            hash_before.as_deref(),
            None,
            "failed",
            Some(&detail),
        );
        return Err(error);
    }
    let destination_hash = hash_file(&destination)?;
    if destination_hash != source_hash {
        let error = json!({
            "code": "EXPORT_HASH_MISMATCH",
            "source_hash": source_hash,
            "destination_hash": destination_hash
        });
        database.record_export(
            conversation_id,
            &stable_export_id,
            &destination_string,
            &source_hash,
            hash_before.as_deref(),
            Some(&destination_hash),
            "failed",
            Some(&error),
        )?;
        return Err(AppError::Conflict(
            "la verificación del archivo exportado no coincide".to_owned(),
        ));
    }
    database.record_export(
        conversation_id,
        &stable_export_id,
        &destination_string,
        &source_hash,
        hash_before.as_deref(),
        Some(&destination_hash),
        "completed",
        None,
    )?;
    Ok(ExportReport {
        destination_path: destination_string,
        source_hash,
        destination_hash,
        overwritten: existed,
    })
}

fn validate_destination(raw: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::Validation(
            "la ruta de exportación debe ser absoluta".to_owned(),
        ));
    }
    let extension = path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase);
    if !matches!(extension.as_deref(), Some("md" | "markdown")) {
        return Err(AppError::Validation(
            "la exportación debe usar extensión .md o .markdown".to_owned(),
        ));
    }
    let parent = path.parent().ok_or_else(|| {
        AppError::Validation("el destino no tiene un directorio válido".to_owned())
    })?;
    let canonical_parent = parent.canonicalize().map_err(|error| {
        AppError::Validation(format!("directorio de destino no válido: {error}"))
    })?;
    let filename = path.file_name().ok_or_else(|| {
        AppError::Validation("el destino no tiene un nombre de archivo".to_owned())
    })?;
    Ok(canonical_parent.join(filename))
}

fn render_conversation_markdown(view: &ConversationView) -> String {
    let title = view.title.replace(['\r', '\n'], " ");
    let mut output = format!("<!-- chatygpt-export:{} -->\n\n# {}\n\n", view.id, title);
    for message in &view.messages {
        render_message(&mut output, message);
    }
    output
}

fn render_message(output: &mut String, message: &ConversationMessage) {
    let role = match message.role.as_str() {
        "user" => "Usuario",
        "assistant" => "ChatyGPT",
        "system" => "Sistema",
        "tool" => "Herramienta",
        _ => "Evento",
    };
    output.push_str(&format!("## {role}\n\n"));
    if let Some(text) = &message.text {
        output.push_str(text.trim());
        output.push_str("\n\n");
    } else if let Some(error) = &message.error {
        output.push_str("> Error: `");
        output.push_str(&error.to_string().replace('`', "'"));
        output.push_str("`\n\n");
    } else {
        output.push_str(&format!("> Estado: {}\n\n", message.status));
    }
    if !message.sources.is_empty() {
        output.push_str("### Fuentes usadas\n\n");
        for source in &message.sources {
            output.push_str("- ");
            output.push_str(&source.title.replace(['\r', '\n'], " "));
            if let Some(media_type) = &source.media_type {
                output.push_str(&format!(" ({media_type})"));
            }
            output.push('\n');
        }
        output.push_str("\n> Estas son fuentes documentales enviadas en el turno; no implican una cita por frase.\n\n");
    }
}

fn atomic_write(destination: &Path, bytes: &[u8]) -> Result<(), AppError> {
    let parent = destination.parent().ok_or_else(|| {
        AppError::Validation("el destino no tiene un directorio válido".to_owned())
    })?;
    let temporary = parent.join(format!(".chatygpt-export-{}.tmp", Uuid::new_v4().simple()));
    let result = (|| {
        let mut file = OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        file.write_all(bytes)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        file.sync_all()
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        fs::rename(&temporary, destination)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        Ok(())
    })();
    if result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    result
}

fn hash_file(path: &Path) -> Result<String, AppError> {
    let mut file = File::open(path).map_err(|error| AppError::DataDirectory(error.to_string()))?;
    let mut hasher = Sha256::new();
    let mut buffer = vec![0_u8; 1024 * 1024];
    loop {
        let read = file
            .read(&mut buffer)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
    }
    Ok(format!("{:x}", hasher.finalize()))
}

fn hash_bytes(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

#[cfg(test)]
mod tests {
    use super::{atomic_write, export_conversation, hash_file};
    use crate::db::{ContextMessage, Database};
    use crate::error::AppError;
    use uuid::Uuid;

    #[test]
    fn atomic_write_replaces_file_and_hashes_final_bytes() {
        let path = std::env::temp_dir().join(format!("chatygpt-export-{}.md", Uuid::new_v4()));
        std::fs::write(&path, b"old").expect("old export should exist");
        atomic_write(&path, b"new export").expect("atomic replacement should work");
        assert_eq!(
            std::fs::read(&path).expect("export should read"),
            b"new export"
        );
        assert_eq!(hash_file(&path).expect("hash should work").len(), 64);
        let _ = std::fs::remove_file(path);
    }

    #[test]
    fn export_detects_external_changes_and_requires_overwrite_confirmation() {
        let root = std::env::temp_dir().join(format!("chatygpt-export-test-{}", Uuid::new_v4()));
        std::fs::create_dir_all(&root).expect("test directory should exist");
        let database_path = root.join("chatygpt.sqlite");
        let database = Database::open(&database_path).expect("database should open");
        let conversation = database
            .create_conversation("Conversación exportable", None)
            .expect("conversation should be created");
        let context = vec![ContextMessage {
            message_id: "export-user-message".to_owned(),
            role: "user".to_owned(),
            text: "Contenido para exportar".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "export-user-message",
                "export-assistant-message",
                "export-local-task",
                "export-idempotency",
                "Contenido para exportar",
                &serde_json::json!({}),
                &context,
                &[],
            )
            .expect("turn should be prepared");
        let destination = root.join("conversation.md");
        let report = export_conversation(
            database.clone(),
            &conversation.id,
            &destination.to_string_lossy(),
            false,
        )
        .expect("new destination should export");
        assert!(!report.overwritten);
        let markdown = std::fs::read_to_string(&destination).expect("export should read");
        assert!(markdown.contains("# Contenido para exportar"));
        assert!(markdown.contains("Contenido para exportar"));

        std::fs::write(&destination, "cambio externo").expect("external edit should work");
        assert!(matches!(
            export_conversation(
                database.clone(),
                &conversation.id,
                &destination.to_string_lossy(),
                false,
            ),
            Err(AppError::Conflict(_))
        ));
        assert_eq!(
            std::fs::read_to_string(&destination).expect("external edit should survive"),
            "cambio externo"
        );
        let forced = export_conversation(
            database,
            &conversation.id,
            &destination.to_string_lossy(),
            true,
        )
        .expect("confirmed overwrite should work");
        assert!(forced.overwritten);
        let _ = std::fs::remove_dir_all(root);
    }
}
