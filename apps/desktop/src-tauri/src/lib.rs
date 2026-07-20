mod broker;
mod db;
mod error;
mod task_runtime;

use broker::{BrokerClient, BrokerDiagnostic};
use db::{ConversationSummary, ConversationView, Database, LocalTaskSnapshot, ProjectSummary};
use error::AppError;
use serde::Serialize;
use tauri::{Manager, State};

struct AppState {
    database: Database,
    broker: BrokerClient,
    recovered_at_start: usize,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapReport {
    app_version: &'static str,
    database_path: String,
    schema_version: i64,
    recovered_tasks: usize,
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
        schema_version: state.database.schema_version()?,
        recovered_tasks: state.recovered_at_start,
    })
}

#[tauri::command]
async fn diagnose_broker(state: State<'_, AppState>) -> Result<BrokerDiagnostic, AppError> {
    Ok(state.broker.diagnose().await)
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
fn rename_project(
    project_id: String,
    name: String,
    state: State<'_, AppState>,
) -> Result<ProjectSummary, AppError> {
    let name = validated_text(&name, "el nombre del proyecto", 120)?;
    state.database.rename_project(&project_id, &name)
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
async fn send_chat_turn(
    conversation_id: String,
    text: String,
    state: State<'_, AppState>,
) -> Result<LocalTaskSnapshot, AppError> {
    task_runtime::start_chat_turn(
        state.database.clone(),
        state.broker.clone(),
        &conversation_id,
        &text,
    )
    .await
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
            let database = Database::open(data_dir.join("chatygpt.db"))?;
            let broker = BrokerClient::from_environment()?;
            let recovered_at_start =
                task_runtime::recover_at_start(database.clone(), broker.clone())?;
            app.manage(AppState {
                database,
                broker,
                recovered_at_start,
            });
            Ok(())
        })
        .invoke_handler(tauri::generate_handler![
            bootstrap_app,
            diagnose_broker,
            start_smoke_task,
            get_local_task,
            cancel_local_task,
            create_conversation,
            list_conversations,
            search_conversations,
            rename_conversation,
            move_conversation,
            archive_conversation,
            delete_conversation,
            get_conversation,
            send_chat_turn,
            create_project,
            list_projects,
            rename_project,
            archive_project
        ])
        .run(tauri::generate_context!())
        .expect("ChatyGPT no pudo iniciar");
}
