//! Dialogos del sistema, exportacion e importacion de adjuntos.
//!
//! Todo lo que abre un dialogo del sistema esta junto: es la frontera por
//! la que entran y salen ficheros de la aplicacion.

use super::*;
use crate::*;

#[tauri::command]
pub(crate) fn pick_attachment_paths(extensions: Vec<String>) -> Result<Vec<String>, AppError> {
    #[cfg(target_os = "windows")]
    {
        use std::os::windows::process::CommandExt;
        let patterns = attachment_filter_patterns(&extensions);
        let script = format!(
            r#"
            Add-Type -AssemblyName System.Windows.Forms
            $dialog = New-Object System.Windows.Forms.OpenFileDialog
            $dialog.Multiselect = $true
            $dialog.Title = 'Seleccionar archivos para ChatyGPT'
            $dialog.Filter = 'Archivos compatibles|{patterns}|Todos los archivos|*.*'
            if ($dialog.ShowDialog() -eq [System.Windows.Forms.DialogResult]::OK) {{
                [Console]::OutputEncoding = [System.Text.Encoding]::UTF8
                $dialog.FileNames | ForEach-Object {{ [Console]::WriteLine($_) }}
            }}
        "#
        );
        let output = std::process::Command::new("powershell.exe")
            .args(["-NoProfile", "-NonInteractive", "-STA", "-Command", &script])
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

pub(crate) fn attachment_filter_patterns(extensions: &[String]) -> String {
    let mut safe = extensions
        .iter()
        .map(|value| value.trim().trim_start_matches('.').to_ascii_lowercase())
        .filter(|value| {
            !value.is_empty()
                && value.len() <= 16
                && value
                    .chars()
                    .all(|character| character.is_ascii_alphanumeric())
        })
        .collect::<Vec<_>>();
    safe.sort();
    safe.dedup();
    if safe.is_empty() {
        return "*.pdf;*.doc;*.docx;*.xls;*.xlsx;*.ppt;*.pptx;*.txt;*.md;*.csv;*.json;*.xml;*.html;*.htm;*.rtf;*.png;*.jpg;*.jpeg;*.gif;*.webp;*.bmp;*.tif;*.tiff;*.mp3;*.wav;*.m4a;*.mp4;*.mov;*.avi;*.webm;*.py;*.js;*.ts;*.tsx;*.jsx;*.rs;*.java;*.cs;*.cpp;*.c;*.h;*.sql".to_owned();
    }
    safe.into_iter()
        .map(|value| format!("*.{value}"))
        .collect::<Vec<_>>()
        .join(";")
}

#[tauri::command]
pub(crate) fn pick_export_path(
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
pub(crate) fn pick_scheduled_history_export_path(
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
pub(crate) fn pick_scheduled_calendar_export_path(
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
pub(crate) fn pick_obsidian_vault(state: State<'_, AppState>) -> Result<Option<String>, AppError> {
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
pub(crate) async fn export_conversation(
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
pub(crate) async fn export_scheduled_history(
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
pub(crate) async fn export_scheduled_calendar(
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
pub(crate) async fn export_conversation_to_obsidian(
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
pub(crate) async fn import_attachment(
    conversation_id: String,
    source_path: String,
    describe_images: bool,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    attachment_runtime::import_attachment(
        state.database.clone(),
        state.broker.clone(),
        state.attachments_dir.clone(),
        conversation_id,
        source_path,
        describe_images,
    )
    .await
}

#[tauri::command]
pub(crate) async fn import_captured_image(
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
pub(crate) fn list_attachments(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state.database.list_attachments(&conversation_id)
}

#[tauri::command]
pub(crate) fn list_project_files(
    conversation_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<AttachmentView>, AppError> {
    state.database.list_project_files(&conversation_id)
}

#[tauri::command]
pub(crate) fn set_project_file(
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
pub(crate) fn use_project_file(
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
pub(crate) fn remove_attachment(
    conversation_id: String,
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<(), AppError> {
    state
        .database
        .remove_conversation_attachment(&conversation_id, &attachment_id)
}

#[tauri::command]
pub(crate) fn retry_attachment(
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
pub(crate) fn retry_attachment_context(
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
pub(crate) async fn retry_attachment_semantic_index(
    attachment_id: String,
    state: State<'_, AppState>,
) -> Result<AttachmentView, AppError> {
    let dependencies_enabled = state
        .broker
        .capabilities()
        .await
        .is_ok_and(|capabilities| capabilities.task_dependencies);
    task_runtime::start_attachment_semantic_index(
        state.database.clone(),
        state.broker.clone(),
        &attachment_id,
        true,
        dependencies_enabled,
    )?;
    state.database.attachment_view(&attachment_id)
}
