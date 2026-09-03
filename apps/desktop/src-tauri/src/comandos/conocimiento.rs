//! Conocimiento y memoria: ficheros de proyecto, del GPT y memoria del usuario.

use super::*;
use crate::*;

#[tauri::command]
pub(crate) fn get_project_knowledge(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectKnowledgeOverview, AppError> {
    state.database.project_knowledge_overview(&project_id)
}

#[tauri::command]
pub(crate) fn remove_project_file(
    project_id: String,
    attachment_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ProjectKnowledgeOverview, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "retirar el archivo del proyecto requiere confirmación explícita".to_owned(),
        ));
    }
    state
        .database
        .remove_project_file(&project_id, &attachment_id)
}

#[tauri::command]
pub(crate) fn set_project_memory_item_enabled(
    project_id: String,
    memory_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<ProjectKnowledgeOverview, AppError> {
    state
        .database
        .set_project_memory_item_enabled(&project_id, &memory_id, enabled)
}

#[tauri::command]
pub(crate) fn list_audit_events(
    state: State<'_, AppState>,
) -> Result<Vec<AuditEventView>, AppError> {
    state.database.list_audit_events(50)
}

#[tauri::command]
pub(crate) fn get_memory_overview(state: State<'_, AppState>) -> Result<MemoryOverview, AppError> {
    state.database.memory_overview()
}

#[tauri::command]
pub(crate) fn get_custom_gpt_knowledge(
    custom_gpt_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::MemoryItemView>, AppError> {
    state.database.custom_gpt_knowledge(&custom_gpt_id)
}

#[tauri::command]
pub(crate) fn list_custom_gpt_files(
    custom_gpt_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state.database.list_custom_gpt_files(&custom_gpt_id)
}

#[tauri::command]
pub(crate) async fn import_custom_gpt_file(
    custom_gpt_id: String,
    source_path: String,
    describe_images: bool,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    attachment_runtime::import_custom_gpt_attachment(
        state.database.clone(),
        state.broker.clone(),
        state.attachments_dir.clone(),
        custom_gpt_id,
        source_path,
        describe_images,
    )
    .await
}

#[tauri::command]
pub(crate) fn remove_custom_gpt_file(
    custom_gpt_id: String,
    attachment_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "retirar el archivo del GPT requiere confirmación".to_owned(),
        ));
    }
    state
        .database
        .remove_custom_gpt_file(&custom_gpt_id, &attachment_id)
}

#[tauri::command]
pub(crate) fn create_custom_gpt_knowledge_item(
    custom_gpt_id: String,
    content: String,
    category: String,
    sensitivity: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::MemoryItemView>, AppError> {
    let content = validated_text(&content, "El conocimiento", 2_000)?;
    if !matches!(category.as_str(), "preference" | "instruction" | "fact") {
        return Err(AppError::Validation(
            "la categoría del conocimiento no es válida".to_owned(),
        ));
    }
    if !matches!(sensitivity.as_str(), "normal" | "sensitive") {
        return Err(AppError::Validation(
            "la sensibilidad del conocimiento no es válida".to_owned(),
        ));
    }
    let (memory_id, _) = state.database.create_custom_gpt_memory_item(
        &custom_gpt_id,
        &content,
        &category,
        &sensitivity,
    )?;
    task_runtime::start_memory_embedding(
        state.database.clone(),
        state.broker.clone(),
        &memory_id,
        &content,
        false,
    )?;
    state.database.custom_gpt_knowledge(&custom_gpt_id)
}

#[tauri::command]
pub(crate) fn set_custom_gpt_knowledge_item_enabled(
    custom_gpt_id: String,
    memory_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<Vec<db::MemoryItemView>, AppError> {
    state
        .database
        .set_custom_gpt_memory_item_enabled(&custom_gpt_id, &memory_id, enabled)
}

#[tauri::command]
pub(crate) fn delete_custom_gpt_knowledge_item(
    custom_gpt_id: String,
    memory_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<Vec<db::MemoryItemView>, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "el borrado del conocimiento requiere confirmación".to_owned(),
        ));
    }
    state
        .database
        .delete_custom_gpt_memory_item(&custom_gpt_id, &memory_id)
}

#[tauri::command]
pub(crate) fn reindex_custom_gpt_knowledge_item(
    custom_gpt_id: String,
    memory_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::MemoryItemView>, AppError> {
    let item = state
        .database
        .custom_gpt_memory_item(&custom_gpt_id, &memory_id)?;
    if item.embedding_status == "indexing" {
        return Err(AppError::Conflict(
            "el conocimiento ya se está indexando".to_owned(),
        ));
    }
    state
        .database
        .clear_custom_gpt_memory_embedding(&custom_gpt_id, &memory_id)?;
    task_runtime::start_memory_embedding(
        state.database.clone(),
        state.broker.clone(),
        &memory_id,
        &item.content,
        true,
    )?;
    state.database.custom_gpt_knowledge(&custom_gpt_id)
}

#[tauri::command]
pub(crate) fn set_memory_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    state.database.set_memory_enabled(enabled)
}

