mod attachment_runtime;
mod broker;
mod db;
mod error;
mod export;
mod logging;
mod metrics;
mod research_tools;
mod scheduler_runtime;
mod secrets;
mod startup;
mod task_runtime;

use broker::{BrokerClient, BrokerDiagnostic};
use db::{
    AttachmentView, AuditEventView, AuthorizedFolderView, ContextSnapshotView,
    ConversationExecutionPreferences, ConversationSummary, ConversationSummaryOverview,
    ConversationView, CustomGptImportReport, CustomGptToolPermissions, CustomGptView, Database,
    LocalTaskSnapshot, MemoryOverview, MemorySearchView, ProjectKnowledgeOverview, ProjectSummary,
    RecoveryItemView, ScheduledRunPageView, ScheduledTaskTemplateView, ScheduledTaskView,
};
use error::AppError;
use serde::{Deserialize, Serialize};
use tauri::{Manager, State};

struct AppState {
    database: Database,
    broker: BrokerClient,
    recovered_at_start: usize,
    recovered_attachments_at_start: usize,
    recovery_items_at_start: Vec<RecoveryItemView>,
    attachments_dir: std::path::PathBuf,
    data_dir: std::path::PathBuf,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapReport {
    app_version: &'static str,
    database_path: String,
    /// Ruta del registro estructurado, para poder revisarlo sin buscarlo a mano.
    log_path: Option<String>,
    schema_version: i64,
    recovered_tasks: usize,
    recovered_attachments: usize,
    recovery_items: Vec<RecoveryItemView>,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
struct ExportPathSelection {
    path: String,
    existed: bool,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomGptExportReport {
    path: String,
    included_knowledge: usize,
    excluded_sensitive: usize,
    excluded_disabled: usize,
    excluded_files: usize,
}

/// Autoriza la carpeta que contiene un archivo recién elegido por la persona.
///
/// Elegir un destino en el selector nativo de Windows *es* la concesión: solo
/// desde ahí puede una carpeta pasar a estar autorizada para escritura.
fn authorize_selected_file(
    database: &Database,
    selected_file: &str,
    purpose: &str,
) -> Result<(), AppError> {
    let path = std::path::Path::new(selected_file);
    let folder = path.parent().ok_or_else(|| {
        AppError::Validation("el destino elegido no tiene carpeta contenedora".to_owned())
    })?;
    let display_name = folder.to_string_lossy().into_owned();
    database.authorize_folder(folder, &display_name, purpose)
}

#[tauri::command]
fn list_authorized_folders(
    state: State<'_, AppState>,
) -> Result<Vec<AuthorizedFolderView>, AppError> {
    state.database.list_authorized_folders()
}

#[tauri::command]
fn revoke_authorized_folder(
    folder_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<Vec<AuthorizedFolderView>, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "revocar una carpeta autorizada requiere confirmación".to_owned(),
        ));
    }
    state.database.revoke_authorized_folder(&folder_id)?;
    state.database.list_authorized_folders()
}

fn validated_text(value: &str, field: &str, maximum: usize) -> Result<String, AppError> {
    let value = value.trim();
    if value.is_empty() {
        return Err(AppError::Validation(format!(
            "{field} no puede estar vacío"
        )));
    }
    if value.chars().count() > maximum {
        return Err(AppError::Validation(format!(
            "{field} supera el límite de {maximum} caracteres"
        )));
    }
    Ok(value.to_owned())
}

#[tauri::command]
fn bootstrap_app(state: State<'_, AppState>) -> Result<BootstrapReport, AppError> {
    Ok(BootstrapReport {
        app_version: env!("CARGO_PKG_VERSION"),
        database_path: state.database.path().display().to_string(),
        log_path: logging::log_path().map(|path| path.display().to_string()),
        schema_version: state.database.schema_version()?,
        recovered_tasks: state.recovered_at_start,
        recovered_attachments: state.recovered_attachments_at_start,
        recovery_items: state.recovery_items_at_start.clone(),
    })
}

#[tauri::command]
async fn diagnose_broker(state: State<'_, AppState>) -> Result<BrokerDiagnostic, AppError> {
    Ok(state.broker.diagnose().await)
}

#[tauri::command]
fn get_broker_credential(
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    Ok(secrets::credential_status(&state.data_dir))
}

/// Guarda el token del Broker cifrado para esta cuenta de Windows.
///
/// El valor nunca se devuelve al frontend ni se persiste en SQLite: solo se
/// informa del estado resultante.
#[tauri::command]
fn set_broker_credential(
    token: String,
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    secrets::store_broker_token(&state.data_dir, &token)?;
    state.broker.replace_admin_token(Some(token.trim()))?;
    // Si el inicio con Windows está activo, su copia protegida se pone al día.
    let _ = startup::refresh_protected_token_if_enabled(&state.data_dir);
    state.database.record_broker_credential_changed(true)?;
    logging::info("broker.credential_stored", None, &[]);
    Ok(secrets::credential_status(&state.data_dir))
}

#[tauri::command]
fn clear_broker_credential(
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<secrets::BrokerCredentialStatus, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "retirar la credencial requiere confirmación".to_owned(),
        ));
    }
    secrets::clear_broker_token(&state.data_dir)?;
    // Tras retirarla, solo queda lo que aporte el entorno de este proceso.
    state
        .broker
        .replace_admin_token(secrets::environment_token().as_deref())?;
    state.database.record_broker_credential_changed(false)?;
    logging::info("broker.credential_cleared", None, &[]);
    Ok(secrets::credential_status(&state.data_dir))
}

#[tauri::command]
fn get_windows_startup_status(
    state: State<'_, AppState>,
) -> Result<startup::WindowsStartupStatus, AppError> {
    startup::status(&state.data_dir)
}

