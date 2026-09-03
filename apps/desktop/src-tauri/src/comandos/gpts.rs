//! GPTs personalizados: versiones, acciones de API, vista previa y portabilidad.

use super::*;
use crate::*;

#[tauri::command]
pub(crate) fn list_custom_gpts(state: State<'_, AppState>) -> Result<Vec<CustomGptView>, AppError> {
    state.database.list_custom_gpts()
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn create_custom_gpt(
    name: String,
    description: Option<String>,
    icon_ref: Option<String>,
    instructions: String,
    conversation_starters: Vec<String>,
    tool_permissions: CustomGptToolPermissions,
    preferred_model: Option<String>,
    default_project_id: Option<String>,
    execution_profile: Option<db::ConversationExecutionPreferences>,
    context_profile: String,
    api_actions: Vec<db::CustomGptApiAction>,
    state: State<'_, AppState>,
) -> Result<CustomGptView, AppError> {
    state.database.create_custom_gpt_with_api_actions(
        &name,
        description.as_deref(),
        icon_ref.as_deref(),
        &instructions,
        &conversation_starters,
        &tool_permissions,
        preferred_model.as_deref(),
        default_project_id.as_deref(),
        execution_profile.as_ref(),
        Some(&context_profile),
        &api_actions,
    )
}

#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub(crate) fn update_custom_gpt(
    custom_gpt_id: String,
    name: String,
    description: Option<String>,
    icon_ref: Option<String>,
    instructions: String,
    conversation_starters: Vec<String>,
    tool_permissions: CustomGptToolPermissions,
    preferred_model: Option<String>,
    default_project_id: Option<String>,
    execution_profile: Option<db::ConversationExecutionPreferences>,
    context_profile: String,
    api_actions: Vec<db::CustomGptApiAction>,
    state: State<'_, AppState>,
) -> Result<CustomGptView, AppError> {
    state.database.update_custom_gpt_with_api_actions(
        &custom_gpt_id,
        &name,
        description.as_deref(),
        icon_ref.as_deref(),
        &instructions,
        &conversation_starters,
        &tool_permissions,
        preferred_model.as_deref(),
        default_project_id.as_deref(),
        execution_profile.as_ref(),
        Some(&context_profile),
        &api_actions,
    )
}

#[tauri::command]
pub(crate) fn list_custom_gpt_versions(
    custom_gpt_id: String,
    state: State<'_, AppState>,
) -> Result<Vec<db::CustomGptVersionView>, AppError> {
    state.database.list_custom_gpt_versions(&custom_gpt_id)
}