#[tauri::command]
pub(crate) fn create_memory_item(
    content: String,
    category: String,
    sensitivity: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    let content = validated_text(&content, "El recuerdo", 2_000)?;
    if !matches!(category.as_str(), "preference" | "instruction" | "fact") {
        return Err(AppError::Validation(
            "la categoría del recuerdo no es válida".to_owned(),
        ));
    }
    if !matches!(sensitivity.as_str(), "normal" | "sensitive") {
        return Err(AppError::Validation(
            "la sensibilidad del recuerdo no es válida".to_owned(),
        ));
    }
    let (memory_id, _) = state.database.create_memory_item(
        &content,
        &category,
        &sensitivity,
        project_id.as_deref(),
    )?;
    task_runtime::start_memory_embedding(
        state.database.clone(),
        state.broker.clone(),
        &memory_id,
        &content,
        false,
    )?;
    state.database.memory_overview()
}

#[tauri::command]
pub(crate) fn update_memory_item(
    memory_id: String,
    content: String,
    category: String,
    sensitivity: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    let content = validated_text(&content, "El recuerdo", 2_000)?;
    if !matches!(category.as_str(), "preference" | "instruction" | "fact") {
        return Err(AppError::Validation(
            "la categoría del recuerdo no es válida".to_owned(),
        ));
    }
    if !matches!(sensitivity.as_str(), "normal" | "sensitive") {
        return Err(AppError::Validation(
            "la sensibilidad del recuerdo no es válida".to_owned(),
        ));
    }
    let (content_changed, overview) = state.database.update_memory_item(
        &memory_id,
        &content,
        &category,
        &sensitivity,
        project_id.as_deref(),
    )?;
    if content_changed {
        task_runtime::start_memory_embedding(
            state.database.clone(),
            state.broker.clone(),
            &memory_id,
            &content,
            true,
        )?;
        return state.database.memory_overview();
    }
    Ok(overview)
}

#[tauri::command]
pub(crate) fn set_memory_item_enabled(
    memory_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    state.database.set_memory_item_enabled(&memory_id, enabled)
}

#[tauri::command]
pub(crate) fn delete_memory_item(
    memory_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "el borrado del recuerdo requiere confirmación".to_owned(),
        ));
    }
    state.database.delete_memory_item(&memory_id)
}

#[tauri::command]
pub(crate) fn reindex_memory_item(
    memory_id: String,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    let item = state.database.memory_item(&memory_id)?;
    if item.embedding_status == "indexing" {
        return Err(AppError::Conflict(
            "el recuerdo ya se está indexando".to_owned(),
        ));
    }
    state.database.clear_memory_embedding(&memory_id)?;
    task_runtime::start_memory_embedding(
        state.database.clone(),
        state.broker.clone(),
        &memory_id,
        &item.content,
        true,
    )?;
    state.database.memory_overview()
}

#[tauri::command]
pub(crate) fn start_memory_search(
    query: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<MemorySearchView, AppError> {
    let query = validated_text(&query, "La consulta", 500)?;
    task_runtime::start_memory_search(
        state.database.clone(),
        state.broker.clone(),
        &query,
        project_id.as_deref(),
    )
}

#[tauri::command]
pub(crate) fn get_memory_search(
    search_id: String,
    state: State<'_, AppState>,
) -> Result<MemorySearchView, AppError> {
    state.database.memory_search(&search_id)
}

#[tauri::command]
pub(crate) fn get_latest_memory_search(
    state: State<'_, AppState>,
) -> Result<Option<MemorySearchView>, AppError> {
    state.database.latest_memory_search()
}

#[tauri::command]
pub(crate) fn rename_project(
    project_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<ProjectSummary, AppError> {
    let name = validated_text(&name, "el nombre del proyecto", 120)?;
    state.database.rename_project(&project_id, &name)
}

#[tauri::command]
pub(crate) fn update_project_instructions(
    project_id: String,
    instructions: String,
    state: State<'_, AppState>,
) -> Result<ProjectSummary, AppError> {
    let instructions = instructions.trim();
    if instructions.chars().count() > 8_000 {
        return Err(AppError::Validation(
            "las instrucciones del proyecto superan el límite de 8.000 caracteres".to_owned(),
        ));
    }
    state.database.update_project_instructions(
        &project_id,
        (!instructions.is_empty()).then_some(instructions),
    )
}

#[tauri::command]
pub(crate) fn archive_project(
    project_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "archivar el proyecto requiere confirmación explícita".to_owned(),
        ));
    }
    state.database.archive_project(&project_id)
}