#[tauri::command]
fn set_windows_startup_enabled(
    enabled: bool,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<startup::WindowsStartupStatus, AppError> {
    let status = startup::set_enabled(
        &state.data_dir,
        &state.broker.base_url(),
        enabled,
        confirmed,
    )?;
    state
        .database
        .record_windows_startup_changed(status.enabled, status.credential_protected)?;
    Ok(status)
}

#[tauri::command]
async fn start_smoke_task(state: State<'_, AppState>) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::start_smoke_task(state.database.clone(), state.broker.clone()).await
}

#[tauri::command]
fn get_local_task(
    local_task_id: String,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    state.database.task_snapshot(&local_task_id)
}

#[tauri::command]
async fn cancel_local_task(
    local_task_id: String,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::cancel_task(state.database.clone(), state.broker.clone(), &local_task_id).await
}

#[tauri::command]
fn list_scheduled_tasks(state: State<'_, AppState>) -> Result<Vec<ScheduledTaskView>, AppError> {
    state.database.list_scheduled_tasks()
}

#[tauri::command]
fn list_scheduled_runs(
    scheduled_task_id: String,
    status_filter: String,
    period_filter: String,
    sort: String,
    page: i64,
    page_size: i64,
    state: State<'_, AppState>,
) -> Result<ScheduledRunPageView, AppError> {
    state.database.scheduled_run_page(
        &scheduled_task_id,
        status_filter.trim(),
        period_filter.trim(),
        sort.trim(),
        page,
        page_size,
    )
}

/// Registra duraciones observadas por la interfaz.
///
/// La orden acepta un lote porque la respuesta inmediata de la interfaz produce
/// una muestra por interacción: agruparlas evita que medir el rendimiento sea,
/// en sí mismo, un coste de rendimiento.
#[tauri::command]
fn record_performance_samples(
    metric: String,
    durations_ms: Vec<i64>,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    let retained = state
        .database
        .record_performance_samples(&metric, &durations_ms)?;
    logging::info(
        "performance.samples_recorded",
        None,
        &[
            ("metric", logging::code(&metric)),
            ("recorded", logging::count(durations_ms.len() as i64)),
            ("retained", logging::count(retained)),
        ],
    );
    Ok(())
}

#[tauri::command]
fn get_performance_report(
    state: State<'_, AppState>,
) -> Result<metrics::PerformanceReportView, AppError> {
    state.database.performance_report()
}

#[tauri::command]
fn clear_performance_samples(
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<metrics::PerformanceReportView, AppError> {
    state.database.clear_performance_samples(confirmed)?;
    state.database.performance_report()
}

#[tauri::command]
fn list_scheduled_task_templates(
    state: State<'_, AppState>,
) -> Result<Vec<ScheduledTaskTemplateView>, AppError> {
    state.database.list_scheduled_task_templates()
}

#[tauri::command]
fn create_scheduled_task_template(
    name: String,
    prompt: String,
    schedule_expression: String,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskTemplateView, AppError> {
    let name = validated_text(&name, "el nombre", 120)?;
    let prompt = validated_text(&prompt, "la instrucción", 20_000)?;
    state
        .database
        .create_scheduled_task_template(&name, &prompt, schedule_expression.trim())
}

#[tauri::command]
fn delete_scheduled_task_template(
    scheduled_task_template_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .database
        .delete_scheduled_task_template(&scheduled_task_template_id, confirmed)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn create_scheduled_task(
    name: String,
    conversation_id: String,
    prompt: String,
    due_at: String,
    timezone: String,
    schedule_expression: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let name = validated_text(&name, "el nombre", 120)?;
    let prompt = validated_text(&prompt, "la instrucción", 20_000)?;
    let timezone = validated_text(&timezone, "la zona horaria", 100)?;
    state.database.create_scheduled_task(
        &name,
        &conversation_id,
        &prompt,
        due_at.trim(),
        &timezone,
        schedule_expression.trim(),
        confirmed,
    )
}

#[tauri::command]
fn set_scheduled_task_enabled(
    scheduled_task_id: String,
    enabled: bool,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    state
        .database
        .set_scheduled_task_enabled(&scheduled_task_id, enabled, confirmed)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn update_scheduled_task(
    scheduled_task_id: String,
    name: String,
    conversation_id: String,
    prompt: String,
    due_at: String,
    timezone: String,
    schedule_expression: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let name = validated_text(&name, "el nombre", 120)?;
    let prompt = validated_text(&prompt, "la instrucción", 20_000)?;
    let timezone = validated_text(&timezone, "la zona horaria", 100)?;
    state.database.update_scheduled_task(
        &scheduled_task_id,
        &name,
        &conversation_id,
        &prompt,
        due_at.trim(),
        &timezone,
        schedule_expression.trim(),
        confirmed,
    )
}

#[tauri::command]
fn delete_scheduled_task(
    scheduled_task_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .database
        .delete_scheduled_task(&scheduled_task_id, confirmed)
}

#[tauri::command]
async fn retry_scheduled_run(
    scheduled_run_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let claim = state
        .database
        .retry_failed_scheduled_run(&scheduled_run_id, confirmed)?;
    match task_runtime::start_chat_turn(
        state.database.clone(),
        state.broker.clone(),
        &claim.conversation_id,
        &claim.prompt,
        &[],
        false,
        false,
        false,
        false,
    )
    .await
    {
        Ok(task) => state
            .database
            .start_scheduled_run(&claim.run_id, &task.id)?,
        Err(error) => {
            state
                .database
                .fail_scheduled_run(&claim.run_id, &error.to_string())?;
            return Err(error);
        }
    }
    state
        .database
        .list_scheduled_tasks()?
        .into_iter()
        .find(|task| task.id == claim.scheduled_task_id)
        .ok_or_else(|| AppError::NotFound("tarea programada reintentada".to_owned()))
}

#[tauri::command]
async fn run_scheduled_task_now(
    scheduled_task_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let claim = state
        .database
        .claim_scheduled_task_now(&scheduled_task_id, confirmed)?;
    match task_runtime::start_chat_turn(
        state.database.clone(),
        state.broker.clone(),
        &claim.conversation_id,
        &claim.prompt,
        &[],
        false,
        false,
        false,
        false,
    )
    .await
    {
        Ok(task) => state
            .database
            .start_scheduled_run(&claim.run_id, &task.id)?,
        Err(error) => {
            state
                .database
                .fail_scheduled_run(&claim.run_id, &error.to_string())?;
            return Err(error);
        }
    }
    state
        .database
        .list_scheduled_tasks()?
        .into_iter()
        .find(|task| task.id == claim.scheduled_task_id)
        .ok_or_else(|| AppError::NotFound("tarea programada iniciada manualmente".to_owned()))
}

#[tauri::command]
async fn cancel_scheduled_run(
    scheduled_run_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<ScheduledTaskView, AppError> {
    let target = state
        .database
        .scheduled_cancellation_target(&scheduled_run_id, confirmed)?;
    let cancelled = task_runtime::cancel_task(
        state.database.clone(),
        state.broker.clone(),
        &target.broker_task_id,
    )
    .await?;
    if cancelled.remote_status != "cancelled" {
        return Err(AppError::Conflict(
            "el Broker no confirmó la cancelación porque la ejecución cambió de estado".to_owned(),
        ));
    }
    state
        .database
        .finish_scheduled_cancellation(&scheduled_run_id, &target.broker_task_id)?;
    state
        .database
        .list_scheduled_tasks()?
        .into_iter()
        .find(|task| task.id == target.scheduled_task_id)
        .ok_or_else(|| AppError::NotFound("tarea programada cancelada".to_owned()))
}

#[tauri::command]
fn create_conversation(
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
fn list_conversations(state: State<'_, AppState>) -> Result<Vec<ConversationSummary>, AppError> {
    state.database.list_conversations()
}

#[tauri::command]
fn search_conversations(
    query: String,
    state: State<'_, AppState>,
) -> Result<Vec<ConversationSummary>, AppError> {
    let query = validated_text(&query, "la búsqueda", 200)?;
    state.database.search_conversations(&query, 50)
}

#[tauri::command]
fn rename_conversation(
    conversation_id: String,
    title: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, AppError> {
    let title = validated_text(&title, "el título", 120)?;
    state.database.rename_conversation(&conversation_id, &title)
}

#[tauri::command]
fn move_conversation(
    conversation_id: String,
    project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConversationSummary, AppError> {
    state
        .database
        .move_conversation(&conversation_id, project_id.as_deref())
}

#[tauri::command]
fn set_conversation_custom_gpt(
    conversation_id: String,
    custom_gpt_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<ConversationView, AppError> {
    state
        .database
        .set_conversation_custom_gpt(&conversation_id, custom_gpt_id.as_deref())
}

#[tauri::command]
fn archive_conversation(
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
fn delete_conversation(
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
fn create_project(
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
fn list_projects(state: State<'_, AppState>) -> Result<Vec<ProjectSummary>, AppError> {
    state.database.list_projects()
}

#[tauri::command]
fn list_custom_gpts(state: State<'_, AppState>) -> Result<Vec<CustomGptView>, AppError> {
    state.database.list_custom_gpts()
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn create_custom_gpt(
    name: String,
    description: Option<String>,
    instructions: String,
    conversation_starters: Vec<String>,
    tool_permissions: CustomGptToolPermissions,
    preferred_model: Option<String>,
    default_project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CustomGptView, AppError> {
    state.database.create_custom_gpt_with_starters(
        &name,
        description.as_deref(),
        &instructions,
        &conversation_starters,
        &tool_permissions,
        preferred_model.as_deref(),
        default_project_id.as_deref(),
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
fn update_custom_gpt(
    custom_gpt_id: String,
    name: String,
    description: Option<String>,
    instructions: String,
    conversation_starters: Vec<String>,
    tool_permissions: CustomGptToolPermissions,
    preferred_model: Option<String>,
    default_project_id: Option<String>,
    state: State<'_, AppState>,
) -> Result<CustomGptView, AppError> {
    state.database.update_custom_gpt_with_starters(
        &custom_gpt_id,
        &name,
        description.as_deref(),
        &instructions,
        &conversation_starters,
        &tool_permissions,
        preferred_model.as_deref(),
        default_project_id.as_deref(),
    )
}

#[tauri::command]
fn list_custom_gpt_versions(
    custom_gpt_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::CustomGptVersionView>, AppError> {
    state.database.list_custom_gpt_versions(&custom_gpt_id)
}

#[tauri::command]
fn restore_custom_gpt_version(
    custom_gpt_id: String,
    version_id: String,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<CustomGptView, AppError> {
    state
        .database
        .restore_custom_gpt_version(&custom_gpt_id, &version_id, confirmed)
}

/// Lo que recibiría el modelo si se usara este GPT, sin enviar nada.
#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct CustomGptPreview {
    custom_gpt_id: String,
    name: String,
    version_no: i64,
    /// Texto exacto que se antepone al mensaje, generado por el mismo código
    /// que construye la petición real.
    prompt_block: String,
    preferred_model: Option<String>,
    default_project_name: Option<String>,
    conversation_starters: Vec<String>,
    tool_permissions: CustomGptToolPermissions,
    active_knowledge_count: usize,
    disabled_knowledge_count: usize,
    sensitive_knowledge_count: usize,
    unindexed_knowledge_count: usize,
    ready_file_count: usize,
    pending_file_count: usize,
    /// Avisos accionables sobre lo que hoy no se usaría.
    warnings: Vec<String>,
}

/// Compone la vista previa de un GPT sin crear tareas ni generar coste.
#[tauri::command]
fn preview_custom_gpt(
    custom_gpt_id: String,
    state: State<'_, AppState>,
) -> Result<CustomGptPreview, AppError> {
    let context = state.database.custom_gpt_context(&custom_gpt_id)?;
    let view = state
        .database
        .list_custom_gpts()?
        .into_iter()
        .find(|item| item.id == custom_gpt_id)
        .ok_or_else(|| AppError::NotFound(format!("GPT personal {custom_gpt_id}")))?;
    let knowledge = state.database.custom_gpt_knowledge(&custom_gpt_id)?;
    let files = state.database.list_custom_gpt_files(&custom_gpt_id)?;

    let active_knowledge: Vec<_> = knowledge.iter().filter(|item| item.enabled).collect();
    let sensitive_knowledge_count = active_knowledge
        .iter()
        .filter(|item| item.sensitivity == "sensitive")
        .count();
    let unindexed_knowledge_count = active_knowledge
        .iter()
        .filter(|item| item.embedding_status != "ready")
        .count();
    let ready_file_count = files
        .iter()
        .filter(|file| file.ingestion_status == "ready" && file.context_status == "ready")
        .count();
    let pending_file_count = files.len() - ready_file_count;

    let default_project_name = view.default_project_id.as_deref().and_then(|project_id| {
        state
            .database
            .list_projects()
            .ok()?
            .into_iter()
            .find(|project| project.id == project_id)
            .map(|project| project.name)
    });

    let mut warnings = Vec::new();
    if active_knowledge.is_empty() && !knowledge.is_empty() {
        warnings.push(
            "Todo el conocimiento de este GPT está desactivado: hoy no se usaría ninguno."
                .to_owned(),
        );
    }
    if unindexed_knowledge_count > 0 {
        warnings.push(format!(
            "{unindexed_knowledge_count} dato(s) activos aún no están indexados y solo se \
             recuperarán por coincidencia literal."
        ));
    }
    if pending_file_count > 0 {
        warnings.push(format!(
            "{pending_file_count} archivo(s) todavía no están preparados y no se consultarán."
        ));
    }
    if sensitive_knowledge_count > 0 {
        warnings.push(format!(
            "{sensitive_knowledge_count} dato(s) marcados como sensibles obligan a mantener \
             la respuesta en local."
        ));
    }
    if view.default_project_id.is_some() && default_project_name.is_none() {
        warnings.push(
            "El proyecto predeterminado ya no existe; los chats nuevos quedarán sin proyecto."
                .to_owned(),
        );
    }
    if view.preferred_model.is_some() {
        warnings.push(
            "El modelo preferido es una preferencia: si no está disponible, el Broker elegirá otro."
                .to_owned(),
        );
    }

    Ok(CustomGptPreview {
        custom_gpt_id: context.custom_gpt_id.clone(),
        name: context.name.clone(),
        version_no: context.version_no,
        prompt_block: task_runtime::custom_gpt_prompt_block(&context)?,
        preferred_model: view.preferred_model,
        default_project_name,
        conversation_starters: view.conversation_starters,
        tool_permissions: view.tool_permissions,
        active_knowledge_count: active_knowledge.len(),
        disabled_knowledge_count: knowledge.len() - active_knowledge.len(),
        sensitive_knowledge_count,
        unindexed_knowledge_count,
        ready_file_count,
        pending_file_count,
        warnings,
    })
}

#[tauri::command]
fn duplicate_custom_gpt(
    custom_gpt_id: String,
    new_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<CustomGptView, AppError> {
    state
        .database
        .duplicate_custom_gpt(&custom_gpt_id, new_name.as_deref())
}

#[tauri::command]
fn pick_custom_gpt_import_path() -> Result<Option<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.OpenFileDialog
            $dialog.Title = 'Importar GPT personal en ChatyGPT'
            $dialog.Filter = 'Configuración de GPT|*.chatygpt.json;*.json|JSON|*.json'
            $dialog.Multiselect = $false
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [Console]::Write($dialog.FileName)
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        Ok((!path.is_empty()).then_some(path))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la importación nativa todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
fn pick_custom_gpt_export_path(
    suggested_name: String,
    state: State<'_, AppState>,
) -> Result<Option<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let safe_name: String = suggested_name
            .chars()
            .map(|character| match character {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                '\r' | '\n' => ' ',
                other => other,
            })
            .take(80)
            .collect();
        let filename = format!(
            "{}.chatygpt.json",
            if safe_name.trim().is_empty() {
                "gpt-personal"
            } else {
                safe_name.trim()
            }
        );
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.SaveFileDialog
            $dialog.Title = 'Exportar GPT personal de ChatyGPT'
            $dialog.Filter = 'Configuración de GPT|*.chatygpt.json|JSON|*.json'
            $dialog.DefaultExt = 'json'
            $dialog.AddExtension = $true
            $dialog.OverwritePrompt = $true
            $dialog.FileName = $env:CHATYGPT_GPT_EXPORT_NAME
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [Console]::Write($dialog.FileName)
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .env("CHATYGPT_GPT_EXPORT_NAME", filename)
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if path.is_empty() {
            return Ok(None);
        }
        authorize_selected_file(&state.database, &path, "custom_gpt_export")?;
        Ok(Some(path))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la exportación nativa todavía solo está disponible en Windows".to_owned(),
    ))
}

fn validated_custom_gpt_json_path(raw: &str) -> Result<std::path::PathBuf, AppError> {
    let path = std::path::PathBuf::from(raw);
    if !path.is_absolute() {
        return Err(AppError::Validation(
            "la ruta del archivo del GPT debe ser absoluta".to_owned(),
        ));
    }
    if path
        .extension()
        .and_then(|value| value.to_str())
        .is_none_or(|extension| !extension.eq_ignore_ascii_case("json"))
    {
        return Err(AppError::Validation(
            "la configuración del GPT debe usar extensión .json".to_owned(),
        ));
    }
    Ok(path)
}

#[tauri::command]
fn export_custom_gpt(
    custom_gpt_id: String,
    destination_path: String,
    include_knowledge: bool,
    state: State<'_, AppState>,
) -> Result<CustomGptExportReport, AppError> {
    let path = validated_custom_gpt_json_path(&destination_path)?;
    let export = state
        .database
        .export_custom_gpt_portable(&custom_gpt_id, include_knowledge)?;
    std::fs::write(&path, export.json.as_bytes())
        .map_err(|error| AppError::DataDirectory(format!("no se pudo exportar el GPT: {error}")))?;
    state
        .database
        .record_custom_gpt_exported(&custom_gpt_id, export.included_knowledge)?;
    Ok(CustomGptExportReport {
        path: path.display().to_string(),
        included_knowledge: export.included_knowledge,
        excluded_sensitive: export.excluded_sensitive,
        excluded_disabled: export.excluded_disabled,
        excluded_files: export.excluded_files,
    })
}

#[tauri::command]
fn import_custom_gpt(
    source_path: String,
    state: State<'_, AppState>,
) -> Result<CustomGptImportReport, AppError> {
    let path = validated_custom_gpt_json_path(&source_path)?;
    let metadata = std::fs::metadata(&path)
        .map_err(|error| AppError::Validation(format!("archivo de GPT no accesible: {error}")))?;
    if metadata.len() > 256_000 {
        return Err(AppError::Validation(
            "el archivo del GPT supera el límite de 256 KB".to_owned(),
        ));
    }
    let json = std::fs::read_to_string(&path)
        .map_err(|error| AppError::Validation(format!("no se pudo leer el GPT: {error}")))?;
    state.database.import_custom_gpt_package_json(&json)
}

#[tauri::command]
fn get_project_knowledge(
    project_id: String,
    state: State<'_, AppState>,
) -> Result<ProjectKnowledgeOverview, AppError> {
    state.database.project_knowledge_overview(&project_id)
}

#[tauri::command]
fn remove_project_file(
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
fn set_project_memory_item_enabled(
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
fn list_audit_events(state: State<'_, AppState>) -> Result<Vec<AuditEventView>, AppError> {
    state.database.list_audit_events(50)
}

#[tauri::command]
fn get_memory_overview(state: State<'_, AppState>) -> Result<MemoryOverview, AppError> {
    state.database.memory_overview()
}

#[tauri::command]
fn get_custom_gpt_knowledge(
    custom_gpt_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::MemoryItemView>, AppError> {
    state.database.custom_gpt_knowledge(&custom_gpt_id)
}

#[tauri::command]
fn list_custom_gpt_files(
    custom_gpt_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state.database.list_custom_gpt_files(&custom_gpt_id)
}

#[tauri::command]
async fn import_custom_gpt_file(
    custom_gpt_id: String,
    source_path: String,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    attachment_runtime::import_custom_gpt_attachment(
        state.database.clone(),
        state.broker.clone(),
        state.attachments_dir.clone(),
        custom_gpt_id,
        source_path,
    )
    .await
}

#[tauri::command]
fn remove_custom_gpt_file(
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
fn create_custom_gpt_knowledge_item(
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
fn set_custom_gpt_knowledge_item_enabled(
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
fn delete_custom_gpt_knowledge_item(
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
fn reindex_custom_gpt_knowledge_item(
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
fn set_memory_enabled(
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    state.database.set_memory_enabled(enabled)
}

#[tauri::command]
fn create_memory_item(
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
fn update_memory_item(
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
fn set_memory_item_enabled(
    memory_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<MemoryOverview, AppError> {
    state.database.set_memory_item_enabled(&memory_id, enabled)
}

#[tauri::command]
fn delete_memory_item(
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
fn reindex_memory_item(
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
fn start_memory_search(
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
fn get_memory_search(
    search_id: String,
    state: State<'_, AppState>,
) -> Result<MemorySearchView, AppError> {
    state.database.memory_search(&search_id)
}

#[tauri::command]
fn get_latest_memory_search(
    state: State<'_, AppState>,
) -> Result<Option<MemorySearchView>, AppError> {
    state.database.latest_memory_search()
}

#[tauri::command]
fn rename_project(
    project_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<ProjectSummary, AppError> {
    let name = validated_text(&name, "el nombre del proyecto", 120)?;
    state.database.rename_project(&project_id, &name)
}

#[tauri::command]
fn update_project_instructions(
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
fn archive_project(
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

#[tauri::command]
fn get_conversation(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationView, AppError> {
    state.database.conversation_view(&conversation_id)
}

#[tauri::command]
fn update_conversation_execution_preferences(
    conversation_id: String,
    preferences: ConversationExecutionPreferences,
    state: State<'_, AppState>,
) -> Result<ConversationExecutionPreferences, AppError> {
    state
        .database
        .update_conversation_execution_preferences(&conversation_id, &preferences)
}

#[tauri::command]
fn get_task_context(
    local_task_id: String,
    state: State<'_, AppState>,
) -> Result<ContextSnapshotView, AppError> {
    state.database.task_context(&local_task_id)
}

fn validated_managed_source_path(
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
fn reveal_context_source(
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
fn get_conversation_summary(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummaryOverview, AppError> {
    state
        .database
        .conversation_summary_overview(&conversation_id)
}

#[tauri::command]
fn start_conversation_summary(
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
fn update_conversation_summary(
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
fn approve_conversation_summary(
    summary_id: String,
    state: State<'_, AppState>,
) -> Result<ConversationSummaryOverview, AppError> {
    state.database.approve_conversation_summary(&summary_id)
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
async fn send_chat_turn(
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
fn resolve_tool_calls(
    local_task_id: String,
    decisions: Vec<task_runtime::ToolDecision>,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::resolve_tool_calls(
        state.database.clone(),
        state.broker.clone(),
        &local_task_id,
        &decisions,
    )
}

#[tauri::command]
fn pick_attachment_paths() -> Result<Vec<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.OpenFileDialog
            $dialog.Multiselect = $true
            $dialog.Title = 'Seleccionar archivos para ChatyGPT'
            $dialog.Filter = 'Archivos compatibles|*.pdf;*.doc;*.docx;*.xls;*.xlsx;*.ppt;*.pptx;*.txt;*.md;*.csv;*.json;*.xml;*.html;*.htm;*.rtf;*.png;*.jpg;*.jpeg;*.gif;*.webp;*.bmp;*.tif;*.tiff;*.mp3;*.wav;*.m4a;*.mp4;*.mov;*.avi;*.webm;*.py;*.js;*.ts;*.tsx;*.jsx;*.rs;*.java;*.cs;*.cpp;*.c;*.h;*.sql|Todos los archivos|*.*'
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                $dialog.FileNames | ForEach-Object { [Console]::WriteLine($_) }
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        Ok(String::from_utf8_lossy(&output.stdout)
            .lines()
            .map(str::trim)
            .filter(|line| !line.is_empty())
            .map(str::to_owned)
            .collect())
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "el selector nativo todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
fn pick_export_path(
    suggested_name: String,
    state: State<'_, AppState>,
) -> Result<Option<ExportPathSelection>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let safe_name: String = suggested_name
            .chars()
            .map(|character| match character {
                '<' | '>' | ':' | '"' | '/' | '\\' | '|' | '?' | '*' => '_',
                '\r' | '\n' => ' ',
                other => other,
            })
            .take(100)
            .collect();
        let filename = if safe_name.trim().is_empty() {
            "conversacion.md".to_owned()
        } else if safe_name.to_ascii_lowercase().ends_with(".md") {
            safe_name
        } else {
            format!("{}.md", safe_name.trim())
        };
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.SaveFileDialog
            $dialog.Title = 'Exportar conversación de ChatyGPT'
            $dialog.Filter = 'Markdown|*.md|Markdown largo|*.markdown'
            $dialog.DefaultExt = 'md'
            $dialog.AddExtension = $true
            $dialog.OverwritePrompt = $true
            $dialog.FileName = $env:CHATYGPT_EXPORT_NAME
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [pscustomobject]@{
                    path = $dialog.FileName
                    existed = [System.IO.File]::Exists($dialog.FileName)
                } | ConvertTo-Json -Compress
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .env("CHATYGPT_EXPORT_NAME", filename)
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let raw = String::from_utf8_lossy(&output.stdout);
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(None);
        }
        let selection: ExportPathSelection = serde_json::from_str(raw)
            .map_err(|error| AppError::Validation(format!("selector inválido: {error}")))?;
        authorize_selected_file(&state.database, &selection.path, "conversation_markdown")?;
        Ok(Some(selection))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la exportación nativa todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
fn pick_scheduled_history_export_path(
    state: State<'_, AppState>,
) -> Result<Option<ExportPathSelection>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.SaveFileDialog
            $dialog.Title = 'Exportar historial de automatizaciones'
            $dialog.Filter = 'Archivo de texto|*.txt'
            $dialog.DefaultExt = 'txt'
            $dialog.AddExtension = $true
            $dialog.OverwritePrompt = $true
            $dialog.FileName = 'historial-automatizaciones.txt'
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [pscustomobject]@{
                    path = $dialog.FileName
                    existed = [System.IO.File]::Exists($dialog.FileName)
                } | ConvertTo-Json -Compress
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let raw = String::from_utf8_lossy(&output.stdout);
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(None);
        }
        let selection: ExportPathSelection = serde_json::from_str(raw)
            .map_err(|error| AppError::Validation(format!("selector inválido: {error}")))?;
        authorize_selected_file(&state.database, &selection.path, "scheduled_history")?;
        Ok(Some(selection))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la exportación nativa todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
fn pick_scheduled_calendar_export_path(
    state: State<'_, AppState>,
) -> Result<Option<ExportPathSelection>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.SaveFileDialog
            $dialog.Title = 'Exportar calendario de automatizaciones'
            $dialog.Filter = 'Calendario iCalendar|*.ics'
            $dialog.DefaultExt = 'ics'
            $dialog.AddExtension = $true
            $dialog.OverwritePrompt = $true
            $dialog.FileName = 'automatizaciones-chatygpt.ics'
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [pscustomobject]@{
                    path = $dialog.FileName
                    existed = [System.IO.File]::Exists($dialog.FileName)
                } | ConvertTo-Json -Compress
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let raw = String::from_utf8_lossy(&output.stdout);
        let raw = raw.trim();
        if raw.is_empty() {
            return Ok(None);
        }
        let selection: ExportPathSelection = serde_json::from_str(raw)
            .map_err(|error| AppError::Validation(format!("selector inválido: {error}")))?;
        authorize_selected_file(&state.database, &selection.path, "scheduled_calendar")?;
        Ok(Some(selection))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la exportación nativa todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
fn pick_obsidian_vault(state: State<'_, AppState>) -> Result<Option<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let script = r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.FolderBrowserDialog
            $dialog.Description = 'Elige tu bóveda de Obsidian o una carpeta para crearla'
            $dialog.ShowNewFolderButton = $true
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                [Console]::Write($dialog.SelectedPath)
            }
        "#;
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", script])
            .creation_flags(0x0800_0000)
            .output()
            .map_err(|error| {
                AppError::Validation(format!("no se pudo abrir el selector: {error}"))
            })?;
        if !output.status.success() {
            return Err(AppError::Validation(
                String::from_utf8_lossy(&output.stderr).trim().to_owned(),
            ));
        }
        let path = String::from_utf8_lossy(&output.stdout).trim().to_owned();
        if path.is_empty() {
            return Ok(None);
        }
        // La bóveda se autoriza entera: su proyección crea subcarpetas dentro.
        state
            .database
            .authorize_folder(std::path::Path::new(&path), &path, "obsidian_vault")?;
        Ok(Some(path))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la selección de la bóveda todavía solo está disponible en Windows".to_owned(),
    ))
}

#[tauri::command]
async fn export_conversation(
    conversation_id: String,
    destination_path: String,
    overwrite_confirmed: bool,
    state: State<'_, AppState>,
) -> Result<export::ExportReport, AppError> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        export::export_conversation(
            database,
            &conversation_id,
            &destination_path,
            overwrite_confirmed,
        )
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))?
}

#[tauri::command]
async fn export_scheduled_history(
    destination_path: String,
    status_filter: String,
    period_filter: String,
    overwrite_confirmed: bool,
    state: State<'_, AppState>,
) -> Result<export::ScheduledHistoryExportReport, AppError> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        export::export_scheduled_history(
            database,
            &destination_path,
            &status_filter,
            &period_filter,
            overwrite_confirmed,
        )
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))?
}

#[tauri::command]
async fn export_scheduled_calendar(
    destination_path: String,
    entries: Vec<export::ScheduledCalendarExportEntry>,
    range_days: u8,
    overwrite_confirmed: bool,
    state: State<'_, AppState>,
) -> Result<export::ScheduledCalendarExportReport, AppError> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        export::export_scheduled_calendar(
            database,
            &destination_path,
            &entries,
            range_days,
            overwrite_confirmed,
        )
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))?
}

#[tauri::command]
async fn export_conversation_to_obsidian(
    conversation_id: String,
    vault_path: String,
    overwrite_confirmed: bool,
    state: State<'_, AppState>,
) -> Result<export::ExportReport, AppError> {
    let database = state.database.clone();
    tauri::async_runtime::spawn_blocking(move || {
        export::export_conversation_to_obsidian(
            database,
            &conversation_id,
            &vault_path,
            overwrite_confirmed,
        )
    })
    .await
    .map_err(|error| AppError::DataDirectory(error.to_string()))?
}

#[tauri::command]
async fn import_attachment(
    conversation_id: String,
    source_path: String,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    attachment_runtime::import_attachment(
        state.database.clone(),
        state.broker.clone(),
        state.attachments_dir.clone(),
        conversation_id,
        source_path,
    )
    .await
}

#[tauri::command]
async fn import_captured_image(
    conversation_id: String,
    display_name: String,
    bytes: Vec<u8>,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    attachment_runtime::import_captured_image(
        state.database.clone(),
        state.broker.clone(),
        state.attachments_dir.clone(),
        conversation_id,
        display_name,
        bytes,
    )
    .await
}

#[tauri::command]
fn list_attachments(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state.database.list_attachments(&conversation_id)
}

#[tauri::command]
fn list_project_files(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state.database.list_project_files(&conversation_id)
}

#[tauri::command]
fn set_project_file(
    conversation_id: String,
    attachment_id: String,
    enabled: bool,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state
        .database
        .set_project_file(&conversation_id, &attachment_id, enabled)?;
    state.database.list_project_files(&conversation_id)
}

#[tauri::command]
fn use_project_file(
    conversation_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state
        .database
        .use_project_file(&conversation_id, &attachment_id)?;
    state.database.list_attachments(&conversation_id)
}

#[tauri::command]
fn remove_attachment(
    conversation_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .database
        .remove_conversation_attachment(&conversation_id, &attachment_id)
}

#[tauri::command]
fn retry_attachment(
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    attachment_runtime::retry_attachment(
        state.database.clone(),
        state.broker.clone(),
        &attachment_id,
    )
}

#[tauri::command]
fn retry_attachment_context(
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    attachment_runtime::retry_attachment_context(
        state.database.clone(),
        state.broker.clone(),
        &attachment_id,
    )
}

#[tauri::command]
fn retry_attachment_semantic_index(
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    task_runtime::start_attachment_semantic_index(
        state.database.clone(),
        state.broker.clone(),
        &attachment_id,
        true,
    )?;
    state.database.attachment_view(&attachment_id)
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .setup(|app| {
            let data_dir = app
                .path()
                .app_local_data_dir()
                .map_err(|error| AppError::DataDirectory(error.to_string()))?;
            std::fs::create_dir_all(&data_dir)
                .map_err(|error| AppError::DataDirectory(error.to_string()))?;
            // El registro se prepara antes que nada para que un fallo posterior
            // de migración o de recuperación deje rastro observable.
            let _ = logging::init(&data_dir);
            let boot = logging::new_correlation_id();
            let _ = startup::refresh_protected_token_if_enabled(&data_dir);
            let database = Database::open(data_dir.join("chatygpt.db")).inspect_err(|error| {
                logging::error(
                    "app.database_failed",
                    Some(&boot),
                    &[("error_kind", logging::error_kind(error))],
                );
            })?;
            let broker = BrokerClient::bootstrap(&data_dir)?;
            let recovery_items_at_start = database.recovery_candidates()?;
            let recovered_at_start =
                task_runtime::recover_at_start(database.clone(), broker.clone())?;
            let recovered_attachments_at_start =
                attachment_runtime::recover_at_start(database.clone(), broker.clone())?;
            logging::info(
                "app.started",
                Some(&boot),
                &[
                    (
                        "schema_version",
                        logging::count(database.schema_version().unwrap_or(-1)),
                    ),
                    ("recovered_tasks", logging::count(recovered_at_start as i64)),
                    (
                        "recovered_attachments",
                        logging::count(recovered_attachments_at_start as i64),
                    ),
                ],
            );
            scheduler_runtime::start(database.clone(), broker.clone());
            let attachments_dir = data_dir.join("attachments");
            app.manage(AppState {
                database,
                broker,
                recovered_at_start,
                recovered_attachments_at_start,
                recovery_items_at_start,
                attachments_dir,
                data_dir,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap_app,
            diagnose_broker,
            get_windows_startup_status,
            set_windows_startup_enabled,
            get_broker_credential,
            set_broker_credential,
            clear_broker_credential,
            start_smoke_task,
            get_local_task,
            cancel_local_task,
            record_performance_samples,
            get_performance_report,
            clear_performance_samples,
            list_scheduled_tasks,
            list_scheduled_runs,
            list_scheduled_task_templates,
            create_scheduled_task_template,
            delete_scheduled_task_template,
            create_scheduled_task,
            set_scheduled_task_enabled,
            update_scheduled_task,
            delete_scheduled_task,
            retry_scheduled_run,
            run_scheduled_task_now,
            cancel_scheduled_run,
            create_conversation,
            list_conversations,
            search_conversations,
            rename_conversation,
            move_conversation,
            set_conversation_custom_gpt,
            archive_conversation,
            delete_conversation,
            get_conversation,
            update_conversation_execution_preferences,
            get_task_context,
            reveal_context_source,
            get_conversation_summary,
            start_conversation_summary,
            update_conversation_summary,
            approve_conversation_summary,
            send_chat_turn,
            resolve_tool_calls,
            create_project,
            list_projects,
            list_custom_gpts,
            create_custom_gpt,
            update_custom_gpt,
            list_custom_gpt_versions,
            restore_custom_gpt_version,
            duplicate_custom_gpt,
            preview_custom_gpt,
            pick_custom_gpt_import_path,
            pick_custom_gpt_export_path,
            import_custom_gpt,
            export_custom_gpt,
            get_project_knowledge,
            remove_project_file,
            set_project_memory_item_enabled,
            list_audit_events,
            list_authorized_folders,
            revoke_authorized_folder,
            get_memory_overview,
            get_custom_gpt_knowledge,
            list_custom_gpt_files,
            import_custom_gpt_file,
            remove_custom_gpt_file,
            create_custom_gpt_knowledge_item,
            set_custom_gpt_knowledge_item_enabled,
            delete_custom_gpt_knowledge_item,
            reindex_custom_gpt_knowledge_item,
            set_memory_enabled,
            create_memory_item,
            update_memory_item,
            set_memory_item_enabled,
            delete_memory_item,
            reindex_memory_item,
            start_memory_search,
            get_memory_search,
            get_latest_memory_search,
            rename_project,
            update_project_instructions,
            archive_project,
            pick_attachment_paths,
            import_attachment,
            import_captured_image,
            list_attachments,
            list_project_files,
            set_project_file,
            use_project_file,
            remove_attachment,
            retry_attachment,
            retry_attachment_context,
            retry_attachment_semantic_index,
            pick_export_path,
            pick_scheduled_history_export_path,
            pick_scheduled_calendar_export_path,
            pick_obsidian_vault,
            export_conversation,
            export_scheduled_history,
            export_scheduled_calendar,
            export_conversation_to_obsidian
        ])
        .run(tauri::generate_context!())
        .expect("ChatyGPT no pudo iniciar");
}

#[cfg(test)]
mod tests {
    use super::validated_managed_source_path;
    use crate::error::AppError;
    use std::fs;
    use uuid::Uuid;

    #[test]
    fn context_source_path_must_exist_inside_managed_storage() {
        let base = std::env::temp_dir().join(format!(
            "chatygpt-source-path-test-{}",
            Uuid::new_v4().simple()
        ));
        let managed = base.join("attachments");
        let source = managed.join("hash").join("documento.pdf");
        let outside = base.join("fuera.pdf");
        fs::create_dir_all(source.parent().expect("source parent should exist"))
            .expect("managed directory should exist");
        fs::write(&source, b"document").expect("managed source should exist");
        fs::write(&outside, b"outside").expect("outside source should exist");

        assert_eq!(
            validated_managed_source_path(&managed, &source)
                .expect("managed source should validate"),
            source.canonicalize().expect("source should canonicalize")
        );
        assert!(matches!(
            validated_managed_source_path(&managed, &outside),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validated_managed_source_path(&managed, &managed.join("missing.pdf")),
            Err(AppError::NotFound(_))
        ));
        fs::remove_dir_all(base).expect("test directory should be removed");
    }
}
