//! Tareas del Broker, lo programado, herramientas y carpetas autorizadas.

use std::path::{Path, MAIN_SEPARATOR};

use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone)]
pub struct BrokerTaskRecord {
    pub id: String,
    pub remote_task_id: Option<String>,
    pub request: Value,
    pub consecutive_poll_errors: u32,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LocalTaskSnapshot {
    pub id: String,
    pub activity: String,
    pub remote_task_id: Option<String>,
    pub remote_status: String,
    pub local_state: String,
    pub consecutive_poll_errors: u32,
    pub result: Option<Value>,
    pub error: Option<Value>,
    pub progress: TaskProgressView,
    pub pending_tool_calls: Vec<ToolCallView>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledRunView {
    pub id: String,
    pub due_at: String,
    pub status: String,
    pub broker_task_id: Option<String>,
    pub workflow_run_id: Option<String>,
    pub attempt: i64,
    pub result: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskView {
    pub id: String,
    pub name: String,
    pub target_kind: String,
    pub conversation_id: Option<String>,
    pub conversation_title: Option<String>,
    pub workflow_id: Option<String>,
    pub workflow_name: Option<String>,
    pub workflow_version_no: Option<i64>,
    pub prompt: String,
    pub schedule_expression: String,
    pub timezone: String,
    pub enabled: bool,
    pub confirmed_at: Option<String>,
    pub next_run_at: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub runs: Vec<ScheduledRunView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledRunPageView {
    pub items: Vec<ScheduledRunView>,
    pub total: i64,
    pub page: i64,
    pub page_size: i64,
    pub sort: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ScheduledTaskTemplateView {
    pub id: String,
    pub name: String,
    pub prompt: String,
    pub schedule_expression: String,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ScheduledHistoryExportRow {
    pub task_name: String,
    pub conversation_title: String,
    pub prompt: String,
    pub schedule_expression: String,
    pub timezone: String,
    pub run_id: String,
    pub due_at: String,
    pub status: String,
    pub attempt: i64,
    pub result: Option<Value>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct ScheduledClaim {
    pub run_id: String,
    pub scheduled_task_id: String,
    pub target_kind: String,
    pub conversation_id: Option<String>,
    pub workflow_id: Option<String>,
    pub workflow_version_id: Option<String>,
    pub prompt: String,
}

#[derive(Debug, Clone)]
pub struct ScheduledCancellationTarget {
    pub scheduled_task_id: String,
    pub broker_task_id: Option<String>,
    pub workflow_run_id: Option<String>,
}

#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TaskProgressView {
    pub phase: Option<String>,
    pub invocations_completed: Option<i64>,
    pub invocations_total: Option<i64>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationExecutionPreferences {
    pub data_classification: String,
    pub strategy: String,
    pub preset: String,
    pub max_cost_usd: f64,
    pub long_context: String,
    #[serde(default = "default_execution_priority")]
    pub priority: u16,
}

pub(crate) fn default_execution_priority() -> u16 {
    100
}

impl Default for ConversationExecutionPreferences {
    fn default() -> Self {
        Self {
            data_classification: "internal".to_owned(),
            strategy: "single".to_owned(),
            preset: "fast".to_owned(),
            max_cost_usd: 0.1,
            long_context: "fail".to_owned(),
            priority: default_execution_priority(),
        }
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolCallView {
    pub tool_call_id: String,
    pub name: String,
    pub arguments: Value,
    pub status: String,
    /// Expediente durable de la confirmación exigida antes de ejecutar.
    pub confirmation: Option<ConfirmationRequestView>,
}

/// Proyección del expediente de confirmación que ve la persona antes de decidir.
///
/// Reúne los siete elementos que la aplicación debe mostrar: acción, herramienta,
/// recursos afectados, datos que se enviarán, destino, consecuencias y alcance.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConfirmationRequestView {
    pub id: String,
    pub action_type: String,
    pub tool_name: Option<String>,
    pub resources: Value,
    pub disclosure: Value,
    pub consequences: String,
    pub status: String,
    pub requested_at: String,
    pub resolved_at: Option<String>,
}

#[derive(Debug, Clone)]
pub struct ToolOutcomeRecord {
    pub tool_call_id: String,
    pub status: String,
    pub content: String,
}

/// Carpeta que la persona autorizó explícitamente para escribir en ella.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AuthorizedFolderView {
    pub id: String,
    pub path: String,
    pub display_name: String,
    pub permissions: Value,
    pub granted_at: String,
    pub revoked_at: Option<String>,
}

/// Clave estable de una carpeta para comparar autorizaciones.
///
/// Canonicaliza cuando la ruta existe, retira el prefijo extendido de Windows y
/// normaliza mayúsculas, porque su sistema de archivos no las distingue. Una
/// ruta que todavía no existe se compara tal cual, nunca se inventa.
pub(crate) fn folder_key(path: &Path) -> String {
    let canonical = path.canonicalize().unwrap_or_else(|_| path.to_path_buf());
    let rendered = canonical.to_string_lossy().into_owned();
    let rendered = rendered
        .strip_prefix(r"\\?\")
        .map(str::to_owned)
        .unwrap_or(rendered);
    let trimmed = rendered
        .trim_end_matches(MAIN_SEPARATOR)
        .trim_end_matches('/')
        .to_owned();
    let trimmed = if trimmed.is_empty() {
        rendered
    } else {
        trimmed
    };
    if cfg!(windows) {
        trimmed.to_lowercase()
    } else {
        trimmed
    }
}

/// Describe una acción propuesta en los términos que exige una confirmación.
///
/// Devuelve el tipo de acción, los recursos afectados, la revelación —datos que
/// se enviarán, destino y alcance temporal— y las consecuencias. Una herramienta
/// desconocida recibe la descripción más restrictiva posible en lugar de una
/// genérica tranquilizadora.
pub(crate) fn confirmation_blueprint(
    tool_name: &str,
    arguments: &Value,
    conversation_id: Option<&str>,
) -> (String, Value, Value, String) {
    if tool_name.starts_with("api_action_") {
        let url = arguments
            .get("url")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let destination = url::Url::parse(url)
            .ok()
            .and_then(|parsed| parsed.host_str().map(str::to_owned))
            .unwrap_or_else(|| "Destino externo no válido".to_owned());
        let values = arguments
            .as_object()
            .map(|object| {
                object
                    .iter()
                    .filter(|(key, _)| {
                        !matches!(key.as_str(), "url" | "credential_ref" | "auth_mode")
                    })
                    .map(|(key, value)| serde_json::json!({"label": key, "value": value}))
                    .collect::<Vec<_>>()
            })
            .unwrap_or_default();
        return (
            "external_api.action".to_owned(),
            serde_json::json!({"kind": "external_service", "label": destination}),
            serde_json::json!({
                "action_label": "Ejecutar una acción API configurada",
                "data_sent": values,
                "destination": "external",
                "destination_label": destination,
                "scope": "one_time",
                "scope_label": "Permitir una vez, solo esta ejecución",
                "credential_label": arguments.get("credential_ref").and_then(Value::as_str)
            }),
            format!(
                "ChatyGPT consultará {url} mediante HTTPS GET y enviará los parámetros visibles. {}La respuesta textual volverá al GPT para este turno.",
                arguments
                    .get("credential_ref")
                    .and_then(Value::as_str)
                    .map(|alias| format!("Usará la credencial protegida «{alias}» sin mostrarla al modelo. "))
                    .unwrap_or_default()
            )
        );
    }
    match tool_name {
        "rename_conversation" => {
            let proposed = arguments
                .get("title")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (
                "conversation.rename".to_owned(),
                serde_json::json!({
                    "kind": "conversation",
                    "conversation_id": conversation_id,
                    "label": "La conversación abierta"
                }),
                serde_json::json!({
                    "action_label": "Renombrar la conversación",
                    "data_sent": [{"label": "Título propuesto", "value": proposed}],
                    "destination": "local",
                    "destination_label": "Solo esta aplicación; nada sale del equipo",
                    "scope": "one_time",
                    "scope_label": "Permitir una vez, solo para esta propuesta"
                }),
                "El título de la conversación se sustituirá por el propuesto. \
                 Es reversible: puedes volver a cambiarlo a mano cuando quieras."
                    .to_owned(),
            )
        }
        "list_authorized_folders" => {
            let folder = arguments
                .get("folder_id")
                .and_then(Value::as_str)
                .unwrap_or("Solo nombres de carpetas autorizadas");
            let relative = arguments
                .get("relative_path")
                .and_then(Value::as_str)
                .unwrap_or("Raíz");
            (
                "folder.list".to_owned(),
                serde_json::json!({"kind": "folder", "label": folder}),
                serde_json::json!({
                    "action_label": "Listar una carpeta autorizada",
                    "data_sent": [{"label": "Subcarpeta", "value": relative}],
                    "destination": "broker_local",
                    "destination_label": "Broker AI, restringido a modelos locales",
                    "scope": "one_time",
                    "scope_label": "Permitir una vez, solo este listado"
                }),
                "El modelo verá nombres de archivos y carpetas, pero no rutas absolutas ni contenidos.".to_owned(),
            )
        }
        "read_authorized_file" => {
            let relative = arguments
                .get("relative_path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (
                "file.read".to_owned(),
                serde_json::json!({"kind": "file", "label": relative}),
                serde_json::json!({
                    "action_label": "Leer un archivo autorizado",
                    "data_sent": [{"label": "Archivo relativo", "value": relative}],
                    "destination": "broker_local",
                    "destination_label": "Broker AI, restringido a modelos locales",
                    "scope": "one_time",
                    "scope_label": "Permitir una vez, solo este archivo"
                }),
                "El contenido del archivo se enviará al modelo local para responder a esta petición.".to_owned(),
            )
        }
        "replace_authorized_file" => {
            let relative = arguments
                .get("relative_path")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let expected_hash = arguments
                .get("expected_sha256")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let new_content = arguments
                .get("content")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (
                "file.replace".to_owned(),
                serde_json::json!({"kind": "file", "label": relative}),
                serde_json::json!({
                    "action_label": "Reemplazar el contenido de un archivo autorizado",
                    "data_sent": [
                        {"label": "Archivo relativo", "value": relative},
                        {"label": "Huella esperada", "value": expected_hash},
                        {"label": "Nuevo tamaño", "value": format!("{} caracteres", new_content.chars().count())}
                    ],
                    "destination": "local_file",
                    "destination_label": "Archivo local dentro de una carpeta autorizada",
                    "scope": "one_time",
                    "scope_label": "Permitir una vez, solo este reemplazo"
                }),
                "El archivo existente se reemplazará de forma atómica. Si cambió desde que el GPT lo leyó, la operación se rechazará.".to_owned(),
            )
        }
        "call_external_api" => {
            let url = arguments
                .get("url")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let destination = url::Url::parse(url)
                .ok()
                .and_then(|parsed| parsed.host_str().map(str::to_owned))
                .unwrap_or_else(|| "Destino externo no válido".to_owned());
            (
                "external_api.get".to_owned(),
                serde_json::json!({"kind": "external_service", "label": destination}),
                serde_json::json!({
                    "action_label": "Consultar una API externa",
                    "data_sent": [{"label": "URL HTTPS completa", "value": url}],
                    "destination": "external",
                    "destination_label": destination,
                    "scope": "one_time",
                    "scope_label": "Permitir una vez, solo esta consulta GET"
                }),
                "ChatyGPT enviará una petición HTTPS GET sin credenciales ni cuerpo. La respuesta textual volverá al modelo para completar este turno.".to_owned(),
            )
        }
        "create_scheduled_task" => {
            let name = arguments
                .get("name")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let prompt = arguments
                .get("prompt")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let due_at = arguments
                .get("due_at")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let timezone = arguments
                .get("timezone")
                .and_then(Value::as_str)
                .unwrap_or_default();
            (
                "scheduled_task.create".to_owned(),
                serde_json::json!({
                    "kind": "conversation",
                    "conversation_id": conversation_id,
                    "label": "La conversación abierta"
                }),
                serde_json::json!({
                    "action_label": "Crear una tarea programada",
                    "data_sent": [
                        {"label": "Nombre", "value": name},
                        {"label": "Instrucción", "value": prompt},
                        {"label": "Fecha", "value": due_at},
                        {"label": "Zona horaria", "value": timezone}
                    ],
                    "destination": "local_scheduler",
                    "destination_label": "Programador local de ChatyGPT y Broker AI al ejecutarse",
                    "scope": "persistent_once",
                    "scope_label": "Una ejecución futura; permanecerá activa hasta ejecutarse o cancelarse"
                }),
                "La instrucción se enviará automáticamente a Broker AI en la fecha indicada, aunque no vuelvas a confirmarla entonces.".to_owned(),
            )
        }
        other => (
            "tool.unknown".to_owned(),
            serde_json::json!({
                "kind": "unknown",
                "conversation_id": conversation_id,
                "label": "Recursos no declarados"
            }),
            serde_json::json!({
                "action_label": format!("Ejecutar la herramienta {other}"),
                "data_sent": [{
                    "label": "Argumentos recibidos",
                    "value": arguments.to_string()
                }],
                "destination": "unknown",
                "destination_label": "Destino no declarado por la aplicación",
                "scope": "one_time",
                "scope_label": "Permitir una vez"
            }),
            "ChatyGPT no reconoce esta herramienta y no puede anticipar sus \
             consecuencias. Recházala salvo que sepas exactamente qué hace."
                .to_owned(),
        ),
    }
}
