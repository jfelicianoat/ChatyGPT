use std::collections::{HashMap, HashSet};
use std::fs::{self, File, OpenOptions};
use std::io::{Read, Write};
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::db::{
    AttachmentRecord, ConversationExportMetadata, ConversationMessage, ConversationSummary,
    ConversationView, Database, MemoryItemView, ProjectExportMetadata, ScheduledHistoryExportRow,
};
use crate::error::AppError;

/// Comprueba que el destino cae dentro de una carpeta autorizada y vigente.
///
/// La autorización solo la concede una elección humana en el selector nativo,
/// de modo que ningún camino de código puede escribir en una carpeta arbitraria
/// del equipo aunque reciba la ruta ya construida.
fn ensure_write_authorized(database: &Database, destination: &Path) -> Result<(), AppError> {
    let folder = destination.parent().unwrap_or(destination);
    if database.write_is_authorized(folder)? {
        return Ok(());
    }
    Err(AppError::Conflict(
        "esa carpeta no está autorizada para escribir; vuelve a elegir el destino \
         en el selector para autorizarla"
            .to_owned(),
    ))
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportReport {
    pub destination_path: String,
    pub source_hash: String,
    pub destination_hash: String,
    pub overwritten: bool,
    pub format: String,
    pub attachment_count: usize,
    pub reused_attachment_count: usize,
    pub project_index_updated: bool,
    pub approved_memory_count: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledHistoryExportReport {
    pub destination_path: String,
    pub destination_hash: String,
    pub overwritten: bool,
    pub run_count: usize,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct ScheduledCalendarExportEntry {
    pub occurrence_id: String,
    pub task_name: String,
    pub conversation_title: String,
    pub starts_at: String,
    pub projected: bool,
    pub overdue: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledCalendarExportReport {
    pub destination_path: String,
    pub destination_hash: String,
    pub overwritten: bool,
    pub event_count: usize,
}

pub fn export_scheduled_history(
    database: Database,
    destination_path: &str,
    status_filter: &str,
    period_filter: &str,
    overwrite_confirmed: bool,
) -> Result<ScheduledHistoryExportReport, AppError> {
    let rows = database.scheduled_history_export_rows(status_filter, period_filter)?;
    let content = render_scheduled_history_text(&rows, status_filter, period_filter);
    let destination = validate_text_destination(destination_path)?;
    ensure_write_authorized(&database, &destination)?;
    let destination_string = destination.to_string_lossy().into_owned();
    let existed = destination.exists();
    if existed && !overwrite_confirmed {
        return Err(AppError::Conflict(
            "el archivo de destino ya existe; confirma la sobrescritura".to_owned(),
        ));
    }
    atomic_write(&destination, content.as_bytes())?;
    let destination_hash = hash_file(&destination)?;
    let expected_hash = hash_bytes(content.as_bytes());
    if destination_hash != expected_hash {
        return Err(AppError::Conflict(
            "la verificación del historial exportado no coincide".to_owned(),
        ));
    }
    database.record_scheduled_history_export(
        &destination_string,
        &destination_hash,
        rows.len(),
        status_filter,
        period_filter,
    )?;
    Ok(ScheduledHistoryExportReport {
        destination_path: destination_string,
        destination_hash,
        overwritten: existed,
        run_count: rows.len(),
    })
}

pub fn export_scheduled_calendar(
    database: Database,
    destination_path: &str,
    entries: &[ScheduledCalendarExportEntry],
    range_days: u8,
    overwrite_confirmed: bool,
) -> Result<ScheduledCalendarExportReport, AppError> {
    if !matches!(range_days, 7 | 14 | 30) {
        return Err(AppError::Validation(
            "el calendario solo admite periodos de 7, 14 o 30 días".to_owned(),
        ));
    }
    if entries.len() > 5_000 {
        return Err(AppError::Validation(
            "el calendario supera el límite de 5.000 eventos".to_owned(),
        ));
    }
    let content = render_scheduled_calendar(entries, range_days)?;
    let destination = validate_calendar_destination(destination_path)?;
    ensure_write_authorized(&database, &destination)?;
    let destination_string = destination.to_string_lossy().into_owned();
    let existed = destination.exists();
    if existed && !overwrite_confirmed {
        return Err(AppError::Conflict(
            "el archivo de calendario ya existe; confirma la sobrescritura".to_owned(),
        ));
    }
    atomic_write(&destination, content.as_bytes())?;
    let destination_hash = hash_file(&destination)?;
    let expected_hash = hash_bytes(content.as_bytes());
    if destination_hash != expected_hash {
        return Err(AppError::Conflict(
            "la verificación del calendario exportado no coincide".to_owned(),
        ));
    }
    database.record_scheduled_calendar_export(
        &destination_string,
        &destination_hash,
        entries.len(),
        range_days,
    )?;
    Ok(ScheduledCalendarExportReport {
        destination_path: destination_string,
        destination_hash,
        overwritten: existed,
        event_count: entries.len(),
    })
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
    ensure_write_authorized(&database, &destination)?;
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
        format: "markdown".to_owned(),
        attachment_count: 0,
        reused_attachment_count: 0,
        project_index_updated: false,
        approved_memory_count: 0,
    })
}

pub fn export_conversation_to_obsidian(
    database: Database,
    conversation_id: &str,
    vault_path: &str,
    overwrite_confirmed: bool,
) -> Result<ExportReport, AppError> {
    let view = database.conversation_view(conversation_id)?;
    let metadata = database.conversation_export_metadata(conversation_id)?;
    let attachments = database.conversation_attachment_records(conversation_id)?;
    let vault = validate_vault_directory(vault_path)?;
    ensure_write_authorized(&database, &vault.join("ChatyGPT"))?;
    let root = vault.join("ChatyGPT");
    let conversations_dir = root.join("Conversaciones");
    let attachments_dir = root.join("Adjuntos");
    let projects_dir = root.join("Proyectos");
    let project_indices_dir = root.join("Indices").join("Proyectos");
    let memory_dir = root.join("Memoria");
    fs::create_dir_all(&conversations_dir)
        .and_then(|_| fs::create_dir_all(&attachments_dir))
        .and_then(|_| fs::create_dir_all(&projects_dir))
        .and_then(|_| fs::create_dir_all(&project_indices_dir))
        .and_then(|_| fs::create_dir_all(&memory_dir))
        .map_err(|error| AppError::DataDirectory(error.to_string()))?;

    let attachment_exports = plan_attachment_exports(&attachments, &attachments_dir)?;
    let markdown = render_obsidian_conversation(&view, &metadata, &attachment_exports);
    let source_hash = hash_bytes(markdown.as_bytes());
    let destination = conversations_dir.join(format!("{}.md", safe_identifier(&view.id)?));
    let destination_string = destination.to_string_lossy().into_owned();
    let stable_export_id = format!("conversation:{conversation_id}:obsidian:v1");
    let existed = destination.exists();
    let hash_before = existed.then(|| hash_file(&destination)).transpose()?;
    let previous_hash =
        database.last_completed_export_hash(&stable_export_id, &destination_string)?;
    let unchanged_previous_export = hash_before.is_some() && hash_before == previous_hash;
    let conversations = database.list_conversations()?;
    let memory = database.memory_overview()?;
    let approved_memories = if memory.enabled {
        memory
            .items
            .into_iter()
            .filter(|item| item.enabled && item.sensitivity == "normal")
            .collect::<Vec<_>>()
    } else {
        Vec::new()
    };
    let mut managed_projections = Vec::new();
    if let Some(project) = &metadata.project {
        let project_conversations = conversations
            .iter()
            .filter(|item| item.project_id.as_deref() == Some(project.id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        let project_memories = approved_memories
            .iter()
            .filter(|item| item.project_id.as_deref() == Some(project.id.as_str()))
            .cloned()
            .collect::<Vec<_>>();
        managed_projections.push(plan_managed_projection(
            &database,
            format!("project:{}:obsidian-index:v1", project.id),
            project_indices_dir.join(format!("{}.md", safe_identifier(&project.id)?)),
            render_project_index(project, &project_conversations, &project_memories),
        )?);
    }
    managed_projections.push(plan_managed_projection(
        &database,
        "memory:approved:obsidian-index:v1".to_owned(),
        memory_dir.join("Aprobada.md"),
        render_memory_index(&approved_memories),
    )?);
    let attachment_conflict = attachment_exports.iter().any(|item| {
        item.destination_hash
            .as_deref()
            .is_some_and(|hash| hash != item.source_hash)
    });

    let managed_projection_conflict = managed_projections
        .iter()
        .any(ManagedProjection::has_conflict);
    if !overwrite_confirmed
        && ((existed && !unchanged_previous_export)
            || attachment_conflict
            || managed_projection_conflict)
    {
        let error = json!({
            "code": "EXPORT_DESTINATION_CONFLICT",
            "message": "La nota o uno de sus adjuntos fue modificado fuera de ChatyGPT"
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
            "la nota o uno de sus adjuntos ya existe con cambios; confirma para reemplazarlo"
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

    let reused_attachment_count = attachment_exports
        .iter()
        .filter(|item| item.destination_hash.as_deref() == Some(item.source_hash.as_str()))
        .count();
    let export_result = (|| {
        for item in &attachment_exports {
            if item.destination_hash.as_deref() != Some(item.source_hash.as_str()) {
                atomic_copy(&item.source, &item.destination)?;
                if hash_file(&item.destination)? != item.source_hash {
                    return Err(AppError::Conflict(format!(
                        "la copia verificada de {} no coincide con el original",
                        item.display_name
                    )));
                }
            }
        }
        write_project_note_if_missing(&metadata, &projects_dir)?;
        for projection in &managed_projections {
            write_managed_projection(&database, conversation_id, projection)?;
        }
        atomic_write(&destination, markdown.as_bytes())
    })();
    if let Err(error) = export_result {
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
            "la verificación de la nota exportada no coincide".to_owned(),
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
        format: "obsidian".to_owned(),
        attachment_count: attachment_exports.len(),
        reused_attachment_count,
        project_index_updated: metadata.project.is_some(),
        approved_memory_count: approved_memories.len(),
    })
}

#[derive(Debug)]
struct ManagedProjection {
    stable_export_id: String,
    destination: PathBuf,
    content: String,
    source_hash: String,
    destination_hash_before: Option<String>,
    previous_completed_hash: Option<String>,
}

impl ManagedProjection {
    fn has_conflict(&self) -> bool {
        self.destination_hash_before.is_some()
            && self.destination_hash_before != self.previous_completed_hash
    }
}

fn plan_managed_projection(
    database: &Database,
    stable_export_id: String,
    destination: PathBuf,
    content: String,
) -> Result<ManagedProjection, AppError> {
    let destination_hash_before = destination
        .is_file()
        .then(|| hash_file(&destination))
        .transpose()?;
    let previous_completed_hash =
        database.last_completed_export_hash(&stable_export_id, &destination.to_string_lossy())?;
    Ok(ManagedProjection {
        stable_export_id,
        destination,
        source_hash: hash_bytes(content.as_bytes()),
        content,
        destination_hash_before,
        previous_completed_hash,
    })
}

fn write_managed_projection(
    database: &Database,
    conversation_id: &str,
    projection: &ManagedProjection,
) -> Result<(), AppError> {
    let destination = projection.destination.to_string_lossy();
    database.record_export(
        conversation_id,
        &projection.stable_export_id,
        &destination,
        &projection.source_hash,
        projection.destination_hash_before.as_deref(),
        None,
        "pending",
        None,
    )?;
    atomic_write(&projection.destination, projection.content.as_bytes())?;
    let destination_hash = hash_file(&projection.destination)?;
    if destination_hash != projection.source_hash {
        return Err(AppError::Conflict(
            "la verificacion de un indice de Obsidian no coincide".to_owned(),
        ));
    }
    database.record_export(
        conversation_id,
        &projection.stable_export_id,
        &destination,
        &projection.source_hash,
        projection.destination_hash_before.as_deref(),
        Some(&destination_hash),
        "completed",
        None,
    )
}

#[derive(Debug)]
struct ObsidianAttachmentExport {
    id: String,
    display_name: String,
    relative_path: String,
    source: PathBuf,
    destination: PathBuf,
    source_hash: String,
    destination_hash: Option<String>,
}

fn plan_attachment_exports(
    attachments: &[AttachmentRecord],
    attachments_dir: &Path,
) -> Result<Vec<ObsidianAttachmentExport>, AppError> {
    attachments
        .iter()
        .map(|attachment| {
            let source = PathBuf::from(&attachment.local_path);
            if !source.is_file() {
                return Err(AppError::NotFound(format!(
                    "el archivo local {}",
                    attachment.display_name
                )));
            }
            let source_hash = hash_file(&source)?;
            if source_hash != attachment.sha256 {
                return Err(AppError::Conflict(format!(
                    "{} cambió desde que se adjuntó",
                    attachment.display_name
                )));
            }
            let file_name = format!(
                "{}--{}",
                safe_identifier(&attachment.id)?,
                safe_file_name(&attachment.display_name)
            );
            let destination = attachments_dir.join(&file_name);
            let destination_hash = destination
                .is_file()
                .then(|| hash_file(&destination))
                .transpose()?;
            Ok(ObsidianAttachmentExport {
                id: attachment.id.clone(),
                display_name: attachment.display_name.clone(),
                relative_path: format!("../Adjuntos/{file_name}"),
                source,
                destination,
                source_hash,
                destination_hash,
            })
        })
        .collect()
}

fn validate_vault_directory(raw: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::Validation(
            "la carpeta de Obsidian debe usar una ruta absoluta".to_owned(),
        ));
    }
    if !path.is_dir() {
        return Err(AppError::Validation(
            "la carpeta elegida ya no está disponible".to_owned(),
        ));
    }
    path.canonicalize()
        .map_err(|error| AppError::Validation(format!("carpeta de Obsidian no válida: {error}")))
}

fn safe_identifier(value: &str) -> Result<String, AppError> {
    if value.is_empty()
        || !value
            .chars()
            .all(|character| character.is_ascii_alphanumeric() || matches!(character, '-' | '_'))
    {
        return Err(AppError::Validation(
            "el identificador interno no es exportable".to_owned(),
        ));
    }
    Ok(value.to_owned())
}

fn safe_file_name(value: &str) -> String {
    let cleaned: String = value
        .chars()
        .map(|character| match character {
            '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
            '\r' | '\n' => ' ',
            other => other,
        })
        .take(140)
        .collect();
    let cleaned = cleaned.trim().trim_end_matches('.');
    if cleaned.is_empty() {
        "archivo".to_owned()
    } else {
        cleaned.to_owned()
    }
}

fn yaml_string(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| "\"\"".to_owned())
}

fn render_obsidian_conversation(
    view: &ConversationView,
    metadata: &ConversationExportMetadata,
    attachments: &[ObsidianAttachmentExport],
) -> String {
    let mut output = String::from("---\n");
    output.push_str(&format!("chatygpt_id: {}\n", yaml_string(&view.id)));
    output.push_str("type: conversation\n");
    output.push_str(&format!("title: {}\n", yaml_string(&view.title)));
    output.push_str(&format!("created: {}\n", yaml_string(&metadata.created_at)));
    output.push_str(&format!("updated: {}\n", yaml_string(&metadata.updated_at)));
    if let Some(project) = &metadata.project {
        output.push_str(&format!("project_id: {}\n", yaml_string(&project.id)));
        output.push_str(&format!("project: {}\n", yaml_string(&project.name)));
    } else {
        output.push_str("project_id: null\nproject: null\n");
    }
    output.push_str("tags:\n  - chatygpt\n  - conversation\n---\n\n");
    output.push_str(&format!("# {}\n\n", view.title.replace(['\r', '\n'], " ")));
    if let Some(project) = &metadata.project {
        output.push_str(&format!(
            "Proyecto: [[../Proyectos/{}|{}]]\n\n",
            project.id,
            project.name.replace(['[', ']', '|', '\r', '\n'], " ")
        ));
    }
    if !attachments.is_empty() {
        output.push_str("## Adjuntos\n\n");
        for attachment in attachments {
            output.push_str(&format!(
                "- [[{}|{}]]\n",
                attachment.relative_path,
                attachment
                    .display_name
                    .replace(['[', ']', '|', '\r', '\n'], " ")
            ));
        }
        output.push('\n');
    }
    let attachment_paths: HashMap<&str, &str> = attachments
        .iter()
        .map(|attachment| (attachment.id.as_str(), attachment.relative_path.as_str()))
        .collect();
    for message in &view.messages {
        render_obsidian_message(&mut output, message, &attachment_paths);
    }
    output
}

fn render_obsidian_message(
    output: &mut String,
    message: &ConversationMessage,
    attachment_paths: &HashMap<&str, &str>,
) {
    output.push_str(&format!("<!-- chatygpt-message:{} -->\n", message.id));
    render_message(output, message);
    if !message.sources.is_empty() {
        output.push_str("### Enlaces de fuente\n\n");
        for source in &message.sources {
            if let Some(path) = source
                .source_attachment_id
                .as_deref()
                .and_then(|id| attachment_paths.get(id))
            {
                output.push_str(&format!(
                    "- [[{}|{}]]\n",
                    path,
                    source.title.replace(['[', ']', '|', '\r', '\n'], " ")
                ));
            } else if let Some(url) = &source.url {
                output.push_str(&format!(
                    "- [{}]({})\n",
                    source.title.replace(['[', ']'], " "),
                    url
                ));
            }
        }
        output.push('\n');
    }
}

fn write_project_note_if_missing(
    metadata: &ConversationExportMetadata,
    projects_dir: &Path,
) -> Result<(), AppError> {
    let Some(project) = &metadata.project else {
        return Ok(());
    };
    let destination = projects_dir.join(format!("{}.md", safe_identifier(&project.id)?));
    if destination.exists() {
        return Ok(());
    }
    let note = format!(
        "---\nchatygpt_id: {}\ntype: project\ntitle: {}\ntags:\n  - chatygpt\n  - project\n---\n\n# {}\n",
        yaml_string(&project.id),
        yaml_string(&project.name),
        project.name.replace(['\r', '\n'], " ")
    );
    atomic_write(&destination, note.as_bytes())
}

fn render_project_index(
    project: &ProjectExportMetadata,
    conversations: &[ConversationSummary],
    memories: &[MemoryItemView],
) -> String {
    let mut output = format!(
        "---\nchatygpt_id: {}\ntype: project-index\ntitle: {}\ntags:\n  - chatygpt\n  - index\n  - project\n---\n\n# {}\n\n",
        yaml_string(&project.id),
        yaml_string(&project.name),
        project.name.replace(['\r', '\n'], " ")
    );
    output.push_str("## Conversaciones\n\n");
    if conversations.is_empty() {
        output.push_str("_No hay conversaciones activas._\n\n");
    } else {
        for conversation in conversations {
            output.push_str(&format!(
                "- [[../../Conversaciones/{}|{}]] · {}\n",
                conversation.id,
                conversation.title.replace(['[', ']', '|', '\r', '\n'], " "),
                conversation.updated_at
            ));
        }
        output.push('\n');
    }
    output.push_str("## Recuerdos aprobados\n\n");
    render_memory_items(&mut output, memories);
    output
}

fn render_memory_index(memories: &[MemoryItemView]) -> String {
    let mut output = String::from(
        "---\ntype: approved-memory-index\ntitle: \"Memoria aprobada\"\ntags:\n  - chatygpt\n  - memory\n  - index\n---\n\n# Memoria aprobada\n\n",
    );
    output.push_str(
        "> Solo se proyectan recuerdos activos y no sensibles. SQLite sigue siendo la fuente de verdad.\n\n",
    );
    render_memory_items(&mut output, memories);
    output
}

fn render_memory_items(output: &mut String, memories: &[MemoryItemView]) {
    if memories.is_empty() {
        output.push_str("_No hay recuerdos aprobados exportables._\n\n");
        return;
    }
    for memory in memories {
        let scope = memory.project_name.as_deref().unwrap_or("Global");
        output.push_str(&format!(
            "- **{} · {}** — {}\n",
            scope.replace(['*', '\r', '\n'], " "),
            memory.category.replace(['*', '\r', '\n'], " "),
            memory.content.replace(['\r', '\n'], " ")
        ));
    }
    output.push('\n');
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

fn validate_text_destination(raw: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::Validation(
            "la ruta de exportación debe ser absoluta".to_owned(),
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        != Some("txt")
    {
        return Err(AppError::Validation(
            "el historial debe exportarse como archivo .txt".to_owned(),
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

fn validate_calendar_destination(raw: &str) -> Result<PathBuf, AppError> {
    let path = PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::Validation(
            "la ruta de exportación debe ser absoluta".to_owned(),
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .map(str::to_ascii_lowercase)
        .as_deref()
        != Some("ics")
    {
        return Err(AppError::Validation(
            "el calendario debe exportarse como archivo .ics".to_owned(),
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

fn render_scheduled_calendar(
    entries: &[ScheduledCalendarExportEntry],
    range_days: u8,
) -> Result<String, AppError> {
    let mut output = String::new();
    for line in [
        "BEGIN:VCALENDAR".to_owned(),
        "VERSION:2.0".to_owned(),
        "PRODID:-//ChatyGPT//Automatizaciones//ES".to_owned(),
        "CALSCALE:GREGORIAN".to_owned(),
        "METHOD:PUBLISH".to_owned(),
        "X-WR-CALNAME:Automatizaciones de ChatyGPT".to_owned(),
        format!("X-CHATYGPT-RANGE-DAYS:{range_days}"),
    ] {
        push_ics_line(&mut output, &line);
    }
    let mut occurrence_ids = HashSet::new();
    for entry in entries {
        if entry.occurrence_id.trim().is_empty() || entry.occurrence_id.chars().count() > 300 {
            return Err(AppError::Validation(
                "el calendario contiene un identificador de evento inválido".to_owned(),
            ));
        }
        if !occurrence_ids.insert(entry.occurrence_id.trim()) {
            return Err(AppError::Validation(
                "el calendario contiene eventos duplicados".to_owned(),
            ));
        }
        if entry.overdue && entry.projected {
            return Err(AppError::Validation(
                "una fecha atrasada no puede marcarse también como proyección".to_owned(),
            ));
        }
        let task_name = bounded_calendar_text(&entry.task_name, "nombre de tarea", 120)?;
        let conversation_title =
            bounded_calendar_text(&entry.conversation_title, "conversación", 200)?;
        let starts_at = ics_utc_timestamp(&entry.starts_at)?;
        let uid = format!(
            "{}@chatygpt.local",
            hash_bytes(entry.occurrence_id.as_bytes())
        );
        let kind = if entry.overdue {
            "Atrasada"
        } else if entry.projected {
            "Proyección informativa"
        } else {
            "Próxima fecha guardada"
        };
        let description = format!(
            "Destino: {}\\nTipo: {}\\nAbre ChatyGPT para revisar la tarea.",
            ics_escape(&conversation_title),
            kind
        );
        for line in [
            "BEGIN:VEVENT".to_owned(),
            format!("UID:{uid}"),
            format!("DTSTAMP:{starts_at}"),
            format!("DTSTART:{starts_at}"),
            format!("SUMMARY:{}", ics_escape(&task_name)),
            format!("DESCRIPTION:{description}"),
            "CATEGORIES:ChatyGPT".to_owned(),
            format!(
                "X-CHATYGPT-DATE-KIND:{}",
                if entry.overdue {
                    "OVERDUE"
                } else if entry.projected {
                    "PROJECTED"
                } else {
                    "DURABLE"
                }
            ),
            "STATUS:CONFIRMED".to_owned(),
            "END:VEVENT".to_owned(),
        ] {
            push_ics_line(&mut output, &line);
        }
    }
    push_ics_line(&mut output, "END:VCALENDAR");
    Ok(output)
}

fn bounded_calendar_text(value: &str, label: &str, maximum: usize) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() || value.chars().count() > maximum {
        return Err(AppError::Validation(format!(
            "el {label} del calendario no es válido"
        )));
    }
    Ok(value.to_owned())
}

fn ics_utc_timestamp(value: &str) -> Result<String, AppError> {
    let bytes = value.as_bytes();
    let timestamp_suffix = value.get(19..).is_some_and(|suffix| {
        suffix == "Z"
            || (suffix.starts_with('.')
                && suffix.ends_with('Z')
                && suffix.len() >= 3
                && suffix[1..suffix.len() - 1]
                    .bytes()
                    .all(|byte| byte.is_ascii_digit()))
    });
    let structural = bytes.len() >= 20
        && bytes[4] == b'-'
        && bytes[7] == b'-'
        && bytes[10] == b'T'
        && bytes[13] == b':'
        && bytes[16] == b':'
        && bytes.last() == Some(&b'Z')
        && timestamp_suffix
        && [0..4, 5..7, 8..10, 11..13, 14..16, 17..19]
            .into_iter()
            .all(|range| bytes[range].iter().all(u8::is_ascii_digit));
    if !structural {
        return Err(AppError::Validation(
            "el calendario contiene una fecha UTC inválida".to_owned(),
        ));
    }
    let parse =
        |range: std::ops::Range<usize>| -> u32 { value[range].parse::<u32>().unwrap_or_default() };
    let month = parse(5..7);
    let day = parse(8..10);
    let hour = parse(11..13);
    let minute = parse(14..16);
    let second = parse(17..19);
    if !(1..=12).contains(&month)
        || !(1..=31).contains(&day)
        || hour > 23
        || minute > 59
        || second > 59
    {
        return Err(AppError::Validation(
            "el calendario contiene una fecha UTC fuera de rango".to_owned(),
        ));
    }
    Ok(format!(
        "{}{}{}T{}{}{}Z",
        &value[0..4],
        &value[5..7],
        &value[8..10],
        &value[11..13],
        &value[14..16],
        &value[17..19]
    ))
}

fn ics_escape(value: &str) -> String {
    value
        .replace('\\', "\\\\")
        .replace(['\r', '\n'], "\\n")
        .replace(';', "\\;")
        .replace(',', "\\,")
}

fn push_ics_line(output: &mut String, line: &str) {
    let mut width = 0;
    for character in line.chars() {
        let character_width = character.len_utf8();
        if width + character_width > 75 {
            output.push_str("\r\n ");
            width = 1;
        }
        output.push(character);
        width += character_width;
    }
    output.push_str("\r\n");
}

fn render_scheduled_history_text(
    rows: &[ScheduledHistoryExportRow],
    status_filter: &str,
    period_filter: &str,
) -> String {
    let status_label = match status_filter {
        "active" => "En curso",
        "completed" => "Completadas",
        "failed" => "Fallidas",
        "cancelled" => "Canceladas",
        _ => "Todos",
    };
    let period_label = match period_filter {
        "today" => "Hoy",
        "7d" => "Últimos 7 días",
        "30d" => "Últimos 30 días",
        _ => "Cualquier fecha",
    };
    let mut output = format!(
        "HISTORIAL DE AUTOMATIZACIONES DE CHATYGPT\n\
         Estado: {status_label}\n\
         Fecha: {period_label}\n\
         Ejecuciones: {}\n\
         {}\n\n",
        rows.len(),
        "=".repeat(72)
    );
    if rows.is_empty() {
        output.push_str("No hay ejecuciones que coincidan con los filtros.\n");
        return output;
    }
    for row in rows {
        let recurrence = match row.schedule_expression.as_str() {
            "daily" => "Diaria",
            "weekly" => "Semanal",
            _ => "Una vez",
        };
        let status = match row.status.as_str() {
            "claimed" => "Preparando",
            "running" => "En ejecución",
            "completed" => "Completada",
            "failed" => "Fallida",
            "cancelled" => "Cancelada",
            "skipped" => "Omitida",
            other => other,
        };
        output.push_str(&format!(
            "Tarea: {}\n\
             Destino: {}\n\
             Estado: {} · intento {}\n\
             Programación: {} · {}\n\
             Fecha prevista: {}\n\
             Creada: {}\n\
             Actualizada: {}\n\
             Instrucción: {}\n\
             Detalle: {}\n\
             Identificador: {}\n\
             {}\n\n",
            one_line(&row.task_name),
            one_line(&row.conversation_title),
            status,
            row.attempt,
            recurrence,
            one_line(&row.timezone),
            row.due_at,
            row.created_at,
            row.updated_at,
            one_line(&row.prompt),
            scheduled_result_text(row),
            row.run_id,
            "-".repeat(72)
        ));
    }
    output
}

fn scheduled_result_text(row: &ScheduledHistoryExportRow) -> String {
    let candidate = row.result.as_ref().and_then(|result| {
        ["message", "result_markdown", "text", "detail"]
            .iter()
            .find_map(|key| result.get(*key).and_then(serde_json::Value::as_str))
            .or_else(|| {
                result
                    .get("error")
                    .and_then(|error| error.get("message"))
                    .and_then(serde_json::Value::as_str)
            })
    });
    let fallback = match row.status.as_str() {
        "completed" => "La respuesta completa está guardada en la conversación.",
        "cancelled" => "Cancelada por el usuario.",
        "failed" => "El Broker no proporcionó más detalles.",
        "claimed" => "Esperando el inicio de la tarea.",
        "running" => "La ejecución continúa en curso.",
        _ => "Sin detalle adicional.",
    };
    truncate_text(candidate.unwrap_or(fallback), 4_000)
}

fn one_line(value: &str) -> String {
    value.replace(['\r', '\n'], " ").trim().to_owned()
}

fn truncate_text(value: &str, maximum_chars: usize) -> String {
    let mut result: String = value.chars().take(maximum_chars).collect();
    if value.chars().count() > maximum_chars {
        result.push('…');
    }
    result.replace('\r', "").replace('\n', "\n         ")
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

fn atomic_copy(source: &Path, destination: &Path) -> Result<(), AppError> {
    let bytes = fs::read(source).map_err(|error| AppError::DataDirectory(error.to_string()))?;
    atomic_write(destination, &bytes)
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
mod tests;
