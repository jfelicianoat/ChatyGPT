use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::collections::HashMap;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Routing,
    Planning,
    ResourcePlanning,
    Converting,
    Chunking,
    Generating,
    Proposing,
    Evaluating,
    Debating,
    Synthesizing,
    Verifying,
    WaitingForMemory,
    WaitingForDependencies,
    WaitingForTools,
    Completed,
    Failed,
    Cancelled,
    #[serde(other)]
    Unknown,
}

impl TaskStatus {
    pub fn is_terminal(&self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Queued => "queued",
            Self::Routing => "routing",
            Self::Planning => "planning",
            Self::ResourcePlanning => "resource_planning",
            Self::Converting => "converting",
            Self::Chunking => "chunking",
            Self::Generating => "generating",
            Self::Proposing => "proposing",
            Self::Evaluating => "evaluating",
            Self::Debating => "debating",
            Self::Synthesizing => "synthesizing",
            Self::Verifying => "verifying",
            Self::WaitingForMemory => "waiting_for_memory",
            Self::WaitingForDependencies => "waiting_for_dependencies",
            Self::WaitingForTools => "waiting_for_tools",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
            Self::Unknown => "working",
        }
    }
}

impl TaskState {
    /// Valida el núcleo estable de `GET /tasks/{id}` antes de que el resto de
    /// la aplicación tome decisiones con él.
    ///
    /// No se aplica `deny_unknown_fields`: el contrato 2.8 es aditivo y los
    /// estados intermedios pueden crecer. Sí se comprueban las invariantes que
    /// ChatyGPT usa para progreso, herramientas y reintentos.
    pub fn from_contract_value(value: Value, expected_task_id: &str) -> Result<Self, String> {
        let object = value
            .as_object()
            .ok_or_else(|| "la respuesta debe ser un objeto JSON".to_owned())?;

        let task_id = required_non_empty_string(object, "task_id")?;
        if task_id != expected_task_id {
            return Err("task_id no coincide con la tarea consultada".to_owned());
        }

        let kind = required_non_empty_string(object, "kind")?;
        if !matches!(kind, "inference" | "ingestion") {
            return Err("kind no pertenece al contrato de tareas".to_owned());
        }
        let status = required_non_empty_string(object, "status")?;
        required_nullable_string(object, "request_id")?;
        required_non_empty_string(object, "created_at")?;
        required_non_empty_string(object, "updated_at")?;
        optional_enum(
            object,
            "execution_strategy",
            &["single", "mixture_of_agents", "agent", "auto"],
        )?;
        optional_enum(
            object,
            "execution_preset",
            &["fast", "slow", "standard", "verified", "high_stakes"],
        )?;
        optional_enum(object, "selection_mode", &["auto", "manual", "hybrid"])?;

        let progress = object
            .get("progress")
            .and_then(Value::as_object)
            .ok_or_else(|| "progress debe ser un objeto".to_owned())?;
        required_non_empty_string(progress, "phase")?;
        if kind == "inference" {
            required_non_negative_integer(progress, "invocations_completed")?;
            required_non_negative_integer(progress, "invocations_total")?;
        }

        required_nullable_object(object, "result")?;
        required_nullable_object(object, "error")?;

        if status == "waiting_for_tools" {
            let result = object
                .get("result")
                .and_then(Value::as_object)
                .ok_or_else(|| "result debe describir las herramientas pendientes".to_owned())?;
            if result.get("status").and_then(Value::as_str) != Some("waiting_for_tools") {
                return Err("result.status debe ser waiting_for_tools".to_owned());
            }
            let calls = result
                .get("pending_tool_calls")
                .and_then(Value::as_array)
                .filter(|calls| !calls.is_empty())
                .ok_or_else(|| "result.pending_tool_calls debe contener llamadas".to_owned())?;
            for call in calls {
                let call = call
                    .as_object()
                    .ok_or_else(|| "cada llamada pendiente debe ser un objeto".to_owned())?;
                required_non_empty_string(call, "id")?;
                required_non_empty_string(call, "name")?;
                if !call.get("arguments").is_some_and(Value::is_object) {
                    return Err("cada llamada pendiente necesita arguments".to_owned());
                }
            }
            if object.get("execution_strategy").and_then(Value::as_str) == Some("agent") {
                required_non_negative_integer(progress, "agent_iteration")?;
                required_positive_integer(progress, "agent_max_iterations")?;
            }
        }

        if status == "failed" {
            let error = object
                .get("error")
                .and_then(Value::as_object)
                .ok_or_else(|| "error debe describir el fallo terminal".to_owned())?;
            required_non_empty_string(error, "code")?;
            if !error.get("message").is_some_and(Value::is_string) {
                return Err("error.message debe ser texto".to_owned());
            }
            if !error.get("retryable").is_some_and(Value::is_boolean) {
                return Err("error.retryable debe indicar si se puede reintentar".to_owned());
            }
        }

        serde_json::from_value(value).map_err(|error| error.to_string())
    }
}

