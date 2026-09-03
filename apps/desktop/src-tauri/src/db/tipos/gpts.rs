//! Resumenes, proyectos y GPTs personalizados, con sus validaciones.
//!
//! Las validaciones viven junto al tipo que validan: un icono, un perfil de
//! contexto o una accion de API mal formada se rechazan al construirse, no
//! al usarse.

use std::collections::HashSet;

use serde::{Deserialize, Serialize};

use crate::error::AppError;

use super::*;

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummary {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummaryRevision {
    pub id: String,
    pub status: String,
    pub draft_text: Option<String>,
    pub approved_text: Option<String>,
    pub source_through_sequence: i64,
    pub broker_task_id: Option<String>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSummaryOverview {
    pub candidate: Option<ConversationSummaryRevision>,
    pub active: Option<ConversationSummaryRevision>,
    pub total_message_count: i64,
    pub active_covered_message_count: i64,
    pub remaining_message_count: i64,
    pub candidate_covered_message_count: Option<i64>,
}

#[derive(Debug, Clone)]
pub struct ConversationSummaryInput {
    pub messages: Vec<ContextMessage>,
    pub source_through_sequence: i64,
    pub included_message_count: i64,
    pub remaining_message_count: i64,
    pub character_count: usize,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SelectedAttachmentChunk {
    pub id: String,
    pub attachment_id: String,
    pub attachment_name: String,
    pub ordinal: i64,
    pub text: String,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub instructions: Option<String>,
    pub conversation_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CustomGptConfiguration {
    pub(crate) schema_version: i64,
    #[serde(default = "default_custom_gpt_icon")]
    pub(crate) icon_ref: String,
    pub(crate) instructions: String,
    pub(crate) conversation_starters: Vec<String>,
    pub(crate) preferred_model: Option<String>,
    pub(crate) tools_enabled: bool,
    /// `None` mantiene el comportamiento histórico: manda la configuración del chat.
    #[serde(default)]
    pub(crate) execution_profile: Option<ConversationExecutionPreferences>,
    /// Presupuesto de contexto propio del GPT. El valor por defecto conserva
    /// exactamente los límites históricos.
    #[serde(default = "default_custom_gpt_context_profile")]
    pub(crate) context_profile: String,
    /// Acciones GET públicas definidas por la persona y congeladas con la versión.
    #[serde(default)]
    pub(crate) api_actions: Vec<CustomGptApiAction>,
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomGptApiAction {
    pub name: String,
    pub description: String,
    pub url: String,
    /// Formato inicial, conservado solo para leer versiones ya guardadas.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub query_parameters: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub parameters: Vec<CustomGptApiParameter>,
    /// Solo conserva el alias; el secreto vive cifrado fuera de SQLite.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub credential_ref: Option<String>,
    #[serde(default = "default_api_auth_mode")]
    pub auth_mode: String,
}

pub(crate) fn default_api_auth_mode() -> String {
    "none".to_owned()
}

#[derive(Debug, Clone, Deserialize, Serialize, PartialEq, Eq)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CustomGptApiParameter {
    pub name: String,
    #[serde(rename = "type")]
    pub value_type: String,
    #[serde(default = "default_required_api_parameter")]
    pub required: bool,
    #[serde(default = "default_api_parameter_location")]
    pub location: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

pub(crate) fn default_required_api_parameter() -> bool {
    true
}

pub(crate) fn default_api_parameter_location() -> String {
    "query".to_owned()
}

pub(crate) fn validated_custom_gpt_api_actions(
    actions: &[CustomGptApiAction],
) -> Result<Vec<CustomGptApiAction>, AppError> {
    if actions.len() > 10 {
        return Err(AppError::Validation(
            "un GPT admite como máximo 10 acciones API".to_owned(),
        ));
    }
    let mut names = HashSet::new();
    actions.iter().map(|action| {
        let name = action.name.trim().to_ascii_lowercase();
        if name.len() < 3 || name.len() > 40 || !name.bytes().enumerate().all(|(index, byte)| {
            byte.is_ascii_lowercase() || byte == b'_' || (index > 0 && byte.is_ascii_digit())
        }) || name.starts_with('_') {
            return Err(AppError::Validation("el nombre de una acción API debe usar entre 3 y 40 letras minúsculas, números o guiones bajos".to_owned()));
        }
        if !names.insert(name.clone()) {
            return Err(AppError::Validation(format!("la acción API {name} está repetida")));
        }
        let description = action.description.trim();
        if description.is_empty() || description.chars().count() > 300 {
            return Err(AppError::Validation("cada acción API necesita una descripción de hasta 300 caracteres".to_owned()));
        }
        let template = action.url.trim();
        let mut validation_url = template.to_owned();
        while let Some(start) = validation_url.find('{') {
            let end = validation_url[start..].find('}').map(|value| start + value).ok_or_else(|| AppError::Validation("la URL contiene una variable de ruta incompleta".to_owned()))?;
            validation_url.replace_range(start..=end, "placeholder");
        }
        let url = crate::research_tools::validate_external_api_url(&validation_url)?;
        if url.query().is_some() || url.fragment().is_some() {
            return Err(AppError::Validation("la URL base de una acción API no puede incluir consulta ni fragmento".to_owned()));
        }
        if !action.query_parameters.is_empty() && !action.parameters.is_empty() {
            return Err(AppError::Validation("una acción API no puede mezclar parámetros antiguos y tipados".to_owned()));
        }
        let auth_mode = action.auth_mode.trim().to_ascii_lowercase();
        if !matches!(auth_mode.as_str(), "none" | "bearer" | "api_key") {
            return Err(AppError::Validation("elige un tipo de autenticación API válido".to_owned()));
        }
        let credential_ref = action.credential_ref.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(crate::secrets::validate_api_credential_name).transpose()?;
        if (auth_mode == "none") != credential_ref.is_none() {
            return Err(AppError::Validation("la autenticación y el alias de credencial deben configurarse juntos".to_owned()));
        }
        let source_parameters = if action.parameters.is_empty() {
            action.query_parameters.iter().map(|name| CustomGptApiParameter {
                name: name.clone(), value_type: "string".to_owned(), required: true, location: "query".to_owned(), description: None,
            }).collect::<Vec<_>>()
        } else { action.parameters.clone() };
        if source_parameters.len() > 8 {
            return Err(AppError::Validation("una acción API admite como máximo 8 parámetros".to_owned()));
        }
        let mut parameters = Vec::new();
        let mut parameter_names = Vec::new();
        for parameter in source_parameters {
            let name = parameter.name.trim().to_ascii_lowercase();
            if name.is_empty() || name.len() > 40 || !name.bytes().all(|byte| byte.is_ascii_alphanumeric() || byte == b'_') || parameter_names.contains(&name) {
                return Err(AppError::Validation("los parámetros API deben ser únicos y usar letras, números o guiones bajos".to_owned()));
            }
            if !matches!(parameter.value_type.as_str(), "string" | "number" | "boolean") {
                return Err(AppError::Validation(format!("el parámetro {name} tiene un tipo no válido")));
            }
            if !matches!(parameter.location.as_str(), "query" | "path") {
                return Err(AppError::Validation(format!("el parámetro {name} tiene una ubicación no válida")));
            }
            let marker = format!("{{{name}}}");
            let occurrences = template.matches(&marker).count();
            if (parameter.location == "path" && (occurrences != 1 || !parameter.required))
                || (parameter.location == "query" && occurrences != 0) {
                return Err(AppError::Validation(format!("la variable de ruta {marker} debe aparecer exactamente una vez y ser obligatoria")));
            }
            let description = parameter.description.as_deref().map(str::trim).filter(|value| !value.is_empty()).map(str::to_owned);
            if description.as_ref().is_some_and(|value| value.chars().count() > 160) {
                return Err(AppError::Validation(format!("la descripción del parámetro {name} supera 160 caracteres")));
            }
            parameter_names.push(name.clone());
            parameters.push(CustomGptApiParameter { name, value_type: parameter.value_type, required: parameter.required, location: parameter.location, description });
        }
        if template.contains('{') || template.contains('}') {
            for marker in template.match_indices('{').filter_map(|(start, _)| template[start..].find('}').map(|end| &template[start..=start+end])) {
                if !parameters.iter().any(|parameter| parameter.location == "path" && marker == format!("{{{}}}", parameter.name)) {
                    return Err(AppError::Validation(format!("la URL contiene una variable no declarada: {marker}")));
                }
            }
        }
        Ok(CustomGptApiAction { name, description: description.to_owned(), url: template.to_owned(), query_parameters: Vec::new(), parameters, credential_ref, auth_mode })
    }).collect()
}

pub(crate) fn validated_custom_gpt_api_action(
    action: &CustomGptApiAction,
) -> Result<CustomGptApiAction, AppError> {
    validated_custom_gpt_api_actions(std::slice::from_ref(action))?
        .into_iter()
        .next()
        .ok_or_else(|| AppError::Validation("la acción API no es válida".to_owned()))
}

pub(crate) fn default_custom_gpt_context_profile() -> String {
    "balanced".to_owned()
}

pub(crate) fn validated_custom_gpt_context_profile(
    value: Option<&str>,
) -> Result<String, AppError> {
    let value = value.unwrap_or("balanced").trim();
    if matches!(value, "focused" | "balanced" | "broad") {
        Ok(value.to_owned())
    } else {
        Err(AppError::Validation(
            "elige un nivel de contexto válido para el GPT".to_owned(),
        ))
    }
}

const CUSTOM_GPT_ICONS: &[&str] = &[
    "spark",
    "research",
    "writing",
    "code",
    "data",
    "teacher",
    "briefcase",
];

pub(crate) fn default_custom_gpt_icon() -> String {
    "spark".to_owned()
}

pub(crate) fn validated_custom_gpt_icon(icon_ref: Option<&str>) -> Result<String, AppError> {
    let icon_ref = icon_ref.unwrap_or("spark").trim();
    if CUSTOM_GPT_ICONS.contains(&icon_ref) {
        Ok(icon_ref.to_owned())
    } else {
        Err(AppError::Validation(
            "elige uno de los iconos disponibles para el GPT".to_owned(),
        ))
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomGptView {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub icon_ref: String,
    pub instructions: String,
    pub conversation_starters: Vec<String>,
    pub tool_permissions: CustomGptToolPermissions,
    /// Modelo que el Broker debe intentar primero; `None` deja decidir al Broker.
    pub preferred_model: Option<String>,
    /// Perfil versionado. `None` significa que hereda los ajustes del chat.
    pub execution_profile: Option<ConversationExecutionPreferences>,
    pub context_profile: String,
    pub api_actions: Vec<CustomGptApiAction>,
    /// Proyecto al que van los chats nuevos que eligen este GPT.
    pub default_project_id: Option<String>,
    pub version_no: i64,
    pub created_at: String,
    pub updated_at: String,
}

/// Revisión guardada de un GPT personal, tal como se muestra en su historial.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomGptVersionView {
    pub id: String,
    pub version_no: i64,
    pub icon_ref: String,
    pub instructions: String,
    pub conversation_starters: Vec<String>,
    pub preferred_model: Option<String>,
    pub execution_profile: Option<ConversationExecutionPreferences>,
    pub context_profile: String,
    pub api_actions: Vec<CustomGptApiAction>,
    pub created_at: String,
    /// Verdadero solo para la versión que se usaría ahora mismo.
    pub active: bool,
    pub tool_permissions: CustomGptToolPermissions,
    /// Respuestas que quedaron congeladas con esta versión exacta.
    pub task_count: i64,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomGptToolPermissions {
    pub run_code: String,
    pub rename_conversation: String,
    #[serde(default = "default_denied_permission")]
    pub read_authorized_folders: String,
    #[serde(default = "default_denied_permission")]
    pub modify_authorized_files: String,
    #[serde(default = "default_denied_permission")]
    pub create_scheduled_tasks: String,
    #[serde(default = "default_denied_permission")]
    pub call_external_apis: String,
}

pub(crate) fn default_denied_permission() -> String {
    "deny".to_owned()
}

impl Default for CustomGptToolPermissions {
    fn default() -> Self {
        Self {
            run_code: "deny".to_owned(),
            rename_conversation: "deny".to_owned(),
            read_authorized_folders: "deny".to_owned(),
            modify_authorized_files: "deny".to_owned(),
            create_scheduled_tasks: "deny".to_owned(),
            call_external_apis: "deny".to_owned(),
        }
    }
}

impl CustomGptToolPermissions {
    pub fn requires_confirmation(&self, tool_name: &str) -> bool {
        match tool_name {
            "run_code" => self.run_code == "confirm",
            "rename_conversation" => self.rename_conversation == "confirm",
            "list_authorized_folders" | "read_authorized_file" => {
                self.read_authorized_folders == "confirm"
            }
            "replace_authorized_file" => self.modify_authorized_files == "confirm",
            "create_scheduled_task" => self.create_scheduled_tasks == "confirm",
            "call_external_api" => self.call_external_apis == "confirm",
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PortableCustomGpt {
    pub(crate) schema_version: i64,
    pub(crate) name: String,
    pub(crate) description: Option<String>,
    #[serde(default = "default_custom_gpt_icon")]
    pub(crate) icon_ref: String,
    pub(crate) instructions: String,
    #[serde(default)]
    pub(crate) conversation_starters: Vec<String>,
    #[serde(default = "default_custom_gpt_context_profile")]
    pub(crate) context_profile: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub(crate) knowledge: Vec<PortableCustomGptKnowledge>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub(crate) struct PortableCustomGptKnowledge {
    pub(crate) category: String,
    pub(crate) content: String,
}

#[derive(Debug, Clone)]
pub struct CustomGptPortableExport {
    pub json: String,
    pub included_knowledge: usize,
    pub excluded_sensitive: usize,
    pub excluded_disabled: usize,
    pub excluded_files: usize,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomGptImportReport {
    pub custom_gpt: CustomGptView,
    pub imported_knowledge: usize,
    pub knowledge_requires_review: bool,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct CustomGptContext {
    pub custom_gpt_id: String,
    pub version_id: String,
    pub name: String,
    #[serde(default = "default_custom_gpt_icon")]
    pub icon_ref: String,
    pub version_no: i64,
    pub instructions: String,
    pub tool_permissions: CustomGptToolPermissions,
    /// Se congela con la versión: cambiar el GPT no altera respuestas ya pedidas.
    #[serde(default)]
    pub preferred_model: Option<String>,
    /// También se congela con la versión para que una tarea sea reproducible.
    #[serde(default)]
    pub execution_profile: Option<ConversationExecutionPreferences>,
    #[serde(default = "default_custom_gpt_context_profile")]
    pub context_profile: String,
    #[serde(default)]
    pub api_actions: Vec<CustomGptApiAction>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectKnowledgeOverview {
    pub project: ProjectSummary,
    pub files: Vec<AttachmentView>,
    pub file_usages: Vec<ProjectFileUsageView>,
    pub memories: Vec<MemoryItemView>,
    pub memory_enabled: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectFileUsageView {
    pub attachment_id: String,
    pub conversations: Vec<ConversationSummary>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ProjectInstructionContext {
    pub project_id: String,
    pub project_name: String,
    pub instructions: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuditEventView {
    pub id: i64,
    pub category: String,
    pub summary: String,
    pub severity: String,
    pub actor: String,
    pub conversation_title: Option<String>,
    pub occurred_at: String,
}

/// Referencia mínima a un run de Athena que quedó abierto al cerrar ChatyGPT.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AthenaRunRecordado {
    pub run_id: String,
    pub objetivo: String,
    pub workspace: String,
    pub ultima_fase: Option<String>,
    pub iniciado_en: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RecoveryItemView {
    pub kind: String,
    pub label: String,
    pub status: String,
    pub conversation_id: Option<String>,
    pub conversation_title: Option<String>,
    pub updated_at: String,
}
