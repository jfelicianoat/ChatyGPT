//! Herramientas del modelo: validacion de argumentos y acceso acotado a ficheros.
//!
//! Todo lo que toca disco pasa por aqui y esta acotado por tamano y por
//! carpeta autorizada: una herramienta no puede leer lo que nadie autorizo.

use super::*;

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolDecision {
    pub tool_call_id: String,
    pub approved: bool,
}

pub(super) const AUTHORIZED_TEXT_FILE_LIMIT: u64 = 256 * 1024;

pub(super) fn validate_authorized_directory_arguments(
    arguments: &serde_json::Value,
) -> Result<(), AppError> {
    if let Some(relative_path) = arguments
        .get("relative_path")
        .and_then(serde_json::Value::as_str)
    {
        let path = Path::new(relative_path);
        if path.is_absolute()
            || relative_path.chars().count() > 240
            || path.components().any(|component| {
                matches!(
                    component,
                    Component::ParentDir | Component::RootDir | Component::Prefix(_)
                )
            })
        {
            return Err(AppError::Validation(
                "la subcarpeta debe ser relativa y permanecer dentro de la carpeta autorizada"
                    .to_owned(),
            ));
        }
    }
    if arguments.get("relative_path").is_some() && arguments.get("folder_id").is_none() {
        return Err(AppError::Validation(
            "relative_path requiere un folder_id".to_owned(),
        ));
    }
    Ok(())
}

pub(super) fn list_bounded_authorized_directory(
    root: &Path,
    relative_path: &str,
) -> Result<Vec<serde_json::Value>, AppError> {
    let canonical_root = root.canonicalize().map_err(|_| {
        AppError::NotFound("la carpeta autorizada ya no está disponible".to_owned())
    })?;
    let target = canonical_root
        .join(relative_path)
        .canonicalize()
        .map_err(|_| AppError::NotFound("la subcarpeta solicitada no existe".to_owned()))?;
    if !target.starts_with(&canonical_root) || !target.is_dir() {
        return Err(AppError::Validation(
            "la subcarpeta solicitada queda fuera de la carpeta autorizada".to_owned(),
        ));
    }
    let mut entries = fs::read_dir(target)
        .map_err(|error| AppError::Validation(format!("no se pudo listar la carpeta: {error}")))?
        .filter_map(Result::ok)
        .take(101)
        .collect::<Vec<_>>();
    entries.sort_by_key(|entry| entry.file_name().to_string_lossy().to_lowercase());
    let truncated = entries.len() > 100;
    entries.truncate(100);
    let mut values = entries
        .into_iter()
        .map(|entry| {
            let kind = entry.file_type().ok();
            serde_json::json!({
                "name": entry.file_name().to_string_lossy(),
                "kind": if kind.as_ref().is_some_and(|kind| kind.is_dir()) { "folder" }
                    else if kind.as_ref().is_some_and(|kind| kind.is_symlink()) { "link" }
                    else { "file" },
                "size_bytes": entry.metadata().ok().filter(|metadata| metadata.is_file()).map(|metadata| metadata.len())
            })
        })
        .collect::<Vec<_>>();
    if truncated {
        values.push(
            serde_json::json!({"kind": "notice", "name": "Listado limitado a 100 elementos"}),
        );
    }
    Ok(values)
}

pub(super) fn validate_authorized_file_arguments(
    arguments: &serde_json::Value,
) -> Result<(&str, &str), AppError> {
    let folder_id = arguments
        .get("folder_id")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("read_authorized_file requiere folder_id".to_owned())
        })?;
    let relative_path = arguments
        .get("relative_path")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            AppError::Validation("read_authorized_file requiere relative_path".to_owned())
        })?;
    let path = Path::new(relative_path);
    if path.is_absolute()
        || relative_path.chars().count() > 240
        || path.components().any(|component| {
            matches!(
                component,
                Component::ParentDir | Component::RootDir | Component::Prefix(_)
            )
        })
    {
        return Err(AppError::Validation(
            "la ruta debe ser relativa y permanecer dentro de la carpeta autorizada".to_owned(),
        ));
    }
    Ok((folder_id, relative_path))
}

pub(super) fn read_bounded_authorized_text(
    root: &Path,
    relative_path: &str,
) -> Result<String, AppError> {
    let canonical_root = root.canonicalize().map_err(|_| {
        AppError::NotFound("la carpeta autorizada ya no está disponible".to_owned())
    })?;
    let candidate = canonical_root.join(PathBuf::from(relative_path));
    let canonical_file = candidate
        .canonicalize()
        .map_err(|_| AppError::NotFound("el archivo solicitado no existe".to_owned()))?;
    if !canonical_file.starts_with(&canonical_root) || !canonical_file.is_file() {
        return Err(AppError::Validation(
            "el archivo solicitado queda fuera de la carpeta autorizada".to_owned(),
        ));
    }
    let allowed = [
        "txt", "md", "csv", "tsv", "json", "jsonl", "yaml", "yml", "toml", "xml", "html", "css",
        "sql", "log", "py", "js", "jsx", "ts", "tsx", "rs", "java", "c", "h", "cpp", "hpp", "cs",
        "go", "rb", "php", "sh", "ps1",
    ];
    let extension = canonical_file
        .extension()
        .and_then(|value| value.to_str())
        .unwrap_or_default()
        .to_ascii_lowercase();
    if !allowed.contains(&extension.as_str()) {
        return Err(AppError::Validation(
            "solo se pueden leer archivos de texto o código compatibles".to_owned(),
        ));
    }
    if fs::metadata(&canonical_file)
        .map_err(|error| {
            AppError::Validation(format!("no se pudo inspeccionar el archivo: {error}"))
        })?
        .len()
        > AUTHORIZED_TEXT_FILE_LIMIT
    {
        return Err(AppError::Validation(format!(
            "el archivo supera el límite de {} KB",
            AUTHORIZED_TEXT_FILE_LIMIT / 1024
        )));
    }
    fs::read_to_string(canonical_file)
        .map_err(|_| AppError::Validation("el archivo no contiene texto UTF-8 legible".to_owned()))
}

