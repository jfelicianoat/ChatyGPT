//! Memoria, busquedas semanticas, auditoria y recuperacion.

use std::collections::HashSet;

use serde::Serialize;
use serde_json::Value;
use url::Url;

use crate::error::AppError;

use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItemView {
    pub id: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub custom_gpt_id: Option<String>,
    pub custom_gpt_name: Option<String>,
    pub category: String,
    pub content: String,
    pub sensitivity: String,
    pub enabled: bool,
    pub embedding_status: String,
    pub embedding_model: Option<String>,
    pub embedding_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub enabled: bool,
    pub items: Vec<MemoryItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchResultView {
    pub memory_id: String,
    pub content: String,
    pub category: String,
    pub project_name: Option<String>,
    pub sensitivity: String,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchView {
    pub id: String,
    pub query: String,
    pub project_id: Option<String>,
    pub status: String,
    pub model: Option<String>,
    pub error: Option<String>,
    pub results: Vec<MemorySearchResultView>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SemanticChatWorkflow {
    pub id: String,
    pub conversation_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
    pub embedding_task_id: String,
    pub chat_task_id: Option<String>,
    pub user_text: String,
    pub context: Vec<ContextMessage>,
    pub project_instruction: Option<ProjectInstructionContext>,
    pub custom_gpt_context: Option<CustomGptContext>,
    pub attachment_ids: Vec<String>,
    pub tools_enabled: bool,
    pub sandbox_enabled: bool,
    pub execution_preferences: ConversationExecutionPreferences,
    /// Plan de Investigación profunda congelado al enviar el turno.
    ///
    /// `None` es un turno semántico ordinario. Cuando existe, la segunda etapa
    /// y cualquier recuperación posterior aplican este plan tal cual, sin
    /// volver a preguntar al Broker qué herramientas anuncia hoy.
    pub research_plan: Option<Value>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct SemanticMemoryMatch {
    pub memory: MemoryItemView,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSourceView {
    pub kind: String,
    pub label: String,
    pub reason: String,
    pub score: Option<f64>,
    pub estimated_tokens: i64,
    pub excerpt: String,
    pub source_reference: Option<String>,
    pub source_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshotView {
    pub strategy: String,
    pub estimated_tokens: i64,
    pub sources: Vec<ContextSourceView>,
}

pub(crate) struct MemorySearchRecord {
    pub(crate) query: String,
    pub(crate) project_id: Option<String>,
    pub(crate) remote_status: String,
    pub(crate) local_state: String,
    pub(crate) error_json: Option<String>,
    pub(crate) model: Option<String>,
    pub(crate) dimensions: Option<i64>,
    pub(crate) blob: Option<Vec<u8>>,
    pub(crate) created_at: String,
}

pub(crate) fn audit_presentation(event_type: &str) -> (&'static str, &'static str, &'static str) {
    match event_type {
        "project.created" => ("project", "Proyecto creado", "info"),
        "project.renamed" => ("project", "Proyecto renombrado", "info"),
        "project.instructions_updated" => {
            ("project", "Instrucciones del proyecto actualizadas", "info")
        }
        "project.archived" => ("project", "Proyecto archivado", "warning"),
        "project.file_added" => ("project", "Archivo guardado en el proyecto", "info"),
        "project.file_removed" => ("project", "Archivo retirado del proyecto", "warning"),
        "project.file_used" => ("project", "Archivo del proyecto añadido al chat", "info"),
        "custom_gpt.created" => ("gpt", "GPT personal creado", "info"),
        "custom_gpt.version_created" => ("gpt", "Nueva versión del GPT guardada", "info"),
        "custom_gpt.imported" => ("gpt", "GPT personal importado", "info"),
        "custom_gpt.exported" => ("gpt", "GPT personal exportado", "info"),
        "custom_gpt.knowledge_created" => ("gpt", "Conocimiento del GPT añadido", "info"),
        "custom_gpt.knowledge_enabled" => ("gpt", "Conocimiento del GPT activado", "info"),
        "custom_gpt.knowledge_disabled" => ("gpt", "Conocimiento del GPT desactivado", "warning"),
        "custom_gpt.knowledge_deleted" => ("gpt", "Conocimiento del GPT eliminado", "warning"),
        "custom_gpt.file_added" => ("gpt", "Archivo de conocimiento del GPT añadido", "info"),
        "custom_gpt.file_removed" => ("gpt", "Archivo de conocimiento del GPT retirado", "warning"),
        "conversation.custom_gpt_updated" => {
            ("gpt", "GPT personal de la conversación actualizado", "info")
        }
        "conversation.created" => ("conversation", "Conversación creada", "info"),
        "conversation.renamed" => ("conversation", "Conversación renombrada", "info"),
        "conversation.moved" => ("conversation", "Conversación movida", "info"),
        "conversation.archived" => ("conversation", "Conversación archivada", "warning"),
        "conversation.deleted" => ("conversation", "Conversación eliminada", "warning"),
        "attachment.added" => ("attachment", "Adjunto añadido", "info"),
        "attachment.removed" => ("attachment", "Adjunto retirado", "info"),
        "attachment.retry_requested" => ("attachment", "Reintento de adjunto solicitado", "info"),
        "local.prepared" => ("task", "Mensaje preparado para enviar", "info"),
        "remote.accepted" => ("task", "Broker AI aceptó la tarea", "info"),
        "remote.status_changed" => ("task", "Cambió el estado de una tarea", "info"),
        "transport.error" => ("task", "Error temporal de conexión", "error"),
        "local.orphaned" => ("task", "Tarea pendiente marcada para revisión", "warning"),
        "task.abandoned_cancelled" => ("task", "Tarea abandonada cerrada en Broker AI", "warning"),
        "local.tool_decisions_prepared" => ("tool", "Decisiones de herramientas guardadas", "info"),
        "athena.permission_granted" => ("athena", "Permiso concedido a Athena", "warning"),
        "athena.permission_denied" => ("athena", "Permiso denegado a Athena", "info"),
        "athena.permission_rejected_by_service" => {
            ("athena", "La respuesta al permiso llegó tarde", "warning")
        }
        "athena.run_started" => ("athena", "Run de Athena iniciado", "info"),
        "athena.run_closed" => ("athena", "Run de Athena cerrado", "info"),
        "remote.tool_results_accepted" => ("tool", "Broker AI aceptó los resultados", "info"),
        "export.pending" => ("export", "Exportación iniciada", "info"),
        "export.completed" => ("export", "Exportación completada", "info"),
        "export.conflict" => ("export", "Exportación detenida por un conflicto", "warning"),
        "export.failed" => ("export", "Error durante la exportación", "error"),
        "scheduled_template.created" => ("scheduler", "Plantilla de automatización creada", "info"),
        "scheduled_template.deleted" => (
            "scheduler",
            "Plantilla de automatización eliminada",
            "warning",
        ),
        "scheduled_run.manual_requested" => (
            "scheduler",
            "Ejecución programada iniciada manualmente",
            "info",
        ),
        "memory.enabled" => ("memory", "Memoria activada", "info"),
        "memory.disabled" => ("memory", "Memoria desactivada", "warning"),
        "memory.created" => ("memory", "Recuerdo creado", "info"),
        "memory.updated" => ("memory", "Recuerdo actualizado", "info"),
        "memory.item_enabled" => ("memory", "Recuerdo activado", "info"),
        "memory.item_disabled" => ("memory", "Recuerdo desactivado", "warning"),
        "memory.deleted" => ("memory", "Recuerdo eliminado", "warning"),
        "summary.generation_started" => ("conversation", "Generación de resumen iniciada", "info"),
        "summary.draft_ready" => ("conversation", "Borrador de resumen preparado", "info"),
        "summary.draft_updated" => ("conversation", "Borrador de resumen editado", "info"),
        "summary.approved" => ("conversation", "Resumen de conversación aprobado", "info"),
        "conversation.execution_preferences_updated" => {
            ("conversation", "Opciones de ejecución actualizadas", "info")
        }
        _ => ("system", "Actividad registrada", "info"),
    }
}

pub(crate) fn validated_custom_gpt_fields(
    name: &str,
    description: Option<&str>,
    instructions: &str,
) -> Result<(String, Option<String>, String), AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation(
            "el nombre del GPT debe contener entre 1 y 80 caracteres".to_owned(),
        ));
    }
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if description
        .as_deref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(AppError::Validation(
            "la descripción del GPT supera el límite de 500 caracteres".to_owned(),
        ));
    }
    let instructions = instructions.trim();
    if instructions.is_empty() || instructions.chars().count() > 12_000 {
        return Err(AppError::Validation(
            "las instrucciones del GPT deben contener entre 1 y 12.000 caracteres".to_owned(),
        ));
    }
    Ok((name.to_owned(), description, instructions.to_owned()))
}

/// Normaliza el modelo preferido de un GPT contra el límite real del Broker.
///
/// `ModelRequirements.preferred_model` admite hasta 128 caracteres; validarlo
/// aquí evita que una configuración inválida se descubra al fallar la tarea.
pub(crate) fn validated_preferred_model(
    preferred_model: Option<&str>,
) -> Result<Option<String>, AppError> {
    let Some(model) = preferred_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if model.chars().count() > 128 {
        return Err(AppError::Validation(
            "el modelo preferido supera los 128 caracteres que admite el Broker".to_owned(),
        ));
    }
    if model.chars().any(|character| character.is_whitespace()) {
        return Err(AppError::Validation(
            "el modelo preferido no puede contener espacios".to_owned(),
        ));
    }
    Ok(Some(model.to_owned()))
}

pub(crate) fn validated_conversation_starters(
    starters: &[String],
) -> Result<Vec<String>, AppError> {
    if starters.len() > 6 {
        return Err(AppError::Validation(
            "un GPT puede tener como máximo 6 iniciadores de conversación".to_owned(),
        ));
    }
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for starter in starters {
        let starter = starter.trim();
        if starter.is_empty() {
            continue;
        }
        if starter.chars().count() > 300 {
            return Err(AppError::Validation(
                "cada iniciador puede tener como máximo 300 caracteres".to_owned(),
            ));
        }
        let key = starter.to_lowercase();
        if seen.insert(key) {
            normalized.push(starter.to_owned());
        }
    }
    Ok(normalized)
}

pub(crate) fn validated_custom_gpt_tool_permissions(
    permissions: &CustomGptToolPermissions,
) -> Result<CustomGptToolPermissions, AppError> {
    for (label, effect) in [
        ("Código aislado", permissions.run_code.as_str()),
        (
            "Renombrar conversación",
            permissions.rename_conversation.as_str(),
        ),
        (
            "Leer carpetas autorizadas",
            permissions.read_authorized_folders.as_str(),
        ),
        (
            "Modificar archivos autorizados",
            permissions.modify_authorized_files.as_str(),
        ),
        (
            "Crear tareas programadas",
            permissions.create_scheduled_tasks.as_str(),
        ),
        (
            "Consultar APIs externas",
            permissions.call_external_apis.as_str(),
        ),
    ] {
        if !matches!(effect, "deny" | "confirm") {
            return Err(AppError::Validation(format!(
                "el permiso «{label}» debe estar denegado o requerir confirmación"
            )));
        }
    }
    if permissions.modify_authorized_files == "confirm"
        && permissions.read_authorized_folders != "confirm"
    {
        return Err(AppError::Validation(
            "modificar archivos requiere también permiso para leer carpetas autorizadas".to_owned(),
        ));
    }
    Ok(permissions.clone())
}

/// Texto principal de una respuesta del contrato actual. `result_markdown` se
/// conserva como alias de lectura para tareas creadas con Brokers anteriores.
pub(crate) fn assistant_result_text(result: &Value) -> Option<&str> {
    result
        .get("assistant_content")
        .and_then(Value::as_str)
        .or_else(|| result.get("result_markdown").and_then(Value::as_str))
}

pub(crate) fn markdown_web_sources(markdown: &str) -> Vec<(String, String)> {
    fn push_source(
        sources: &mut Vec<(String, String)>,
        seen: &mut HashSet<String>,
        title: &str,
        raw_url: &str,
    ) {
        if sources.len() >= 50 || raw_url.chars().count() > 2_048 {
            return;
        }
        let Ok(mut url) = Url::parse(raw_url) else {
            return;
        };
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return;
        }
        url.set_fragment(None);
        let normalized = url.to_string();
        if !seen.insert(normalized.clone()) {
            return;
        }
        let title = title.trim();
        let title = if title.is_empty() {
            url.host_str().unwrap_or("Fuente web").to_owned()
        } else {
            title.chars().take(300).collect()
        };
        sources.push((title, normalized));
    }

    let mut sources = Vec::new();
    let mut seen = HashSet::new();
    let mut remaining = markdown;
    while let Some(close_label) = remaining.find("](") {
        let before = &remaining[..close_label];
        let Some(open_label) = before.rfind('[') else {
            remaining = &remaining[close_label + 2..];
            continue;
        };
        let after = &remaining[close_label + 2..];
        let Some(close_url) = after.find(')') else {
            break;
        };
        push_source(
            &mut sources,
            &mut seen,
            &before[open_label + 1..],
            after[..close_url].trim(),
        );
        remaining = &after[close_url + 1..];
    }
    for token in markdown.split_whitespace() {
        let candidate = token.trim_matches(|character: char| {
            matches!(
                character,
                '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        });
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            push_source(&mut sources, &mut seen, "", candidate.trim_end_matches('.'));
        }
    }
    sources
}
