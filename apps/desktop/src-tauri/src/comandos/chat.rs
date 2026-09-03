//! El turno de chat: contexto, resumen, envio y resolucion de herramientas.

use super::*;
use crate::*;

#[tauri::command]
pub(crate) fn get_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationView, AppError> {
    state.database.conversation_view(&conversation_id)
}

#[tauri::command]
pub(crate) fn update_conversation_execution_preferences(
    conversation_id: String,
    preferences: ConversationExecutionPreferences,
    state: State<'_, AppState>,
) -> Result<ConversationExecutionPreferences, AppError> {
    state
        .database
        .update_conversation_execution_preferences(&conversation_id, &preferences)
}

#[tauri::command]
pub(crate) fn get_task_context(
    local_task_id: String,
    state: State<'_, AppState>,
) -> Result<ContextSnapshotView, AppError> {
    state.database.task_context(&local_task_id)
}

pub(crate) fn validated_managed_source_path(
    managed_root: &std::path::Path,
    candidate: &std::path::Path,
) -> Result<std::path::PathBuf, AppError> {
    let managed_root = managed_root
        .canonicalize()
        .map_err(|_| AppError::NotFound("almacenamiento local de adjuntos".to_owned()))?;
    let candidate = candidate
        .canonicalize()
        .map_err(|_| AppError::NotFound("archivo local de la fuente".to_owned()))?;
    if !candidate.is_file() || !candidate.starts_with(&managed_root) {
        return Err(AppError::Validation(
            "la fuente no pertenece al almacenamiento administrado de ChatyGPT".to_owned(),
        ));
    }
    Ok(candidate)
}

#[tauri::command]
pub(crate) fn reveal_context_source(
    local_task_id: String,
    source_reference: String,
    state: State<'_, AppState>,
) -> Result<String, AppError> {
    let source = state
        .database
        .context_source_file(&local_task_id, &source_reference)?;
    let path = validated_managed_source_path(
        &state.attachments_dir,
        std::path::Path::new(&source.local_path),
    )?;
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        std::process::Command::new("explorer.exe")
            .arg(format!("/select,{}", path.display()))
            .creation_flags(0x0800_0000)
            .spawn()
            .map_err(|error| {
                AppError::Validation(format!(
                    "no se pudo mostrar el archivo en el Explorador: {error}"
                ))
            })?;
        Ok(source.display_name)
    }
    #[cfg(not(target_os = "windows"))]
    {
        let _ = path;
        Err(AppError::Validation(
            "mostrar la fuente todavía solo está disponible en Windows".to_owned(),
        ))
    }
}

#[tauri::command]
pub(crate) fn get_conversation_summary(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummaryOverview, AppError> {
    state
        .database
        .conversation_summary_overview(&conversation_id)
}

#[tauri::command]
pub(crate) fn start_conversation_summary(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummaryOverview, AppError> {
    task_runtime::start_conversation_summary(
        state.database.clone(),
        state.broker.clone(),
        &conversation_id,
    )
}

#[tauri::command]
pub(crate) fn update_conversation_summary(
    summary_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummaryOverview, AppError> {
    let text = validated_text(&text, "el resumen", 10_000)?;
    state
        .database
        .update_conversation_summary_draft(&summary_id, &text)
}

#[tauri::command]
pub(crate) fn approve_conversation_summary(
    summary_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummaryOverview, AppError> {
    state.database.approve_conversation_summary(&summary_id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) async fn send_chat_turn(
    conversation_id: String,
    text: String,
    attachment_ids: Vec<String>,
    tools_enabled: bool,
    sandbox_enabled: bool,
    semantic_memory_enabled: bool,
    research_mode: bool,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::start_chat_turn(
        state.database.clone(),
        state.broker.clone(),
        &conversation_id,
        &text,
        &attachment_ids,
        tools_enabled,
        sandbox_enabled,
        semantic_memory_enabled,
        research_mode,
    )
    .await
}

#[tauri::command]
pub(crate) async fn resolve_tool_calls(
    local_task_id: String,
    decisions: Vec<task_runtime::ToolDecision>,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::resolve_tool_calls(
        state.database.clone(),
        state.broker.clone(),
        &state.data_dir,
        &local_task_id,
        &decisions,
    )
    .await
}