pub(super) fn validate_authorized_file_replacement(
    arguments: &serde_json::Value,
) -> Result<(&str, &str, &str, &str), AppError> {
    let (folder_id, relative_path) = validate_authorized_file_arguments(arguments)?;
    let expected_sha256 = arguments
        .get("expected_sha256")
        .and_then(serde_json::Value::as_str)
        .map(str::trim)
        .filter(|value| value.len() == 64 && value.bytes().all(|byte| byte.is_ascii_hexdigit()))
        .ok_or_else(|| {
            AppError::Validation(
                "replace_authorized_file requiere la huella SHA-256 obtenida al leer".to_owned(),
            )
        })?;
    let content = arguments
        .get("content")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            AppError::Validation(
                "replace_authorized_file requiere el contenido completo nuevo".to_owned(),
            )
        })?;
    if content.len() as u64 > AUTHORIZED_TEXT_FILE_LIMIT {
        return Err(AppError::Validation(format!(
            "el contenido nuevo supera el límite de {} KB",
            AUTHORIZED_TEXT_FILE_LIMIT / 1024
        )));
    }
    Ok((folder_id, relative_path, expected_sha256, content))
}

pub(super) fn validate_scheduled_task_arguments(
    arguments: &serde_json::Value,
) -> Result<(&str, &str, &str, &str), AppError> {
    let required = |key: &str, label: &str, maximum: usize| {
        arguments
            .get(key)
            .and_then(serde_json::Value::as_str)
            .map(str::trim)
            .filter(|value| !value.is_empty() && value.chars().count() <= maximum)
            .ok_or_else(|| {
                AppError::Validation(format!("create_scheduled_task requiere {label} válido"))
            })
    };
    Ok((
        required("name", "un nombre de hasta 120 caracteres", 120)?,
        required(
            "prompt",
            "una instrucción de hasta 20.000 caracteres",
            20_000,
        )?,
        required("due_at", "una fecha ISO 8601 futura", 64)?,
        required("timezone", "una zona horaria de hasta 100 caracteres", 100)?,
    ))
}

pub(super) fn replace_bounded_authorized_text(
    root: &Path,
    relative_path: &str,
    expected_sha256: &str,
    content: &str,
) -> Result<String, AppError> {
    let current = read_bounded_authorized_text(root, relative_path)?;
    let current_sha256 = format!("{:x}", Sha256::digest(current.as_bytes()));
    if !current_sha256.eq_ignore_ascii_case(expected_sha256) {
        return Err(AppError::Conflict(
            "el archivo cambió después de que el GPT lo leyera; vuelve a leerlo antes de modificarlo".to_owned(),
        ));
    }
    let canonical_root = root.canonicalize().map_err(|_| {
        AppError::NotFound("la carpeta autorizada ya no está disponible".to_owned())
    })?;
    let destination = canonical_root
        .join(relative_path)
        .canonicalize()
        .map_err(|_| AppError::NotFound("el archivo solicitado no existe".to_owned()))?;
    if !destination.starts_with(&canonical_root) || !destination.is_file() {
        return Err(AppError::Validation(
            "el archivo solicitado queda fuera de la carpeta autorizada".to_owned(),
        ));
    }
    let parent = destination.parent().ok_or_else(|| {
        AppError::Validation("el archivo no tiene una carpeta contenedora válida".to_owned())
    })?;
    let temporary = parent.join(format!(".chatygpt-edit-{}.tmp", Uuid::new_v4().simple()));
    let write_result = (|| {
        use std::io::Write;
        let mut file = fs::OpenOptions::new()
            .write(true)
            .create_new(true)
            .open(&temporary)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        file.write_all(content.as_bytes())
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        file.sync_all()
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        fs::rename(&temporary, &destination)
            .map_err(|error| AppError::DataDirectory(error.to_string()))?;
        Ok::<(), AppError>(())
    })();
    if write_result.is_err() {
        let _ = fs::remove_file(&temporary);
    }
    write_result?;
    Ok(format!("{:x}", Sha256::digest(content.as_bytes())))
}