#[tauri::command]
pub(crate) fn restore_custom_gpt_version(
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
pub(crate) struct CustomGptPreview {
    pub(crate) custom_gpt_id: String,
    pub(crate) name: String,
    pub(crate) icon_ref: String,
    pub(crate) version_no: i64,
    /// Texto exacto que se antepone al mensaje, generado por el mismo código
    /// que construye la petición real.
    pub(crate) prompt_block: String,
    pub(crate) preferred_model: Option<String>,
    pub(crate) execution_profile: Option<db::ConversationExecutionPreferences>,
    pub(crate) context_profile: String,
    pub(crate) default_project_name: Option<String>,
    pub(crate) conversation_starters: Vec<String>,
    pub(crate) tool_permissions: CustomGptToolPermissions,
    pub(crate) active_knowledge_count: usize,
    pub(crate) disabled_knowledge_count: usize,
    pub(crate) sensitive_knowledge_count: usize,
    pub(crate) unindexed_knowledge_count: usize,
    pub(crate) ready_file_count: usize,
    pub(crate) pending_file_count: usize,
    /// Avisos accionables sobre lo que hoy no se usaría.
    pub(crate) warnings: Vec<String>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CustomGptApiActionPreview {
    pub(crate) final_url: String,
    pub(crate) destination: String,
    pub(crate) method: &'static str,
    pub(crate) data_sent: Vec<ApiActionPreviewValue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CustomGptApiActionTestResult {
    pub(crate) final_url: String,
    pub(crate) destination: String,
    pub(crate) status: u16,
    pub(crate) content_type: Option<String>,
    pub(crate) body: String,
    pub(crate) truncated: bool,
    pub(crate) duration_ms: u128,
}

#[derive(Debug, Serialize)]
pub(crate) struct ApiActionPreviewValue {
    pub(crate) name: String,
    pub(crate) value: serde_json::Value,
}

#[tauri::command]
pub(crate) fn preview_custom_gpt_api_action(
    action: db::CustomGptApiAction,
    sample_values: serde_json::Value,
) -> Result<CustomGptApiActionPreview, AppError> {
    let action = db::validated_custom_gpt_api_action(&action)?;
    let action_json =
        serde_json::to_value(&action).map_err(|error| AppError::Validation(error.to_string()))?;
    let mut arguments = sample_values
        .as_object()
        .cloned()
        .ok_or_else(|| AppError::Validation("los valores de prueba no son válidos".to_owned()))?;
    arguments.insert("url".to_owned(), serde_json::json!(action.url));
    if let Some(credential_ref) = &action.credential_ref {
        arguments.insert(
            "credential_ref".to_owned(),
            serde_json::json!(credential_ref),
        );
        arguments.insert("auth_mode".to_owned(), serde_json::json!(action.auth_mode));
    }
    let final_url = task_runtime::configured_api_url(
        &action_json,
        &serde_json::Value::Object(arguments.clone()),
    )?;
    let destination = url::Url::parse(&final_url)
        .ok()
        .and_then(|url| url.host_str().map(str::to_owned))
        .ok_or_else(|| AppError::Validation("el destino final no es válido".to_owned()))?;
    let data_sent = arguments
        .into_iter()
        .filter(|(key, _)| key != "url" && key != "credential_ref" && key != "auth_mode")
        .map(|(name, value)| ApiActionPreviewValue { name, value })
        .collect();
    Ok(CustomGptApiActionPreview {
        final_url,
        destination,
        method: "GET",
        data_sent,
    })
}

/// Ejecuta una prueba explícita de la acción que se está editando. La URL se
/// recompone con el mismo código que usa la ejecución real y la confirmación
/// viaja también al backend para que la interfaz no pueda omitirla por error.
#[tauri::command]
pub(crate) async fn test_custom_gpt_api_action(
    action: db::CustomGptApiAction,
    sample_values: serde_json::Value,
    confirmed: bool,
    state: State<'_, AppState>,
) -> Result<CustomGptApiActionTestResult, AppError> {
    test_custom_gpt_api_action_impl(action, sample_values, confirmed, &state.data_dir).await
}

pub(crate) async fn test_custom_gpt_api_action_impl(
    action: db::CustomGptApiAction,
    sample_values: serde_json::Value,
    confirmed: bool,
    data_dir: &std::path::Path,
) -> Result<CustomGptApiActionTestResult, AppError> {
    if !confirmed {
        return Err(AppError::Validation(
            "debes confirmar la conexión de prueba antes de abrir la API".to_owned(),
        ));
    }
    let preview = preview_custom_gpt_api_action(action.clone(), sample_values)?;
    let authentication = match action.auth_mode.as_str() {
        "none" => None,
        mode @ ("bearer" | "api_key") => {
            let credential_ref = action.credential_ref.as_deref().ok_or_else(|| {
                AppError::Validation("la acción API no indica su credencial".to_owned())
            })?;
            let secret =
                secrets::load_api_credential(data_dir, credential_ref).ok_or_else(|| {
                    AppError::Validation(format!(
                        "la credencial API {credential_ref} no está disponible en este equipo"
                    ))
                })?;
            Some((mode.to_owned(), secret))
        }
        _ => {
            return Err(AppError::Validation(
                "el tipo de autenticación API no es válido".to_owned(),
            ))
        }
    };
    let final_url = preview.final_url;
    let requested_url = final_url.clone();
    let started = std::time::Instant::now();
    let response = tauri::async_runtime::spawn_blocking(move || {
        crate::research_tools::external_api_get_with_auth(
            &requested_url,
            authentication
                .as_ref()
                .map(|(mode, secret)| (mode.as_str(), secret.as_str())),
        )
    })
    .await
    .map_err(|error| AppError::BrokerTransport(error.to_string()))??;
    Ok(CustomGptApiActionTestResult {
        final_url,
        destination: preview.destination,
        status: response.status,
        content_type: response.content_type,
        body: response.body,
        truncated: response.truncated,
        duration_ms: started.elapsed().as_millis(),
    })
}

/// Compone la vista previa de un GPT sin crear tareas ni generar coste.
#[tauri::command]
pub(crate) fn preview_custom_gpt(
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
        icon_ref: context.icon_ref.clone(),
        version_no: context.version_no,
        prompt_block: task_runtime::custom_gpt_prompt_block(&context)?,
        preferred_model: view.preferred_model,
        execution_profile: view.execution_profile,
        context_profile: view.context_profile,
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
pub(crate) fn duplicate_custom_gpt(
    custom_gpt_id: String,
    new_name: Option<String>,
    state: State<'_, AppState>,
) -> Result<CustomGptView, AppError> {
    state
        .database
        .duplicate_custom_gpt(&custom_gpt_id, new_name.as_deref())
}

#[tauri::command]
pub(crate) fn pick_custom_gpt_import_path() -> Result<Option<String>, AppError> {
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
pub(crate) fn pick_custom_gpt_export_path(
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

pub(crate) fn validated_custom_gpt_json_path(raw: &str) -> Result<std::path::PathBuf, AppError> {
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
pub(crate) fn export_custom_gpt(
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
pub(crate) fn import_custom_gpt(
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