fn required_non_empty_string<'a>(
    object: &'a serde_json::Map<String, Value>,
    field: &str,
) -> Result<&'a str, String> {
    object
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.trim().is_empty())
        .ok_or_else(|| format!("{field} debe ser texto no vacío"))
}

fn required_nullable_string(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    match object.get(field) {
        Some(Value::Null | Value::String(_)) => Ok(()),
        _ => Err(format!("{field} debe ser texto o null")),
    }
}

fn required_nullable_object(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    match object.get(field) {
        Some(Value::Null | Value::Object(_)) => Ok(()),
        _ => Err(format!("{field} debe ser un objeto o null")),
    }
}

fn optional_enum(
    object: &serde_json::Map<String, Value>,
    field: &str,
    allowed: &[&str],
) -> Result<(), String> {
    match object.get(field) {
        None | Some(Value::Null) => Ok(()),
        Some(Value::String(value)) if allowed.contains(&value.as_str()) => Ok(()),
        _ => Err(format!("{field} contiene un valor no admitido")),
    }
}

fn required_non_negative_integer(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    match object.get(field).and_then(Value::as_i64) {
        Some(value) if value >= 0 => Ok(()),
        _ => Err(format!("{field} debe ser un entero no negativo")),
    }
}

fn required_positive_integer(
    object: &serde_json::Map<String, Value>,
    field: &str,
) -> Result<(), String> {
    match object.get(field).and_then(Value::as_i64) {
        Some(value) if value > 0 => Ok(()),
        _ => Err(format!("{field} debe ser un entero positivo")),
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskAccepted {
    pub task_id: String,
    pub status: TaskStatus,
    pub execution_strategy: String,
    pub execution_preset: String,
    pub selection_mode: String,
    pub status_url: String,
    pub cancel_url: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TaskState {
    pub task_id: String,
    #[serde(default)]
    pub kind: Option<String>,
    pub status: TaskStatus,
    #[serde(default)]
    pub request_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    #[serde(default)]
    pub execution_strategy: Option<String>,
    #[serde(default)]
    pub execution_preset: Option<String>,
    #[serde(default)]
    pub selection_mode: Option<String>,
    #[serde(default)]
    pub progress: Value,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct BrokerCapabilities {
    #[serde(default)]
    pub contract_version: String,
    #[serde(default)]
    pub derived_data_boundary: bool,
    #[serde(default)]
    pub work_lanes: Vec<String>,
    #[serde(default)]
    pub strategies: Vec<String>,
    #[serde(default)]
    pub presets: Value,
    #[serde(default)]
    pub scheduling_by_preset: Value,
    #[serde(default)]
    pub agent_skills: Vec<String>,
    #[serde(default)]
    pub agent_skills_egress: Vec<String>,
    #[serde(default)]
    pub task_dependencies: bool,
    #[serde(default)]
    pub sandbox_run_code: bool,
    #[serde(default)]
    pub file_ingestion: bool,
    #[serde(default)]
    pub ingestion_formats: HashMap<String, Vec<String>>,
    #[serde(default)]
    pub long_context_map_reduce: bool,
    #[serde(default)]
    pub max_active_workflows: Option<u64>,
    /// Campo histórico que algunos Brokers publican. El contrato 2.7 ya
    /// garantiza `client_tools` dentro de la estrategia `agent`, por lo que su
    /// ausencia no equivale a `false`.
    #[serde(default)]
    pub client_tool_passthrough: Option<bool>,
}

#[cfg(test)]
mod tests {
    use super::{BrokerCapabilities, FileAccepted, TaskState, TaskStatus};
    use serde_json::Value;

    #[test]
    fn contract_2_6_accepts_ingestion_states_with_nullable_execution_fields() {
        let state: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "task-ingestion",
            "kind": "ingestion",
            "status": "converting",
            "request_id": null,
            "created_at": "2026-07-26T10:00:00Z",
            "updated_at": "2026-07-26T10:00:01Z",
            "execution_strategy": null,
            "execution_preset": null,
            "selection_mode": null,
            "progress": {"phase": "converting"},
            "result": null,
            "error": null
        }))
        .expect("2.6 ingestion state should deserialize");

        assert!(matches!(state.status, TaskStatus::Converting));
        assert_eq!(state.kind.as_deref(), Some("ingestion"));
        assert!(state.execution_strategy.is_none());
    }

    #[test]
    fn contract_2_6_discovers_data_boundary_lanes_and_long_context() {
        let capabilities: BrokerCapabilities = serde_json::from_value(serde_json::json!({
            "contract_version": "2.6",
            "derived_data_boundary": true,
            "work_lanes": ["inference", "ingestion"],
            "strategies": ["single", "agent", "auto"],
            "long_context_map_reduce": true,
            "max_active_workflows": 1
        }))
        .expect("2.6 capabilities should deserialize");

        assert!(capabilities.derived_data_boundary);
        assert_eq!(capabilities.work_lanes, ["inference", "ingestion"]);
        assert!(capabilities.long_context_map_reduce);
        assert_eq!(capabilities.max_active_workflows, Some(1));
    }

    #[test]
    fn contract_accepts_grouped_ingestion_formats_and_additive_fields() {
        let capabilities = serde_json::from_value::<BrokerCapabilities>(serde_json::json!({
            "contract_version": "2.7",
            "file_ingestion": true,
            "ingestion_formats": {
                "documents": ["pdf", "docx"],
                "tabular": ["csv", "tsv", "xlsx"]
            },
            "future_capability": {
                "enabled": true
            }
        }));

        let capabilities = capabilities
            .expect("grouped ingestion_formats must follow the Broker capabilities contract");
        assert_eq!(capabilities.ingestion_formats["documents"], ["pdf", "docx"]);
        assert_eq!(
            capabilities.ingestion_formats["tabular"],
            ["csv", "tsv", "xlsx"]
        );
        assert_eq!(capabilities.client_tool_passthrough, None);
    }

    #[test]
    fn contract_2_7_accepts_the_documented_minimal_file_response() {
        let accepted: FileAccepted = serde_json::from_value(serde_json::json!({
            "file_id": "file-abc",
            "status": "received",
            "created": true,
            "status_url": "/api/v1/files/file-abc",
            "describe_images": false
        }))
        .expect("the abbreviated response published for clients must deserialize");

        assert_eq!(accepted.file_id, "file-abc");
        assert!(accepted.filename.is_empty());
        assert_eq!(accepted.size_bytes, 0);
        assert!(accepted.sha256.is_empty());
        assert_eq!(accepted.describe_images, Some(false));
    }

    #[test]
    fn contract_2_7_treats_memory_wait_as_non_terminal() {
        let state: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "task-memory-wait",
            "kind": "inference",
            "status": "waiting_for_memory",
            "request_id": "request-memory-wait",
            "created_at": "2026-07-28T10:00:00Z",
            "updated_at": "2026-07-28T10:00:01Z",
            "execution_strategy": "single",
            "execution_preset": "fast",
            "selection_mode": "auto",
            "progress": {"phase": "waiting_for_memory"},
            "result": null,
            "error": null
        }))
        .expect("2.7 memory wait state should deserialize");

        assert_eq!(state.status.as_str(), "waiting_for_memory");
        assert!(!state.status.is_terminal());
    }

    #[test]
    fn contract_2_8_discovers_dependencies_egress_and_waits_without_finishing() {
        let capabilities: BrokerCapabilities = serde_json::from_value(serde_json::json!({
            "contract_version": "2.8",
            "agent_skills": ["web_search", "calculator"],
            "agent_skills_egress": ["web_search", "fetch_url"],
            "task_dependencies": true
        }))
        .expect("2.8 capabilities should deserialize");
        assert!(capabilities.task_dependencies);
        assert_eq!(
            capabilities.agent_skills_egress,
            ["web_search", "fetch_url"]
        );

        let state = TaskState::from_contract_value(
            serde_json::json!({
                "task_id": "task-dependency-wait",
                "kind": "inference",
                "status": "waiting_for_dependencies",
                "request_id": "request-dependency-wait",
                "created_at": "2026-08-15T10:00:00Z",
                "updated_at": "2026-08-15T10:00:01Z",
                "execution_strategy": "single",
                "execution_preset": "fast",
                "selection_mode": "auto",
                "progress": {
                    "phase": "waiting_for_dependencies",
                    "invocations_completed": 0,
                    "invocations_total": 1
                },
                "result": null,
                "error": null
            }),
            "task-dependency-wait",
        )
        .expect("dependency wait is a valid non-terminal state");
        assert_eq!(state.status.as_str(), "waiting_for_dependencies");
        assert!(!state.status.is_terminal());
    }

    #[test]
    fn contract_2_7_accepts_a_degraded_consensus_result() {
        let state: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "task-degraded-consensus",
            "kind": "inference",
            "status": "completed",
            "request_id": "request-degraded-consensus",
            "created_at": "2026-08-12T20:00:00Z",
            "updated_at": "2026-08-12T20:00:10Z",
            "execution_strategy": "mixture_of_agents",
            "execution_preset": "slow",
            "selection_mode": "auto",
            "progress": {"phase": "completed", "invocations_completed": 4, "invocations_total": 4},
            "result": {
                "assistant_content": "La mejor propuesta disponible.",
                "model_used": {"provider": "ollama", "deployment": "local", "model": "qwen"},
                "consensus": {
                    "synthesized": false,
                    "warnings": ["No fue posible sintetizar con los árbitros disponibles"]
                },
                "arbiter_failures": [
                    {"model": "arbiter-a", "code": "PROVIDER_UNAVAILABLE", "message": "offline"}
                ]
            },
            "error": null
        }))
        .expect("degraded consensus is still a completed result");

        assert!(state.status.is_terminal());
        assert_eq!(
            state
                .result
                .as_ref()
                .and_then(|result| result.pointer("/consensus/synthesized"))
                .and_then(Value::as_bool),
            Some(false)
        );
    }

    #[test]
    fn unknown_intermediate_task_states_remain_pollable() {
        let status: TaskStatus =
            serde_json::from_str("\"future_planning_stage\"").expect("unknown state should parse");

        assert!(!status.is_terminal());
        assert_eq!(status.as_str(), "working");
    }

    #[test]
    fn runtime_contract_rejects_incomplete_progress_and_crossed_task_ids() {
        let incomplete = serde_json::json!({
            "task_id": "task-one",
            "kind": "inference",
            "status": "generating",
            "request_id": "request-one",
            "created_at": "2026-08-14T10:00:00Z",
            "updated_at": "2026-08-14T10:00:01Z",
            "execution_strategy": "single",
            "execution_preset": "fast",
            "selection_mode": "auto",
            "progress": {"phase": "generating"},
            "result": null,
            "error": null
        });
        let error = TaskState::from_contract_value(incomplete, "task-one")
            .expect_err("una inferencia sin contadores no cumple el contrato");
        assert!(error.contains("invocations_completed"));

        let crossed = serde_json::json!({
            "task_id": "task-two",
            "kind": "ingestion",
            "status": "future_ingestion_stage",
            "request_id": null,
            "created_at": "2026-08-14T10:00:00Z",
            "updated_at": "2026-08-14T10:00:01Z",
            "progress": {"phase": "future_ingestion_stage"},
            "result": null,
            "error": null
        });
        let error = TaskState::from_contract_value(crossed, "task-one")
            .expect_err("una respuesta de otra tarea no debe mezclarse");
        assert!(error.contains("task_id"));
    }

    #[test]
    fn runtime_contract_remains_forward_compatible_with_new_working_states() {
        let state = TaskState::from_contract_value(
            serde_json::json!({
                "task_id": "task-future",
                "kind": "inference",
                "status": "future_planning_stage",
                "request_id": "request-future",
                "created_at": "2026-08-14T10:00:00Z",
                "updated_at": "2026-08-14T10:00:01Z",
                "execution_strategy": "single",
                "execution_preset": "fast",
                "selection_mode": "auto",
                "progress": {
                    "phase": "future_planning_stage",
                    "invocations_completed": 0,
                    "invocations_total": 2,
                    "future_detail": true
                },
                "result": null,
                "error": null,
                "future_field": {"enabled": true}
            }),
            "task-future",
        )
        .expect("los estados de trabajo y campos aditivos deben seguir siendo compatibles");

        assert_eq!(state.status.as_str(), "working");
        assert!(!state.status.is_terminal());
    }
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileAccepted {
    pub file_id: String,
    pub status: String,
    #[serde(default)]
    pub filename: String,
    #[serde(default)]
    pub size_bytes: i64,
    #[serde(default)]
    pub sha256: String,
    pub created: bool,
    pub status_url: String,
    #[serde(default)]
    pub describe_images: Option<bool>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct FileState {
    pub file_id: String,
    pub status: String,
    #[serde(default)]
    pub filename: String,
    pub kind: Option<String>,
    pub engine: Option<String>,
    #[serde(default)]
    pub size_bytes: i64,
    #[serde(default)]
    pub sha256: String,
    #[serde(default)]
    pub meta: Value,
    pub error: Option<Value>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    pub markdown_url: Option<String>,
    #[serde(default)]
    pub describe_images: Option<bool>,
}