pub(super) fn persisted_custom_gpt_allows_tool(
    request: &serde_json::Value,
    tool_name: &str,
) -> bool {
    let metadata = &request["content"]["metadata"];
    let has_custom_gpt = metadata
        .get("custom_gpt_id")
        .is_some_and(|value| !value.is_null());
    if !has_custom_gpt {
        return true;
    }
    if tool_name.starts_with("api_action_") {
        return metadata["custom_gpt_tool_permissions"]["callExternalApis"].as_str()
            == Some("confirm")
            && metadata["custom_gpt_api_actions"]
                .as_array()
                .is_some_and(|actions| {
                    actions.iter().any(|action| {
                        action["name"]
                            .as_str()
                            .is_some_and(|name| tool_name == format!("api_action_{name}"))
                    })
                });
    }
    let permission_key = match tool_name {
        "rename_conversation" => "renameConversation",
        "run_code" => "runCode",
        "list_authorized_folders" | "read_authorized_file" => "readAuthorizedFolders",
        "replace_authorized_file" => "modifyAuthorizedFiles",
        "create_scheduled_task" => "createScheduledTasks",
        "call_external_api" => "callExternalApis",
        _ => return false,
    };
    metadata["custom_gpt_tool_permissions"][permission_key].as_str() == Some("confirm")
}

pub(super) fn configured_api_action<'a>(
    request: &'a serde_json::Value,
    tool_name: &str,
) -> Option<&'a serde_json::Value> {
    let name = tool_name.strip_prefix("api_action_")?;
    request["content"]["metadata"]["custom_gpt_api_actions"]
        .as_array()?
        .iter()
        .find(|action| action["name"].as_str() == Some(name))
}

pub(crate) fn configured_api_url(
    action: &serde_json::Value,
    arguments: &serde_json::Value,
) -> Result<String, AppError> {
    let raw = action["url"].as_str().ok_or_else(|| {
        AppError::Validation("la acción API no contiene una URL válida".to_owned())
    })?;
    let parameters = action["parameters"].as_array().cloned().unwrap_or_else(|| {
        action["queryParameters"]
            .as_array()
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(|name| json!({"name": name, "type": "string", "required": true, "location": "query"}))
            .collect()
    });
    let object = arguments.as_object().ok_or_else(|| {
        AppError::Validation("los parámetros de la acción API no son válidos".to_owned())
    })?;
    let configured_credential = action["credentialRef"].as_str();
    let configured_auth = configured_credential.and(action["authMode"].as_str());
    if object.get("url").and_then(serde_json::Value::as_str) != Some(raw)
        || object
            .get("credential_ref")
            .and_then(serde_json::Value::as_str)
            != configured_credential
        || object.get("auth_mode").and_then(serde_json::Value::as_str) != configured_auth
        || object.keys().any(|key| {
            key != "url"
                && key != "credential_ref"
                && key != "auth_mode"
                && !parameters
                    .iter()
                    .any(|parameter| parameter["name"].as_str() == Some(key))
        })
        || parameters.iter().any(|parameter| {
            parameter["required"].as_bool().unwrap_or(true)
                && !object.contains_key(parameter["name"].as_str().unwrap_or_default())
        })
    {
        return Err(AppError::Validation(
            "la acción API no recibió exactamente sus parámetros configurados".to_owned(),
        ));
    }
    let mut rendered_url = raw.to_owned();
    for parameter in &parameters {
        if parameter["location"].as_str().unwrap_or("query") != "path" {
            continue;
        }
        let key = parameter["name"].as_str().unwrap_or_default();
        let value = object
            .get(key)
            .and_then(serde_json::Value::as_str)
            .filter(|value| value.chars().count() <= 200)
            .ok_or_else(|| {
                AppError::Validation(format!("el parámetro de ruta {key} debe ser texto"))
            })?;
        let mut encoder = url::Url::parse("https://path.invalid/").expect("constant URL");
        encoder
            .path_segments_mut()
            .expect("hierarchical URL")
            .push(value);
        let encoded = encoder.path().trim_start_matches('/');
        rendered_url = rendered_url.replace(&format!("{{{key}}}"), encoded);
    }
    let mut url = crate::research_tools::validate_external_api_url(&rendered_url)?;
    if parameters
        .iter()
        .any(|parameter| parameter["location"].as_str().unwrap_or("query") != "path")
    {
        let mut query = url.query_pairs_mut();
        for parameter in parameters {
            if parameter["location"].as_str().unwrap_or("query") == "path" {
                continue;
            }
            let key = parameter["name"].as_str().unwrap_or_default();
            let Some(value) = object.get(key) else {
                continue;
            };
            let rendered = match parameter["type"].as_str().unwrap_or("string") {
                "string" => value
                    .as_str()
                    .filter(|value| value.chars().count() <= 500)
                    .map(str::to_owned),
                "number" => value
                    .as_f64()
                    .filter(|value| value.is_finite())
                    .map(|value| value.to_string()),
                "boolean" => value.as_bool().map(|value| value.to_string()),
                _ => None,
            }
            .ok_or_else(|| {
                AppError::Validation(format!(
                    "el parámetro {key} no coincide con su tipo configurado"
                ))
            })?;
            query.append_pair(key, &rendered);
        }
    }
    Ok(url.to_string())
}

