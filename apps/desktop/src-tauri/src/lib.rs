mod attachment_runtime;
mod broker;
mod db;
mod error;
mod export;
mod task_runtime;

use broker::{BrokerClient, BrokerDiagnostic};
use db::{
    AttachmentView, AuditEventView, ConversationSummary, ConversationView, Database,
    LocalTaskSnapshot, MemoryOverview, MemorySearchView, ProjectSummary, RecoveryItemView,
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
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
struct BootstrapReport {
    app_version: &'static str,
    database_path: String,
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
        recovered_attachments: state.recovered_attachments_at_start,
        recovery_items: state.recovery_items_at_start.clone(),
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
fn list_audit_events(state: State<'_, AppState>) -> Result<Vec<AuditEventView>, AppError> {
    state.database.list_audit_events(50)
}

#[tauri::command]
fn get_memory_overview(state: State<'_, AppState>) -> Result<MemoryOverview, AppError> {
    state.database.memory_overview()
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
    attachment_ids: Vec<String>,
    tools_enabled: bool,
    sandbox_enabled: bool,
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
fn pick_export_path(suggested_name: String) -> Result<Option<ExportPathSelection>, AppError> {
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
        let selection = serde_json::from_str(raw)
            .map_err(|error| AppError::Validation(format!("selector inválido: {error}")))?;
        Ok(Some(selection))
    }
    #[cfg(not(target_os = "windows"))]
    Err(AppError::Validation(
        "la exportación nativa todavía solo está disponible en Windows".to_owned(),
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
fn list_attachments(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
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
            let recovery_items_at_start = database.recovery_candidates()?;
            let recovered_at_start =
                task_runtime::recover_at_start(database.clone(), broker.clone())?;
            let recovered_attachments_at_start =
                attachment_runtime::recover_at_start(database.clone(), broker.clone())?;
            let attachments_dir = data_dir.join("attachments");
            app.manage(AppState {
                database,
                broker,
                recovered_at_start,
                recovered_attachments_at_start,
                recovery_items_at_start,
                attachments_dir,
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
            resolve_tool_calls,
            create_project,
            list_projects,
            list_audit_events,
            get_memory_overview,
            set_memory_enabled,
            create_memory_item,
            set_memory_item_enabled,
            delete_memory_item,
            reindex_memory_item,
            start_memory_search,
            get_memory_search,
            get_latest_memory_search,
            rename_project,
            archive_project,
            pick_attachment_paths,
            import_attachment,
            list_attachments,
            remove_attachment,
            retry_attachment,
            pick_export_path,
            export_conversation
        ])
        .run(tauri::generate_context!())
        .expect("ChatyGPT no pudo iniciar");
}
