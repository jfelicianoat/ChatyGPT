//! Conversaciones y proyectos: alta, busqueda, movimiento y archivado.

use super::*;
use crate::*;

#[tauri::command]
pub(crate) fn create_conversation(
    title: Option<String>,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, AppError> {
    let title = title
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .unwrap_or("Nueva conversación");
    state
        .database
        .create_conversation(title, project_id.as_deref())
}

#[tauri::command]
pub(crate) fn list_conversations(
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSummary>, AppError> {
    state.database.list_conversations()
}

#[tauri::command]
pub(crate) fn search_conversations(
    query: String,
    include_archived: Option<bool>,
    state: State<'_, AppState>,
) -> Result<Vec<crate::db::ConversationSearchHit>, AppError> {
    let query = validated_text(&query, "la búsqueda", 200)?;
    state
        .database
        .search_conversation_hits(&query, 50, include_archived.unwrap_or(false))
}

/// Archivo y papelera: lo que se puede recuperar (H14).
#[tauri::command]
pub(crate) fn get_archive_overview(
    state: State<'_, AppState>,
) -> Result<crate::db::ArchiveOverview, AppError> {
    state.database.archive_overview()
}

#[tauri::command]
pub(crate) fn restore_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, AppError> {
    state.database.restore_conversation(&conversation_id)
}

#[tauri::command]
pub(crate) fn restore_project(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectSummary, AppError> {
    state.database.restore_project(&project_id)
}

/// Borra para siempre una conversación de la papelera y las copias
/// gestionadas de sus adjuntos que ya nadie usa.
#[tauri::command]
pub(crate) fn purge_conversation(
    conversation_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let orphaned = state
        .database
        .purge_conversation(&conversation_id, confirmed)?;
    let managed = std::fs::canonicalize(&state.attachments_dir).ok();
    for path in orphaned {
        // Solo se toca lo que vive dentro del almacén gestionado: una ruta de
        // la base nunca puede llevar a borrar un fichero de la persona.
        let inside = std::fs::canonicalize(&path)
            .ok()
            .zip(managed.as_ref())
            .is_some_and(|(file, root)| file.starts_with(root));
        if inside {
            let _ = std::fs::remove_file(&path);
        }
    }
    Ok(())
}

#[tauri::command]
pub(crate) fn rename_conversation(
    conversation_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, AppError> {
    let title = validated_text(&title, "el título", 120)?;
    state.database.rename_conversation(&conversation_id, &title)
}

#[tauri::command]
pub(crate) fn move_conversation(
    conversation_id: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, AppError> {
    state
        .database
        .move_conversation(&conversation_id, project_id.as_deref())
}

#[tauri::command]
pub(crate) fn set_conversation_custom_gpt(
    conversation_id: String,
    custom_gpt_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConversationView, AppError> {
    state
        .database
        .set_conversation_custom_gpt(&conversation_id, custom_gpt_id.as_deref())
}

#[tauri::command]
pub(crate) fn archive_conversation(
    conversation_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "archivar requiere confirmación explícita".to_owned(),
        ));
    }
    state.database.archive_conversation(&conversation_id)
}

#[tauri::command]
pub(crate) fn delete_conversation(
    conversation_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "eliminar requiere confirmación explícita".to_owned(),
        ));
    }
    state.database.delete_conversation(&conversation_id)
}

#[tauri::command]
pub(crate) fn create_project(
    name: String,
    description: Option<String>,
    state: State<'_, AppState>,
) -> Result<ProjectSummary, AppError> {
    let name = validated_text(&name, "el nombre del proyecto", 120)?;
    let description = description
        .as_deref()
        .map(str::trim)
        .filter(|value| !value.is_empty());
    if description.is_some_and(|value| value.chars().count() > 2_000) {
        return Err(AppError::Validation(
            "la descripción supera el límite de 2.000 caracteres".to_owned(),
        ));
    }
    state.database.create_project(&name, description)
}

#[tauri::command]
pub(crate) fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSummary>, AppError> {
    state.database.list_projects()
}