pub async fn resolve_tool_calls(
    database: Database,
    broker: BrokerClient,
    data_dir: &std::path::Path,
    local_task_id: &str,
    decisions: &[ToolDecision],
) -> Result<LocalTaskSnapshot, AppError> {
    let pending = database.pending_tool_calls(local_task_id)?;
    let persisted_request = database.task_record(local_task_id)?.request;
    let expected: HashSet<&str> = pending
        .iter()
        .map(|call| call.tool_call_id.as_str())
        .collect();
    let provided: HashSet<&str> = decisions
        .iter()
        .map(|decision| decision.tool_call_id.as_str())
        .collect();
    if expected != provided || decisions.len() != provided.len() || pending.is_empty() {
        return Err(AppError::Validation(
            "debe aprobar o rechazar cada herramienta pendiente exactamente una vez".to_owned(),
        ));
    }
    let decisions_by_id: HashMap<&str, bool> = decisions
        .iter()
        .map(|decision| (decision.tool_call_id.as_str(), decision.approved))
        .collect();
    for call in &pending {
        // El expediente durable manda: una confirmación ya resuelta no vuelve a
        // ejecutarse, aunque la interfaz reenvíe la decisión.
        if let Some(confirmation) = &call.confirmation {
            if confirmation.status != "pending" {
                return Err(AppError::Conflict(format!(
                    "la confirmación de {} ya se resolvió como {}",
                    call.name, confirmation.status
                )));
            }
        }
        if decisions_by_id[call.tool_call_id.as_str()] {
            if !persisted_custom_gpt_allows_tool(&persisted_request, &call.name) {
                return Err(AppError::Conflict(format!(
                    "la versión del GPT usada por esta tarea mantiene {} denegado",
                    call.name
                )));
            }
            match call.name.as_str() {
                "rename_conversation" => {
                    let title = call
                        .arguments
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            AppError::Validation(
                                "rename_conversation requiere un título".to_owned(),
                            )
                        })?;
                    if title.chars().count() > 120 {
                        return Err(AppError::Validation(
                            "el título propuesto supera 120 caracteres".to_owned(),
                        ));
                    }
                }
                "list_authorized_folders" => {
                    validate_authorized_directory_arguments(&call.arguments)?;
                }
                "read_authorized_file" => {
                    validate_authorized_file_arguments(&call.arguments)?;
                }
                "replace_authorized_file" => {
                    validate_authorized_file_replacement(&call.arguments)?;
                }
                "create_scheduled_task" => {
                    validate_scheduled_task_arguments(&call.arguments)?;
                }
                "call_external_api" => {
                    validate_external_api_arguments(&call.arguments)?;
                }
                other if other.starts_with("api_action_") => {
                    let action =
                        configured_api_action(&persisted_request, other).ok_or_else(|| {
                            AppError::Validation(
                                "la acción API ya no pertenece a la versión del GPT".to_owned(),
                            )
                        })?;
                    configured_api_url(action, &call.arguments)?;
                }
                other => {
                    return Err(AppError::Validation(format!(
                        "herramienta de cliente no admitida: {other}"
                    )))
                }
            }
        }
    }
    let conversation_id = database.task_conversation_id(local_task_id)?;
    let mut outcomes = Vec::with_capacity(pending.len());
    for call in pending {
        let approved = decisions_by_id[call.tool_call_id.as_str()];
        let (status, content) = if approved {
            match call.name.as_str() {
                "rename_conversation" => {
                    let title = call
                        .arguments
                        .get("title")
                        .and_then(serde_json::Value::as_str)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            AppError::Validation(
                                "rename_conversation requiere un título".to_owned(),
                            )
                        })?;
                    if title.chars().count() > 120 {
                        return Err(AppError::Validation(
                            "el título propuesto supera 120 caracteres".to_owned(),
                        ));
                    }
                    database.rename_conversation(&conversation_id, title)?;
                    (
                        "approved",
                        serde_json::json!({"ok": true, "title": title}).to_string(),
                    )
                }
                "list_authorized_folders" => {
                    let folder_id = call
                        .arguments
                        .get("folder_id")
                        .and_then(serde_json::Value::as_str);
                    let relative_path = call
                        .arguments
                        .get("relative_path")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or("");
                    let value = if let Some(folder_id) = folder_id {
                        let (root, folder_name) = database.authorized_folder_for_read(folder_id)?;
                        serde_json::json!({
                            "ok": true,
                            "folder": folder_name,
                            "relative_path": relative_path,
                            "entries": list_bounded_authorized_directory(&root, relative_path)?
                        })
                    } else {
                        let folders = database.list_read_authorized_folders()?;
                        serde_json::json!({
                            "ok": true,
                            "folders": folders.into_iter().map(|folder| serde_json::json!({
                                "folder_id": folder.id,
                                "name": folder.display_name
                            })).collect::<Vec<_>>()
                        })
                    };
                    ("approved", value.to_string())
                }
                "read_authorized_file" => {
                    let (folder_id, relative_path) =
                        validate_authorized_file_arguments(&call.arguments)?;
                    let (root, folder_name) = database.authorized_folder_for_read(folder_id)?;
                    let content = read_bounded_authorized_text(&root, relative_path)?;
                    let sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
                    (
                        "approved",
                        serde_json::json!({
                            "ok": true,
                            "folder": folder_name,
                            "relative_path": relative_path,
                            "content": content,
                            "sha256": sha256
                        })
                        .to_string(),
                    )
                }
                "replace_authorized_file" => {
                    let (folder_id, relative_path, expected_sha256, content) =
                        validate_authorized_file_replacement(&call.arguments)?;
                    let (root, folder_name) = database.authorized_folder_for_modify(folder_id)?;
                    let after_sha256 = replace_bounded_authorized_text(
                        &root,
                        relative_path,
                        expected_sha256,
                        content,
                    )?;
                    database.record_authorized_file_modified(
                        folder_id,
                        expected_sha256,
                        &after_sha256,
                    )?;
                    (
                        "approved",
                        serde_json::json!({
                            "ok": true,
                            "folder": folder_name,
                            "relative_path": relative_path,
                            "before_sha256": expected_sha256,
                            "after_sha256": after_sha256
                        })
                        .to_string(),
                    )
                }
                "create_scheduled_task" => {
                    let (name, prompt, due_at, timezone) =
                        validate_scheduled_task_arguments(&call.arguments)?;
                    let scheduled = database.create_scheduled_task_from_tool(
                        &call.tool_call_id,
                        name,
                        &conversation_id,
                        prompt,
                        due_at,
                        timezone,
                    )?;
                    (
                        "approved",
                        serde_json::json!({
                            "ok": true,
                            "scheduled_task_id": scheduled.id,
                            "name": scheduled.name,
                            "next_run_at": scheduled.next_run_at
                        })
                        .to_string(),
                    )
                }
                "call_external_api" => {
                    let url = validate_external_api_arguments(&call.arguments)?;
                    let url = url.to_owned();
                    let response = tauri::async_runtime::spawn_blocking(move || {
                        crate::research_tools::external_api_get(&url)
                    })
                    .await
                    .map_err(|error| AppError::BrokerTransport(error.to_string()))??;
                    (
                        "approved",
                        serde_json::to_string(&response)
                            .map_err(|error| AppError::BrokerContract(error.to_string()))?,
                    )
                }
                other if other.starts_with("api_action_") => {
                    let action =
                        configured_api_action(&persisted_request, other).ok_or_else(|| {
                            AppError::Validation(
                                "la acción API ya no pertenece a la versión del GPT".to_owned(),
                            )
                        })?;
                    let url = configured_api_url(action, &call.arguments)?;
                    let authentication = match action["authMode"].as_str().unwrap_or("none") {
                        "none" => None,
                        mode @ ("bearer" | "api_key") => {
                            let credential_ref =
                                action["credentialRef"].as_str().ok_or_else(|| {
                                    AppError::Validation(
                                        "la acción API no indica su credencial".to_owned(),
                                    )
                                })?;
                            let secret = crate::secrets::load_api_credential(data_dir, credential_ref).ok_or_else(|| AppError::Validation(format!("la credencial API {credential_ref} no está disponible en este equipo")))?;
                            Some((mode.to_owned(), secret))
                        }
                        _ => {
                            return Err(AppError::Validation(
                                "el tipo de autenticación API no es válido".to_owned(),
                            ))
                        }
                    };
                    let response = tauri::async_runtime::spawn_blocking(move || {
                        crate::research_tools::external_api_get_with_auth(
                            &url,
                            authentication
                                .as_ref()
                                .map(|(mode, secret)| (mode.as_str(), secret.as_str())),
                        )
                    })
                    .await
                    .map_err(|error| AppError::BrokerTransport(error.to_string()))??;
                    (
                        "approved",
                        serde_json::to_string(&response)
                            .map_err(|error| AppError::BrokerContract(error.to_string()))?,
                    )
                }
                other => {
                    return Err(AppError::Validation(format!(
                        "herramienta de cliente no admitida: {other}"
                    )))
                }
            }
        } else {
            (
                "cancelled",
                serde_json::json!({
                    "ok": false,
                    "rejected_by_user": true,
                    "message": "El usuario rechazó esta acción"
                })
                .to_string(),
            )
        };
        outcomes.push(ToolOutcomeRecord {
            tool_call_id: call.tool_call_id,
            status: status.to_owned(),
            content,
        });
    }
    database.prepare_tool_outcomes(local_task_id, &outcomes)?;
    spawn_tool_resume(database.clone(), broker, local_task_id.to_owned());
    database.task_snapshot(local_task_id)
}

/// Resuelve por su cuenta las herramientas de una investigación.
///
/// Solo actúa si la tarea es una investigación y **todas** sus llamadas
/// pendientes son herramientas que ChatyGPT sabe ejecutar. Si aparece
/// cualquier otra, no toca nada y la tarea se queda esperando la decisión de la
/// persona: el automatismo no puede convertirse en una vía para ejecutar sin
/// confirmar algo que sí la necesita.
///
/// El contrato exige responder a todas las llamadas en una sola petición, así
/// que se ejecutan todas y se envían juntas; una que falle viaja como resultado
/// de error, no como silencio, para que el modelo pueda reaccionar.
pub(super) fn spawn_research_tool_execution(
    database: Database,
    broker: BrokerClient,
    local_task_id: String,
) {
    tauri::async_runtime::spawn(async move {
        let Ok(request) = database
            .task_record(&local_task_id)
            .map(|record| record.request)
        else {
            return;
        };
        if request["content"]["metadata"]["workflow_kind"] != json!("deep_research") {
            return;
        }
        let Ok(pending) = database.pending_tool_calls(&local_task_id) else {
            return;
        };
        if pending.is_empty()
            || !pending
                .iter()
                .all(|call| RESEARCH_CLIENT_TOOLS.contains(&call.name.as_str()))
        {
            return;
        }
        let Ok(client) = crate::research_tools::web_client() else {
            return;
        };
        let mut outcomes = Vec::with_capacity(pending.len());
        for call in &pending {
            let (status, content) = match call.name.as_str() {
                "fetch_url" => {
                    let url = call
                        .arguments
                        .get("url")
                        .and_then(serde_json::Value::as_str)
                        .unwrap_or_default()
                        .to_owned();
                    match crate::research_tools::fetch_url(&client, &url).await {
                        Ok(page) => {
                            logging::info(
                                "research.tool_executed",
                                Some(&local_task_id),
                                &[
                                    ("tool", logging::code("fetch_url")),
                                    (
                                        "characters",
                                        logging::count(page.text.chars().count() as i64),
                                    ),
                                    ("truncated", logging::flag(page.truncated)),
                                ],
                            );
                            let content = serde_json::to_string(&page).unwrap_or_default();
                            let _ = database.record_research_tool_step(
                                &local_task_id,
                                &call.tool_call_id,
                                "fetch_url",
                                &url,
                                "completed",
                                &serde_json::json!({
                                    "url": page.url,
                                    "title": page.title,
                                    "truncated": page.truncated
                                }),
                            );
                            ("approved", content)
                        }
                        Err(error) => {
                            // El motivo viaja al modelo para que pueda probar
                            // otra fuente; al registro solo va su clase.
                            logging::warn(
                                "research.tool_failed",
                                Some(&local_task_id),
                                &[
                                    ("tool", logging::code("fetch_url")),
                                    ("error_kind", logging::error_kind(&error)),
                                ],
                            );
                            let _ = database.record_research_tool_step(
                                &local_task_id,
                                &call.tool_call_id,
                                "fetch_url",
                                &url,
                                "failed",
                                &serde_json::json!({"error": error.to_string()}),
                            );
                            // La herramienta **se ejecutó**: `approved` describe
                            // la decisión, no el desenlace. Que la página no se
                            // pudiera abrir es el contenido que recibe el
                            // modelo, y el estado real del paso se guarda como
                            // fallido en el expediente de la investigación.
                            (
                                "approved",
                                serde_json::json!({
                                    "ok": false,
                                    "error": error.to_string()
                                })
                                .to_string(),
                            )
                        }
                    }
                }
                _ => return,
            };
            outcomes.push(ToolOutcomeRecord {
                tool_call_id: call.tool_call_id.clone(),
                status: status.to_owned(),
                content,
            });
        }
        if database
            .prepare_tool_outcomes(&local_task_id, &outcomes)
            .is_err()
        {
            return;
        }
        spawn_tool_resume(database, broker, local_task_id);
    });
}

pub(super) fn spawn_tool_resume(database: Database, broker: BrokerClient, local_task_id: String) {
    tauri::async_runtime::spawn(async move {
        let policy = PollPolicy::default();
        let mut failures = 0_u32;
        loop {
            let record = match database.task_record(&local_task_id) {
                Ok(record) => record,
                Err(_) => return,
            };
            let Some(remote_id) = record.remote_task_id else {
                return;
            };
            let payload = match database.prepared_tool_results(&local_task_id) {
                Ok(payload) => payload,
                Err(_) => return,
            };
            match broker.submit_tool_results(&remote_id, &payload).await {
                Ok(state) => {
                    if database
                        .mark_tool_results_submitted(&local_task_id)
                        .and_then(|()| database.record_remote_state(&local_task_id, &state))
                        .is_ok()
                    {
                        spawn_polling(database, broker, local_task_id);
                    }
                    return;
                }
                Err(error) if is_permanent(&error) => {
                    match broker.get_task(&remote_id).await {
                        Ok(state) if state.status.as_str() != "waiting_for_tools" => {
                            if database
                                .mark_tool_results_submitted(&local_task_id)
                                .and_then(|()| database.record_remote_state(&local_task_id, &state))
                                .is_ok()
                            {
                                spawn_polling(database, broker, local_task_id);
                            }
                        }
                        _ => {
                            let _ = database.mark_orphaned(&local_task_id, &error.to_string());
                        }
                    }
                    return;
                }
                Err(error) => {
                    failures = failures.saturating_add(1);
                    let _ = database.record_transport_error(&local_task_id, &error.to_string());
                    let delay = policy.delay_ms(
                        failures,
                        deterministic_jitter(&local_task_id, failures as u64),
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                }
            }
        }
    });
}

pub async fn cancel_task(
    database: Database,
    broker: BrokerClient,
    local_id: &str,
) -> Result<LocalTaskSnapshot, AppError> {
    let record = database.task_record(local_id)?;
    let remote_id = record.remote_task_id.ok_or_else(|| {
        AppError::BrokerContract("la tarea todavía no tiene identificador remoto".to_owned())
    })?;
    let state = broker.cancel_task(&remote_id).await?;
    logging::info(
        "task.cancel_requested",
        Some(local_id),
        &[("status", logging::code(state.status.as_str()))],
    );
    database.record_remote_state(local_id, &state)?;
    advance_semantic_chat(database.clone(), broker, local_id);
    database.task_snapshot(local_id)
}

pub(super) async fn submit_or_resume(
    database: Database,
    broker: BrokerClient,
    record: BrokerTaskRecord,
) -> Result<(), AppError> {
    if record.remote_task_id.is_some() {
        return Ok(());
    }
    database.mark_submitting(&record.id)?;
    match broker.create_task(&record.request).await {
        Ok(accepted) => {
            // Enlaza la identidad local con la remota: es la traza que permite
            // reconstruir después qué tarea del Broker atendió este turno.
            logging::info(
                "task.submitted",
                Some(&record.id),
                &[
                    ("remote_task_id", logging::id(&accepted.task_id)),
                    ("status", logging::code(accepted.status.as_str())),
                ],
            );
            database.attach_remote_task(&record.id, &accepted)
        }
        Err(error) => {
            if is_permanent(&error) {
                logging::error(
                    "task.orphaned",
                    Some(&record.id),
                    &[
                        ("phase", logging::code("submit")),
                        ("error_kind", logging::error_kind(&error)),
                    ],
                );
                database.mark_orphaned(&record.id, &error.to_string())?;
            } else {
                logging::warn(
                    "task.submit_retry",
                    Some(&record.id),
                    &[("error_kind", logging::error_kind(&error))],
                );
                database.record_transport_error(&record.id, &error.to_string())?;
            }
            Err(error)
        }
    }
}

pub(super) fn spawn_submission_and_poll(
    database: Database,
    broker: BrokerClient,
    initial_record: BrokerTaskRecord,
) {
    tauri::async_runtime::spawn(async move {
        let local_id = initial_record.id.clone();
        let policy = PollPolicy::default();
        let mut record = initial_record;
        loop {
            match submit_or_resume(database.clone(), broker.clone(), record).await {
                Ok(()) => {
                    spawn_polling(database, broker, local_id);
                    return;
                }
                Err(error) if is_permanent(&error) => {
                    advance_semantic_chat(database, broker, &local_id);
                    return;
                }
                Err(_) => {
                    let current = match database.task_record(&local_id) {
                        Ok(current) => current,
                        Err(_) => return,
                    };
                    let delay = policy.delay_ms(
                        current.consecutive_poll_errors,
                        deterministic_jitter(&local_id, current.consecutive_poll_errors as u64),
                    );
                    tokio::time::sleep(Duration::from_millis(delay)).await;
                    record = match database.task_record(&local_id) {
                        Ok(record) => record,
                        Err(_) => return,
                    };
                }
            }
        }
    });
}

pub(super) fn spawn_polling(database: Database, broker: BrokerClient, local_id: String) {
    tauri::async_runtime::spawn(async move {
        let policy = PollPolicy::default();
        let mut unchanged_polls = 0_u32;
        let mut last_status = String::new();
        let mut poll_no = 0_u64;

        loop {
            let record = match database.task_record(&local_id) {
                Ok(record) => record,
                Err(_) => return,
            };
            let Some(remote_id) = record.remote_task_id else {
                return;
            };

            match broker.get_task(&remote_id).await {
                Ok(state) => {
                    let status = state.status.as_str().to_owned();
                    if database.record_remote_state(&local_id, &state).is_err() {
                        return;
                    }
                    if state.status.is_terminal() || status == "waiting_for_tools" {
                        logging::info(
                            "task.state_settled",
                            Some(&local_id),
                            &[
                                ("status", logging::code(&status)),
                                ("polls", logging::count(poll_no as i64)),
                            ],
                        );
                        if state.status.is_terminal() {
                            advance_semantic_chat(database.clone(), broker.clone(), &local_id);
                        } else {
                            // Una investigación resuelve sus propias herramientas:
                            // la persona autorizó el recorrido entero al activarla,
                            // y detenerse a preguntar por cada fuente lo haría
                            // inservible. Un turno corriente sigue esperando su
                            // decisión, que es lo que protege las acciones
                            // sensibles.
                            spawn_research_tool_execution(
                                database.clone(),
                                broker.clone(),
                                local_id.clone(),
                            );
                        }
                        return;
                    }
                    if status == last_status {
                        unchanged_polls = unchanged_polls.saturating_add(1);
                    } else {
                        last_status = status;
                        unchanged_polls = 0;
                    }
                }
                Err(error) => {
                    if is_permanent(&error) {
                        logging::error(
                            "task.orphaned",
                            Some(&local_id),
                            &[
                                ("phase", logging::code("poll")),
                                ("error_kind", logging::error_kind(&error)),
                            ],
                        );
                        let _ = database.mark_orphaned(&local_id, &error.to_string());
                        return;
                    }
                    logging::warn(
                        "task.poll_error",
                        Some(&local_id),
                        &[("error_kind", logging::error_kind(&error))],
                    );
                    let _ = database.record_transport_error(&local_id, &error.to_string());
                    unchanged_polls = unchanged_polls.saturating_add(1);
                }
            }

            let jitter = deterministic_jitter(&local_id, poll_no);
            poll_no = poll_no.saturating_add(1);
            tokio::time::sleep(Duration::from_millis(
                policy.delay_ms(unchanged_polls, jitter),
            ))
            .await;
        }
    });
}

pub(super) fn advance_semantic_chat(
    database: Database,
    broker: BrokerClient,
    embedding_task_id: &str,
) {
    let Some(workflow) = database
        .semantic_chat_workflow_for_task(embedding_task_id)
        .ok()
        .flatten()
    else {
        return;
    };
    if workflow.embedding_task_id != embedding_task_id || workflow.status != "searching" {
        return;
    }
    let task = match database.task_snapshot(embedding_task_id) {
        Ok(task) => task,
        Err(_) => return,
    };
    match task.remote_status.as_str() {
        "completed" => {
            let result = (|| {
                let context_budget =
                    custom_gpt_context_budget(workflow.custom_gpt_context.as_ref());
                let selected = if database.semantic_workflow_uses_memory(&workflow.id)? {
                    database.semantic_memory_matches_with_limit(
                        &workflow.id,
                        context_budget.memory_items.min(10),
                    )?
                } else {
                    Vec::new()
                };
                let mut used_memory_characters = 0_usize;
                let memories = selected
                    .iter()
                    .map(|item| item.memory.clone())
                    .filter(|memory| {
                        used_memory_characters += memory.content.chars().count();
                        used_memory_characters <= context_budget.memory_characters
                    })
                    .collect::<Vec<_>>();
                let attachments = database.ready_attachments_for_turn(
                    &workflow.conversation_id,
                    &workflow.attachment_ids,
                )?;
                let document_chunks = database.select_attachment_chunks_hybrid(
                    &workflow.conversation_id,
                    &workflow.attachment_ids,
                    &workflow.user_text,
                    context_budget.document_chunks,
                    context_budget.document_characters,
                    &workflow.id,
                )?;
                let chat_task_id = format!("local_{}", Uuid::new_v4().simple());
                let idempotency_key = format!("chatygpt:semantic-chat:{}", workflow.id);
                let mut request = chat_request_with_project_instruction(
                    &workflow.conversation_id,
                    &idempotency_key,
                    &workflow.user_text,
                    &workflow.context,
                    &attachments,
                    &document_chunks,
                    &memories,
                    workflow.project_instruction.as_ref(),
                    workflow.custom_gpt_context.as_ref(),
                    ChatExecutionOptions {
                        tools_enabled: workflow.tools_enabled,
                        sandbox_enabled: workflow.sandbox_enabled,
                        execution_preferences: workflow.execution_preferences,
                    },
                )?;
                // La investigación se aplica sobre el contexto ya recuperado:
                // los recuerdos y fragmentos seleccionados por similitud forman
                // parte del objetivo que se investiga, no se descartan.
                if let Some(plan) = workflow.research_plan.as_ref() {
                    let plan: ResearchPlan = serde_json::from_value(plan.clone())
                        .map_err(|error| AppError::BrokerContract(error.to_string()))?;
                    request = apply_deep_research_plan(request, &plan)?;
                }
                database.prepare_semantic_chat_submission(
                    &workflow.id,
                    &chat_task_id,
                    &idempotency_key,
                    &request,
                    &selected,
                    &document_chunks,
                )
            })();
            match result {
                Ok(record) => spawn_submission_and_poll(database, broker, record),
                Err(error) => {
                    let _ = database.finish_semantic_chat_without_submission(
                        embedding_task_id,
                        false,
                        &error.to_string(),
                    );
                }
            }
        }
        "cancelled" => {
            let _ = database.finish_semantic_chat_without_submission(
                embedding_task_id,
                true,
                "La recuperación semántica fue cancelada.",
            );
        }
        "failed" => {
            let message = task
                .error
                .as_ref()
                .and_then(|error| error.get("message"))
                .and_then(serde_json::Value::as_str)
                .unwrap_or("Broker AI no pudo vectorizar la consulta.");
            let _ =
                database.finish_semantic_chat_without_submission(embedding_task_id, false, message);
        }
        _ if task.local_state == "orphaned" => {
            let _ = database.finish_semantic_chat_without_submission(
                embedding_task_id,
                false,
                "La búsqueda semántica no pudo enviarse a Broker AI.",
            );
        }
        _ => {}
    }
}

pub(super) fn is_permanent(error: &AppError) -> bool {
    matches!(
        error,
        AppError::BrokerResponse { status, .. }
            if (400..500).contains(status) && !matches!(*status, 401 | 403 | 408 | 429)
    )
}

pub(super) fn deterministic_jitter(local_id: &str, poll_no: u64) -> i32 {
    let mut hasher = DefaultHasher::new();
    local_id.hash(&mut hasher);
    poll_no.hash(&mut hasher);
    (hasher.finish() % 3_001) as i32 - 1_500
}
