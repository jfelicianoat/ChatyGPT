use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use url::Url;
use uuid::Uuid;

use crate::broker::{TaskAccepted, TaskState};
use crate::error::AppError;
use crate::metrics::{self, PerformanceMetric, PerformanceReportView};

const INITIAL_MIGRATION: &str = include_str!("../../migrations/0001_initial.sql");
const ATTACHMENTS_MIGRATION: &str = include_str!("../../migrations/0002_attachments.sql");
const ATTACHMENT_SOURCES_MIGRATION: &str =
    include_str!("../../migrations/0003_attachment_sources.sql");
const MEMORY_SEARCHES_MIGRATION: &str = include_str!("../../migrations/0004_memory_searches.sql");
const SEMANTIC_CHAT_MEMORY_MIGRATION: &str =
    include_str!("../../migrations/0005_semantic_chat_memory.sql");
const CONVERSATION_SUMMARIES_MIGRATION: &str =
    include_str!("../../migrations/0006_conversation_summaries.sql");
const ATTACHMENT_CHUNKS_MIGRATION: &str =
    include_str!("../../migrations/0007_attachment_chunks.sql");
const ATTACHMENT_CONTEXT_STATUS_MIGRATION: &str =
    include_str!("../../migrations/0008_attachment_context_status.sql");
const CONVERSATION_EXECUTION_PREFERENCES_MIGRATION: &str =
    include_str!("../../migrations/0009_conversation_execution_preferences.sql");
const PROJECT_INSTRUCTIONS_MIGRATION: &str =
    include_str!("../../migrations/0010_project_instructions.sql");
const CUSTOM_GPTS_MIGRATION: &str = include_str!("../../migrations/0011_custom_gpts.sql");
const CONVERSATION_CUSTOM_GPTS_MIGRATION: &str =
    include_str!("../../migrations/0012_conversation_custom_gpts.sql");
const CUSTOM_GPT_FILES_MIGRATION: &str = include_str!("../../migrations/0013_custom_gpt_files.sql");
const RESEARCH_RUNS_MIGRATION: &str = include_str!("../../migrations/0014_research_runs.sql");
const SCHEDULED_TASK_TEMPLATES_MIGRATION: &str =
    include_str!("../../migrations/0015_scheduled_task_templates.sql");
const CONFIRMATION_REQUESTS_MIGRATION: &str =
    include_str!("../../migrations/0016_confirmation_requests.sql");
const PERFORMANCE_SAMPLES_MIGRATION: &str =
    include_str!("../../migrations/0017_performance_samples.sql");
const SEMANTIC_RESEARCH_WORKFLOW_MIGRATION: &str =
    include_str!("../../migrations/0018_semantic_research_workflow.sql");
const ATTACHMENT_IMAGE_POLICY_MIGRATION: &str =
    include_str!("../../migrations/0019_attachment_image_policy.sql");
const WORKFLOWS_MIGRATION: &str = include_str!("../../migrations/0020_workflows.sql");
const SCHEDULED_WORKFLOWS_MIGRATION: &str =
    include_str!("../../migrations/0021_scheduled_workflows.sql");
const RECOVER_NON_TERMINAL_TASKS: &str =
    include_str!("../../queries/recover_non_terminal_tasks.sql");
pub const SCHEMA_VERSION: i64 = 21;

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
}

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

fn default_execution_priority() -> u16 {
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
fn folder_key(path: &Path) -> String {
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
struct CustomGptConfiguration {
    schema_version: i64,
    #[serde(default = "default_custom_gpt_icon")]
    icon_ref: String,
    instructions: String,
    conversation_starters: Vec<String>,
    preferred_model: Option<String>,
    tools_enabled: bool,
    /// `None` mantiene el comportamiento histórico: manda la configuración del chat.
    #[serde(default)]
    execution_profile: Option<ConversationExecutionPreferences>,
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

fn default_custom_gpt_icon() -> String {
    "spark".to_owned()
}

fn validated_custom_gpt_icon(icon_ref: Option<&str>) -> Result<String, AppError> {
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
}

impl Default for CustomGptToolPermissions {
    fn default() -> Self {
        Self {
            run_code: "deny".to_owned(),
            rename_conversation: "deny".to_owned(),
        }
    }
}

impl CustomGptToolPermissions {
    pub fn requires_confirmation(&self, tool_name: &str) -> bool {
        match tool_name {
            "run_code" => self.run_code == "confirm",
            "rename_conversation" => self.rename_conversation == "confirm",
            _ => false,
        }
    }
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PortableCustomGpt {
    schema_version: i64,
    name: String,
    description: Option<String>,
    #[serde(default = "default_custom_gpt_icon")]
    icon_ref: String,
    instructions: String,
    #[serde(default)]
    conversation_starters: Vec<String>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    knowledge: Vec<PortableCustomGptKnowledge>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct PortableCustomGptKnowledge {
    category: String,
    content: String,
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

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryItemView {
    pub id: String,
    pub project_id: Option<String>,
    pub project_name: Option<String>,
    pub custom_gpt_id: Option<String>,
    pub custom_gpt_name: Option<String>,
    pub category: String,
    pub content: String,
    pub sensitivity: String,
    pub enabled: bool,
    pub embedding_status: String,
    pub embedding_model: Option<String>,
    pub embedding_error: Option<String>,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemoryOverview {
    pub enabled: bool,
    pub items: Vec<MemoryItemView>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchResultView {
    pub memory_id: String,
    pub content: String,
    pub category: String,
    pub project_name: Option<String>,
    pub sensitivity: String,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MemorySearchView {
    pub id: String,
    pub query: String,
    pub project_id: Option<String>,
    pub status: String,
    pub model: Option<String>,
    pub error: Option<String>,
    pub results: Vec<MemorySearchResultView>,
    pub created_at: String,
}

#[derive(Debug, Clone)]
pub struct SemanticChatWorkflow {
    pub id: String,
    pub conversation_id: String,
    pub user_message_id: String,
    pub assistant_message_id: String,
    pub embedding_task_id: String,
    pub chat_task_id: Option<String>,
    pub user_text: String,
    pub context: Vec<ContextMessage>,
    pub project_instruction: Option<ProjectInstructionContext>,
    pub custom_gpt_context: Option<CustomGptContext>,
    pub attachment_ids: Vec<String>,
    pub tools_enabled: bool,
    pub sandbox_enabled: bool,
    pub execution_preferences: ConversationExecutionPreferences,
    /// Plan de Investigación profunda congelado al enviar el turno.
    ///
    /// `None` es un turno semántico ordinario. Cuando existe, la segunda etapa
    /// y cualquier recuperación posterior aplican este plan tal cual, sin
    /// volver a preguntar al Broker qué herramientas anuncia hoy.
    pub research_plan: Option<Value>,
    pub status: String,
}

#[derive(Debug, Clone)]
pub struct SemanticMemoryMatch {
    pub memory: MemoryItemView,
    pub score: f64,
    pub reason: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSourceView {
    pub kind: String,
    pub label: String,
    pub reason: String,
    pub score: Option<f64>,
    pub estimated_tokens: i64,
    pub excerpt: String,
    pub source_reference: Option<String>,
    pub source_available: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextSnapshotView {
    pub strategy: String,
    pub estimated_tokens: i64,
    pub sources: Vec<ContextSourceView>,
}

struct MemorySearchRecord {
    query: String,
    project_id: Option<String>,
    remote_status: String,
    local_state: String,
    error_json: Option<String>,
    model: Option<String>,
    dimensions: Option<i64>,
    blob: Option<Vec<u8>>,
    created_at: String,
}

fn audit_presentation(event_type: &str) -> (&'static str, &'static str, &'static str) {
    match event_type {
        "project.created" => ("project", "Proyecto creado", "info"),
        "project.renamed" => ("project", "Proyecto renombrado", "info"),
        "project.instructions_updated" => {
            ("project", "Instrucciones del proyecto actualizadas", "info")
        }
        "project.archived" => ("project", "Proyecto archivado", "warning"),
        "project.file_added" => ("project", "Archivo guardado en el proyecto", "info"),
        "project.file_removed" => ("project", "Archivo retirado del proyecto", "warning"),
        "project.file_used" => ("project", "Archivo del proyecto añadido al chat", "info"),
        "custom_gpt.created" => ("gpt", "GPT personal creado", "info"),
        "custom_gpt.version_created" => ("gpt", "Nueva versión del GPT guardada", "info"),
        "custom_gpt.imported" => ("gpt", "GPT personal importado", "info"),
        "custom_gpt.exported" => ("gpt", "GPT personal exportado", "info"),
        "custom_gpt.knowledge_created" => ("gpt", "Conocimiento del GPT añadido", "info"),
        "custom_gpt.knowledge_enabled" => ("gpt", "Conocimiento del GPT activado", "info"),
        "custom_gpt.knowledge_disabled" => ("gpt", "Conocimiento del GPT desactivado", "warning"),
        "custom_gpt.knowledge_deleted" => ("gpt", "Conocimiento del GPT eliminado", "warning"),
        "custom_gpt.file_added" => ("gpt", "Archivo de conocimiento del GPT añadido", "info"),
        "custom_gpt.file_removed" => ("gpt", "Archivo de conocimiento del GPT retirado", "warning"),
        "conversation.custom_gpt_updated" => {
            ("gpt", "GPT personal de la conversación actualizado", "info")
        }
        "conversation.created" => ("conversation", "Conversación creada", "info"),
        "conversation.renamed" => ("conversation", "Conversación renombrada", "info"),
        "conversation.moved" => ("conversation", "Conversación movida", "info"),
        "conversation.archived" => ("conversation", "Conversación archivada", "warning"),
        "conversation.deleted" => ("conversation", "Conversación eliminada", "warning"),
        "attachment.added" => ("attachment", "Adjunto añadido", "info"),
        "attachment.removed" => ("attachment", "Adjunto retirado", "info"),
        "attachment.retry_requested" => ("attachment", "Reintento de adjunto solicitado", "info"),
        "local.prepared" => ("task", "Mensaje preparado para enviar", "info"),
        "remote.accepted" => ("task", "Broker AI aceptó la tarea", "info"),
        "remote.status_changed" => ("task", "Cambió el estado de una tarea", "info"),
        "transport.error" => ("task", "Error temporal de conexión", "error"),
        "local.orphaned" => ("task", "Tarea pendiente marcada para revisión", "warning"),
        "task.abandoned_cancelled" => ("task", "Tarea abandonada cerrada en Broker AI", "warning"),
        "local.tool_decisions_prepared" => ("tool", "Decisiones de herramientas guardadas", "info"),
        "remote.tool_results_accepted" => ("tool", "Broker AI aceptó los resultados", "info"),
        "export.pending" => ("export", "Exportación iniciada", "info"),
        "export.completed" => ("export", "Exportación completada", "info"),
        "export.conflict" => ("export", "Exportación detenida por un conflicto", "warning"),
        "export.failed" => ("export", "Error durante la exportación", "error"),
        "scheduled_template.created" => ("scheduler", "Plantilla de automatización creada", "info"),
        "scheduled_template.deleted" => (
            "scheduler",
            "Plantilla de automatización eliminada",
            "warning",
        ),
        "scheduled_run.manual_requested" => (
            "scheduler",
            "Ejecución programada iniciada manualmente",
            "info",
        ),
        "memory.enabled" => ("memory", "Memoria activada", "info"),
        "memory.disabled" => ("memory", "Memoria desactivada", "warning"),
        "memory.created" => ("memory", "Recuerdo creado", "info"),
        "memory.updated" => ("memory", "Recuerdo actualizado", "info"),
        "memory.item_enabled" => ("memory", "Recuerdo activado", "info"),
        "memory.item_disabled" => ("memory", "Recuerdo desactivado", "warning"),
        "memory.deleted" => ("memory", "Recuerdo eliminado", "warning"),
        "summary.generation_started" => ("conversation", "Generación de resumen iniciada", "info"),
        "summary.draft_ready" => ("conversation", "Borrador de resumen preparado", "info"),
        "summary.draft_updated" => ("conversation", "Borrador de resumen editado", "info"),
        "summary.approved" => ("conversation", "Resumen de conversación aprobado", "info"),
        "conversation.execution_preferences_updated" => {
            ("conversation", "Opciones de ejecución actualizadas", "info")
        }
        _ => ("system", "Actividad registrada", "info"),
    }
}

fn validated_custom_gpt_fields(
    name: &str,
    description: Option<&str>,
    instructions: &str,
) -> Result<(String, Option<String>, String), AppError> {
    let name = name.trim();
    if name.is_empty() || name.chars().count() > 80 {
        return Err(AppError::Validation(
            "el nombre del GPT debe contener entre 1 y 80 caracteres".to_owned(),
        ));
    }
    let description = description
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(str::to_owned);
    if description
        .as_deref()
        .is_some_and(|value| value.chars().count() > 500)
    {
        return Err(AppError::Validation(
            "la descripción del GPT supera el límite de 500 caracteres".to_owned(),
        ));
    }
    let instructions = instructions.trim();
    if instructions.is_empty() || instructions.chars().count() > 12_000 {
        return Err(AppError::Validation(
            "las instrucciones del GPT deben contener entre 1 y 12.000 caracteres".to_owned(),
        ));
    }
    Ok((name.to_owned(), description, instructions.to_owned()))
}

/// Normaliza el modelo preferido de un GPT contra el límite real del Broker.
///
/// `ModelRequirements.preferred_model` admite hasta 128 caracteres; validarlo
/// aquí evita que una configuración inválida se descubra al fallar la tarea.
fn validated_preferred_model(preferred_model: Option<&str>) -> Result<Option<String>, AppError> {
    let Some(model) = preferred_model
        .map(str::trim)
        .filter(|value| !value.is_empty())
    else {
        return Ok(None);
    };
    if model.chars().count() > 128 {
        return Err(AppError::Validation(
            "el modelo preferido supera los 128 caracteres que admite el Broker".to_owned(),
        ));
    }
    if model.chars().any(|character| character.is_whitespace()) {
        return Err(AppError::Validation(
            "el modelo preferido no puede contener espacios".to_owned(),
        ));
    }
    Ok(Some(model.to_owned()))
}

fn validated_conversation_starters(starters: &[String]) -> Result<Vec<String>, AppError> {
    if starters.len() > 6 {
        return Err(AppError::Validation(
            "un GPT puede tener como máximo 6 iniciadores de conversación".to_owned(),
        ));
    }
    let mut normalized = Vec::new();
    let mut seen = HashSet::new();
    for starter in starters {
        let starter = starter.trim();
        if starter.is_empty() {
            continue;
        }
        if starter.chars().count() > 300 {
            return Err(AppError::Validation(
                "cada iniciador puede tener como máximo 300 caracteres".to_owned(),
            ));
        }
        let key = starter.to_lowercase();
        if seen.insert(key) {
            normalized.push(starter.to_owned());
        }
    }
    Ok(normalized)
}

fn validated_custom_gpt_tool_permissions(
    permissions: &CustomGptToolPermissions,
) -> Result<CustomGptToolPermissions, AppError> {
    for (label, effect) in [
        ("Código aislado", permissions.run_code.as_str()),
        (
            "Renombrar conversación",
            permissions.rename_conversation.as_str(),
        ),
    ] {
        if !matches!(effect, "deny" | "confirm") {
            return Err(AppError::Validation(format!(
                "el permiso «{label}» debe estar denegado o requerir confirmación"
            )));
        }
    }
    Ok(permissions.clone())
}

/// Texto principal de una respuesta del contrato actual. `result_markdown` se
/// conserva como alias de lectura para tareas creadas con Brokers anteriores.
fn assistant_result_text(result: &Value) -> Option<&str> {
    result
        .get("assistant_content")
        .and_then(Value::as_str)
        .or_else(|| result.get("result_markdown").and_then(Value::as_str))
}

fn markdown_web_sources(markdown: &str) -> Vec<(String, String)> {
    fn push_source(
        sources: &mut Vec<(String, String)>,
        seen: &mut HashSet<String>,
        title: &str,
        raw_url: &str,
    ) {
        if sources.len() >= 50 || raw_url.chars().count() > 2_048 {
            return;
        }
        let Ok(mut url) = Url::parse(raw_url) else {
            return;
        };
        if !matches!(url.scheme(), "http" | "https")
            || url.host_str().is_none()
            || !url.username().is_empty()
            || url.password().is_some()
        {
            return;
        }
        url.set_fragment(None);
        let normalized = url.to_string();
        if !seen.insert(normalized.clone()) {
            return;
        }
        let title = title.trim();
        let title = if title.is_empty() {
            url.host_str().unwrap_or("Fuente web").to_owned()
        } else {
            title.chars().take(300).collect()
        };
        sources.push((title, normalized));
    }

    let mut sources = Vec::new();
    let mut seen = HashSet::new();
    let mut remaining = markdown;
    while let Some(close_label) = remaining.find("](") {
        let before = &remaining[..close_label];
        let Some(open_label) = before.rfind('[') else {
            remaining = &remaining[close_label + 2..];
            continue;
        };
        let after = &remaining[close_label + 2..];
        let Some(close_url) = after.find(')') else {
            break;
        };
        push_source(
            &mut sources,
            &mut seen,
            &before[open_label + 1..],
            after[..close_url].trim(),
        );
        remaining = &after[close_url + 1..];
    }
    for token in markdown.split_whitespace() {
        let candidate = token.trim_matches(|character: char| {
            matches!(
                character,
                '"' | '\'' | '<' | '>' | '(' | ')' | '[' | ']' | '{' | '}' | ',' | ';'
            )
        });
        if candidate.starts_with("http://") || candidate.starts_with("https://") {
            push_source(&mut sources, &mut seen, "", candidate.trim_end_matches('.'));
        }
    }
    sources
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationMessage {
    pub id: String,
    pub role: String,
    pub status: String,
    pub sequence_no: i64,
    pub broker_task_id: Option<String>,
    pub task_remote_status: Option<String>,
    pub task_local_state: Option<String>,
    pub text: Option<String>,
    pub error: Option<Value>,
    pub model_used: Option<ModelUsedView>,
    pub response_duration_ms: Option<i64>,
    pub usage: Option<Value>,
    pub fallback_used: Option<bool>,
    pub long_context: Option<Value>,
    pub consensus_synthesized: Option<bool>,
    pub consensus_warnings: Vec<String>,
    pub arbiter_failure_count: i64,
    pub sources: Vec<ConversationSource>,
    pub created_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ModelUsedView {
    pub provider: String,
    pub deployment: String,
    pub model: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationSource {
    pub id: String,
    pub title: String,
    pub source_attachment_id: Option<String>,
    pub media_type: Option<String>,
    pub size_bytes: Option<i64>,
    pub url: Option<String>,
    pub quote_text: Option<String>,
    pub claim_text: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchStepView {
    pub id: String,
    pub kind: String,
    pub title: String,
    pub status: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ResearchRunView {
    pub id: String,
    pub broker_task_id: String,
    pub objective: String,
    pub status: String,
    pub steps: Vec<ResearchStepView>,
    pub source_count: i64,
    pub created_at: String,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ConversationView {
    pub id: String,
    pub title: String,
    pub project_id: Option<String>,
    pub custom_gpt_id: Option<String>,
    pub execution_preferences: ConversationExecutionPreferences,
    pub messages: Vec<ConversationMessage>,
    pub research_runs: Vec<ResearchRunView>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct ContextMessage {
    pub message_id: String,
    pub role: String,
    pub text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct AttachmentView {
    pub id: String,
    pub display_name: String,
    pub media_type: Option<String>,
    pub size_bytes: i64,
    pub sha256: String,
    pub broker_file_id: Option<String>,
    pub ingestion_status: String,
    pub ingestion_error: Option<Value>,
    pub context_status: String,
    pub context_error: Option<Value>,
    pub chunk_count: i64,
    pub indexed_characters: i64,
    pub semantic_indexed_chunks: i64,
    pub semantic_index_status: String,
    pub semantic_index_model: Option<String>,
    pub describe_images: Option<bool>,
    pub updated_at: String,
}

#[derive(Debug, Clone)]
pub struct AttachmentRecord {
    pub id: String,
    pub local_path: String,
    pub display_name: String,
    pub media_type: Option<String>,
    pub size_bytes: i64,
    pub sha256: String,
    pub broker_file_id: Option<String>,
    pub ingestion_status: String,
    pub describe_images: Option<bool>,
}

#[derive(Debug, Clone)]
pub struct ConversationExportMetadata {
    pub created_at: String,
    pub updated_at: String,
    pub project: Option<ProjectExportMetadata>,
}

#[derive(Debug, Clone)]
pub struct ProjectExportMetadata {
    pub id: String,
    pub name: String,
}

#[derive(Debug, Clone)]
pub struct AttachmentChunkEmbeddingInput {
    pub id: String,
    pub text: String,
    pub content_sha256: String,
}

#[derive(Debug, Clone)]
pub struct ContextSourceFile {
    pub local_path: String,
    pub display_name: String,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowDefinition {
    pub nodes: Vec<WorkflowNode>,
    pub edges: Vec<WorkflowEdge>,
    /// Contexto del proyecto fijado al publicar. Las referencias se vuelven a
    /// autorizar al ejecutar para que una retirada posterior sea efectiva.
    #[serde(default)]
    pub project_context: Option<WorkflowProjectContext>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowProjectContext {
    pub project_id: String,
    pub project_name: String,
    #[serde(default)]
    pub instructions: Option<String>,
    #[serde(default)]
    pub memory_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNode {
    pub id: String,
    pub kind: String,
    pub label: String,
    pub x: f64,
    pub y: f64,
    #[serde(default)]
    pub custom_gpt_id: Option<String>,
    #[serde(default)]
    pub custom_gpt_version_id: Option<String>,
    #[serde(default)]
    pub custom_gpt_name: Option<String>,
    #[serde(default)]
    pub custom_gpt_icon_ref: Option<String>,
    #[serde(default)]
    pub custom_gpt_instructions: Option<String>,
    #[serde(default)]
    pub preferred_model: Option<String>,
    #[serde(default)]
    pub execution_profile: Option<ConversationExecutionPreferences>,
    /// Identificadores del conocimiento textual activo al publicar.
    /// Se resuelven de nuevo al ejecutar para respetar una revocación posterior.
    #[serde(default)]
    pub custom_gpt_memory_ids: Vec<String>,
    /// Archivos propios del GPT que estaban preparados al publicar.
    /// La pertenencia al GPT se vuelve a comprobar antes de cada uso.
    #[serde(default)]
    pub custom_gpt_attachment_ids: Vec<String>,
    #[serde(default)]
    pub instruction: Option<String>,
    #[serde(default)]
    pub attachment_ids: Vec<String>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowEdge {
    pub id: String,
    pub source: String,
    pub target: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowSummary {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub project_id: Option<String>,
    pub published_version_no: Option<i64>,
    pub node_count: i64,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowView {
    #[serde(flatten)]
    pub summary: WorkflowSummary,
    pub definition: WorkflowDefinition,
}

#[derive(Debug, Clone)]
pub struct WorkflowExecutionRecord {
    pub run_id: String,
    pub workflow_id: String,
    pub version_id: String,
    pub definition: WorkflowDefinition,
    pub input_text: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowNodeRunView {
    pub id: String,
    pub node_id: String,
    pub node_kind: String,
    pub node_label: String,
    pub status: String,
    pub input_text: Option<String>,
    pub output_text: Option<String>,
    pub broker_task_id: Option<String>,
    pub error: Option<Value>,
    pub updated_at: String,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct WorkflowRunView {
    pub id: String,
    pub workflow_id: String,
    pub workflow_version_id: String,
    pub version_no: i64,
    pub status: String,
    pub input_text: String,
    pub outputs: Value,
    pub error: Option<Value>,
    pub node_runs: Vec<WorkflowNodeRunView>,
    pub started_at: Option<String>,
    pub completed_at: Option<String>,
    pub updated_at: String,
}

impl Database {
    pub fn open(path: impl AsRef<Path>) -> Result<Self, AppError> {
        let path = path.as_ref().to_path_buf();
        let mut connection = Connection::open_with_flags(
            &path,
            OpenFlags::SQLITE_OPEN_READ_WRITE
                | OpenFlags::SQLITE_OPEN_CREATE
                | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        connection.pragma_update(None, "journal_mode", "WAL")?;
        connection.pragma_update(None, "synchronous", "NORMAL")?;
        Self::migrate(&mut connection)?;
        Ok(Self { path })
    }

    fn migrate(connection: &mut Connection) -> Result<(), AppError> {
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 1 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(INITIAL_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 1)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 2 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 2)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 3 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_SOURCES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 3)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 4 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(MEMORY_SEARCHES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 4)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 5 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(SEMANTIC_CHAT_MEMORY_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 5)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 6 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONVERSATION_SUMMARIES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 6)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 7 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_CHUNKS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 7)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 8 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_CONTEXT_STATUS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 8)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 9 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONVERSATION_EXECUTION_PREFERENCES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 9)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 10 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(PROJECT_INSTRUCTIONS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 10)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 11 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CUSTOM_GPTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 11)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 12 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONVERSATION_CUSTOM_GPTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 12)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 13 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CUSTOM_GPT_FILES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 13)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 14 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(RESEARCH_RUNS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 14)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 15 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(SCHEDULED_TASK_TEMPLATES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 15)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 16 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(CONFIRMATION_REQUESTS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 16)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 17 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(PERFORMANCE_SAMPLES_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 17)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 18 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(SEMANTIC_RESEARCH_WORKFLOW_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 18)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < 19 {
            let transaction = connection.transaction()?;
            transaction.execute_batch(ATTACHMENT_IMAGE_POLICY_MIGRATION)?;
            transaction.pragma_update(None, "user_version", 19)?;
            transaction.commit()?;
        }
        let current: i64 = connection.pragma_query_value(None, "user_version", |row| row.get(0))?;
        if current < SCHEMA_VERSION {
            let transaction = connection.transaction()?;
            if current < 20 {
                transaction.execute_batch(WORKFLOWS_MIGRATION)?;
            }
            transaction.execute_batch(SCHEDULED_WORKFLOWS_MIGRATION)?;
            transaction.pragma_update(None, "user_version", SCHEMA_VERSION)?;
            transaction.commit()?;
        }
        Ok(())
    }

    fn connect(&self) -> Result<Connection, AppError> {
        let connection = Connection::open_with_flags(
            &self.path,
            OpenFlags::SQLITE_OPEN_READ_WRITE | OpenFlags::SQLITE_OPEN_FULL_MUTEX,
        )?;
        connection.busy_timeout(std::time::Duration::from_secs(5))?;
        connection.pragma_update(None, "foreign_keys", "ON")?;
        Ok(connection)
    }

    pub fn schema_version(&self) -> Result<i64, AppError> {
        Ok(self
            .connect()?
            .pragma_query_value(None, "user_version", |row| row.get(0))?)
    }

    pub fn recover_non_terminal_tasks(&self) -> Result<usize, AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(RECOVER_NON_TERMINAL_TASKS, [])?;
        Ok(changed)
    }

    pub fn recovery_candidates(&self) -> Result<Vec<RecoveryItemView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT bt.remote_status, bt.conversation_id, c.title, bt.updated_at,
                    json_extract(bt.request_json, '$.inference_kind'),
                    json_extract(bt.request_json, '$.content.metadata.source_type')
             FROM broker_tasks bt
             LEFT JOIN conversations c ON c.id = bt.conversation_id
             WHERE bt.remote_status NOT IN ('completed', 'failed', 'cancelled')
               AND bt.local_state != 'orphaned'
             ORDER BY bt.updated_at DESC",
        )?;
        let items = statement
            .query_map([], |row| {
                let conversation_id: Option<String> = row.get(1)?;
                let inference_kind: Option<String> = row.get(4)?;
                let embedding_source: Option<String> = row.get(5)?;
                let is_embedding = inference_kind.as_deref() == Some("embedding");
                Ok(RecoveryItemView {
                    kind: if is_embedding { "embedding" } else { "task" }.to_owned(),
                    label: if embedding_source.as_deref() == Some("conversation_summary") {
                        "Resumen de conversación pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("chat_memory_search") {
                        "Selección semántica de contexto pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("chat_document_search") {
                        "Búsqueda semántica documental pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("attachment_chunk") {
                        "Índice documental pendiente".to_owned()
                    } else if embedding_source.as_deref() == Some("memory_search") {
                        "Búsqueda semántica pendiente".to_owned()
                    } else if is_embedding {
                        "Indexación de memoria pendiente".to_owned()
                    } else if conversation_id.is_some() {
                        "Respuesta pendiente".to_owned()
                    } else {
                        "Prueba de inferencia pendiente".to_owned()
                    },
                    status: row.get(0)?,
                    conversation_id,
                    conversation_title: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(items)
    }

    pub fn prepare_broker_task(
        &self,
        id: &str,
        idempotency_key: &str,
        request: &Value,
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO broker_tasks(
                id, idempotency_key, request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, 'not_submitted', 'created')",
            params![id, idempotency_key, request_json],
        )?;
        connection.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![id],
        )?;
        self.task_record(id)
    }

    pub fn prepare_conversation_summary(
        &self,
        conversation_id: &str,
        summary_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        request: &Value,
        source_through_sequence: i64,
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let active: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL
             )",
            params![conversation_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(AppError::NotFound(format!(
                "conversación activa {conversation_id}"
            )));
        }
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, idempotency_key, request_json,
                remote_status, local_state
             ) VALUES (?1, ?2, ?3, ?4, 'not_submitted', 'created')",
            params![
                local_task_id,
                conversation_id,
                idempotency_key,
                request_json
            ],
        )?;
        transaction.execute(
            "INSERT INTO conversation_summaries(
                id, conversation_id, broker_task_id,
                source_through_sequence, status
             ) VALUES (?1, ?2, ?3, ?4, 'generating')",
            params![
                summary_id,
                conversation_id,
                local_task_id,
                source_through_sequence
            ],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![local_task_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('summary.generation_started', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "summary_id": summary_id,
                    "source_through_sequence": source_through_sequence
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.task_record(local_task_id)
    }

    pub fn conversation_summary_overview(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationSummaryOverview, AppError> {
        let connection = self.connect()?;
        let load = |status_filter: &str| -> Result<Option<ConversationSummaryRevision>, AppError> {
            connection
                .query_row(
                    "SELECT id, status, draft_text, approved_text,
                            source_through_sequence, broker_task_id, updated_at
                     FROM conversation_summaries
                     WHERE conversation_id = ?1 AND status = ?2
                     ORDER BY updated_at DESC, rowid DESC
                     LIMIT 1",
                    params![conversation_id, status_filter],
                    |row| {
                        Ok(ConversationSummaryRevision {
                            id: row.get(0)?,
                            status: row.get(1)?,
                            draft_text: row.get(2)?,
                            approved_text: row.get(3)?,
                            source_through_sequence: row.get(4)?,
                            broker_task_id: row.get(5)?,
                            updated_at: row.get(6)?,
                        })
                    },
                )
                .optional()
                .map_err(AppError::from)
        };
        let candidate = load("draft")?.or(load("generating")?);
        let active = load("approved")?;
        let total_message_count: i64 = connection.query_row(
            "SELECT COUNT(*)
             FROM messages
             WHERE conversation_id = ?1
               AND status = 'complete'
               AND role IN ('user', 'assistant')",
            params![conversation_id],
            |row| row.get(0),
        )?;
        let covered_count = |through_sequence: i64| -> Result<i64, AppError> {
            connection
                .query_row(
                    "SELECT COUNT(*)
                     FROM messages
                     WHERE conversation_id = ?1
                       AND status = 'complete'
                       AND role IN ('user', 'assistant')
                       AND sequence_no <= ?2",
                    params![conversation_id, through_sequence],
                    |row| row.get(0),
                )
                .map_err(AppError::from)
        };
        let active_covered_message_count = active
            .as_ref()
            .map(|revision| covered_count(revision.source_through_sequence))
            .transpose()?
            .unwrap_or(0);
        let candidate_covered_message_count = candidate
            .as_ref()
            .map(|revision| covered_count(revision.source_through_sequence))
            .transpose()?;
        Ok(ConversationSummaryOverview {
            candidate,
            active,
            total_message_count,
            active_covered_message_count,
            remaining_message_count: total_message_count
                .saturating_sub(active_covered_message_count),
            candidate_covered_message_count,
        })
    }

    pub fn update_conversation_summary_draft(
        &self,
        summary_id: &str,
        text: &str,
    ) -> Result<ConversationSummaryOverview, AppError> {
        let text = text.trim();
        if text.is_empty() || text.chars().count() > 10_000 {
            return Err(AppError::Validation(
                "el resumen debe contener entre 1 y 10.000 caracteres".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let conversation_id: String = transaction
            .query_row(
                "SELECT conversation_id FROM conversation_summaries
                 WHERE id = ?1 AND status = 'draft'",
                params![summary_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "solo se puede editar un resumen que esté en borrador".to_owned(),
                )
            })?;
        transaction.execute(
            "UPDATE conversation_summaries
             SET draft_text = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![summary_id, text],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('summary.draft_updated', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({"summary_id": summary_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary_overview(&conversation_id)
    }

    pub fn approve_conversation_summary(
        &self,
        summary_id: &str,
    ) -> Result<ConversationSummaryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (conversation_id, draft_text): (String, String) = transaction
            .query_row(
                "SELECT conversation_id, draft_text
                 FROM conversation_summaries
                 WHERE id = ?1 AND status = 'draft' AND draft_text IS NOT NULL",
                params![summary_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "solo se puede aprobar un resumen que esté en borrador".to_owned(),
                )
            })?;
        transaction.execute(
            "UPDATE conversation_summaries
             SET status = 'superseded', updated_at = datetime('now')
             WHERE conversation_id = ?1 AND status = 'approved'",
            params![conversation_id],
        )?;
        transaction.execute(
            "UPDATE conversation_summaries
             SET status = 'approved', approved_text = ?2,
                 approved_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1",
            params![summary_id, draft_text],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('summary.approved', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({"summary_id": summary_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary_overview(&conversation_id)
    }

    pub fn create_project(
        &self,
        name: &str,
        description: Option<&str>,
    ) -> Result<ProjectSummary, AppError> {
        let id = format!("project_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO projects(id, name, description) VALUES (?1, ?2, ?3)",
            params![id, name, description],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.created', 'user', ?1)",
            params![serde_json::json!({"project_id": id, "name": name}).to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(&id)
    }

    pub fn list_projects(&self) -> Result<Vec<ProjectSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT p.id, p.name, p.description, p.instructions, COUNT(c.id), p.updated_at
             FROM projects p
             LEFT JOIN conversations c
               ON c.project_id = p.id
              AND c.archived_at IS NULL
              AND c.deleted_at IS NULL
             WHERE p.archived_at IS NULL
             GROUP BY p.id, p.name, p.description, p.instructions, p.updated_at
             ORDER BY p.updated_at DESC, p.name COLLATE NOCASE",
        )?;
        let projects = statement
            .query_map([], |row| {
                Ok(ProjectSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    instructions: row.get(3)?,
                    conversation_count: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(projects)
    }

    pub fn create_workflow(
        &self,
        name: &str,
        project_id: Option<&str>,
    ) -> Result<WorkflowView, AppError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(AppError::Validation(
                "el nombre del flujo debe tener entre 1 y 120 caracteres".to_owned(),
            ));
        }
        let connection = self.connect()?;
        if let Some(project_id) = project_id {
            let exists: bool = connection.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        let id = format!("workflow_{}", Uuid::new_v4().simple());
        let input_id = format!("node_{}", Uuid::new_v4().simple());
        let result_id = format!("node_{}", Uuid::new_v4().simple());
        let definition = WorkflowDefinition {
            nodes: vec![
                WorkflowNode {
                    id: input_id.clone(),
                    kind: "input".to_owned(),
                    label: "Entrada".to_owned(),
                    x: 70.0,
                    y: 170.0,
                    custom_gpt_id: None,
                    custom_gpt_version_id: None,
                    custom_gpt_name: None,
                    custom_gpt_icon_ref: None,
                    custom_gpt_instructions: None,
                    preferred_model: None,
                    execution_profile: None,
                    custom_gpt_memory_ids: Vec::new(),
                    custom_gpt_attachment_ids: Vec::new(),
                    instruction: None,
                    attachment_ids: Vec::new(),
                },
                WorkflowNode {
                    id: result_id.clone(),
                    kind: "result".to_owned(),
                    label: "Resultado".to_owned(),
                    x: 650.0,
                    y: 170.0,
                    custom_gpt_id: None,
                    custom_gpt_version_id: None,
                    custom_gpt_name: None,
                    custom_gpt_icon_ref: None,
                    custom_gpt_instructions: None,
                    preferred_model: None,
                    execution_profile: None,
                    custom_gpt_memory_ids: Vec::new(),
                    custom_gpt_attachment_ids: Vec::new(),
                    instruction: None,
                    attachment_ids: Vec::new(),
                },
            ],
            edges: vec![WorkflowEdge {
                id: format!("edge_{}", Uuid::new_v4().simple()),
                source: input_id.clone(),
                target: result_id,
            }],
            project_context: None,
        };
        let definition_json = serde_json::to_string(&definition)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        connection.execute(
            "INSERT INTO workflows(id, name, project_id, draft_definition_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![id, name, project_id, definition_json],
        )?;
        self.workflow_view(&id)
    }

    pub fn list_workflows(&self) -> Result<Vec<WorkflowSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT workflow.id, workflow.name, workflow.description, workflow.project_id,
                    version.version_no,
                    json_array_length(json_extract(workflow.draft_definition_json, '$.nodes')),
                    workflow.updated_at
             FROM workflows workflow
             LEFT JOIN workflow_versions version ON version.id = workflow.published_version_id
             WHERE workflow.archived_at IS NULL
             ORDER BY workflow.updated_at DESC, workflow.name COLLATE NOCASE",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok(WorkflowSummary {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    description: row.get(2)?,
                    project_id: row.get(3)?,
                    published_version_no: row.get(4)?,
                    node_count: row.get(5)?,
                    updated_at: row.get(6)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    pub fn workflow_view(&self, id: &str) -> Result<WorkflowView, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT workflow.id, workflow.name, workflow.description, workflow.project_id,
                        version.version_no, workflow.draft_definition_json, workflow.updated_at
                 FROM workflows workflow
                 LEFT JOIN workflow_versions version ON version.id = workflow.published_version_id
                 WHERE workflow.id = ?1 AND workflow.archived_at IS NULL",
                params![id],
                |row| {
                    let definition_json: String = row.get(5)?;
                    let definition: WorkflowDefinition = serde_json::from_str(&definition_json)
                        .map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                5,
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    Ok(WorkflowView {
                        summary: WorkflowSummary {
                            id: row.get(0)?,
                            name: row.get(1)?,
                            description: row.get(2)?,
                            project_id: row.get(3)?,
                            published_version_no: row.get(4)?,
                            node_count: definition.nodes.len() as i64,
                            updated_at: row.get(6)?,
                        },
                        definition,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo {id}")))
    }

    pub fn update_workflow(
        &self,
        id: &str,
        name: &str,
        description: Option<&str>,
        project_id: Option<&str>,
        definition: &WorkflowDefinition,
    ) -> Result<WorkflowView, AppError> {
        let name = name.trim();
        if name.is_empty() || name.chars().count() > 120 {
            return Err(AppError::Validation(
                "el nombre del flujo debe tener entre 1 y 120 caracteres".to_owned(),
            ));
        }
        let definition_json = serde_json::to_string(definition)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE workflows
             SET name = ?2, description = ?3, project_id = ?4,
                 draft_definition_json = ?5, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, name, description, project_id, definition_json],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("flujo {id}")));
        }
        self.workflow_view(id)
    }

    pub fn publish_workflow(&self, id: &str) -> Result<WorkflowView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (draft_definition_json, project_id): (String, Option<String>) = transaction
            .query_row(
                "SELECT draft_definition_json, project_id FROM workflows
                 WHERE id = ?1 AND archived_at IS NULL",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo {id}")))?;
        let mut definition: WorkflowDefinition = serde_json::from_str(&draft_definition_json)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        definition.project_context = if let Some(project_id) = project_id {
            let project = self.project_summary(&project_id)?;
            let memory = self.memory_overview()?;
            let mut used_characters = 0_usize;
            let memory_ids = if memory.enabled {
                memory
                    .items
                    .into_iter()
                    .filter(|item| item.enabled && item.project_id.as_deref() == Some(&project_id))
                    .filter(|item| {
                        used_characters += item.content.chars().count();
                        used_characters <= 8_000
                    })
                    .take(20)
                    .map(|item| item.id)
                    .collect()
            } else {
                Vec::new()
            };
            Some(WorkflowProjectContext {
                project_id,
                project_name: project.name,
                instructions: project.instructions,
                memory_ids,
            })
        } else {
            None
        };
        for node in &mut definition.nodes {
            if node.kind == "custom_gpt" {
                let custom_gpt_id = node.custom_gpt_id.as_deref().ok_or_else(|| {
                    AppError::Validation(format!(
                        "el nodo «{}» no tiene un GPT seleccionado",
                        node.label
                    ))
                })?;
                let context = self.custom_gpt_context(custom_gpt_id)?;
                node.custom_gpt_version_id = Some(context.version_id);
                node.custom_gpt_name = Some(context.name);
                node.custom_gpt_icon_ref = Some(context.icon_ref);
                node.custom_gpt_instructions = Some(context.instructions);
                node.preferred_model = context.preferred_model;
                node.execution_profile = context.execution_profile;
                let mut used_characters = 0_usize;
                node.custom_gpt_memory_ids = self
                    .custom_gpt_knowledge(custom_gpt_id)?
                    .into_iter()
                    .filter(|item| item.enabled)
                    .filter(|item| {
                        used_characters += item.content.chars().count();
                        used_characters <= 8_000
                    })
                    .take(20)
                    .map(|item| item.id)
                    .collect();
                node.custom_gpt_attachment_ids = self
                    .list_custom_gpt_files(custom_gpt_id)?
                    .into_iter()
                    .filter(|file| {
                        file.ingestion_status == "ready" && file.broker_file_id.is_some()
                    })
                    .map(|file| file.id)
                    .collect();
                let total_files = node
                    .attachment_ids
                    .iter()
                    .chain(node.custom_gpt_attachment_ids.iter())
                    .collect::<HashSet<_>>()
                    .len();
                if total_files > 20 {
                    return Err(AppError::Validation(format!(
                        "el nodo «{}» supera el límite de 20 archivos al sumar los del proyecto y los del GPT",
                        node.label
                    )));
                }
            }
        }
        let definition_json = serde_json::to_string(&definition)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let version_no: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(version_no), 0) + 1 FROM workflow_versions WHERE workflow_id = ?1",
            params![id],
            |row| row.get(0),
        )?;
        let version_id = format!("workflow_version_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO workflow_versions(id, workflow_id, version_no, definition_json)
             VALUES (?1, ?2, ?3, ?4)",
            params![version_id, id, version_no, definition_json],
        )?;
        transaction.execute(
            "UPDATE workflows SET published_version_id = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, version_id],
        )?;
        transaction.commit()?;
        self.workflow_view(id)
    }

    pub fn create_workflow_run(
        &self,
        workflow_id: &str,
        input_text: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        self.create_workflow_run_for_version(workflow_id, None, input_text)
    }

    pub fn create_workflow_run_from_version(
        &self,
        workflow_id: &str,
        workflow_version_id: &str,
        input_text: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        self.create_workflow_run_for_version(workflow_id, Some(workflow_version_id), input_text)
    }

    fn create_workflow_run_for_version(
        &self,
        workflow_id: &str,
        workflow_version_id: Option<&str>,
        input_text: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (version_id, definition_json): (String, String) = transaction
            .query_row(
                "SELECT version.id, version.definition_json
                 FROM workflows workflow
                 JOIN workflow_versions version ON version.workflow_id = workflow.id
                 WHERE workflow.id = ?1 AND workflow.archived_at IS NULL
                   AND version.id = COALESCE(?2, workflow.published_version_id)",
                params![workflow_id, workflow_version_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::Conflict("publica el flujo antes de ejecutarlo".to_owned()))?;
        let definition: WorkflowDefinition = serde_json::from_str(&definition_json)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let run_id = format!("workflow_run_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO workflow_runs(
                id, workflow_id, workflow_version_id, status, input_text, started_at
             ) VALUES (?1, ?2, ?3, 'queued', ?4, datetime('now'))",
            params![run_id, workflow_id, version_id, input_text],
        )?;
        for node in &definition.nodes {
            transaction.execute(
                "INSERT INTO workflow_node_runs(id, run_id, node_id, node_kind, node_label)
                 VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    format!("workflow_node_run_{}", Uuid::new_v4().simple()),
                    run_id,
                    node.id,
                    node.kind,
                    node.label
                ],
            )?;
        }
        transaction.commit()?;
        Ok(WorkflowExecutionRecord {
            run_id,
            workflow_id: workflow_id.to_owned(),
            version_id,
            definition,
            input_text: input_text.to_owned(),
        })
    }

    pub fn workflow_execution_record(
        &self,
        run_id: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT run.id, run.workflow_id, run.workflow_version_id,
                        version.definition_json, run.input_text
                 FROM workflow_runs run
                 JOIN workflow_versions version ON version.id = run.workflow_version_id
                 WHERE run.id = ?1",
                params![run_id],
                |row| {
                    let definition_json: String = row.get(3)?;
                    let definition = serde_json::from_str(&definition_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            3,
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok(WorkflowExecutionRecord {
                        run_id: row.get(0)?,
                        workflow_id: row.get(1)?,
                        version_id: row.get(2)?,
                        definition,
                        input_text: row.get(4)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("ejecución de flujo {run_id}")))
    }

    pub fn retry_workflow_run(
        &self,
        previous_run_id: &str,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (workflow_id, version_id, input_text, definition_json): (
            String,
            String,
            String,
            String,
        ) = transaction
            .query_row(
                "SELECT run.workflow_id, run.workflow_version_id, run.input_text,
                            version.definition_json
                     FROM workflow_runs run
                     JOIN workflow_versions version ON version.id = run.workflow_version_id
                     WHERE run.id = ?1 AND run.status IN ('failed', 'partial_failed')",
                params![previous_run_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?)),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict("solo se pueden reintentar ejecuciones fallidas".to_owned())
            })?;
        let definition: WorkflowDefinition = serde_json::from_str(&definition_json)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let run_id = format!("workflow_run_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO workflow_runs(
                id, workflow_id, workflow_version_id, status, input_text, started_at
             ) VALUES (?1, ?2, ?3, 'queued', ?4, datetime('now'))",
            params![run_id, workflow_id, version_id, input_text],
        )?;
        for node in &definition.nodes {
            let reusable: Option<(String, String)> = if node.kind == "result" {
                None
            } else {
                transaction
                    .query_row(
                        "SELECT input_text, output_text FROM workflow_node_runs
                         WHERE run_id = ?1 AND node_id = ?2 AND status = 'completed'
                           AND input_text IS NOT NULL AND output_text IS NOT NULL",
                        params![previous_run_id, node.id],
                        |row| Ok((row.get(0)?, row.get(1)?)),
                    )
                    .optional()?
            };
            let (status, previous_input, previous_output) = reusable
                .map(|(input, output)| ("completed", Some(input), Some(output)))
                .unwrap_or(("pending", None, None));
            transaction.execute(
                "INSERT INTO workflow_node_runs(
                    id, run_id, node_id, node_kind, node_label, status,
                    input_text, output_text, completed_at
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8,
                           CASE WHEN ?6 = 'completed' THEN datetime('now') END)",
                params![
                    format!("workflow_node_run_{}", Uuid::new_v4().simple()),
                    run_id,
                    node.id,
                    node.kind,
                    node.label,
                    status,
                    previous_input,
                    previous_output
                ],
            )?;
        }
        transaction.commit()?;
        Ok(WorkflowExecutionRecord {
            run_id,
            workflow_id,
            version_id,
            definition,
            input_text,
        })
    }

    pub fn workflow_run(&self, run_id: &str) -> Result<WorkflowRunView, AppError> {
        let connection = self.connect()?;
        let mut run = connection
            .query_row(
                "SELECT run.id, run.workflow_id, run.workflow_version_id, version.version_no,
                        run.status, run.input_text, run.output_json, run.error_json,
                        run.started_at, run.completed_at, run.updated_at
                 FROM workflow_runs run
                 JOIN workflow_versions version ON version.id = run.workflow_version_id
                 WHERE run.id = ?1",
                params![run_id],
                |row| {
                    let output_json: Option<String> = row.get(6)?;
                    let error_json: Option<String> = row.get(7)?;
                    Ok(WorkflowRunView {
                        id: row.get(0)?,
                        workflow_id: row.get(1)?,
                        workflow_version_id: row.get(2)?,
                        version_no: row.get(3)?,
                        status: row.get(4)?,
                        input_text: row.get(5)?,
                        outputs: output_json
                            .and_then(|value| serde_json::from_str(&value).ok())
                            .unwrap_or_else(|| Value::Object(Default::default())),
                        error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                        node_runs: Vec::new(),
                        started_at: row.get(8)?,
                        completed_at: row.get(9)?,
                        updated_at: row.get(10)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("ejecución de flujo {run_id}")))?;
        let mut statement = connection.prepare(
            "SELECT id, node_id, node_kind, node_label, status, input_text, output_text,
                    broker_task_id, error_json, updated_at
             FROM workflow_node_runs WHERE run_id = ?1 ORDER BY rowid",
        )?;
        run.node_runs = statement
            .query_map(params![run_id], |row| {
                let error_json: Option<String> = row.get(8)?;
                Ok(WorkflowNodeRunView {
                    id: row.get(0)?,
                    node_id: row.get(1)?,
                    node_kind: row.get(2)?,
                    node_label: row.get(3)?,
                    status: row.get(4)?,
                    input_text: row.get(5)?,
                    output_text: row.get(6)?,
                    broker_task_id: row.get(7)?,
                    error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                    updated_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(run)
    }

    pub fn list_workflow_runs(&self, workflow_id: &str) -> Result<Vec<WorkflowRunView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM workflow_runs WHERE workflow_id = ?1 ORDER BY created_at DESC LIMIT 25",
        )?;
        let ids = statement
            .query_map(params![workflow_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter().map(|id| self.workflow_run(&id)).collect()
    }

    pub fn update_workflow_run_status(
        &self,
        run_id: &str,
        status: &str,
        outputs: Option<&Value>,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let terminal = matches!(
            status,
            "completed" | "partial_failed" | "failed" | "cancelled"
        );
        connection.execute(
            "UPDATE workflow_runs
             SET status = ?2, output_json = COALESCE(?3, output_json),
                 error_json = ?4,
                 started_at = COALESCE(started_at, datetime('now')),
                 completed_at = CASE WHEN ?5 THEN datetime('now') ELSE completed_at END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                run_id,
                status,
                outputs.map(Value::to_string),
                error.map(Value::to_string),
                terminal
            ],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_workflow_node_run(
        &self,
        run_id: &str,
        node_id: &str,
        status: &str,
        input_text: Option<&str>,
        output_text: Option<&str>,
        broker_task_id: Option<&str>,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let terminal = matches!(status, "completed" | "failed" | "skipped" | "cancelled");
        connection.execute(
            "UPDATE workflow_node_runs
             SET status = ?3, input_text = COALESCE(?4, input_text),
                 output_text = COALESCE(?5, output_text),
                 broker_task_id = COALESCE(?6, broker_task_id), error_json = ?7,
                 started_at = CASE WHEN ?3 = 'running' THEN COALESCE(started_at, datetime('now')) ELSE started_at END,
                 completed_at = CASE WHEN ?8 THEN datetime('now') ELSE completed_at END,
                 updated_at = datetime('now')
             WHERE run_id = ?1 AND node_id = ?2",
            params![
                run_id,
                node_id,
                status,
                input_text,
                output_text,
                broker_task_id,
                error.map(Value::to_string),
                terminal
            ],
        )?;
        Ok(())
    }

    pub fn workflow_run_cancelled(&self, run_id: &str) -> Result<bool, AppError> {
        Ok(self.connect()?.query_row(
            "SELECT status = 'cancelled' FROM workflow_runs WHERE id = ?1",
            params![run_id],
            |row| row.get(0),
        )?)
    }

    pub fn decide_workflow_approval(
        &self,
        run_id: &str,
        node_id: &str,
        approved: bool,
    ) -> Result<WorkflowExecutionRecord, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = if approved {
            transaction.execute(
                "UPDATE workflow_node_runs
                 SET status = 'completed', output_text = input_text, error_json = NULL,
                     completed_at = datetime('now'), updated_at = datetime('now')
                 WHERE run_id = ?1 AND node_id = ?2 AND node_kind = 'approval'
                   AND status = 'waiting_approval'",
                params![run_id, node_id],
            )?
        } else {
            transaction.execute(
                "UPDATE workflow_node_runs
                 SET status = 'failed', error_json = ?3,
                     completed_at = datetime('now'), updated_at = datetime('now')
                 WHERE run_id = ?1 AND node_id = ?2 AND node_kind = 'approval'
                   AND status = 'waiting_approval'",
                params![
                    run_id,
                    node_id,
                    json!({"message": "La persona responsable rechazó esta rama"}).to_string()
                ],
            )?
        };
        if changed == 0 {
            return Err(AppError::Conflict(
                "esta aprobación ya fue resuelta o no está pendiente".to_owned(),
            ));
        }
        transaction.execute(
            "UPDATE workflow_runs SET status = 'queued', error_json = NULL,
                    updated_at = datetime('now')
             WHERE id = ?1 AND status = 'waiting_approval'",
            params![run_id],
        )?;
        transaction.commit()?;
        self.workflow_execution_record(run_id)
    }

    pub fn cancel_workflow_run_locally(&self, run_id: &str) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE workflow_runs SET status = 'cancelled', completed_at = datetime('now'),
                    updated_at = datetime('now')
             WHERE id = ?1 AND status IN ('queued', 'running', 'waiting_approval')",
            params![run_id],
        )?;
        transaction.execute(
            "UPDATE workflow_node_runs SET status = 'cancelled', completed_at = datetime('now'),
                    updated_at = datetime('now')
             WHERE run_id = ?1 AND status IN ('pending', 'running', 'waiting_approval')",
            params![run_id],
        )?;
        let mut statement = transaction.prepare(
            "SELECT broker_task_id FROM workflow_node_runs
             WHERE run_id = ?1 AND broker_task_id IS NOT NULL",
        )?;
        let task_ids = statement
            .query_map(params![run_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        transaction.commit()?;
        Ok(task_ids)
    }

    pub fn recoverable_workflow_run_ids(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM workflow_runs WHERE status IN ('queued', 'running') ORDER BY created_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    pub fn ready_workflow_attachments(
        &self,
        workflow_id: &str,
        attachment_ids: &[String],
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        if attachment_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada nodo admite como máximo 20 archivos".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let project_id: Option<String> = connection
            .query_row(
                "SELECT project_id FROM workflows WHERE id = ?1 AND archived_at IS NULL",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo {workflow_id}")))?;
        if !attachment_ids.is_empty() && project_id.is_none() {
            return Err(AppError::Conflict(
                "asocia el flujo a un proyecto para asignarle archivos".to_owned(),
            ));
        }
        let mut records = Vec::with_capacity(attachment_ids.len());
        for attachment_id in attachment_ids {
            let allowed: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM project_files
                    WHERE project_id = ?1 AND attachment_id = ?2
                 )",
                params![project_id, attachment_id],
                |row| row.get(0),
            )?;
            if !allowed {
                return Err(AppError::Conflict(format!(
                    "el archivo {attachment_id} no pertenece al proyecto del flujo"
                )));
            }
            let record = self.attachment_record(attachment_id)?;
            if record.ingestion_status != "ready" || record.broker_file_id.is_none() {
                return Err(AppError::Conflict(format!(
                    "el archivo {} todavía no está preparado",
                    record.display_name
                )));
            }
            records.push(record);
        }
        Ok(records)
    }

    /// Resuelve únicamente los archivos que siguen perteneciendo al GPT.
    ///
    /// La versión publicada conserva los identificadores que estaban activos,
    /// pero retirar después un archivo es una revocación efectiva: no se envía
    /// gracias a una copia histórica ni queda pegado al flujo.
    pub fn ready_custom_gpt_attachments_for_workflow(
        &self,
        custom_gpt_id: &str,
        attachment_ids: &[String],
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        if attachment_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada GPT de un flujo admite como máximo 20 archivos propios".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let mut records = Vec::with_capacity(attachment_ids.len());
        for attachment_id in attachment_ids {
            let still_linked: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM custom_gpt_files
                    WHERE custom_gpt_id = ?1 AND attachment_id = ?2
                 )",
                params![custom_gpt_id, attachment_id],
                |row| row.get(0),
            )?;
            if !still_linked {
                continue;
            }
            let record = self.attachment_record(attachment_id)?;
            if record.ingestion_status != "ready" || record.broker_file_id.is_none() {
                return Err(AppError::Conflict(format!(
                    "el archivo {} del GPT todavía no está preparado",
                    record.display_name
                )));
            }
            records.push(record);
        }
        Ok(records)
    }

    /// Recupera el conocimiento que estaba seleccionado al publicar y que aún
    /// continúa habilitado. El orden congelado se conserva y los elementos
    /// revocados simplemente dejan de formar parte del contexto.
    pub fn custom_gpt_memories_for_workflow(
        &self,
        custom_gpt_id: &str,
        memory_ids: &[String],
    ) -> Result<Vec<MemoryItemView>, AppError> {
        if memory_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada GPT de un flujo admite como máximo 20 elementos de conocimiento".to_owned(),
            ));
        }
        let available = self
            .custom_gpt_knowledge(custom_gpt_id)?
            .into_iter()
            .filter(|item| item.enabled)
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut used_characters = 0_usize;
        Ok(memory_ids
            .iter()
            .filter_map(|id| available.get(id))
            .filter(|item| {
                used_characters += item.content.chars().count();
                used_characters <= 8_000
            })
            .cloned()
            .collect())
    }

    /// Devuelve las instrucciones solo mientras el proyecto siga activo y el
    /// texto publicado continúe siendo el autorizado actualmente. Editarlas o
    /// retirarlas revoca versiones antiguas hasta que el flujo se publique otra vez.
    pub fn project_instruction_for_workflow(
        &self,
        context: &WorkflowProjectContext,
    ) -> Result<Option<ProjectInstructionContext>, AppError> {
        let current = self
            .connect()?
            .query_row(
                "SELECT name, instructions FROM projects
                 WHERE id = ?1 AND archived_at IS NULL",
                params![context.project_id],
                |row| Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?)),
            )
            .optional()?;
        let Some((project_name, instructions)) = current else {
            return Ok(None);
        };
        let current = instructions.filter(|value| !value.trim().is_empty());
        if current != context.instructions {
            return Ok(None);
        }
        Ok(current.map(|instructions| ProjectInstructionContext {
            project_id: context.project_id.clone(),
            project_name,
            instructions,
        }))
    }

    /// Resuelve los recuerdos del proyecto que estaban autorizados al publicar
    /// y que continúan activos. La memoria global apagada también los revoca.
    pub fn project_memories_for_workflow(
        &self,
        context: &WorkflowProjectContext,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        if context.memory_ids.len() > 20 {
            return Err(AppError::Validation(
                "cada flujo admite como máximo 20 recuerdos del proyecto".to_owned(),
            ));
        }
        let active: bool = self.connect()?.query_row(
            "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
            params![context.project_id],
            |row| row.get(0),
        )?;
        let overview = self.memory_overview()?;
        if !active || !overview.enabled {
            return Ok(Vec::new());
        }
        let available = overview
            .items
            .into_iter()
            .filter(|item| {
                item.enabled && item.project_id.as_deref() == Some(context.project_id.as_str())
            })
            .map(|item| (item.id.clone(), item))
            .collect::<HashMap<_, _>>();
        let mut used_characters = 0_usize;
        Ok(context
            .memory_ids
            .iter()
            .filter_map(|id| available.get(id))
            .filter(|item| {
                used_characters += item.content.chars().count();
                used_characters <= 8_000
            })
            .cloned()
            .collect())
    }

    #[cfg(test)]
    pub fn create_custom_gpt(
        &self,
        name: &str,
        description: Option<&str>,
        instructions: &str,
    ) -> Result<CustomGptView, AppError> {
        self.create_custom_gpt_with_starters(
            name,
            description,
            instructions,
            &[],
            &CustomGptToolPermissions::default(),
            None,
            None,
            None,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_custom_gpt_with_starters(
        &self,
        name: &str,
        description: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
    ) -> Result<CustomGptView, AppError> {
        self.create_custom_gpt_with_icon(
            name,
            description,
            None,
            instructions,
            conversation_starters,
            tool_permissions,
            preferred_model,
            default_project_id,
            execution_profile,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_custom_gpt_with_icon(
        &self,
        name: &str,
        description: Option<&str>,
        icon_ref: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
    ) -> Result<CustomGptView, AppError> {
        let (name, description, instructions) =
            validated_custom_gpt_fields(name, description, instructions)?;
        let icon_ref = validated_custom_gpt_icon(icon_ref)?;
        let conversation_starters = validated_conversation_starters(conversation_starters)?;
        let tool_permissions = validated_custom_gpt_tool_permissions(tool_permissions)?;
        let preferred_model = validated_preferred_model(preferred_model)?;
        if let Some(profile) = execution_profile {
            validate_execution_preferences(profile)?;
        }
        let custom_gpt_id = format!("gpt_{}", Uuid::new_v4().simple());
        let version_id = format!("gpt_version_{}", Uuid::new_v4().simple());
        let configuration = CustomGptConfiguration {
            schema_version: 2,
            icon_ref,
            instructions,
            conversation_starters,
            preferred_model,
            tools_enabled: tool_permissions.requires_confirmation("run_code")
                || tool_permissions.requires_confirmation("rename_conversation"),
            execution_profile: execution_profile.cloned(),
        };
        let configuration_json = serde_json::to_string(&configuration)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "INSERT INTO custom_gpts(id, name, description, default_project_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![custom_gpt_id, name, description, default_project_id],
        )?;
        transaction.execute(
            "INSERT INTO gpt_versions(
                id, custom_gpt_id, version_no, configuration_json
             ) VALUES (?1, ?2, 1, ?3)",
            params![version_id, custom_gpt_id, configuration_json],
        )?;
        transaction.execute(
            "UPDATE custom_gpts
             SET active_version_id = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![custom_gpt_id, version_id],
        )?;
        for (tool_name, effect) in [
            ("run_code", tool_permissions.run_code.as_str()),
            (
                "rename_conversation",
                tool_permissions.rename_conversation.as_str(),
            ),
        ] {
            transaction.execute(
                "INSERT INTO gpt_tool_permissions(
                    id, gpt_version_id, tool_name, effect, scope_json
                 ) VALUES (?1, ?2, ?3, ?4, '{}')",
                params![
                    format!("gpt_permission_{}", Uuid::new_v4().simple()),
                    version_id,
                    tool_name,
                    effect
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.created', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "version_no": 1
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_view(&custom_gpt_id)
    }

    #[cfg(test)]
    pub fn update_custom_gpt(
        &self,
        custom_gpt_id: &str,
        name: &str,
        description: Option<&str>,
        instructions: &str,
    ) -> Result<CustomGptView, AppError> {
        let current = self.custom_gpt_view(custom_gpt_id)?;
        self.update_custom_gpt_with_starters(
            custom_gpt_id,
            name,
            description,
            instructions,
            &current.conversation_starters,
            &current.tool_permissions,
            current.preferred_model.as_deref(),
            current.default_project_id.as_deref(),
            current.execution_profile.as_ref(),
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_custom_gpt_with_starters(
        &self,
        custom_gpt_id: &str,
        name: &str,
        description: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
    ) -> Result<CustomGptView, AppError> {
        let icon_ref = self.custom_gpt_view(custom_gpt_id)?.icon_ref;
        self.update_custom_gpt_with_icon(
            custom_gpt_id,
            name,
            description,
            Some(&icon_ref),
            instructions,
            conversation_starters,
            tool_permissions,
            preferred_model,
            default_project_id,
            execution_profile,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_custom_gpt_with_icon(
        &self,
        custom_gpt_id: &str,
        name: &str,
        description: Option<&str>,
        icon_ref: Option<&str>,
        instructions: &str,
        conversation_starters: &[String],
        tool_permissions: &CustomGptToolPermissions,
        preferred_model: Option<&str>,
        default_project_id: Option<&str>,
        execution_profile: Option<&ConversationExecutionPreferences>,
    ) -> Result<CustomGptView, AppError> {
        let (name, description, instructions) =
            validated_custom_gpt_fields(name, description, instructions)?;
        let icon_ref = validated_custom_gpt_icon(icon_ref)?;
        let conversation_starters = validated_conversation_starters(conversation_starters)?;
        let tool_permissions = validated_custom_gpt_tool_permissions(tool_permissions)?;
        let preferred_model = validated_preferred_model(preferred_model)?;
        if let Some(profile) = execution_profile {
            validate_execution_preferences(profile)?;
        }
        let version_id = format!("gpt_version_{}", Uuid::new_v4().simple());
        let configuration = CustomGptConfiguration {
            schema_version: 2,
            icon_ref,
            instructions,
            conversation_starters,
            preferred_model,
            tools_enabled: tool_permissions.requires_confirmation("run_code")
                || tool_permissions.requires_confirmation("rename_conversation"),
            execution_profile: execution_profile.cloned(),
        };
        let configuration_json = serde_json::to_string(&configuration)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let next_version = transaction
            .query_row(
                "SELECT COALESCE(MAX(version.version_no), 0) + 1
                 FROM gpt_versions version
                 JOIN custom_gpts gpt ON gpt.id = version.custom_gpt_id
                 WHERE gpt.id = ?1 AND gpt.archived_at IS NULL",
                params![custom_gpt_id],
                |row| row.get::<_, i64>(0),
            )
            .optional()?
            .filter(|version| *version > 1)
            .ok_or_else(|| AppError::NotFound(format!("GPT personal {custom_gpt_id}")))?;
        transaction.execute(
            "INSERT INTO gpt_versions(
                id, custom_gpt_id, version_no, configuration_json
             ) VALUES (?1, ?2, ?3, ?4)",
            params![version_id, custom_gpt_id, next_version, configuration_json],
        )?;
        transaction.execute(
            "UPDATE custom_gpts
             SET name = ?2, description = ?3, active_version_id = ?4,
                 default_project_id = ?5, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![
                custom_gpt_id,
                name,
                description,
                version_id,
                default_project_id
            ],
        )?;
        for (tool_name, effect) in [
            ("run_code", tool_permissions.run_code.as_str()),
            (
                "rename_conversation",
                tool_permissions.rename_conversation.as_str(),
            ),
        ] {
            transaction.execute(
                "INSERT INTO gpt_tool_permissions(
                    id, gpt_version_id, tool_name, effect, scope_json
                 ) VALUES (?1, ?2, ?3, ?4, '{}')",
                params![
                    format!("gpt_permission_{}", Uuid::new_v4().simple()),
                    version_id,
                    tool_name,
                    effect
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.version_created', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "version_no": next_version
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_view(custom_gpt_id)
    }

    pub fn list_custom_gpts(&self) -> Result<Vec<CustomGptView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT gpt.id, gpt.name, gpt.description, version.configuration_json,
                    version.version_no, gpt.created_at, gpt.updated_at,
                    gpt.default_project_id,
                    COALESCE((
                      SELECT permission.effect FROM gpt_tool_permissions permission
                      WHERE permission.gpt_version_id = version.id
                        AND permission.tool_name = 'run_code'
                    ), 'deny'),
                    COALESCE((
                      SELECT permission.effect FROM gpt_tool_permissions permission
                      WHERE permission.gpt_version_id = version.id
                        AND permission.tool_name = 'rename_conversation'
                    ), 'deny')
             FROM custom_gpts gpt
             JOIN gpt_versions version ON version.id = gpt.active_version_id
             WHERE gpt.archived_at IS NULL
             ORDER BY gpt.updated_at DESC, gpt.name COLLATE NOCASE",
        )?;
        let rows = statement
            .query_map([], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, Option<String>>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                    row.get::<_, Option<String>>(7)?,
                    row.get::<_, String>(8)?,
                    row.get::<_, String>(9)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        rows.into_iter()
            .map(
                |(
                    id,
                    name,
                    description,
                    configuration_json,
                    version_no,
                    created_at,
                    updated_at,
                    default_project_id,
                    run_code,
                    rename_conversation,
                )| {
                    let configuration: CustomGptConfiguration =
                        serde_json::from_str(&configuration_json).map_err(|error| {
                            AppError::Conflict(format!(
                                "la versión activa del GPT {id} no es legible: {error}"
                            ))
                        })?;
                    Ok(CustomGptView {
                        id,
                        name,
                        description,
                        icon_ref: configuration.icon_ref,
                        instructions: configuration.instructions,
                        conversation_starters: configuration.conversation_starters,
                        tool_permissions: CustomGptToolPermissions {
                            run_code,
                            rename_conversation,
                        },
                        preferred_model: configuration.preferred_model,
                        execution_profile: configuration.execution_profile,
                        default_project_id,
                        version_no,
                        created_at,
                        updated_at,
                    })
                },
            )
            .collect()
    }

    /// Crea una copia independiente de un GPT a partir de su versión activa.
    ///
    /// Deliberadamente **no** arrastra permisos, conocimiento ni archivos: un
    /// duplicado nace tan restringido como un GPT importado, para que copiar un
    /// asistente no sea una vía silenciosa de propagar accesos o datos sensibles.
    pub fn duplicate_custom_gpt(
        &self,
        custom_gpt_id: &str,
        new_name: Option<&str>,
    ) -> Result<CustomGptView, AppError> {
        let source = self.custom_gpt_view(custom_gpt_id)?;
        let proposed = match new_name.map(str::trim).filter(|value| !value.is_empty()) {
            Some(name) => name.to_owned(),
            None => {
                // El sufijo se recorta para no superar el límite de nombre.
                let suffix = " (copia)";
                let room = 80 - suffix.chars().count();
                let base: String = source.name.chars().take(room).collect();
                format!("{base}{suffix}")
            }
        };
        let duplicate = self.create_custom_gpt_with_icon(
            &proposed,
            source.description.as_deref(),
            Some(&source.icon_ref),
            &source.instructions,
            &source.conversation_starters,
            // Permisos denegados por defecto, como en la importación.
            &CustomGptToolPermissions::default(),
            // El modelo y el proyecto sí se heredan: son preferencias, no accesos.
            source.preferred_model.as_deref(),
            source.default_project_id.as_deref(),
            source.execution_profile.as_ref(),
        )?;
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.duplicated', 'user', ?1)",
            params![serde_json::json!({
                "source_custom_gpt_id": custom_gpt_id,
                "custom_gpt_id": duplicate.id
            })
            .to_string()],
        )?;
        Ok(duplicate)
    }

    /// Devuelve todas las revisiones guardadas, de la más reciente a la primera.
    ///
    /// El historial es solo lectura: ninguna versión anterior se modifica jamás,
    /// porque las tareas ya enviadas conservan congelada la que usaron.
    pub fn list_custom_gpt_versions(
        &self,
        custom_gpt_id: &str,
    ) -> Result<Vec<CustomGptVersionView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT version.id, version.version_no, version.configuration_json,
                    version.created_at,
                    version.id = COALESCE(gpt.active_version_id, ''),
                    COALESCE((
                      SELECT permission.effect FROM gpt_tool_permissions permission
                      WHERE permission.gpt_version_id = version.id
                        AND permission.tool_name = 'run_code'
                    ), 'deny'),
                    COALESCE((
                      SELECT permission.effect FROM gpt_tool_permissions permission
                      WHERE permission.gpt_version_id = version.id
                        AND permission.tool_name = 'rename_conversation'
                    ), 'deny'),
                    (SELECT COUNT(*) FROM broker_tasks task
                     WHERE task.gpt_version_id = version.id)
             FROM gpt_versions version
             JOIN custom_gpts gpt ON gpt.id = version.custom_gpt_id
             WHERE version.custom_gpt_id = ?1
             ORDER BY version.version_no DESC",
        )?;
        let versions = statement
            .query_map(params![custom_gpt_id], |row| {
                let configuration_json: String = row.get(2)?;
                let configuration: CustomGptConfiguration =
                    serde_json::from_str(&configuration_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            configuration_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                Ok(CustomGptVersionView {
                    id: row.get(0)?,
                    version_no: row.get(1)?,
                    icon_ref: configuration.icon_ref,
                    instructions: configuration.instructions,
                    conversation_starters: configuration.conversation_starters,
                    preferred_model: configuration.preferred_model,
                    execution_profile: configuration.execution_profile,
                    created_at: row.get(3)?,
                    active: row.get(4)?,
                    tool_permissions: CustomGptToolPermissions {
                        run_code: row.get(5)?,
                        rename_conversation: row.get(6)?,
                    },
                    task_count: row.get(7)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if versions.is_empty() {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        Ok(versions)
    }

    /// Restaura una revisión anterior **creando una versión nueva** con su
    /// contenido.
    ///
    /// No se reactiva la fila antigua ni se borra nada: así el historial sigue
    /// siendo un registro fiel de lo que ocurrió y las respuestas ya emitidas
    /// mantienen intacta la versión con la que se generaron.
    pub fn restore_custom_gpt_version(
        &self,
        custom_gpt_id: &str,
        version_id: &str,
        confirmed: bool,
    ) -> Result<CustomGptView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "restaurar una versión anterior requiere confirmación".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (source_version_no, configuration_json): (i64, String) = transaction
            .query_row(
                "SELECT version.version_no, version.configuration_json
                 FROM gpt_versions version
                 JOIN custom_gpts gpt ON gpt.id = version.custom_gpt_id
                 WHERE version.id = ?1 AND version.custom_gpt_id = ?2
                   AND gpt.archived_at IS NULL",
                params![version_id, custom_gpt_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::NotFound(format!("la versión {version_id} no pertenece a este GPT"))
            })?;
        let active_version_id: Option<String> = transaction.query_row(
            "SELECT active_version_id FROM custom_gpts WHERE id = ?1",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if active_version_id.as_deref() == Some(version_id) {
            return Err(AppError::Conflict("esa versión ya es la activa".to_owned()));
        }
        let next_version: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(version_no), 0) + 1 FROM gpt_versions WHERE custom_gpt_id = ?1",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        let new_version_id = format!("gpt_version_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO gpt_versions(
                id, custom_gpt_id, version_no, configuration_json
             ) VALUES (?1, ?2, ?3, ?4)",
            params![
                new_version_id,
                custom_gpt_id,
                next_version,
                configuration_json
            ],
        )?;
        // Los permisos también se copian de la versión restaurada: restaurar sin
        // ellos daría un GPT que se comporta distinto al que se pidió recuperar.
        transaction.execute(
            "INSERT INTO gpt_tool_permissions(
                id, gpt_version_id, tool_name, effect, scope_json
             )
             SELECT lower(hex(randomblob(16))), ?1, tool_name, effect, scope_json
             FROM gpt_tool_permissions
             WHERE gpt_version_id = ?2",
            params![new_version_id, version_id],
        )?;
        transaction.execute(
            "UPDATE custom_gpts
             SET active_version_id = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![custom_gpt_id, new_version_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.version_restored', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "restored_from_version_no": source_version_no,
                "version_no": next_version
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_view(custom_gpt_id)
    }

    fn custom_gpt_view(&self, custom_gpt_id: &str) -> Result<CustomGptView, AppError> {
        self.list_custom_gpts()?
            .into_iter()
            .find(|item| item.id == custom_gpt_id)
            .ok_or_else(|| AppError::NotFound(format!("GPT personal {custom_gpt_id}")))
    }

    #[cfg(test)]
    pub fn export_custom_gpt_json(&self, custom_gpt_id: &str) -> Result<String, AppError> {
        Ok(self.export_custom_gpt_portable(custom_gpt_id, false)?.json)
    }

    pub fn export_custom_gpt_portable(
        &self,
        custom_gpt_id: &str,
        include_knowledge: bool,
    ) -> Result<CustomGptPortableExport, AppError> {
        let view = self.custom_gpt_view(custom_gpt_id)?;
        let knowledge_items = self.custom_gpt_knowledge(custom_gpt_id)?;
        let included = if include_knowledge {
            knowledge_items
                .iter()
                .filter(|item| item.enabled && item.sensitivity == "normal")
                .map(|item| PortableCustomGptKnowledge {
                    category: item.category.clone(),
                    content: item.content.clone(),
                })
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        let included_knowledge = included.len();
        let excluded_sensitive = knowledge_items
            .iter()
            .filter(|item| item.sensitivity == "sensitive")
            .count();
        let excluded_disabled = knowledge_items
            .iter()
            .filter(|item| !item.enabled && item.sensitivity != "sensitive")
            .count();
        let excluded_files = self.list_custom_gpt_files(custom_gpt_id)?.len();
        let json = serde_json::to_string_pretty(&PortableCustomGpt {
            schema_version: if include_knowledge { 2 } else { 1 },
            name: view.name,
            description: view.description,
            icon_ref: view.icon_ref,
            instructions: view.instructions,
            conversation_starters: view.conversation_starters,
            knowledge: included,
        })
        .map_err(|error| AppError::Validation(error.to_string()))?;
        Ok(CustomGptPortableExport {
            json,
            included_knowledge,
            excluded_sensitive,
            excluded_disabled,
            excluded_files,
        })
    }

    pub fn record_custom_gpt_exported(
        &self,
        custom_gpt_id: &str,
        included_knowledge: usize,
    ) -> Result<(), AppError> {
        let view = self.custom_gpt_view(custom_gpt_id)?;
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.exported', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "version_no": view.version_no,
                "included_knowledge": included_knowledge
            })
            .to_string()],
        )?;
        Ok(())
    }

    #[cfg(test)]
    pub fn import_custom_gpt_json(&self, source: &str) -> Result<CustomGptView, AppError> {
        Ok(self.import_custom_gpt_package_json(source)?.custom_gpt)
    }

    pub fn import_custom_gpt_package_json(
        &self,
        source: &str,
    ) -> Result<CustomGptImportReport, AppError> {
        if source.len() > 256_000 {
            return Err(AppError::Validation(
                "el archivo del GPT supera el límite de 256 KB".to_owned(),
            ));
        }
        let portable: PortableCustomGpt = serde_json::from_str(source)
            .map_err(|error| AppError::Validation(format!("JSON de GPT no válido: {error}")))?;
        if !matches!(portable.schema_version, 1 | 2) {
            return Err(AppError::Validation(format!(
                "versión de archivo de GPT no compatible: {}",
                portable.schema_version
            )));
        }
        if portable.schema_version == 1 && !portable.knowledge.is_empty() {
            return Err(AppError::Validation(
                "un archivo de GPT versión 1 no puede incluir conocimiento".to_owned(),
            ));
        }
        if portable.knowledge.len() > 100 {
            return Err(AppError::Validation(
                "el paquete del GPT supera el límite de 100 elementos de conocimiento".to_owned(),
            ));
        }
        let mut normalized_knowledge = Vec::new();
        let mut seen_knowledge = HashSet::new();
        for item in &portable.knowledge {
            if !matches!(
                item.category.as_str(),
                "preference" | "instruction" | "fact"
            ) {
                return Err(AppError::Validation(
                    "el paquete contiene una categoría de conocimiento no válida".to_owned(),
                ));
            }
            let content = item.content.trim();
            if content.is_empty() || content.chars().count() > 2_000 {
                return Err(AppError::Validation(
                    "cada conocimiento importado debe contener entre 1 y 2.000 caracteres"
                        .to_owned(),
                ));
            }
            let key = format!("{}\0{}", item.category, content.to_lowercase());
            if seen_knowledge.insert(key) {
                normalized_knowledge.push((item.category.clone(), content.to_owned()));
            }
        }
        let imported = self.create_custom_gpt_with_icon(
            &portable.name,
            portable.description.as_deref(),
            Some(&portable.icon_ref),
            &portable.instructions,
            &portable.conversation_starters,
            &CustomGptToolPermissions::default(),
            // Un paquete importado no impone modelo ni proyecto: ambos son
            // decisiones locales de quien lo recibe.
            None,
            None,
            None,
        )?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        for (category, content) in &normalized_knowledge {
            transaction.execute(
                "INSERT INTO memory_items(
                    id, custom_gpt_id, category, content, sensitivity,
                    enabled, provenance_type
                 ) VALUES (?1, ?2, ?3, ?4, 'normal', 0, 'import')",
                params![
                    format!("memory_{}", Uuid::new_v4().simple()),
                    imported.id,
                    category,
                    content
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.imported', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": imported.id,
                "version_no": imported.version_no,
                "imported_knowledge": normalized_knowledge.len(),
                "knowledge_requires_review": !normalized_knowledge.is_empty()
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok(CustomGptImportReport {
            custom_gpt: imported,
            imported_knowledge: normalized_knowledge.len(),
            knowledge_requires_review: !normalized_knowledge.is_empty(),
        })
    }

    /// Contexto congelable de un GPT por su identificador, sin conversación.
    ///
    /// Lo usa la vista previa para mostrar exactamente lo que se enviaría si se
    /// eligiera este GPT, sin crear ninguna tarea.
    pub fn custom_gpt_context(&self, custom_gpt_id: &str) -> Result<CustomGptContext, AppError> {
        let view = self.custom_gpt_view(custom_gpt_id)?;
        let version_id: String = self.connect()?.query_row(
            "SELECT active_version_id FROM custom_gpts
             WHERE id = ?1 AND archived_at IS NULL AND active_version_id IS NOT NULL",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        Ok(CustomGptContext {
            custom_gpt_id: view.id,
            version_id,
            name: view.name,
            icon_ref: view.icon_ref,
            version_no: view.version_no,
            instructions: view.instructions,
            tool_permissions: view.tool_permissions,
            preferred_model: view.preferred_model,
            execution_profile: view.execution_profile,
        })
    }

    pub fn custom_gpt_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<CustomGptContext>, AppError> {
        let row = self
            .connect()?
            .query_row(
                "SELECT gpt.id, version.id, gpt.name, version.version_no,
                        version.configuration_json,
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'run_code'
                        ), 'deny'),
                        COALESCE((
                          SELECT permission.effect FROM gpt_tool_permissions permission
                          WHERE permission.gpt_version_id = version.id
                            AND permission.tool_name = 'rename_conversation'
                        ), 'deny')
                 FROM conversations conversation
                 JOIN custom_gpts gpt
                   ON gpt.id = conversation.custom_gpt_id
                  AND gpt.archived_at IS NULL
                 JOIN gpt_versions version ON version.id = gpt.active_version_id
                 WHERE conversation.id = ?1
                   AND conversation.archived_at IS NULL
                   AND conversation.deleted_at IS NULL",
                params![conversation_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, String>(6)?,
                    ))
                },
            )
            .optional()?;
        row.map(
            |(
                custom_gpt_id,
                version_id,
                name,
                version_no,
                configuration_json,
                run_code,
                rename_conversation,
            )| {
                let configuration: CustomGptConfiguration =
                    serde_json::from_str(&configuration_json).map_err(|error| {
                        AppError::Conflict(format!(
                            "la versión activa del GPT {custom_gpt_id} no es legible: {error}"
                        ))
                    })?;
                Ok(CustomGptContext {
                    custom_gpt_id,
                    version_id,
                    name,
                    icon_ref: configuration.icon_ref,
                    version_no,
                    instructions: configuration.instructions,
                    tool_permissions: CustomGptToolPermissions {
                        run_code,
                        rename_conversation,
                    },
                    preferred_model: configuration.preferred_model,
                    execution_profile: configuration.execution_profile,
                })
            },
        )
        .transpose()
    }

    pub fn list_audit_events(&self, limit: u32) -> Result<Vec<AuditEventView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT ae.id, ae.event_type, ae.actor, c.title, ae.occurred_at
             FROM audit_events ae
             LEFT JOIN conversations c ON c.id = ae.conversation_id
             ORDER BY ae.occurred_at DESC, ae.id DESC
             LIMIT ?1",
        )?;
        let events = statement
            .query_map(params![i64::from(limit.clamp(1, 100))], |row| {
                let event_type: String = row.get(1)?;
                let (category, summary, severity) = audit_presentation(&event_type);
                Ok(AuditEventView {
                    id: row.get(0)?,
                    category: category.to_owned(),
                    summary: summary.to_owned(),
                    severity: severity.to_owned(),
                    actor: row.get(2)?,
                    conversation_title: row.get(3)?,
                    occurred_at: row.get(4)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(events)
    }

    pub fn memory_overview(&self) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let enabled = connection.query_row(
            "SELECT enabled FROM feature_flags WHERE key = 'memory'",
            [],
            |row| row.get(0),
        )?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.project_id, p.name, m.category, m.content,
                    m.sensitivity, m.enabled, m.created_at, m.updated_at,
                    CASE
                      WHEN er.id IS NOT NULL THEN 'ready'
                       WHEN EXISTS(
                         SELECT 1 FROM broker_tasks bt
                         WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                           AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                           AND json_extract(bt.request_json, '$.content.prompt') = m.content
                           AND bt.local_state NOT IN ('terminal', 'orphaned')
                       ) THEN 'indexing'
                       WHEN EXISTS(
                         SELECT 1 FROM broker_tasks bt
                         WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                           AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                           AND json_extract(bt.request_json, '$.content.prompt') = m.content
                           AND (bt.remote_status = 'failed' OR bt.local_state = 'orphaned')
                       ) THEN 'failed'
                      ELSE 'missing'
                    END,
                    er.model,
                    (
                      SELECT substr(json_extract(failed.error_json, '$.message'), 1, 500)
                       FROM broker_tasks failed
                       WHERE json_extract(failed.request_json, '$.content.metadata.source_type') = 'memory'
                         AND json_extract(failed.request_json, '$.content.metadata.source_id') = m.id
                         AND json_extract(failed.request_json, '$.content.prompt') = m.content
                         AND failed.error_json IS NOT NULL
                      ORDER BY failed.updated_at DESC, failed.rowid DESC LIMIT 1
                    )
             FROM memory_items m
             LEFT JOIN projects p ON p.id = m.project_id
             LEFT JOIN embedding_records er ON er.id = (
                SELECT candidate.id FROM embedding_records candidate
                WHERE candidate.source_type = 'memory' AND candidate.source_id = m.id
                ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
             )
             WHERE m.custom_gpt_id IS NULL
             ORDER BY m.updated_at DESC, m.id DESC",
        )?;
        let items = statement
            .query_map([], |row| {
                Ok(MemoryItemView {
                    id: row.get(0)?,
                    project_id: row.get(1)?,
                    project_name: row.get(2)?,
                    custom_gpt_id: None,
                    custom_gpt_name: None,
                    category: row.get(3)?,
                    content: row.get(4)?,
                    sensitivity: row.get(5)?,
                    enabled: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    embedding_status: row.get(9)?,
                    embedding_model: row.get(10)?,
                    embedding_error: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(MemoryOverview { enabled, items })
    }

    pub fn custom_gpt_knowledge(
        &self,
        custom_gpt_id: &str,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let connection = self.connect()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custom_gpts WHERE id = ?1 AND archived_at IS NULL
             )",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        let mut statement = connection.prepare(
            "SELECT m.id, m.custom_gpt_id, g.name, m.category, m.content,
                    m.sensitivity, m.enabled, m.created_at, m.updated_at,
                    CASE
                      WHEN er.id IS NOT NULL THEN 'ready'
                      WHEN EXISTS(
                        SELECT 1 FROM broker_tasks bt
                        WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                          AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                          AND json_extract(bt.request_json, '$.content.prompt') = m.content
                          AND bt.local_state NOT IN ('terminal', 'orphaned')
                      ) THEN 'indexing'
                      WHEN EXISTS(
                        SELECT 1 FROM broker_tasks bt
                        WHERE json_extract(bt.request_json, '$.content.metadata.source_type') = 'memory'
                          AND json_extract(bt.request_json, '$.content.metadata.source_id') = m.id
                          AND json_extract(bt.request_json, '$.content.prompt') = m.content
                          AND (bt.remote_status = 'failed' OR bt.local_state = 'orphaned')
                      ) THEN 'failed'
                      ELSE 'missing'
                    END,
                    er.model,
                    (
                      SELECT substr(json_extract(failed.error_json, '$.message'), 1, 500)
                      FROM broker_tasks failed
                      WHERE json_extract(failed.request_json, '$.content.metadata.source_type') = 'memory'
                        AND json_extract(failed.request_json, '$.content.metadata.source_id') = m.id
                        AND json_extract(failed.request_json, '$.content.prompt') = m.content
                        AND failed.error_json IS NOT NULL
                      ORDER BY failed.updated_at DESC, failed.rowid DESC LIMIT 1
                    )
             FROM memory_items m
             JOIN custom_gpts g ON g.id = m.custom_gpt_id
             LEFT JOIN embedding_records er ON er.id = (
                SELECT candidate.id FROM embedding_records candidate
                WHERE candidate.source_type = 'memory' AND candidate.source_id = m.id
                ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
             )
             WHERE m.custom_gpt_id = ?1
             ORDER BY m.updated_at DESC, m.id DESC",
        )?;
        let items = statement
            .query_map(params![custom_gpt_id], |row| {
                Ok(MemoryItemView {
                    id: row.get(0)?,
                    project_id: None,
                    project_name: None,
                    custom_gpt_id: row.get(1)?,
                    custom_gpt_name: row.get(2)?,
                    category: row.get(3)?,
                    content: row.get(4)?,
                    sensitivity: row.get(5)?,
                    enabled: row.get(6)?,
                    created_at: row.get(7)?,
                    updated_at: row.get(8)?,
                    embedding_status: row.get(9)?,
                    embedding_model: row.get(10)?,
                    embedding_error: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(items)
    }

    pub fn create_custom_gpt_memory_item(
        &self,
        custom_gpt_id: &str,
        content: &str,
        category: &str,
        sensitivity: &str,
    ) -> Result<(String, Vec<MemoryItemView>), AppError> {
        let id = format!("memory_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custom_gpts WHERE id = ?1 AND archived_at IS NULL
             )",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        transaction.execute(
            "INSERT INTO memory_items(
                id, custom_gpt_id, category, content, sensitivity,
                enabled, provenance_type
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'manual')",
            params![id, custom_gpt_id, category, content, sensitivity],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.knowledge_created', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "custom_gpt_id": custom_gpt_id,
                "category": category,
                "sensitivity": sensitivity
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok((id, self.custom_gpt_knowledge(custom_gpt_id)?))
    }

    pub fn set_custom_gpt_memory_item_enabled(
        &self,
        custom_gpt_id: &str,
        id: &str,
        enabled: bool,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_items
             SET enabled = ?3, updated_at = datetime('now')
             WHERE id = ?1 AND custom_gpt_id = ?2",
            params![id, custom_gpt_id, enabled],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!(
                "conocimiento {id} del GPT personal"
            )));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', ?2)",
            params![
                if enabled {
                    "custom_gpt.knowledge_enabled"
                } else {
                    "custom_gpt.knowledge_disabled"
                },
                serde_json::json!({
                    "memory_id": id,
                    "custom_gpt_id": custom_gpt_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.custom_gpt_knowledge(custom_gpt_id)
    }

    pub fn delete_custom_gpt_memory_item(
        &self,
        custom_gpt_id: &str,
        id: &str,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let owned: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM memory_items WHERE id = ?1 AND custom_gpt_id = ?2
             )",
            params![id, custom_gpt_id],
            |row| row.get(0),
        )?;
        if !owned {
            return Err(AppError::NotFound(format!(
                "conocimiento {id} del GPT personal"
            )));
        }
        transaction.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        transaction.execute(
            "DELETE FROM memory_items WHERE id = ?1 AND custom_gpt_id = ?2",
            params![id, custom_gpt_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.knowledge_deleted', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "custom_gpt_id": custom_gpt_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.custom_gpt_knowledge(custom_gpt_id)
    }

    pub fn custom_gpt_memory_item(
        &self,
        custom_gpt_id: &str,
        id: &str,
    ) -> Result<MemoryItemView, AppError> {
        self.custom_gpt_knowledge(custom_gpt_id)?
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(format!("conocimiento {id} del GPT personal")))
    }

    pub fn clear_custom_gpt_memory_embedding(
        &self,
        custom_gpt_id: &str,
        id: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let owned: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM memory_items WHERE id = ?1 AND custom_gpt_id = ?2
             )",
            params![id, custom_gpt_id],
            |row| row.get(0),
        )?;
        if !owned {
            return Err(AppError::NotFound(format!(
                "conocimiento {id} del GPT personal"
            )));
        }
        connection.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn set_memory_enabled(&self, enabled: bool) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE feature_flags
             SET enabled = ?1, updated_at = datetime('now')
             WHERE key = 'memory'",
            params![enabled],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', '{}')",
            params![if enabled {
                "memory.enabled"
            } else {
                "memory.disabled"
            }],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn create_memory_item(
        &self,
        content: &str,
        category: &str,
        sensitivity: &str,
        project_id: Option<&str>,
    ) -> Result<(String, MemoryOverview), AppError> {
        let id = format!("memory_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO memory_items(
                id, project_id, category, content, sensitivity,
                enabled, provenance_type
             ) VALUES (?1, ?2, ?3, ?4, ?5, 1, 'manual')",
            params![id, project_id, category, content, sensitivity],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.created', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "category": category,
                "sensitivity": sensitivity,
                "project_id": project_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok((id, self.memory_overview()?))
    }

    pub fn update_memory_item(
        &self,
        id: &str,
        content: &str,
        category: &str,
        sensitivity: &str,
        project_id: Option<&str>,
    ) -> Result<(bool, MemoryOverview), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let current = transaction
            .query_row(
                "SELECT content, category, sensitivity, project_id
                 FROM memory_items
                 WHERE id = ?1 AND custom_gpt_id IS NULL",
                params![id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("recuerdo {id}")))?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        let content_changed = current.0 != content;
        let unchanged = !content_changed
            && current.1 == category
            && current.2 == sensitivity
            && current.3.as_deref() == project_id;
        if unchanged {
            transaction.commit()?;
            return Ok((false, self.memory_overview()?));
        }
        transaction.execute(
            "UPDATE memory_items
             SET content = ?2,
                 category = ?3,
                 sensitivity = ?4,
                 project_id = ?5,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, content, category, sensitivity, project_id],
        )?;
        if content_changed {
            transaction.execute(
                "DELETE FROM embedding_records
                 WHERE source_type = 'memory' AND source_id = ?1",
                params![id],
            )?;
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.updated', 'user', ?1)",
            params![serde_json::json!({
                "memory_id": id,
                "category": category,
                "sensitivity": sensitivity,
                "project_id": project_id,
                "content_changed": content_changed
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok((content_changed, self.memory_overview()?))
    }

    pub fn set_memory_item_enabled(
        &self,
        id: &str,
        enabled: bool,
    ) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_items
             SET enabled = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND custom_gpt_id IS NULL",
            params![id, enabled],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("recuerdo {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', ?2)",
            params![
                if enabled {
                    "memory.item_enabled"
                } else {
                    "memory.item_disabled"
                },
                serde_json::json!({"memory_id": id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn delete_memory_item(&self, id: &str) -> Result<MemoryOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "DELETE FROM embedding_records WHERE source_type = 'memory' AND source_id = ?1",
            params![id],
        )?;
        let changed = transaction.execute(
            "DELETE FROM memory_items WHERE id = ?1 AND custom_gpt_id IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("recuerdo {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('memory.deleted', 'user', ?1)",
            params![serde_json::json!({"memory_id": id}).to_string()],
        )?;
        transaction.commit()?;
        self.memory_overview()
    }

    pub fn memory_item(&self, id: &str) -> Result<MemoryItemView, AppError> {
        self.memory_overview()?
            .items
            .into_iter()
            .find(|item| item.id == id)
            .ok_or_else(|| AppError::NotFound(format!("recuerdo {id}")))
    }

    pub fn clear_memory_embedding(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "DELETE FROM embedding_records
             WHERE source_type = 'memory' AND source_id = ?1
               AND EXISTS(
                 SELECT 1 FROM memory_items
                 WHERE id = ?1 AND custom_gpt_id IS NULL
               )",
            params![id],
        )?;
        Ok(())
    }

    pub fn prepare_memory_search(
        &self,
        search_id: &str,
        query: &str,
        project_id: Option<&str>,
        task_id: &str,
        idempotency_key: &str,
        request: &Value,
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(SELECT 1 FROM projects WHERE id = ?1 AND archived_at IS NULL)",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, idempotency_key, request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, 'not_submitted', 'created')",
            params![task_id, idempotency_key, request_json],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![task_id],
        )?;
        transaction.execute(
            "INSERT INTO memory_searches(id, query_text, project_id, broker_task_id)
             VALUES (?1, ?2, ?3, ?4)",
            params![search_id, query, project_id, task_id],
        )?;
        transaction.commit()?;
        self.task_record(task_id)
    }

    pub fn memory_search(&self, id: &str) -> Result<MemorySearchView, AppError> {
        let connection = self.connect()?;
        let record = connection
            .query_row(
                "SELECT ms.query_text, ms.project_id, bt.remote_status, bt.local_state,
                        bt.error_json, er.model, er.dimensions, er.vector_blob, ms.created_at
                 FROM memory_searches ms
                 JOIN broker_tasks bt ON bt.id = ms.broker_task_id
                 LEFT JOIN embedding_records er ON er.id = (
                    SELECT candidate.id FROM embedding_records candidate
                    WHERE candidate.source_type = 'memory_search'
                      AND candidate.source_id = ms.id
                    ORDER BY candidate.created_at DESC, candidate.rowid DESC LIMIT 1
                 )
                 WHERE ms.id = ?1",
                params![id],
                |row| {
                    Ok(MemorySearchRecord {
                        query: row.get(0)?,
                        project_id: row.get(1)?,
                        remote_status: row.get(2)?,
                        local_state: row.get(3)?,
                        error_json: row.get(4)?,
                        model: row.get(5)?,
                        dimensions: row.get(6)?,
                        blob: row.get(7)?,
                        created_at: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("búsqueda de memoria {id}")))?;

        let error = record
            .error_json
            .as_deref()
            .and_then(|raw| serde_json::from_str::<Value>(raw).ok())
            .and_then(|value| {
                value
                    .get("message")
                    .and_then(Value::as_str)
                    .map(str::to_owned)
            });
        let status = if record.blob.is_some() {
            "completed"
        } else if record.remote_status == "failed"
            || record.local_state == "orphaned"
            || record.remote_status == "completed"
        {
            "failed"
        } else {
            "searching"
        };
        let mut results = Vec::new();
        if let (Some(model_name), Some(dimensions), Some(search_blob)) = (
            record.model.as_deref(),
            record.dimensions,
            record.blob.as_deref(),
        ) {
            let search_vector = decode_embedding(search_blob, dimensions)?;
            let mut statement = connection.prepare(
                "SELECT m.id, m.content, m.category, p.name, m.sensitivity,
                        er.dimensions, er.vector_blob
                 FROM memory_items m
                 JOIN embedding_records er
                   ON er.source_type = 'memory' AND er.source_id = m.id
                  AND er.model = ?1 AND er.dimensions = ?2
                 LEFT JOIN projects p ON p.id = m.project_id
                 WHERE m.enabled = 1 AND m.custom_gpt_id IS NULL
                   AND (m.project_id IS NULL OR (?3 IS NOT NULL AND m.project_id = ?3))
                 ORDER BY m.updated_at DESC",
            )?;
            let candidates = statement
                .query_map(params![model_name, dimensions, record.project_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                        row.get::<_, i64>(5)?,
                        row.get::<_, Vec<u8>>(6)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (memory_id, content, category, project_name, sensitivity, dims, candidate_blob) in
                candidates
            {
                let candidate = decode_embedding(&candidate_blob, dims)?;
                let score = cosine_similarity(&search_vector, &candidate);
                if score.is_finite() && score >= 0.25 {
                    let reason = if score >= 0.75 {
                        "Coincidencia semántica alta"
                    } else if score >= 0.5 {
                        "Coincidencia semántica media"
                    } else {
                        "Coincidencia semántica baja"
                    };
                    results.push(MemorySearchResultView {
                        memory_id,
                        content,
                        category,
                        project_name,
                        sensitivity,
                        score: (score * 1000.0).round() / 1000.0,
                        reason: reason.to_owned(),
                    });
                }
            }
            results.sort_by(|left, right| {
                right
                    .score
                    .partial_cmp(&left.score)
                    .unwrap_or(std::cmp::Ordering::Equal)
            });
            results.truncate(5);
        }
        Ok(MemorySearchView {
            id: id.to_owned(),
            query: record.query,
            project_id: record.project_id,
            status: status.to_owned(),
            model: record.model,
            error: error.or_else(|| {
                (record.remote_status == "completed" && status == "failed").then(|| {
                    "Broker AI completó la tarea sin devolver un vector utilizable".to_owned()
                })
            }),
            results,
            created_at: record.created_at,
        })
    }

    pub fn latest_memory_search(&self) -> Result<Option<MemorySearchView>, AppError> {
        let id = self
            .connect()?
            .query_row(
                "SELECT id FROM memory_searches ORDER BY created_at DESC, rowid DESC LIMIT 1",
                [],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        id.map(|id| self.memory_search(&id)).transpose()
    }

    pub fn active_memories_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<MemoryItemView>, AppError> {
        let overview = self.memory_overview()?;
        let (project_id, custom_gpt_id): (Option<String>, Option<String>) =
            self.connect()?.query_row(
                "SELECT project_id, custom_gpt_id FROM conversations
             WHERE id = ?1 AND deleted_at IS NULL",
                params![conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        let mut candidates = if overview.enabled {
            overview
                .items
                .into_iter()
                .filter(|item| item.enabled)
                .filter(|item| item.project_id.is_none() || item.project_id == project_id)
                .collect::<Vec<_>>()
        } else {
            Vec::new()
        };
        if let Some(custom_gpt_id) = custom_gpt_id {
            let mut scoped = self
                .custom_gpt_knowledge(&custom_gpt_id)?
                .into_iter()
                .filter(|item| item.enabled)
                .collect::<Vec<_>>();
            scoped.append(&mut candidates);
            candidates = scoped;
        }
        let mut total_chars = 0_usize;
        Ok(candidates
            .into_iter()
            .filter(|item| {
                total_chars += item.content.chars().count();
                total_chars <= 8_000
            })
            .take(20)
            .collect())
    }

    pub fn task_context(&self, task_id: &str) -> Result<ContextSnapshotView, AppError> {
        let connection = self.connect()?;
        let (strategy_version, estimated_tokens): (String, i64) = connection
            .query_row(
                "SELECT strategy_version, COALESCE(estimated_tokens, 0)
                 FROM context_snapshots
                 WHERE broker_task_id = ?1",
                params![task_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("contexto de la respuesta".to_owned()))?;
        let mut strategy = match strategy_version.as_str() {
            "window-memory-v1" => "Ventana reciente + memoria",
            "window-summary-v1" => "Resumen aprobado + ventana reciente",
            "window-summary-memory-v1" => "Resumen aprobado + ventana reciente + memoria",
            "window-summary-semantic-memory-v1" => {
                "Resumen aprobado + ventana reciente + memoria semántica"
            }
            "window-semantic-memory-v1" => "Ventana reciente + memoria semántica",
            "window-summary-semantic-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + memoria semántica + documentos"
            }
            "window-semantic-memory-document-v1" => {
                "Ventana reciente + memoria semántica + documentos"
            }
            "window-summary-document-v1" => "Resumen aprobado + ventana reciente + documentos",
            "window-summary-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + memoria + documentos"
            }
            "window-summary-project-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto"
            }
            "window-summary-project-memory-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria"
            }
            "window-project-v1" => "Ventana reciente + instrucciones del proyecto",
            "window-project-memory-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria"
            }
            "window-summary-project-document-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + documentos"
            }
            "window-summary-project-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria + documentos"
            }
            "window-project-document-v1" => {
                "Ventana reciente + instrucciones del proyecto + documentos"
            }
            "window-project-memory-document-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria + documentos"
            }
            "window-summary-project-semantic-memory-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria semántica"
            }
            "window-summary-project-semantic-memory-document-v1" => {
                "Resumen aprobado + ventana reciente + instrucciones del proyecto + memoria semántica + documentos"
            }
            "window-project-semantic-memory-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria semántica"
            }
            "window-project-semantic-memory-document-v1" => {
                "Ventana reciente + instrucciones del proyecto + memoria semántica + documentos"
            }
            "window-document-v1" => "Ventana reciente + documentos",
            "window-memory-document-v1" => "Ventana reciente + memoria + documentos",
            "window-v1" => "Ventana reciente",
            other => other,
        }
        .to_owned();
        let mut statement = connection.prepare(
            "SELECT source.source_type, source.reason, source.score,
                    COALESCE(source.estimated_tokens, 0),
                    COALESCE(source.excerpt, ''), memory.category,
                    attachment.display_name, chunk.ordinal,
                    source.id, attachment.local_path, memory.custom_gpt_id,
                    custom_gpt.name
             FROM context_sources source
             LEFT JOIN memory_items memory
               ON source.source_type = 'memory' AND memory.id = source.source_id
             LEFT JOIN custom_gpts custom_gpt ON custom_gpt.id = memory.custom_gpt_id
             LEFT JOIN attachment_chunks chunk
               ON source.source_type = 'attachment_chunk' AND chunk.id = source.source_id
             LEFT JOIN attachments attachment ON attachment.id = chunk.attachment_id
             JOIN context_snapshots snapshot ON snapshot.id = source.snapshot_id
             WHERE snapshot.broker_task_id = ?1
             ORDER BY source.ordinal",
        )?;
        let sources = statement
            .query_map(params![task_id], |row| {
                let kind: String = row.get(0)?;
                let stored_reason: String = row.get(1)?;
                let category: Option<String> = row.get(5)?;
                let attachment_name: Option<String> = row.get(6)?;
                let chunk_ordinal: Option<i64> = row.get(7)?;
                let source_id: String = row.get(8)?;
                let attachment_path: Option<String> = row.get(9)?;
                let custom_gpt_id: Option<String> = row.get(10)?;
                let custom_gpt_name: Option<String> = row.get(11)?;
                let label = match (
                    kind.as_str(),
                    stored_reason.as_str(),
                    category.as_deref(),
                    custom_gpt_id.as_deref(),
                ) {
                    ("message", "current_user_turn", _, _) => "Mensaje actual".to_owned(),
                    ("message", _, _, _) => "Mensaje reciente".to_owned(),
                    ("summary", _, _, _) => "Resumen aprobado".to_owned(),
                    ("project_instruction", _, _, _) => "Instrucciones del proyecto".to_owned(),
                    ("custom_gpt", _, _, _) => "GPT personal".to_owned(),
                    ("memory", _, Some("preference"), Some(_)) => format!(
                        "Conocimiento GPT · Preferencia · {}",
                        custom_gpt_name.as_deref().unwrap_or("GPT personal")
                    ),
                    ("memory", _, Some("instruction"), Some(_)) => format!(
                        "Conocimiento GPT · Instrucción · {}",
                        custom_gpt_name.as_deref().unwrap_or("GPT personal")
                    ),
                    ("memory", _, Some("fact"), Some(_)) => format!(
                        "Conocimiento GPT · Hecho · {}",
                        custom_gpt_name.as_deref().unwrap_or("GPT personal")
                    ),
                    ("memory", _, Some("preference"), _) => "Recuerdo · Preferencia".to_owned(),
                    ("memory", _, Some("instruction"), _) => "Recuerdo · Instrucción".to_owned(),
                    ("memory", _, Some("fact"), _) => "Recuerdo · Hecho".to_owned(),
                    ("memory", _, _, _) => "Recuerdo".to_owned(),
                    ("attachment_chunk", _, _, _) => format!(
                        "{} · fragmento {}",
                        attachment_name.as_deref().unwrap_or("Documento"),
                        chunk_ordinal.unwrap_or(0) + 1
                    ),
                    _ => "Fuente de contexto".to_owned(),
                };
                let reason = match stored_reason.as_str() {
                    "current_user_turn" => "Petición que acabas de enviar".to_owned(),
                    "recent_conversation_window" => {
                        "Mensaje reciente de la conversación".to_owned()
                    }
                    "approved_conversation_summary" => {
                        "Resumen revisado y aprobado por ti".to_owned()
                    }
                    "Instrucciones configuradas para el proyecto" => {
                        "Configuración reutilizable del proyecto".to_owned()
                    }
                    "Versión del GPT personal seleccionada al enviar" => {
                        "Versión exacta congelada al enviar".to_owned()
                    }
                    _ => stored_reason,
                };
                let excerpt: String = row.get(4)?;
                let source_reference = (kind == "attachment_chunk").then_some(source_id);
                Ok(ContextSourceView {
                    kind,
                    label,
                    reason,
                    score: row.get(2)?,
                    estimated_tokens: row.get(3)?,
                    excerpt: excerpt.chars().take(600).collect(),
                    source_reference,
                    source_available: attachment_path
                        .as_deref()
                        .is_some_and(|path| Path::new(path).is_file()),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        if sources.iter().any(|source| source.kind == "custom_gpt") {
            strategy.push_str(" + GPT personal");
        }
        if sources.iter().any(|source| {
            source.kind == "attachment_chunk"
                && source.reason.contains("Vista global del documento")
        }) {
            strategy.push_str(" · Vista global del documento");
        }
        Ok(ContextSnapshotView {
            strategy,
            estimated_tokens,
            sources,
        })
    }

    pub fn context_source_file(
        &self,
        task_id: &str,
        source_reference: &str,
    ) -> Result<ContextSourceFile, AppError> {
        self.connect()?
            .query_row(
                "SELECT attachment.local_path, attachment.display_name
                 FROM context_sources source
                 JOIN context_snapshots snapshot ON snapshot.id = source.snapshot_id
                 JOIN attachment_chunks chunk
                   ON source.source_type = 'attachment_chunk' AND chunk.id = source.source_id
                 JOIN attachments attachment ON attachment.id = chunk.attachment_id
                 WHERE snapshot.broker_task_id = ?1 AND source.id = ?2",
                params![task_id, source_reference],
                |row| {
                    Ok(ContextSourceFile {
                        local_path: row.get(0)?,
                        display_name: row.get(1)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("fuente documental de la respuesta".to_owned()))
    }

    fn project_summary(&self, id: &str) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT p.id, p.name, p.description, p.instructions, COUNT(c.id), p.updated_at
                 FROM projects p
                 LEFT JOIN conversations c
                   ON c.project_id = p.id
                  AND c.archived_at IS NULL
                  AND c.deleted_at IS NULL
                 WHERE p.id = ?1 AND p.archived_at IS NULL
                 GROUP BY p.id, p.name, p.description, p.instructions, p.updated_at",
                params![id],
                |row| {
                    Ok(ProjectSummary {
                        id: row.get(0)?,
                        name: row.get(1)?,
                        description: row.get(2)?,
                        instructions: row.get(3)?,
                        conversation_count: row.get(4)?,
                        updated_at: row.get(5)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("proyecto {id}")))
    }

    pub fn rename_project(&self, id: &str, name: &str) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET name = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, name],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.renamed', 'user', ?1)",
            params![serde_json::json!({"project_id": id, "name": name}).to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(id)
    }

    pub fn update_project_instructions(
        &self,
        id: &str,
        instructions: Option<&str>,
    ) -> Result<ProjectSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET instructions = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id, instructions],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.instructions_updated', 'user', ?1)",
            params![serde_json::json!({
                "project_id": id,
                "enabled": instructions.is_some()
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.project_summary(id)
    }

    pub fn project_instruction_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Option<ProjectInstructionContext>, AppError> {
        self.connect()?
            .query_row(
                "SELECT project.id, project.name, project.instructions
                 FROM conversations conversation
                 JOIN projects project ON project.id = conversation.project_id
                 WHERE conversation.id = ?1
                   AND conversation.deleted_at IS NULL
                   AND project.archived_at IS NULL
                   AND project.instructions IS NOT NULL
                   AND trim(project.instructions) != ''",
                params![conversation_id],
                |row| {
                    Ok(ProjectInstructionContext {
                        project_id: row.get(0)?,
                        project_name: row.get(1)?,
                        instructions: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn project_knowledge_overview(
        &self,
        project_id: &str,
    ) -> Result<ProjectKnowledgeOverview, AppError> {
        let project = self.project_summary(project_id)?;
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT project_file.attachment_id
             FROM project_files project_file
             JOIN attachments attachment ON attachment.id = project_file.attachment_id
             WHERE project_file.project_id = ?1
             ORDER BY attachment.display_name COLLATE NOCASE, project_file.attachment_id",
        )?;
        let attachment_ids = statement
            .query_map(params![project_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        let mut statement = connection.prepare(
            "SELECT link.attachment_id, conversation.id, conversation.title,
                    conversation.project_id, conversation.updated_at
             FROM conversation_attachments link
             JOIN conversations conversation ON conversation.id = link.conversation_id
             JOIN project_files project_file
               ON project_file.attachment_id = link.attachment_id
              AND project_file.project_id = conversation.project_id
             WHERE project_file.project_id = ?1
               AND conversation.archived_at IS NULL
               AND conversation.deleted_at IS NULL
             ORDER BY link.attachment_id, conversation.updated_at DESC, conversation.id",
        )?;
        let usage_rows = statement
            .query_map(params![project_id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ConversationSummary {
                        id: row.get(1)?,
                        title: row.get(2)?,
                        project_id: row.get(3)?,
                        updated_at: row.get(4)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        drop(statement);
        drop(connection);
        let mut conversations_by_attachment: HashMap<String, Vec<ConversationSummary>> =
            HashMap::new();
        for (attachment_id, conversation) in usage_rows {
            conversations_by_attachment
                .entry(attachment_id)
                .or_default()
                .push(conversation);
        }
        let files = attachment_ids
            .iter()
            .map(|attachment_id| self.attachment_view(attachment_id))
            .collect::<Result<Vec<_>, _>>()?;
        let file_usages = attachment_ids
            .iter()
            .map(|attachment_id| ProjectFileUsageView {
                attachment_id: attachment_id.clone(),
                conversations: conversations_by_attachment
                    .remove(attachment_id)
                    .unwrap_or_default(),
            })
            .collect();
        let memory = self.memory_overview()?;
        let memories = memory
            .items
            .into_iter()
            .filter(|item| item.project_id.as_deref() == Some(project_id))
            .collect();
        Ok(ProjectKnowledgeOverview {
            project,
            files,
            file_usages,
            memories,
            memory_enabled: memory.enabled,
        })
    }

    pub fn remove_project_file(
        &self,
        project_id: &str,
        attachment_id: &str,
    ) -> Result<ProjectKnowledgeOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "DELETE FROM project_files
             WHERE project_id = ?1
               AND attachment_id = ?2
               AND EXISTS(
                   SELECT 1 FROM projects
                   WHERE id = ?1 AND archived_at IS NULL
               )",
            params![project_id, attachment_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(
                "archivo reutilizable del proyecto".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.file_removed', 'user', ?1)",
            params![serde_json::json!({
                "project_id": project_id,
                "attachment_id": attachment_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.project_knowledge_overview(project_id)
    }

    pub fn set_project_memory_item_enabled(
        &self,
        project_id: &str,
        memory_id: &str,
        enabled: bool,
    ) -> Result<ProjectKnowledgeOverview, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE memory_items
             SET enabled = ?3, updated_at = datetime('now')
             WHERE id = ?2
               AND project_id = ?1
               AND custom_gpt_id IS NULL",
            params![project_id, memory_id, enabled],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(
                "recuerdo limitado al proyecto".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES (?1, 'user', ?2)",
            params![
                if enabled {
                    "memory.item_enabled"
                } else {
                    "memory.item_disabled"
                },
                serde_json::json!({
                    "memory_id": memory_id,
                    "project_id": project_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.project_knowledge_overview(project_id)
    }

    pub fn archive_project(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE projects
             SET archived_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("proyecto {id}")));
        }
        transaction.execute(
            "UPDATE conversations
             SET project_id = NULL, updated_at = datetime('now')
             WHERE project_id = ?1 AND deleted_at IS NULL",
            params![id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('project.archived', 'user', ?1)",
            params![serde_json::json!({"project_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn create_conversation(
        &self,
        title: &str,
        project_id: Option<&str>,
    ) -> Result<ConversationSummary, AppError> {
        let id = format!("conv_{}", Uuid::new_v4().simple());
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM projects
                    WHERE id = ?1 AND archived_at IS NULL
                 )",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        transaction.execute(
            "INSERT INTO conversations(id, project_id, title) VALUES (?1, ?2, ?3)",
            params![id, project_id, title],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.created', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "project_id": project_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(&id)
    }

    pub fn list_conversations(&self) -> Result<Vec<ConversationSummary>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, title, project_id, updated_at
             FROM conversations
             WHERE archived_at IS NULL AND deleted_at IS NULL
             ORDER BY updated_at DESC",
        )?;
        let conversations = statement
            .query_map([], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    pub fn search_conversations(
        &self,
        query: &str,
        limit: usize,
    ) -> Result<Vec<ConversationSummary>, AppError> {
        let connection = self.connect()?;
        let escaped = query
            .replace('!', "!!")
            .replace('%', "!%")
            .replace('_', "!_");
        let pattern = format!("%{escaped}%");
        let mut statement = connection.prepare(
            "SELECT c.id, c.title, c.project_id, c.updated_at
             FROM conversations c
             WHERE c.archived_at IS NULL
               AND c.deleted_at IS NULL
               AND (
                    c.title LIKE ?1 ESCAPE '!' COLLATE NOCASE
                    OR EXISTS(
                        SELECT 1
                        FROM messages m
                        JOIN message_parts mp ON mp.message_id = m.id
                        WHERE m.conversation_id = c.id
                          AND mp.content_text LIKE ?1 ESCAPE '!' COLLATE NOCASE
                    )
               )
             ORDER BY c.updated_at DESC
             LIMIT ?2",
        )?;
        let conversations = statement
            .query_map(params![pattern, limit as i64], |row| {
                Ok(ConversationSummary {
                    id: row.get(0)?,
                    title: row.get(1)?,
                    project_id: row.get(2)?,
                    updated_at: row.get(3)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(conversations)
    }

    fn conversation_summary(&self, id: &str) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, title, project_id, updated_at
                 FROM conversations
                 WHERE id = ?1 AND deleted_at IS NULL",
                params![id],
                |row| {
                    Ok(ConversationSummary {
                        id: row.get(0)?,
                        title: row.get(1)?,
                        project_id: row.get(2)?,
                        updated_at: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversación {id}")))
    }

    pub fn rename_conversation(
        &self,
        id: &str,
        title: &str,
    ) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET title = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id, title],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.renamed', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "title": title}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(id)
    }

    pub fn move_conversation(
        &self,
        id: &str,
        project_id: Option<&str>,
    ) -> Result<ConversationSummary, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(project_id) = project_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM projects
                    WHERE id = ?1 AND archived_at IS NULL
                 )",
                params![project_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("proyecto {project_id}")));
            }
        }
        let changed = transaction.execute(
            "UPDATE conversations
             SET project_id = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id, project_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.moved', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({"conversation_id": id, "project_id": project_id}).to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_summary(id)
    }

    pub fn set_conversation_custom_gpt(
        &self,
        id: &str,
        custom_gpt_id: Option<&str>,
    ) -> Result<ConversationView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        if let Some(custom_gpt_id) = custom_gpt_id {
            let exists: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM custom_gpts
                    WHERE id = ?1 AND archived_at IS NULL AND active_version_id IS NOT NULL
                 )",
                params![custom_gpt_id],
                |row| row.get(0),
            )?;
            if !exists {
                return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
            }
        }
        let changed = transaction.execute(
            "UPDATE conversations
             SET custom_gpt_id = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id, custom_gpt_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        // El proyecto predeterminado del GPT solo se aplica a una conversación que
        // todavía no pertenece a ninguno: nunca mueve un chat ya clasificado.
        if let Some(custom_gpt_id) = custom_gpt_id {
            let adopted = transaction.execute(
                "UPDATE conversations
                 SET project_id = (
                       SELECT gpt.default_project_id FROM custom_gpts gpt
                       WHERE gpt.id = ?2 AND gpt.default_project_id IS NOT NULL
                     ),
                     updated_at = datetime('now')
                 WHERE id = ?1 AND project_id IS NULL
                   AND EXISTS(
                     SELECT 1 FROM custom_gpts gpt
                     WHERE gpt.id = ?2 AND gpt.default_project_id IS NOT NULL
                   )",
                params![id, custom_gpt_id],
            )?;
            if adopted > 0 {
                transaction.execute(
                    "INSERT INTO audit_events(
                        event_type, actor, conversation_id, payload_json
                     ) VALUES ('conversation.default_project_applied', 'user', ?1, ?2)",
                    params![
                        id,
                        serde_json::json!({"custom_gpt_id": custom_gpt_id}).to_string()
                    ],
                )?;
            }
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.custom_gpt_updated', 'user', ?1, ?2)",
            params![
                id,
                serde_json::json!({
                    "conversation_id": id,
                    "custom_gpt_id": custom_gpt_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.conversation_view(id)
    }

    fn ensure_conversation_can_hide(
        transaction: &rusqlite::Transaction<'_>,
        id: &str,
    ) -> Result<(), AppError> {
        let active_tasks: i64 = transaction.query_row(
            "SELECT COUNT(*)
             FROM broker_tasks
             WHERE conversation_id = ?1
               AND local_state NOT IN ('terminal', 'orphaned')",
            params![id],
            |row| row.get(0),
        )?;
        if active_tasks > 0 {
            return Err(AppError::Conflict(
                "la conversación tiene una tarea en curso; cancélala o espera a que termine"
                    .to_owned(),
            ));
        }
        Ok(())
    }

    pub fn archive_conversation(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        Self::ensure_conversation_can_hide(&transaction, id)?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET archived_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación activa {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.archived', 'user', ?1, ?2)",
            params![id, serde_json::json!({"conversation_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_conversation(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        Self::ensure_conversation_can_hide(&transaction, id)?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET deleted_at = datetime('now'), updated_at = datetime('now')
             WHERE id = ?1 AND deleted_at IS NULL",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("conversación {id}")));
        }
        transaction.execute(
            "INSERT INTO audit_events(
                event_type, actor, conversation_id, payload_json
             ) VALUES ('conversation.deleted', 'user', ?1, ?2)",
            params![id, serde_json::json!({"conversation_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn recent_context(
        &self,
        conversation_id: &str,
        message_limit: usize,
        character_limit: usize,
    ) -> Result<Vec<ContextMessage>, AppError> {
        let connection = self.connect()?;
        let approved_summary: Option<(String, String, i64)> = connection
            .query_row(
                "SELECT id, approved_text, source_through_sequence
                 FROM conversation_summaries
                 WHERE conversation_id = ?1 AND status = 'approved'
                 LIMIT 1",
                params![conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let source_through_sequence = approved_summary
            .as_ref()
            .map(|(_, _, sequence)| *sequence)
            .unwrap_or(0);
        let summary_characters = approved_summary
            .as_ref()
            .map(|(_, text, _)| text.chars().count())
            .unwrap_or(0);
        let message_character_limit = character_limit.saturating_sub(summary_characters);
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, mp.content_text
             FROM messages m
             JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             WHERE m.conversation_id = ?1
               AND m.status = 'complete'
               AND m.role IN ('user', 'assistant')
               AND mp.kind IN ('text', 'markdown')
               AND m.sequence_no > ?3
             ORDER BY m.sequence_no DESC
             LIMIT ?2",
        )?;
        let mut newest_first = statement
            .query_map(
                params![
                    conversation_id,
                    message_limit as i64,
                    source_through_sequence
                ],
                |row| {
                    Ok(ContextMessage {
                        message_id: row.get(0)?,
                        role: row.get(1)?,
                        text: row.get(2)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        newest_first.reverse();

        let mut selected = Vec::new();
        let mut used = 0_usize;
        for message in newest_first.into_iter().rev() {
            let remaining = message_character_limit.saturating_sub(used);
            if remaining == 0 {
                break;
            }
            let mut message = message;
            if message.text.chars().count() > remaining {
                message.text = message
                    .text
                    .chars()
                    .rev()
                    .take(remaining)
                    .collect::<String>()
                    .chars()
                    .rev()
                    .collect();
            }
            used += message.text.chars().count();
            selected.push(message);
        }
        selected.reverse();
        if let Some((summary_id, summary_text, _)) = approved_summary {
            selected.insert(
                0,
                ContextMessage {
                    message_id: summary_id,
                    role: "summary".to_owned(),
                    text: summary_text,
                },
            );
        }
        Ok(selected)
    }

    pub fn conversation_summary_input(
        &self,
        conversation_id: &str,
        character_budget: usize,
    ) -> Result<ConversationSummaryInput, AppError> {
        let connection = self.connect()?;
        let approved_summary: Option<(String, String, i64)> = connection
            .query_row(
                "SELECT id, approved_text, source_through_sequence
                 FROM conversation_summaries
                 WHERE conversation_id = ?1 AND status = 'approved'
                 LIMIT 1",
                params![conversation_id],
                |row| Ok((row.get(0)?, row.get(1)?, row.get(2)?)),
            )
            .optional()?;
        let previous_source_through_sequence = approved_summary
            .as_ref()
            .map(|(_, _, sequence)| *sequence)
            .unwrap_or(0);
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, mp.content_text, m.sequence_no
             FROM messages m
             JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             WHERE m.conversation_id = ?1
               AND m.status = 'complete'
               AND m.role IN ('user', 'assistant')
               AND mp.kind IN ('text', 'markdown')
               AND m.sequence_no > ?2
             ORDER BY m.sequence_no",
        )?;
        let rows = statement
            .query_map(
                params![conversation_id, previous_source_through_sequence],
                |row| {
                    Ok((
                        ContextMessage {
                            message_id: row.get(0)?,
                            role: row.get(1)?,
                            text: row.get(2)?,
                        },
                        row.get::<_, i64>(3)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;

        let total_message_count = rows.len() as i64;
        let mut messages = Vec::new();
        let mut character_count = 0_usize;
        let mut source_through_sequence = previous_source_through_sequence;
        if let Some((summary_id, summary_text, _)) = approved_summary {
            character_count = summary_text.chars().count();
            messages.push(ContextMessage {
                message_id: summary_id,
                role: "summary".to_owned(),
                text: summary_text,
            });
        }
        let base_context_count = messages.len();
        for (message, sequence) in rows {
            let message_characters = message.text.chars().count();
            if character_count.saturating_add(message_characters) > character_budget {
                break;
            }
            character_count += message_characters;
            source_through_sequence = sequence;
            messages.push(message);
        }
        let included_message_count = (messages.len() - base_context_count) as i64;
        Ok(ConversationSummaryInput {
            messages,
            source_through_sequence,
            included_message_count,
            remaining_message_count: total_message_count - included_message_count,
            character_count,
        })
    }

    pub fn register_attachment(
        &self,
        conversation_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
    ) -> Result<AttachmentView, AppError> {
        self.register_attachment_with_image_policy(
            conversation_id,
            local_path,
            display_name,
            media_type,
            size_bytes,
            sha256,
            None,
        )
    }

    pub fn register_attachment_with_image_policy(
        &self,
        conversation_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
        describe_images: Option<bool>,
    ) -> Result<AttachmentView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let active: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL
             )",
            params![conversation_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(AppError::NotFound(format!(
                "conversación activa {conversation_id}"
            )));
        }
        let existing: Option<String> = match describe_images {
            Some(true) => transaction
                .query_row(
                    "SELECT id FROM attachments
                     WHERE sha256 = ?1 AND describe_images = 1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            Some(false) => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY CASE WHEN describe_images = 0 THEN 0 ELSE 1 END, created_at, id
                     LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            None => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
        };
        let reused_attachment = existing.is_some();
        let attachment_id =
            existing.unwrap_or_else(|| format!("attachment_{}", Uuid::new_v4().simple()));
        transaction.execute(
            "INSERT OR IGNORE INTO attachments(
                id, local_path, display_name, media_type, size_bytes, sha256, describe_images
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attachment_id,
                local_path,
                display_name,
                media_type,
                size_bytes,
                sha256,
                describe_images
            ],
        )?;
        if reused_attachment {
            let restarted = transaction.execute(
                "UPDATE attachments
                 SET broker_file_id = NULL,
                     ingestion_status = 'local',
                     ingestion_error_json = NULL,
                     context_status = 'pending',
                     context_error_json = NULL,
                     kind = NULL,
                     engine = NULL,
                     ingestion_meta_json = NULL,
                     updated_at = datetime('now')
                 WHERE id = ?1 AND ingestion_status = 'failed'",
                params![attachment_id],
            )?;
            if restarted > 0 {
                transaction.execute(
                    "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
                     VALUES ('attachment.retry_requested', 'user', ?1, ?2)",
                    params![
                        conversation_id,
                        serde_json::json!({
                            "attachment_id": attachment_id,
                            "reason": "reattached_failed_file"
                        })
                        .to_string()
                    ],
                )?;
            }
        }
        transaction.execute(
            "INSERT OR IGNORE INTO conversation_attachments(conversation_id, attachment_id)
             VALUES (?1, ?2)",
            params![conversation_id, attachment_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('attachment.added', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "attachment_id": attachment_id,
                    "sha256": sha256,
                    "size_bytes": size_bytes
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.attachment_view(&attachment_id)
    }

    pub fn register_custom_gpt_attachment(
        &self,
        custom_gpt_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
    ) -> Result<AttachmentView, AppError> {
        self.register_custom_gpt_attachment_with_image_policy(
            custom_gpt_id,
            local_path,
            display_name,
            media_type,
            size_bytes,
            sha256,
            None,
        )
    }

    pub fn register_custom_gpt_attachment_with_image_policy(
        &self,
        custom_gpt_id: &str,
        local_path: &str,
        display_name: &str,
        media_type: Option<&str>,
        size_bytes: i64,
        sha256: &str,
        describe_images: Option<bool>,
    ) -> Result<AttachmentView, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let active: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custom_gpts WHERE id = ?1 AND archived_at IS NULL
             )",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if !active {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        let current_count: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM custom_gpt_files WHERE custom_gpt_id = ?1",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if current_count >= 20 {
            return Err(AppError::Conflict(
                "cada GPT personal admite hasta 20 archivos de conocimiento".to_owned(),
            ));
        }
        let existing: Option<String> = match describe_images {
            Some(true) => transaction
                .query_row(
                    "SELECT id FROM attachments
                     WHERE sha256 = ?1 AND describe_images = 1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            Some(false) => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY CASE WHEN describe_images = 0 THEN 0 ELSE 1 END, created_at, id
                     LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
            None => transaction
                .query_row(
                    "SELECT id FROM attachments WHERE sha256 = ?1
                     ORDER BY created_at, id LIMIT 1",
                    params![sha256],
                    |row| row.get(0),
                )
                .optional()?,
        };
        let reused_attachment = existing.is_some();
        let attachment_id =
            existing.unwrap_or_else(|| format!("attachment_{}", Uuid::new_v4().simple()));
        transaction.execute(
            "INSERT OR IGNORE INTO attachments(
                id, local_path, display_name, media_type, size_bytes, sha256, describe_images
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
            params![
                attachment_id,
                local_path,
                display_name,
                media_type,
                size_bytes,
                sha256,
                describe_images
            ],
        )?;
        if reused_attachment {
            transaction.execute(
                "UPDATE attachments
                 SET broker_file_id = NULL,
                     ingestion_status = 'local',
                     ingestion_error_json = NULL,
                     context_status = 'pending',
                     context_error_json = NULL,
                     kind = NULL,
                     engine = NULL,
                     ingestion_meta_json = NULL,
                     updated_at = datetime('now')
                 WHERE id = ?1 AND ingestion_status = 'failed'",
                params![attachment_id],
            )?;
        }
        transaction.execute(
            "INSERT OR IGNORE INTO custom_gpt_files(custom_gpt_id, attachment_id)
             VALUES (?1, ?2)",
            params![custom_gpt_id, attachment_id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.file_added', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "attachment_id": attachment_id,
                "sha256": sha256,
                "size_bytes": size_bytes
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.attachment_view(&attachment_id)
    }

    pub fn list_custom_gpt_files(
        &self,
        custom_gpt_id: &str,
    ) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM custom_gpts WHERE id = ?1 AND archived_at IS NULL
             )",
            params![custom_gpt_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound(format!("GPT personal {custom_gpt_id}")));
        }
        let mut statement = connection.prepare(
            "SELECT attachment_id
             FROM custom_gpt_files
             WHERE custom_gpt_id = ?1
             ORDER BY added_at, attachment_id",
        )?;
        let ids = statement
            .query_map(params![custom_gpt_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|attachment_id| self.attachment_view(&attachment_id))
            .collect()
    }

    pub fn remove_custom_gpt_file(
        &self,
        custom_gpt_id: &str,
        attachment_id: &str,
    ) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "DELETE FROM custom_gpt_files
             WHERE custom_gpt_id = ?1 AND attachment_id = ?2",
            params![custom_gpt_id, attachment_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(
                "archivo de conocimiento del GPT personal".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('custom_gpt.file_removed', 'user', ?1)",
            params![serde_json::json!({
                "custom_gpt_id": custom_gpt_id,
                "attachment_id": attachment_id
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.list_custom_gpt_files(custom_gpt_id)
    }

    pub fn ready_custom_gpt_file_ids_for_conversation(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT file.attachment_id
             FROM conversations conversation
             JOIN custom_gpt_files file ON file.custom_gpt_id = conversation.custom_gpt_id
             JOIN attachments attachment ON attachment.id = file.attachment_id
             WHERE conversation.id = ?1
               AND conversation.archived_at IS NULL
               AND conversation.deleted_at IS NULL
               AND attachment.ingestion_status = 'ready'
               AND attachment.broker_file_id IS NOT NULL
             ORDER BY file.added_at, file.attachment_id",
        )?;
        let ids = statement
            .query_map(params![conversation_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(ids)
    }

    fn attachment_available_to_conversation(
        connection: &Connection,
        conversation_id: &str,
        attachment_id: &str,
    ) -> Result<bool, AppError> {
        connection
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM conversation_attachments
                    WHERE conversation_id = ?1 AND attachment_id = ?2
                    UNION ALL
                    SELECT 1
                    FROM conversations conversation
                    JOIN custom_gpt_files file
                      ON file.custom_gpt_id = conversation.custom_gpt_id
                    WHERE conversation.id = ?1
                      AND conversation.archived_at IS NULL
                      AND conversation.deleted_at IS NULL
                      AND file.attachment_id = ?2
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )
            .map_err(AppError::from)
    }

    pub fn list_attachments(&self, conversation_id: &str) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT a.id, a.display_name, a.media_type, a.size_bytes, a.sha256,
                    a.broker_file_id, a.ingestion_status, a.ingestion_error_json,
                    a.context_status, a.context_error_json,
                    (SELECT COUNT(*) FROM attachment_chunks chunk
                     WHERE chunk.attachment_id = a.id),
                    (SELECT COALESCE(SUM(length(chunk.content_text)), 0)
                     FROM attachment_chunks chunk
                     WHERE chunk.attachment_id = a.id),
                    (SELECT COUNT(*) FROM attachment_chunks chunk
                     WHERE chunk.attachment_id = a.id
                       AND EXISTS(
                         SELECT 1 FROM embedding_records embedding
                         WHERE embedding.source_type = 'attachment_chunk'
                           AND embedding.source_id = chunk.id
                           AND embedding.content_sha256 = chunk.content_sha256
                       )),
                    EXISTS(
                      SELECT 1 FROM broker_tasks task
                      JOIN attachment_chunks chunk
                        ON chunk.id = json_extract(task.request_json, '$.content.metadata.source_id')
                      WHERE chunk.attachment_id = a.id
                        AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                        AND task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
                    ),
                    (SELECT COUNT(DISTINCT chunk.id)
                     FROM attachment_chunks chunk
                     JOIN broker_tasks task
                       ON json_extract(task.request_json, '$.content.metadata.source_id') = chunk.id
                     WHERE chunk.attachment_id = a.id
                       AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                       AND task.local_state IN ('terminal', 'orphaned')
                       AND task.remote_status != 'completed'),
                    (SELECT embedding.model
                     FROM attachment_chunks chunk
                     JOIN embedding_records embedding
                       ON embedding.source_type = 'attachment_chunk'
                      AND embedding.source_id = chunk.id
                      AND embedding.content_sha256 = chunk.content_sha256
                     WHERE chunk.attachment_id = a.id
                     ORDER BY embedding.created_at DESC, embedding.rowid DESC
                     LIMIT 1),
                    a.describe_images, a.updated_at
             FROM conversation_attachments ca
             JOIN attachments a ON a.id = ca.attachment_id
             WHERE ca.conversation_id = ?1
             ORDER BY ca.added_at, a.created_at",
        )?;
        let attachments = statement
            .query_map(params![conversation_id], Self::map_attachment_view)?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(attachments)
    }

    pub fn list_project_files(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<AttachmentView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT pf.attachment_id
             FROM conversations c
             JOIN project_files pf ON pf.project_id = c.project_id
             JOIN attachments a ON a.id = pf.attachment_id
             WHERE c.id = ?1
               AND c.archived_at IS NULL
               AND c.deleted_at IS NULL
             ORDER BY pf.added_at, a.created_at",
        )?;
        let ids = statement
            .query_map(params![conversation_id], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|attachment_id| self.attachment_view(&attachment_id))
            .collect()
    }

    pub fn set_project_file(
        &self,
        conversation_id: &str,
        attachment_id: &str,
        enabled: bool,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let project_id = transaction
            .query_row(
                "SELECT project_id
                 FROM conversations
                 WHERE id = ?1
                   AND project_id IS NOT NULL
                   AND archived_at IS NULL
                   AND deleted_at IS NULL",
                params![conversation_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "la conversación debe pertenecer a un proyecto para compartir archivos"
                        .to_owned(),
                )
            })?;
        let changed = if enabled {
            let linked: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM conversation_attachments
                    WHERE conversation_id = ?1 AND attachment_id = ?2
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )?;
            if !linked {
                return Err(AppError::NotFound(format!(
                    "adjunto {attachment_id} en la conversación"
                )));
            }
            transaction.execute(
                "INSERT OR IGNORE INTO project_files(project_id, attachment_id)
                 VALUES (?1, ?2)",
                params![project_id, attachment_id],
            )?
        } else {
            transaction.execute(
                "DELETE FROM project_files
                 WHERE project_id = ?1 AND attachment_id = ?2",
                params![project_id, attachment_id],
            )?
        };
        if changed > 0 {
            transaction.execute(
                "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
                 VALUES (?1, 'user', ?2, ?3)",
                params![
                    if enabled {
                        "project.file_added"
                    } else {
                        "project.file_removed"
                    },
                    conversation_id,
                    serde_json::json!({
                        "project_id": project_id,
                        "attachment_id": attachment_id
                    })
                    .to_string()
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn use_project_file(
        &self,
        conversation_id: &str,
        attachment_id: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "INSERT OR IGNORE INTO conversation_attachments(conversation_id, attachment_id)
             SELECT c.id, pf.attachment_id
             FROM conversations c
             JOIN project_files pf ON pf.project_id = c.project_id
             WHERE c.id = ?1
               AND pf.attachment_id = ?2
               AND c.archived_at IS NULL
               AND c.deleted_at IS NULL",
            params![conversation_id, attachment_id],
        )?;
        if changed == 0 {
            let already_linked: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM conversation_attachments ca
                    JOIN conversations c ON c.id = ca.conversation_id
                    JOIN project_files pf
                      ON pf.project_id = c.project_id
                     AND pf.attachment_id = ca.attachment_id
                    WHERE ca.conversation_id = ?1 AND ca.attachment_id = ?2
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )?;
            if !already_linked {
                return Err(AppError::NotFound(format!(
                    "archivo de proyecto {attachment_id}"
                )));
            }
        } else {
            transaction.execute(
                "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
                 VALUES ('project.file_used', 'user', ?1, ?2)",
                params![
                    conversation_id,
                    serde_json::json!({"attachment_id": attachment_id}).to_string()
                ],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn conversation_attachment_records(
        &self,
        conversation_id: &str,
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT a.id, a.local_path, a.display_name, a.media_type, a.size_bytes,
                    a.sha256, a.broker_file_id, a.ingestion_status, a.describe_images
             FROM conversation_attachments ca
             JOIN attachments a ON a.id = ca.attachment_id
             WHERE ca.conversation_id = ?1
             ORDER BY ca.added_at, a.created_at",
        )?;
        let records = statement
            .query_map(params![conversation_id], |row| {
                Ok(AttachmentRecord {
                    id: row.get(0)?,
                    local_path: row.get(1)?,
                    display_name: row.get(2)?,
                    media_type: row.get(3)?,
                    size_bytes: row.get(4)?,
                    sha256: row.get(5)?,
                    broker_file_id: row.get(6)?,
                    ingestion_status: row.get(7)?,
                    describe_images: row.get(8)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(records)
    }

    pub fn remove_conversation_attachment(
        &self,
        conversation_id: &str,
        attachment_id: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "DELETE FROM conversation_attachments
             WHERE conversation_id = ?1 AND attachment_id = ?2",
            params![conversation_id, attachment_id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {attachment_id}")));
        }
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('attachment.removed', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({"attachment_id": attachment_id}).to_string()
            ],
        )?;
        Ok(())
    }

    fn map_attachment_view(row: &rusqlite::Row<'_>) -> rusqlite::Result<AttachmentView> {
        let error_json: Option<String> = row.get(7)?;
        let context_error_json: Option<String> = row.get(9)?;
        let chunk_count: i64 = row.get(10)?;
        let semantic_indexed_chunks: i64 = row.get(12)?;
        let semantic_active: bool = row.get(13)?;
        let semantic_failed_chunks: i64 = row.get(14)?;
        let semantic_index_status = if chunk_count == 0 {
            "unavailable"
        } else if semantic_indexed_chunks == chunk_count {
            "ready"
        } else if semantic_active {
            "indexing"
        } else if semantic_failed_chunks > 0 && semantic_indexed_chunks > 0 {
            "partial"
        } else if semantic_failed_chunks > 0 {
            "failed"
        } else {
            "pending"
        };
        Ok(AttachmentView {
            id: row.get(0)?,
            display_name: row.get(1)?,
            media_type: row.get(2)?,
            size_bytes: row.get(3)?,
            sha256: row.get(4)?,
            broker_file_id: row.get(5)?,
            ingestion_status: row.get(6)?,
            ingestion_error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
            context_status: row.get(8)?,
            context_error: context_error_json.and_then(|value| serde_json::from_str(&value).ok()),
            chunk_count,
            indexed_characters: row.get(11)?,
            semantic_indexed_chunks,
            semantic_index_status: semantic_index_status.to_owned(),
            semantic_index_model: row.get(15)?,
            describe_images: row.get(16)?,
            updated_at: row.get(17)?,
        })
    }

    pub fn attachment_view(&self, id: &str) -> Result<AttachmentView, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, display_name, media_type, size_bytes, sha256,
                        broker_file_id, ingestion_status, ingestion_error_json,
                        context_status, context_error_json,
                        (SELECT COUNT(*) FROM attachment_chunks chunk
                         WHERE chunk.attachment_id = attachments.id),
                        (SELECT COALESCE(SUM(length(chunk.content_text)), 0)
                         FROM attachment_chunks chunk
                         WHERE chunk.attachment_id = attachments.id),
                        (SELECT COUNT(*) FROM attachment_chunks chunk
                         WHERE chunk.attachment_id = attachments.id
                           AND EXISTS(
                             SELECT 1 FROM embedding_records embedding
                             WHERE embedding.source_type = 'attachment_chunk'
                               AND embedding.source_id = chunk.id
                               AND embedding.content_sha256 = chunk.content_sha256
                           )),
                        EXISTS(
                          SELECT 1 FROM broker_tasks task
                          JOIN attachment_chunks chunk
                            ON chunk.id = json_extract(task.request_json, '$.content.metadata.source_id')
                          WHERE chunk.attachment_id = attachments.id
                            AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                            AND task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
                        ),
                        (SELECT COUNT(DISTINCT chunk.id)
                         FROM attachment_chunks chunk
                         JOIN broker_tasks task
                           ON json_extract(task.request_json, '$.content.metadata.source_id') = chunk.id
                         WHERE chunk.attachment_id = attachments.id
                           AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                           AND task.local_state IN ('terminal', 'orphaned')
                           AND task.remote_status != 'completed'),
                        (SELECT embedding.model
                         FROM attachment_chunks chunk
                         JOIN embedding_records embedding
                           ON embedding.source_type = 'attachment_chunk'
                          AND embedding.source_id = chunk.id
                          AND embedding.content_sha256 = chunk.content_sha256
                         WHERE chunk.attachment_id = attachments.id
                         ORDER BY embedding.created_at DESC, embedding.rowid DESC
                         LIMIT 1),
                        describe_images, updated_at
                 FROM attachments WHERE id = ?1",
                params![id],
                Self::map_attachment_view,
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("adjunto {id}")))
    }

    pub fn attachment_record(&self, id: &str) -> Result<AttachmentRecord, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, local_path, display_name, media_type, size_bytes, sha256,
                        broker_file_id, ingestion_status, describe_images
                 FROM attachments WHERE id = ?1",
                params![id],
                |row| {
                    Ok(AttachmentRecord {
                        id: row.get(0)?,
                        local_path: row.get(1)?,
                        display_name: row.get(2)?,
                        media_type: row.get(3)?,
                        size_bytes: row.get(4)?,
                        sha256: row.get(5)?,
                        broker_file_id: row.get(6)?,
                        ingestion_status: row.get(7)?,
                        describe_images: row.get(8)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("adjunto {id}")))
    }

    pub fn set_attachment_describe_images(
        &self,
        id: &str,
        describe_images: bool,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE attachments
             SET describe_images = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![id, describe_images],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn recoverable_attachments(&self) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM attachments
             WHERE ingestion_status IN ('uploading', 'received', 'converting')
             ORDER BY updated_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| self.attachment_record(&id))
            .collect()
    }

    pub fn ready_attachments_without_chunks(&self) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT attachment.id
             FROM attachments attachment
             WHERE attachment.ingestion_status = 'ready'
               AND attachment.broker_file_id IS NOT NULL
               AND attachment.context_status IN ('pending', 'preparing')
               AND NOT EXISTS(
                   SELECT 1 FROM attachment_chunks chunk
                   WHERE chunk.attachment_id = attachment.id
               )
             ORDER BY attachment.updated_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter()
            .map(|id| self.attachment_record(&id))
            .collect()
    }

    pub fn mark_attachment_uploading(&self, id: &str) -> Result<(), AppError> {
        self.update_attachment_ingestion(id, "uploading", None, None, None, None, None)
    }

    pub fn mark_attachment_context_preparing(&self, id: &str) -> Result<(), AppError> {
        let changed = self.connect()?.execute(
            "UPDATE attachments
             SET context_status = 'preparing',
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn record_attachment_context_failure(
        &self,
        id: &str,
        error: &Value,
    ) -> Result<(), AppError> {
        let error_json = serde_json::to_string(error)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let changed = self.connect()?.execute(
            "UPDATE attachments
             SET context_status = 'failed',
                 context_error_json = ?2,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, error_json],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn mark_attachment_context_unavailable(&self, id: &str) -> Result<(), AppError> {
        let changed = self.connect()?.execute(
            "UPDATE attachments
             SET context_status = 'unavailable',
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn reset_attachment_context_for_retry(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE attachments
             SET context_status = 'pending',
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1
               AND ingestion_status = 'ready'
               AND context_status IN ('failed', 'unavailable')",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "el contexto de este adjunto no admite reintento".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('attachment.context_retry_requested', 'user', ?1)",
            params![serde_json::json!({"attachment_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reset_failed_attachment_for_retry(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE attachments
             SET broker_file_id = NULL,
                 ingestion_status = 'local',
                 ingestion_error_json = NULL,
                 context_status = 'pending',
                 context_error_json = NULL,
                 kind = NULL,
                 engine = NULL,
                 ingestion_meta_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1 AND ingestion_status = 'failed'",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "solo se puede reintentar un adjunto fallido".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('attachment.retry_requested', 'user', ?1)",
            params![serde_json::json!({"attachment_id": id}).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_attachment_ingestion(
        &self,
        id: &str,
        status: &str,
        broker_file_id: Option<&str>,
        kind: Option<&str>,
        engine: Option<&str>,
        meta: Option<&Value>,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let meta_json = meta
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let error_json = error
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE attachments
             SET ingestion_status = ?2,
                 broker_file_id = COALESCE(?3, broker_file_id),
                 kind = COALESCE(?4, kind),
                 engine = COALESCE(?5, engine),
                 ingestion_meta_json = COALESCE(?6, ingestion_meta_json),
                 ingestion_error_json = ?7,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                id,
                status,
                broker_file_id,
                kind,
                engine,
                meta_json,
                error_json
            ],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!("adjunto {id}")));
        }
        Ok(())
    }

    pub fn ready_attachments_for_turn(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
    ) -> Result<Vec<AttachmentRecord>, AppError> {
        let connection = self.connect()?;
        let mut result = Vec::with_capacity(attachment_ids.len());
        for id in attachment_ids {
            let record = self.attachment_record(id)?;
            let linked =
                Self::attachment_available_to_conversation(&connection, conversation_id, id)?;
            if !linked {
                return Err(AppError::Validation(format!(
                    "el adjunto {} no pertenece a esta conversación",
                    record.display_name
                )));
            }
            if record.ingestion_status != "ready" || record.broker_file_id.is_none() {
                return Err(AppError::Conflict(format!(
                    "el adjunto {} todavía no está listo",
                    record.display_name
                )));
            }
            result.push(record);
        }
        Ok(result)
    }

    pub fn replace_attachment_chunks(
        &self,
        attachment_id: &str,
        chunks: &[String],
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM attachments WHERE id = ?1)",
            params![attachment_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound(format!("adjunto {attachment_id}")));
        }
        transaction.execute(
            "DELETE FROM attachment_chunks WHERE attachment_id = ?1",
            params![attachment_id],
        )?;
        let mut stored_chunks = 0_i64;
        for (ordinal, text) in chunks.iter().enumerate() {
            let text = text.trim();
            if text.is_empty() {
                continue;
            }
            let content_sha256 = format!("{:x}", Sha256::digest(text.as_bytes()));
            transaction.execute(
                "INSERT INTO attachment_chunks(
                    id, attachment_id, ordinal, content_text, content_sha256
                 ) VALUES (?1, ?2, ?3, ?4, ?5)",
                params![
                    format!("chunk_{}_{}", attachment_id, ordinal),
                    attachment_id,
                    ordinal as i64,
                    text,
                    content_sha256
                ],
            )?;
            stored_chunks += 1;
        }
        transaction.execute(
            "UPDATE attachments
             SET context_status = ?2,
                 context_error_json = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                attachment_id,
                if stored_chunks > 0 {
                    "ready"
                } else {
                    "unavailable"
                }
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn next_attachment_chunk_for_embedding(
        &self,
        attachment_id: &str,
        retry_failed: bool,
    ) -> Result<Option<AttachmentChunkEmbeddingInput>, AppError> {
        let connection = self.connect()?;
        let active: bool = connection.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM broker_tasks task
                JOIN attachment_chunks chunk
                  ON chunk.id = json_extract(task.request_json, '$.content.metadata.source_id')
                WHERE chunk.attachment_id = ?1
                  AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                  AND task.local_state IN ('created', 'submitting', 'polling', 'recovery_pending')
             )",
            params![attachment_id],
            |row| row.get(0),
        )?;
        if active {
            return Ok(None);
        }
        connection
            .query_row(
                "SELECT chunk.id, chunk.content_text, chunk.content_sha256
                 FROM attachment_chunks chunk
                 JOIN attachments attachment ON attachment.id = chunk.attachment_id
                 WHERE chunk.attachment_id = ?1
                   AND attachment.context_status = 'ready'
                   AND NOT EXISTS(
                     SELECT 1 FROM embedding_records embedding
                     WHERE embedding.source_type = 'attachment_chunk'
                       AND embedding.source_id = chunk.id
                       AND embedding.content_sha256 = chunk.content_sha256
                   )
                   AND (
                     ?2 = 1 OR NOT EXISTS(
                       SELECT 1 FROM broker_tasks task
                       WHERE json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'
                         AND json_extract(task.request_json, '$.content.metadata.source_id') = chunk.id
                         AND json_extract(task.request_json, '$.content.metadata.content_sha256') = chunk.content_sha256
                         AND task.local_state IN ('terminal', 'orphaned')
                     )
                   )
                 ORDER BY chunk.ordinal
                 LIMIT 1",
                params![attachment_id, retry_failed],
                |row| {
                    Ok(AttachmentChunkEmbeddingInput {
                        id: row.get(0)?,
                        text: row.get(1)?,
                        content_sha256: row.get(2)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn attachments_needing_semantic_index(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT attachment.id
             FROM attachments attachment
             WHERE attachment.context_status = 'ready'
               AND EXISTS(
                 SELECT 1 FROM attachment_chunks chunk
                 WHERE chunk.attachment_id = attachment.id
                   AND NOT EXISTS(
                     SELECT 1 FROM embedding_records embedding
                     WHERE embedding.source_type = 'attachment_chunk'
                       AND embedding.source_id = chunk.id
                       AND embedding.content_sha256 = chunk.content_sha256
                   )
               )
             ORDER BY attachment.updated_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ids)
    }

    pub fn attachment_for_embedding_task(&self, task_id: &str) -> Result<Option<String>, AppError> {
        self.connect()?
            .query_row(
                "SELECT chunk.attachment_id
                 FROM broker_tasks task
                 JOIN attachment_chunks chunk
                   ON chunk.id = json_extract(task.request_json, '$.content.metadata.source_id')
                 WHERE task.id = ?1
                   AND json_extract(task.request_json, '$.content.metadata.source_type') = 'attachment_chunk'",
                params![task_id],
                |row| row.get(0),
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn attachments_have_semantic_index(
        &self,
        attachment_ids: &[String],
    ) -> Result<bool, AppError> {
        if attachment_ids.is_empty() {
            return Ok(false);
        }
        let connection = self.connect()?;
        for attachment_id in attachment_ids {
            let indexed: bool = connection.query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM attachment_chunks chunk
                    JOIN embedding_records embedding
                      ON embedding.source_type = 'attachment_chunk'
                     AND embedding.source_id = chunk.id
                     AND embedding.content_sha256 = chunk.content_sha256
                    WHERE chunk.attachment_id = ?1
                 )",
                params![attachment_id],
                |row| row.get(0),
            )?;
            if indexed {
                return Ok(true);
            }
        }
        Ok(false)
    }

    pub fn select_attachment_chunks(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
        query: &str,
        maximum_chunks: usize,
        character_budget: usize,
    ) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
        self.select_attachment_chunks_with_query(
            conversation_id,
            attachment_ids,
            query,
            maximum_chunks,
            character_budget,
            None,
        )
    }

    pub fn select_attachment_chunks_hybrid(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
        query: &str,
        maximum_chunks: usize,
        character_budget: usize,
        semantic_query_id: &str,
    ) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
        self.select_attachment_chunks_with_query(
            conversation_id,
            attachment_ids,
            query,
            maximum_chunks,
            character_budget,
            Some(semantic_query_id),
        )
    }

    fn select_attachment_chunks_with_query(
        &self,
        conversation_id: &str,
        attachment_ids: &[String],
        query: &str,
        maximum_chunks: usize,
        character_budget: usize,
        semantic_query_id: Option<&str>,
    ) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
        if attachment_ids.is_empty() || maximum_chunks == 0 || character_budget == 0 {
            return Ok(Vec::new());
        }
        let connection = self.connect()?;
        let query_terms = lexical_terms(query);
        let mut semantic_scores = HashMap::new();
        if let Some(semantic_query_id) = semantic_query_id {
            let query_embedding = connection
                .query_row(
                    "SELECT model, dimensions, vector_blob
                     FROM embedding_records
                     WHERE source_type IN ('chat_memory_search', 'chat_document_search')
                       AND source_id = ?1
                     ORDER BY created_at DESC, rowid DESC
                     LIMIT 1",
                    params![semantic_query_id],
                    |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, i64>(1)?,
                            row.get::<_, Vec<u8>>(2)?,
                        ))
                    },
                )
                .optional()?;
            if let Some((model, dimensions, query_blob)) = query_embedding {
                let query_vector = decode_embedding(&query_blob, dimensions)?;
                let allowed_attachments = attachment_ids.iter().collect::<HashSet<_>>();
                let mut statement = connection.prepare(
                    "SELECT chunk.id, chunk.attachment_id,
                            embedding.dimensions, embedding.vector_blob
                     FROM attachment_chunks chunk
                     JOIN embedding_records embedding
                       ON embedding.source_type = 'attachment_chunk'
                      AND embedding.source_id = chunk.id
                      AND embedding.content_sha256 = chunk.content_sha256
                     WHERE embedding.model = ?1 AND embedding.dimensions = ?2",
                )?;
                let rows = statement
                    .query_map(params![model, dimensions], |row| {
                        Ok((
                            row.get::<_, String>(0)?,
                            row.get::<_, String>(1)?,
                            row.get::<_, i64>(2)?,
                            row.get::<_, Vec<u8>>(3)?,
                        ))
                    })?
                    .collect::<Result<Vec<_>, _>>()?;
                for (chunk_id, attachment_id, candidate_dimensions, candidate_blob) in rows {
                    if !allowed_attachments.contains(&attachment_id) {
                        continue;
                    }
                    let candidate = decode_embedding(&candidate_blob, candidate_dimensions)?;
                    let score = cosine_similarity(&query_vector, &candidate);
                    if score.is_finite() {
                        semantic_scores.insert(chunk_id, score.max(0.0));
                    }
                }
            }
        }
        let mut candidates = Vec::new();
        for attachment_id in attachment_ids {
            let linked = Self::attachment_available_to_conversation(
                &connection,
                conversation_id,
                attachment_id,
            )?;
            if !linked {
                return Err(AppError::Validation(format!(
                    "el adjunto {attachment_id} no pertenece a esta conversación"
                )));
            }
            let mut statement = connection.prepare(
                "SELECT chunk.id, chunk.attachment_id, attachment.display_name,
                        chunk.ordinal, chunk.content_text
                 FROM attachment_chunks chunk
                 JOIN attachments attachment ON attachment.id = chunk.attachment_id
                 WHERE chunk.attachment_id = ?1
                 ORDER BY chunk.ordinal",
            )?;
            let rows = statement
                .query_map(params![attachment_id], |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, i64>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                })?
                .collect::<Result<Vec<_>, _>>()?;
            for (id, attachment_id, attachment_name, ordinal, text) in rows {
                let chunk_terms = lexical_terms(&text);
                let matched = query_terms.intersection(&chunk_terms).count();
                let lexical_score = if query_terms.is_empty() {
                    0.0
                } else {
                    matched as f64 / query_terms.len() as f64
                };
                let semantic_score = semantic_scores.get(&id).copied();
                let score = semantic_score
                    .map(|semantic| lexical_score * 0.35 + semantic * 0.65)
                    .unwrap_or(lexical_score);
                candidates.push(SelectedAttachmentChunk {
                    id,
                    attachment_id,
                    attachment_name,
                    ordinal,
                    text,
                    score,
                    reason: if matched > 0 && semantic_score.is_some() {
                        "Coincidencia léxica y semántica".to_owned()
                    } else if semantic_score.is_some_and(|semantic| semantic >= 0.25) {
                        "Coincidencia semántica".to_owned()
                    } else if matched > 0 {
                        "Coincidencia con la pregunta".to_owned()
                    } else {
                        "Inicio del documento".to_owned()
                    },
                });
            }
        }
        if is_global_document_request(query) {
            return select_global_document_chunks(candidates, maximum_chunks, character_budget);
        }
        let has_relevant = candidates.iter().any(|candidate| {
            candidate.score > 0.0
                && (candidate.reason != "Coincidencia semántica" || candidate.score >= 0.1625)
        });
        if has_relevant {
            let all_candidates = candidates;
            let mut relevant = all_candidates
                .iter()
                .filter(|candidate| {
                    candidate.score > 0.0
                        && (candidate.reason != "Coincidencia semántica"
                            || candidate.score >= 0.1625)
                })
                .cloned()
                .collect::<Vec<_>>();
            relevant.sort_by(|left, right| {
                right
                    .score
                    .total_cmp(&left.score)
                    .then_with(|| left.ordinal.cmp(&right.ordinal))
            });
            let mut expanded = relevant.clone();
            let mut included = relevant
                .iter()
                .map(|candidate| candidate.id.clone())
                .collect::<HashSet<_>>();
            for candidate in relevant {
                for neighbor_ordinal in [candidate.ordinal - 1, candidate.ordinal + 1] {
                    if let Some(neighbor) = all_candidates.iter().find(|other| {
                        other.attachment_id == candidate.attachment_id
                            && other.ordinal == neighbor_ordinal
                    }) {
                        if included.insert(neighbor.id.clone()) {
                            let mut neighbor = neighbor.clone();
                            neighbor.score = 0.0;
                            neighbor.reason = "Contexto próximo al fragmento relevante".to_owned();
                            expanded.push(neighbor);
                        }
                    }
                }
            }
            candidates = expanded;
        } else {
            candidates.sort_by(|left, right| {
                left.attachment_id
                    .cmp(&right.attachment_id)
                    .then_with(|| left.ordinal.cmp(&right.ordinal))
            });
        }
        let mut selected = Vec::new();
        let mut used_characters = 0_usize;
        for candidate in candidates {
            let candidate_characters = candidate.text.chars().count();
            if used_characters.saturating_add(candidate_characters) > character_budget {
                continue;
            }
            used_characters += candidate_characters;
            selected.push(candidate);
            if selected.len() == maximum_chunks {
                break;
            }
        }
        Ok(selected)
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub fn prepare_semantic_chat_turn(
        &self,
        workflow_id: &str,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        embedding_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        embedding_request: &Value,
        context: &[ContextMessage],
        attachment_ids: &[String],
        tools_enabled: bool,
        sandbox_enabled: bool,
        execution_preferences: &ConversationExecutionPreferences,
        research_plan: Option<&Value>,
    ) -> Result<BrokerTaskRecord, AppError> {
        self.prepare_semantic_chat_turn_with_project_instruction(
            workflow_id,
            conversation_id,
            user_message_id,
            assistant_message_id,
            embedding_task_id,
            idempotency_key,
            user_text,
            embedding_request,
            context,
            None,
            None,
            attachment_ids,
            tools_enabled,
            sandbox_enabled,
            execution_preferences,
            research_plan,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_semantic_chat_turn_with_project_instruction(
        &self,
        workflow_id: &str,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        embedding_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        embedding_request: &Value,
        context: &[ContextMessage],
        project_instruction: Option<&ProjectInstructionContext>,
        custom_gpt_context: Option<&CustomGptContext>,
        attachment_ids: &[String],
        tools_enabled: bool,
        sandbox_enabled: bool,
        execution_preferences: &ConversationExecutionPreferences,
        research_plan: Option<&Value>,
    ) -> Result<BrokerTaskRecord, AppError> {
        let research_plan_json = research_plan
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let request_json = serde_json::to_string(embedding_request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let context_json = serde_json::to_string(context)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let project_instruction_json =
            project_instruction
                .map(serde_json::to_string)
                .transpose()
                .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let custom_gpt_context_json = custom_gpt_context
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let attachment_ids_json = serde_json::to_string(attachment_ids)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        validate_execution_preferences(execution_preferences)?;
        let execution_preferences_json = serde_json::to_string(execution_preferences)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let next_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence_no), 0) + 1
             FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO messages(id, conversation_id, role, status, sequence_no)
             VALUES (?1, ?2, 'user', 'complete', ?3)",
            params![user_message_id, conversation_id, next_sequence],
        )?;
        transaction.execute(
            "INSERT INTO message_parts(id, message_id, ordinal, kind, content_text)
             VALUES (?1, ?2, 0, 'text', ?3)",
            params![
                format!("part_{}", Uuid::new_v4().simple()),
                user_message_id,
                user_text
            ],
        )?;
        for (ordinal, attachment_id) in attachment_ids.iter().enumerate() {
            let usable: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM attachments a
                    WHERE a.id = ?2
                      AND a.ingestion_status = 'ready'
                      AND a.broker_file_id IS NOT NULL
                      AND (
                        EXISTS(
                          SELECT 1 FROM conversation_attachments ca
                          WHERE ca.conversation_id = ?1 AND ca.attachment_id = a.id
                        )
                        OR EXISTS(
                          SELECT 1
                          FROM conversations conversation
                          JOIN custom_gpt_files file
                            ON file.custom_gpt_id = conversation.custom_gpt_id
                          WHERE conversation.id = ?1
                            AND conversation.archived_at IS NULL
                            AND conversation.deleted_at IS NULL
                            AND file.attachment_id = a.id
                        )
                      )
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )?;
            if !usable {
                return Err(AppError::Conflict(
                    "uno de los adjuntos ya no está listo para enviar".to_owned(),
                ));
            }
            transaction.execute(
                "INSERT INTO message_attachments(message_id, attachment_id, ordinal)
                 VALUES (?1, ?2, ?3)",
                params![user_message_id, attachment_id, ordinal as i64],
            )?;
        }
        transaction.execute(
            "INSERT INTO messages(
                id, conversation_id, role, status, sequence_no, created_at, updated_at
             ) VALUES (
                ?1, ?2, 'assistant', 'pending', ?3,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![assistant_message_id, conversation_id, next_sequence + 1],
        )?;
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, request_message_id, idempotency_key,
                request_json, remote_status, local_state
             ) VALUES (?1, ?2, ?3, ?4, ?5, 'not_submitted', 'created')",
            params![
                embedding_task_id,
                conversation_id,
                user_message_id,
                idempotency_key,
                request_json
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET broker_task_id = ?2 WHERE id = ?1",
            params![assistant_message_id, embedding_task_id],
        )?;
        transaction.execute(
            "INSERT INTO semantic_chat_workflows(
                id, conversation_id, user_message_id, assistant_message_id,
                embedding_task_id, user_text, context_json, attachment_ids_json,
                tools_enabled, sandbox_enabled, execution_preferences_json,
                project_instruction_json, custom_gpt_context_json,
                research_plan_json
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14)",
            params![
                workflow_id,
                conversation_id,
                user_message_id,
                assistant_message_id,
                embedding_task_id,
                user_text,
                context_json,
                attachment_ids_json,
                i64::from(tools_enabled),
                i64::from(sandbox_enabled),
                execution_preferences_json,
                project_instruction_json,
                custom_gpt_context_json,
                research_plan_json
            ],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![embedding_task_id],
        )?;
        transaction.execute(
            "UPDATE conversations
             SET title = CASE WHEN NOT EXISTS(
                    SELECT 1 FROM messages
                    WHERE conversation_id = ?1 AND sequence_no < ?2
                 ) THEN substr(?3, 1, 80) ELSE title END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![conversation_id, next_sequence, user_text],
        )?;
        transaction.commit()?;
        self.task_record(embedding_task_id)
    }

    pub fn semantic_chat_workflow_for_task(
        &self,
        task_id: &str,
    ) -> Result<Option<SemanticChatWorkflow>, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, conversation_id, user_message_id, assistant_message_id,
                        embedding_task_id, chat_task_id, user_text, context_json,
                        attachment_ids_json, tools_enabled, sandbox_enabled,
                        execution_preferences_json, status, project_instruction_json,
                        custom_gpt_context_json, research_plan_json
                 FROM semantic_chat_workflows
                 WHERE embedding_task_id = ?1 OR chat_task_id = ?1",
                params![task_id],
                |row| {
                    let context_json: String = row.get(7)?;
                    let attachment_ids_json: String = row.get(8)?;
                    let context = serde_json::from_str(&context_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            context_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let attachment_ids =
                        serde_json::from_str(&attachment_ids_json).map_err(|error| {
                            rusqlite::Error::FromSqlConversionFailure(
                                attachment_ids_json.len(),
                                rusqlite::types::Type::Text,
                                Box::new(error),
                            )
                        })?;
                    let execution_preferences_json: String = row.get(11)?;
                    let execution_preferences = serde_json::from_str(&execution_preferences_json)
                        .map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            execution_preferences_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    let project_instruction_json: Option<String> = row.get(13)?;
                    let project_instruction = project_instruction_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    value.len(),
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    let custom_gpt_context_json: Option<String> = row.get(14)?;
                    let custom_gpt_context = custom_gpt_context_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    value.len(),
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    let research_plan_json: Option<String> = row.get(15)?;
                    let research_plan = research_plan_json
                        .map(|value| {
                            serde_json::from_str(&value).map_err(|error| {
                                rusqlite::Error::FromSqlConversionFailure(
                                    value.len(),
                                    rusqlite::types::Type::Text,
                                    Box::new(error),
                                )
                            })
                        })
                        .transpose()?;
                    Ok(SemanticChatWorkflow {
                        id: row.get(0)?,
                        conversation_id: row.get(1)?,
                        user_message_id: row.get(2)?,
                        assistant_message_id: row.get(3)?,
                        embedding_task_id: row.get(4)?,
                        chat_task_id: row.get(5)?,
                        user_text: row.get(6)?,
                        context,
                        project_instruction,
                        custom_gpt_context,
                        attachment_ids,
                        tools_enabled: row.get(9)?,
                        sandbox_enabled: row.get(10)?,
                        execution_preferences,
                        research_plan,
                        status: row.get(12)?,
                    })
                },
            )
            .optional()
            .map_err(AppError::from)
    }

    pub fn semantic_memory_matches(
        &self,
        workflow_id: &str,
    ) -> Result<Vec<SemanticMemoryMatch>, AppError> {
        let overview = self.memory_overview()?;
        let connection = self.connect()?;
        let (project_id, custom_gpt_id, model, dimensions, query_blob): (
            Option<String>,
            Option<String>,
            String,
            i64,
            Vec<u8>,
        ) = connection
            .query_row(
                "SELECT c.project_id, c.custom_gpt_id, er.model, er.dimensions, er.vector_blob
                 FROM semantic_chat_workflows workflow
                 JOIN conversations c ON c.id = workflow.conversation_id
                 JOIN embedding_records er
                   ON er.source_type = 'chat_memory_search'
                  AND er.source_id = workflow.id
                 WHERE workflow.id = ?1
                 ORDER BY er.created_at DESC, er.rowid DESC
                 LIMIT 1",
                params![workflow_id],
                |row| {
                    Ok((
                        row.get(0)?,
                        row.get(1)?,
                        row.get(2)?,
                        row.get(3)?,
                        row.get(4)?,
                    ))
                },
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "la consulta semántica todavía no tiene un vector utilizable".to_owned(),
                )
            })?;
        if !overview.enabled && custom_gpt_id.is_none() {
            return Ok(Vec::new());
        }
        let query_vector = decode_embedding(&query_blob, dimensions)?;
        let mut statement = connection.prepare(
            "SELECT m.id, er.dimensions, er.vector_blob
             FROM memory_items m
             JOIN embedding_records er
              ON er.source_type = 'memory' AND er.source_id = m.id
              AND er.model = ?1 AND er.dimensions = ?2
             WHERE m.enabled = 1
               AND (
                 (?4 = 1 AND m.custom_gpt_id IS NULL
                    AND (m.project_id IS NULL OR (?3 IS NOT NULL AND m.project_id = ?3)))
                 OR (?5 IS NOT NULL AND m.custom_gpt_id = ?5)
               )
             ORDER BY m.updated_at DESC",
        )?;
        let candidates = statement
            .query_map(
                params![
                    model,
                    dimensions,
                    project_id.as_deref(),
                    overview.enabled,
                    custom_gpt_id.as_deref()
                ],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, i64>(1)?,
                        row.get::<_, Vec<u8>>(2)?,
                    ))
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        let mut visible_items = if overview.enabled {
            overview.items
        } else {
            Vec::new()
        };
        if let Some(custom_gpt_id) = custom_gpt_id.as_deref() {
            visible_items.extend(self.custom_gpt_knowledge(custom_gpt_id)?);
        }
        let items_by_id: HashMap<String, MemoryItemView> = visible_items
            .into_iter()
            .map(|item| (item.id.clone(), item))
            .collect();
        let mut matches = Vec::new();
        for (memory_id, candidate_dimensions, candidate_blob) in candidates {
            let Some(memory) = items_by_id.get(&memory_id) else {
                continue;
            };
            let candidate = decode_embedding(&candidate_blob, candidate_dimensions)?;
            let score = cosine_similarity(&query_vector, &candidate);
            if !score.is_finite() || score < 0.25 {
                continue;
            }
            let reason = if score >= 0.75 {
                "Coincidencia semántica alta"
            } else if score >= 0.5 {
                "Coincidencia semántica media"
            } else {
                "Coincidencia semántica baja"
            };
            matches.push(SemanticMemoryMatch {
                memory: memory.clone(),
                score: (score * 1000.0).round() / 1000.0,
                reason: reason.to_owned(),
            });
        }
        matches.sort_by(|left, right| {
            right
                .score
                .partial_cmp(&left.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
        matches.truncate(5);
        Ok(matches)
    }

    pub fn semantic_workflow_uses_memory(&self, workflow_id: &str) -> Result<bool, AppError> {
        self.connect()?
            .query_row(
                "SELECT json_extract(task.request_json, '$.content.metadata.source_type')
                         = 'chat_memory_search'
                 FROM semantic_chat_workflows workflow
                 JOIN broker_tasks task ON task.id = workflow.embedding_task_id
                 WHERE workflow.id = ?1",
                params![workflow_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("flujo semántico {workflow_id}")))
    }

    pub fn prepare_semantic_chat_submission(
        &self,
        workflow_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        request: &Value,
        memories: &[SemanticMemoryMatch],
        document_chunks: &[SelectedAttachmentChunk],
    ) -> Result<BrokerTaskRecord, AppError> {
        let workflow = self
            .semantic_chat_workflow_for_id(workflow_id)?
            .ok_or_else(|| AppError::NotFound(format!("flujo semántico {workflow_id}")))?;
        if workflow.status != "searching" {
            if let Some(chat_task_id) = workflow.chat_task_id {
                return self.task_record(&chat_task_id);
            }
            return Err(AppError::Conflict(
                "el flujo semántico ya no admite preparar otra tarea".to_owned(),
            ));
        }
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let memory_items = memories
            .iter()
            .map(|item| item.memory.clone())
            .collect::<Vec<_>>();
        let final_context_json = serde_json::to_string(&serde_json::json!({
            "messages": workflow.context,
            "projectInstruction": workflow.project_instruction,
            "customGpt": workflow.custom_gpt_context,
            "memories": memory_items,
            "documentChunks": document_chunks
        }))
        .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let claimed = transaction.execute(
            "UPDATE semantic_chat_workflows
             SET status = 'preparing_chat', updated_at = datetime('now')
             WHERE id = ?1 AND status = 'searching'",
            params![workflow_id],
        )?;
        if claimed == 0 {
            let existing_task_id: Option<String> = transaction
                .query_row(
                    "SELECT chat_task_id FROM semantic_chat_workflows WHERE id = ?1",
                    params![workflow_id],
                    |row| row.get(0),
                )
                .optional()?
                .flatten();
            if let Some(existing_task_id) = existing_task_id {
                drop(transaction);
                return self.task_record(&existing_task_id);
            }
            return Err(AppError::Conflict(
                "el flujo semántico está siendo preparado por otra operación".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, request_message_id, response_message_id,
                idempotency_key, request_json, remote_status, local_state,
                gpt_version_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'not_submitted', 'created', ?7)",
            params![
                local_task_id,
                workflow.conversation_id,
                workflow.user_message_id,
                workflow.assistant_message_id,
                idempotency_key,
                request_json,
                workflow
                    .custom_gpt_context
                    .as_ref()
                    .map(|context| context.version_id.as_str())
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET broker_task_id = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![workflow.assistant_message_id, local_task_id],
        )?;
        insert_research_run_if_needed(
            &transaction,
            request,
            &workflow.conversation_id,
            local_task_id,
            &workflow.user_text,
        )?;
        let snapshot_id = format!("ctx_{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO context_snapshots(
                id, broker_task_id, strategy_version, token_budget,
                estimated_tokens, final_context_json
             ) VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
            params![
                snapshot_id,
                local_task_id,
                match (
                    workflow
                        .context
                        .iter()
                        .any(|source| source.role == "summary"),
                    workflow.project_instruction.is_some(),
                    document_chunks.is_empty(),
                ) {
                    (true, false, true) => "window-summary-semantic-memory-v1",
                    (true, false, false) => "window-summary-semantic-memory-document-v1",
                    (false, false, true) => "window-semantic-memory-v1",
                    (false, false, false) => "window-semantic-memory-document-v1",
                    (true, true, true) => "window-summary-project-semantic-memory-v1",
                    (true, true, false) => {
                        "window-summary-project-semantic-memory-document-v1"
                    }
                    (false, true, true) => "window-project-semantic-memory-v1",
                    (false, true, false) => "window-project-semantic-memory-document-v1",
                },
                (final_context_json.chars().count() as i64 + 3) / 4,
                final_context_json
            ],
        )?;
        for (ordinal, source) in workflow.context.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    if source.role == "summary" {
                        "summary"
                    } else {
                        "message"
                    },
                    source.message_id,
                    ordinal as i64,
                    if source.role == "summary" {
                        "approved_conversation_summary"
                    } else if source.message_id == workflow.user_message_id {
                        "current_user_turn"
                    } else {
                        "recent_conversation_window"
                    },
                    (source.text.chars().count() as i64 + 3) / 4,
                    source.text
                ],
            )?;
        }
        if let Some(project_instruction) = workflow.project_instruction.as_ref() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'project_instruction', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    project_instruction.project_id,
                    workflow.context.len() as i64,
                    "Instrucciones configuradas para el proyecto",
                    (project_instruction.instructions.chars().count() as i64 + 3) / 4,
                    project_instruction.instructions
                ],
            )?;
        }
        if let Some(custom_gpt) = workflow.custom_gpt_context.as_ref() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'custom_gpt', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    custom_gpt.version_id,
                    (workflow.context.len() + usize::from(workflow.project_instruction.is_some()))
                        as i64,
                    "Versión del GPT personal seleccionada al enviar",
                    (custom_gpt.instructions.chars().count() as i64 + 3) / 4,
                    format!(
                        "{} · versión {}\n{}\nPermisos: código aislado = {}; renombrar chat = {}",
                        custom_gpt.name,
                        custom_gpt.version_no,
                        custom_gpt.instructions,
                        custom_gpt.tool_permissions.run_code,
                        custom_gpt.tool_permissions.rename_conversation
                    )
                ],
            )?;
        }
        for (index, selected) in memories.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, score, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'memory', ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    selected.memory.id,
                    (workflow.context.len()
                        + usize::from(workflow.project_instruction.is_some())
                        + usize::from(workflow.custom_gpt_context.is_some())
                        + index) as i64,
                    selected.reason,
                    selected.score,
                    (selected.memory.content.chars().count() as i64 + 3) / 4,
                    selected.memory.content
                ],
            )?;
        }
        for (index, chunk) in document_chunks.iter().enumerate() {
            let from_custom_gpt = if let Some(custom_gpt) = workflow.custom_gpt_context.as_ref() {
                transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM custom_gpt_files
                        WHERE custom_gpt_id = ?1 AND attachment_id = ?2
                     )",
                    params![custom_gpt.custom_gpt_id, chunk.attachment_id],
                    |row| row.get::<_, bool>(0),
                )?
            } else {
                false
            };
            let reason = if from_custom_gpt {
                format!(
                    "Archivo de conocimiento del GPT personal seleccionado · {}",
                    chunk.reason
                )
            } else {
                chunk.reason.clone()
            };
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, score, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'attachment_chunk', ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    chunk.id,
                    (workflow.context.len()
                        + usize::from(workflow.project_instruction.is_some())
                        + usize::from(workflow.custom_gpt_context.is_some())
                        + memories.len()
                        + index) as i64,
                    reason,
                    chunk.score,
                    (chunk.text.chars().count() as i64 + 3) / 4,
                    chunk.text
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![local_task_id],
        )?;
        transaction.execute(
            "UPDATE semantic_chat_workflows
             SET chat_task_id = ?2, status = 'submitted', updated_at = datetime('now')
             WHERE id = ?1 AND status = 'preparing_chat'",
            params![workflow_id, local_task_id],
        )?;
        transaction.commit()?;
        self.task_record(local_task_id)
    }

    fn semantic_chat_workflow_for_id(
        &self,
        workflow_id: &str,
    ) -> Result<Option<SemanticChatWorkflow>, AppError> {
        let task_id = self
            .connect()?
            .query_row(
                "SELECT embedding_task_id FROM semantic_chat_workflows WHERE id = ?1",
                params![workflow_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?;
        task_id
            .map(|task_id| self.semantic_chat_workflow_for_task(&task_id))
            .transpose()
            .map(Option::flatten)
    }

    pub fn semantic_chat_workflows_ready_to_continue(&self) -> Result<Vec<String>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT workflow.embedding_task_id
             FROM semantic_chat_workflows workflow
             JOIN broker_tasks task ON task.id = workflow.embedding_task_id
             WHERE workflow.status = 'searching'
               AND (task.remote_status IN ('completed', 'failed', 'cancelled')
                    OR task.local_state = 'orphaned')
             ORDER BY workflow.created_at",
        )?;
        let task_ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(task_ids)
    }

    pub fn finish_semantic_chat_without_submission(
        &self,
        embedding_task_id: &str,
        cancelled: bool,
        message: &str,
    ) -> Result<(), AppError> {
        let Some(workflow) = self.semantic_chat_workflow_for_task(embedding_task_id)? else {
            return Ok(());
        };
        if workflow.status != "searching" {
            return Ok(());
        }
        let error = serde_json::json!({
            "code": if cancelled { "CANCELLED" } else { "SEMANTIC_MEMORY_FAILED" },
            "message": message
        });
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE semantic_chat_workflows
             SET status = ?2, error_json = ?3, updated_at = datetime('now')
             WHERE id = ?1 AND status = 'searching'",
            params![
                workflow.id,
                if cancelled { "cancelled" } else { "failed" },
                error.to_string()
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET status = ?2, updated_at = datetime('now')
             WHERE id = ?1",
            params![
                workflow.assistant_message_id,
                if cancelled { "cancelled" } else { "failed" }
            ],
        )?;
        transaction.execute(
            "INSERT INTO message_parts(
                id, message_id, ordinal, kind, content_json
             ) VALUES (?1, ?2, 0, 'error', ?3)
             ON CONFLICT(message_id, ordinal) DO UPDATE SET
                kind = excluded.kind,
                content_text = NULL,
                content_json = excluded.content_json",
            params![
                format!("part_{}", Uuid::new_v4().simple()),
                workflow.assistant_message_id,
                error.to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    #[cfg(test)]
    pub fn prepare_chat_turn(
        &self,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        request: &Value,
        context: &[ContextMessage],
        memories: &[MemoryItemView],
        document_chunks: &[SelectedAttachmentChunk],
        attachment_ids: &[String],
    ) -> Result<BrokerTaskRecord, AppError> {
        self.prepare_chat_turn_with_project_instruction(
            conversation_id,
            user_message_id,
            assistant_message_id,
            local_task_id,
            idempotency_key,
            user_text,
            request,
            context,
            None,
            None,
            memories,
            document_chunks,
            attachment_ids,
        )
    }

    #[allow(clippy::too_many_arguments)]
    pub fn prepare_chat_turn_with_project_instruction(
        &self,
        conversation_id: &str,
        user_message_id: &str,
        assistant_message_id: &str,
        local_task_id: &str,
        idempotency_key: &str,
        user_text: &str,
        request: &Value,
        context: &[ContextMessage],
        project_instruction: Option<&ProjectInstructionContext>,
        custom_gpt_context: Option<&CustomGptContext>,
        memories: &[MemoryItemView],
        document_chunks: &[SelectedAttachmentChunk],
        attachment_ids: &[String],
    ) -> Result<BrokerTaskRecord, AppError> {
        let request_json = serde_json::to_string(request)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let context_json = serde_json::to_string(&serde_json::json!({
            "messages": context,
            "projectInstruction": project_instruction,
            "customGpt": custom_gpt_context,
            "memories": memories,
            "documentChunks": document_chunks
        }))
        .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let next_sequence: i64 = transaction.query_row(
            "SELECT COALESCE(MAX(sequence_no), 0) + 1
             FROM messages WHERE conversation_id = ?1",
            params![conversation_id],
            |row| row.get(0),
        )?;
        transaction.execute(
            "INSERT INTO messages(
                id, conversation_id, role, status, sequence_no
             ) VALUES (?1, ?2, 'user', 'complete', ?3)",
            params![user_message_id, conversation_id, next_sequence],
        )?;
        for (ordinal, attachment_id) in attachment_ids.iter().enumerate() {
            let usable: bool = transaction.query_row(
                "SELECT EXISTS(
                    SELECT 1
                    FROM attachments a
                    WHERE a.id = ?2
                      AND a.ingestion_status = 'ready'
                      AND a.broker_file_id IS NOT NULL
                      AND (
                        EXISTS(
                          SELECT 1 FROM conversation_attachments ca
                          WHERE ca.conversation_id = ?1 AND ca.attachment_id = a.id
                        )
                        OR EXISTS(
                          SELECT 1
                          FROM conversations conversation
                          JOIN custom_gpt_files file
                            ON file.custom_gpt_id = conversation.custom_gpt_id
                          WHERE conversation.id = ?1
                            AND conversation.archived_at IS NULL
                            AND conversation.deleted_at IS NULL
                            AND file.attachment_id = a.id
                        )
                      )
                 )",
                params![conversation_id, attachment_id],
                |row| row.get(0),
            )?;
            if !usable {
                return Err(AppError::Conflict(
                    "uno de los adjuntos ya no esta listo para enviar".to_owned(),
                ));
            }
            transaction.execute(
                "INSERT INTO message_attachments(message_id, attachment_id, ordinal)
                 VALUES (?1, ?2, ?3)",
                params![user_message_id, attachment_id, ordinal as i64],
            )?;
        }
        transaction.execute(
            "INSERT INTO message_parts(
                id, message_id, ordinal, kind, content_text
             ) VALUES (?1, ?2, 0, 'text', ?3)",
            params![
                format!("part_{}", Uuid::new_v4().simple()),
                user_message_id,
                user_text
            ],
        )?;
        transaction.execute(
            "INSERT INTO messages(
                id, conversation_id, role, status, sequence_no, created_at, updated_at
             ) VALUES (
                ?1, ?2, 'assistant', 'pending', ?3,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![assistant_message_id, conversation_id, next_sequence + 1],
        )?;
        transaction.execute(
            "INSERT INTO broker_tasks(
                id, conversation_id, request_message_id, response_message_id,
                idempotency_key, request_json, remote_status, local_state,
                gpt_version_id
             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'not_submitted', 'created', ?7)",
            params![
                local_task_id,
                conversation_id,
                user_message_id,
                assistant_message_id,
                idempotency_key,
                request_json,
                custom_gpt_context.map(|context| context.version_id.as_str())
            ],
        )?;
        transaction.execute(
            "UPDATE messages SET broker_task_id = ?2 WHERE id = ?1",
            params![assistant_message_id, local_task_id],
        )?;
        insert_research_run_if_needed(
            &transaction,
            request,
            conversation_id,
            local_task_id,
            user_text,
        )?;
        let snapshot_id = format!("ctx_{}", Uuid::new_v4().simple());
        let has_summary = context.iter().any(|source| source.role == "summary");
        let strategy_version = match (
            has_summary,
            project_instruction.is_some(),
            memories.is_empty(),
            document_chunks.is_empty(),
        ) {
            (true, false, true, true) => "window-summary-v1",
            (true, false, false, true) => "window-summary-memory-v1",
            (false, false, true, true) => "window-v1",
            (false, false, false, true) => "window-memory-v1",
            (true, false, true, false) => "window-summary-document-v1",
            (true, false, false, false) => "window-summary-memory-document-v1",
            (false, false, true, false) => "window-document-v1",
            (false, false, false, false) => "window-memory-document-v1",
            (true, true, true, true) => "window-summary-project-v1",
            (true, true, false, true) => "window-summary-project-memory-v1",
            (false, true, true, true) => "window-project-v1",
            (false, true, false, true) => "window-project-memory-v1",
            (true, true, true, false) => "window-summary-project-document-v1",
            (true, true, false, false) => "window-summary-project-memory-document-v1",
            (false, true, true, false) => "window-project-document-v1",
            (false, true, false, false) => "window-project-memory-document-v1",
        };
        transaction.execute(
            "INSERT INTO context_snapshots(
                id, broker_task_id, strategy_version, token_budget,
                estimated_tokens, final_context_json
             ) VALUES (?1, ?2, ?3, NULL, ?4, ?5)",
            params![
                snapshot_id,
                local_task_id,
                strategy_version,
                (context_json.chars().count() as i64 + 3) / 4,
                context_json
            ],
        )?;
        for (ordinal, source) in context.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    if source.role == "summary" {
                        "summary"
                    } else {
                        "message"
                    },
                    source.message_id,
                    ordinal as i64,
                    if source.role == "summary" {
                        "approved_conversation_summary"
                    } else if source.message_id == user_message_id {
                        "current_user_turn"
                    } else {
                        "recent_conversation_window"
                    },
                    (source.text.chars().count() as i64 + 3) / 4,
                    source.text
                ],
            )?;
        }
        if let Some(project_instruction) = project_instruction {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'project_instruction', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    project_instruction.project_id,
                    context.len() as i64,
                    "Instrucciones configuradas para el proyecto",
                    (project_instruction.instructions.chars().count() as i64 + 3) / 4,
                    project_instruction.instructions
                ],
            )?;
        }
        if let Some(custom_gpt) = custom_gpt_context {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'custom_gpt', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    custom_gpt.version_id,
                    (context.len() + usize::from(project_instruction.is_some())) as i64,
                    "Versión del GPT personal seleccionada al enviar",
                    (custom_gpt.instructions.chars().count() as i64 + 3) / 4,
                    format!(
                        "{} · versión {}\n{}\nPermisos: código aislado = {}; renombrar chat = {}",
                        custom_gpt.name,
                        custom_gpt.version_no,
                        custom_gpt.instructions,
                        custom_gpt.tool_permissions.run_code,
                        custom_gpt.tool_permissions.rename_conversation
                    )
                ],
            )?;
        }
        for (index, memory) in memories.iter().enumerate() {
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'memory', ?3, ?4, ?5, ?6, ?7)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    memory.id,
                    (context.len()
                        + usize::from(project_instruction.is_some())
                        + usize::from(custom_gpt_context.is_some())
                        + index) as i64,
                    if memory.custom_gpt_id.is_some() {
                        "Conocimiento privado del GPT personal seleccionado"
                    } else {
                        "Recuerdo activado explícitamente por el usuario"
                    },
                    (memory.content.chars().count() as i64 + 3) / 4,
                    memory.content
                ],
            )?;
        }
        for (index, chunk) in document_chunks.iter().enumerate() {
            let from_custom_gpt = if let Some(custom_gpt) = custom_gpt_context {
                transaction.query_row(
                    "SELECT EXISTS(
                        SELECT 1 FROM custom_gpt_files
                        WHERE custom_gpt_id = ?1 AND attachment_id = ?2
                     )",
                    params![custom_gpt.custom_gpt_id, chunk.attachment_id],
                    |row| row.get::<_, bool>(0),
                )?
            } else {
                false
            };
            let reason = if from_custom_gpt {
                format!(
                    "Archivo de conocimiento del GPT personal seleccionado · {}",
                    chunk.reason
                )
            } else {
                chunk.reason.clone()
            };
            transaction.execute(
                "INSERT INTO context_sources(
                    id, snapshot_id, source_type, source_id, ordinal,
                    reason, score, estimated_tokens, excerpt
                 ) VALUES (?1, ?2, 'attachment_chunk', ?3, ?4, ?5, ?6, ?7, ?8)",
                params![
                    format!("ctxsrc_{}", Uuid::new_v4().simple()),
                    snapshot_id,
                    chunk.id,
                    (context.len()
                        + usize::from(project_instruction.is_some())
                        + usize::from(custom_gpt_context.is_some())
                        + memories.len()
                        + index) as i64,
                    reason,
                    chunk.score,
                    (chunk.text.chars().count() as i64 + 3) / 4,
                    chunk.text
                ],
            )?;
        }
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.prepared', 'not_submitted', '{}', datetime('now'))",
            params![local_task_id],
        )?;
        transaction.execute(
            "UPDATE conversations
             SET title = CASE WHEN NOT EXISTS(
                    SELECT 1 FROM messages
                    WHERE conversation_id = ?1 AND sequence_no < ?2
                 ) THEN substr(?3, 1, 80) ELSE title END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![conversation_id, next_sequence, user_text],
        )?;
        transaction.commit()?;
        self.task_record(local_task_id)
    }

    pub fn conversation_view(&self, id: &str) -> Result<ConversationView, AppError> {
        let summary = self.conversation_summary(id)?;
        let connection = self.connect()?;
        let (execution_preferences_json, custom_gpt_id): (String, Option<String>) = connection
            .query_row(
                "SELECT execution_preferences_json, custom_gpt_id
             FROM conversations WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        let execution_preferences = serde_json::from_str(&execution_preferences_json)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let mut statement = connection.prepare(
            "SELECT m.id, m.role, m.status, m.sequence_no,
                    m.broker_task_id, bt.remote_status, bt.local_state,
                    mp.content_text, mp.content_json, m.created_at,
                    json_extract(bt.result_json, '$.model_used.provider'),
                    json_extract(bt.result_json, '$.model_used.deployment'),
                    json_extract(bt.result_json, '$.model_used.model'),
                    CASE
                        WHEN bt.terminal_at IS NULL THEN NULL
                        ELSE CAST(ROUND(
                            MAX(
                                0,
                                (julianday(bt.terminal_at) - julianday(m.created_at))
                                    * 86400000.0
                            )
                        ) AS INTEGER)
                    END,
                    json_extract(bt.result_json, '$.usage'),
                    json_extract(bt.result_json, '$.fallback_used'),
                    json_extract(bt.result_json, '$.long_context'),
                    json_extract(bt.result_json, '$.consensus.synthesized'),
                    json_extract(bt.result_json, '$.consensus.warnings'),
                    json_extract(bt.result_json, '$.arbiter_failures')
             FROM messages m
             LEFT JOIN message_parts mp ON mp.message_id = m.id AND mp.ordinal = 0
             LEFT JOIN broker_tasks bt ON bt.id = m.broker_task_id
             WHERE m.conversation_id = ?1
             ORDER BY m.sequence_no",
        )?;
        let messages = statement
            .query_map(params![id], |row| {
                let error_json: Option<String> = row.get(8)?;
                let model_provider: Option<String> = row.get(10)?;
                let model_deployment: Option<String> = row.get(11)?;
                let model_name: Option<String> = row.get(12)?;
                let usage_json: Option<String> = row.get(14)?;
                let fallback_used: Option<i64> = row.get(15)?;
                let long_context_json: Option<String> = row.get(16)?;
                let consensus_synthesized: Option<i64> = row.get(17)?;
                let consensus_warnings_json: Option<String> = row.get(18)?;
                let arbiter_failures_json: Option<String> = row.get(19)?;
                let consensus_warnings = consensus_warnings_json
                    .and_then(|value| serde_json::from_str::<Value>(&value).ok())
                    .and_then(|value| value.as_array().cloned())
                    .unwrap_or_default()
                    .into_iter()
                    .filter_map(|warning| warning.as_str().map(str::to_owned))
                    .collect::<Vec<_>>();
                let arbiter_failure_count = arbiter_failures_json
                    .and_then(|value| serde_json::from_str::<Value>(&value).ok())
                    .and_then(|value| value.as_array().map(|failures| failures.len() as i64))
                    .unwrap_or(0);
                Ok(ConversationMessage {
                    id: row.get(0)?,
                    role: row.get(1)?,
                    status: row.get(2)?,
                    sequence_no: row.get(3)?,
                    broker_task_id: row.get(4)?,
                    task_remote_status: row.get(5)?,
                    task_local_state: row.get(6)?,
                    text: row.get(7)?,
                    error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                    model_used: match (model_provider, model_deployment, model_name) {
                        (Some(provider), Some(deployment), Some(model)) => Some(ModelUsedView {
                            provider,
                            deployment,
                            model,
                        }),
                        _ => None,
                    },
                    response_duration_ms: row.get(13)?,
                    usage: usage_json.and_then(|value| serde_json::from_str(&value).ok()),
                    fallback_used: fallback_used.map(|value| value != 0),
                    long_context: long_context_json
                        .and_then(|value| serde_json::from_str(&value).ok()),
                    consensus_synthesized: consensus_synthesized.map(|value| value != 0),
                    consensus_warnings,
                    arbiter_failure_count,
                    sources: Vec::new(),
                    created_at: row.get(9)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut source_statement = connection.prepare(
            "SELECT c.message_id, c.id,
                    COALESCE(c.title, a.display_name, 'Fuente'),
                    c.source_attachment_id, a.media_type, a.size_bytes,
                    c.url, c.quote_text, c.claim_text
             FROM citations c
             JOIN messages m ON m.id = c.message_id
             LEFT JOIN attachments a ON a.id = c.source_attachment_id
             WHERE m.conversation_id = ?1
             ORDER BY c.message_id, c.ordinal",
        )?;
        let source_rows = source_statement
            .query_map(params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    ConversationSource {
                        id: row.get(1)?,
                        title: row.get(2)?,
                        source_attachment_id: row.get(3)?,
                        media_type: row.get(4)?,
                        size_bytes: row.get(5)?,
                        url: row.get(6)?,
                        quote_text: row.get(7)?,
                        claim_text: row.get(8)?,
                    },
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut sources_by_message: HashMap<String, Vec<ConversationSource>> = HashMap::new();
        for (message_id, source) in source_rows {
            sources_by_message
                .entry(message_id)
                .or_default()
                .push(source);
        }
        let messages = messages
            .into_iter()
            .map(|mut message| {
                message.sources = sources_by_message.remove(&message.id).unwrap_or_default();
                message
            })
            .collect();
        let mut research_statement = connection.prepare(
            "SELECT run.id, run.broker_task_id, run.objective, run.status,
                    COUNT(citation.id), run.created_at, run.updated_at
             FROM research_runs run
             JOIN broker_tasks task ON task.id = run.broker_task_id
             LEFT JOIN citations citation ON citation.message_id = task.response_message_id
             WHERE run.conversation_id = ?1
             GROUP BY run.id
             ORDER BY run.created_at DESC, run.id DESC",
        )?;
        let research_rows = research_statement
            .query_map(params![id], |row| {
                Ok((
                    row.get::<_, String>(0)?,
                    row.get::<_, String>(1)?,
                    row.get::<_, String>(2)?,
                    row.get::<_, String>(3)?,
                    row.get::<_, i64>(4)?,
                    row.get::<_, String>(5)?,
                    row.get::<_, String>(6)?,
                ))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        let mut research_runs = Vec::with_capacity(research_rows.len());
        for (run_id, broker_task_id, objective, status, source_count, created_at, updated_at) in
            research_rows
        {
            let mut step_statement = connection.prepare(
                "SELECT id, COALESCE(kind, 'research'),
                        COALESCE(title, objective), status
                 FROM research_steps
                 WHERE research_run_id = ?1
                 ORDER BY ordinal",
            )?;
            let steps = step_statement
                .query_map(params![run_id], |row| {
                    Ok(ResearchStepView {
                        id: row.get(0)?,
                        kind: row.get(1)?,
                        title: row.get(2)?,
                        status: row.get(3)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
            research_runs.push(ResearchRunView {
                id: run_id,
                broker_task_id,
                objective,
                status,
                steps,
                source_count,
                created_at,
                updated_at,
            });
        }
        Ok(ConversationView {
            id: summary.id,
            title: summary.title,
            project_id: summary.project_id,
            custom_gpt_id,
            execution_preferences,
            messages,
            research_runs,
        })
    }

    pub fn conversation_export_metadata(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationExportMetadata, AppError> {
        self.connect()?
            .query_row(
                "SELECT c.created_at, c.updated_at, p.id, p.name
                 FROM conversations c
                 LEFT JOIN projects p ON p.id = c.project_id
                 WHERE c.id = ?1 AND c.deleted_at IS NULL",
                params![conversation_id],
                |row| {
                    let project_id: Option<String> = row.get(2)?;
                    let project_name: Option<String> = row.get(3)?;
                    Ok(ConversationExportMetadata {
                        created_at: row.get(0)?,
                        updated_at: row.get(1)?,
                        project: project_id
                            .zip(project_name)
                            .map(|(id, name)| ProjectExportMetadata { id, name }),
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversación {conversation_id}")))
    }

    pub fn conversation_execution_preferences(
        &self,
        conversation_id: &str,
    ) -> Result<ConversationExecutionPreferences, AppError> {
        let connection = self.connect()?;
        let value: String = connection
            .query_row(
                "SELECT execution_preferences_json
                 FROM conversations
                 WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
                params![conversation_id],
                |row| row.get(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound(format!("conversación activa {conversation_id}")))?;
        serde_json::from_str(&value).map_err(|error| AppError::BrokerContract(error.to_string()))
    }

    pub fn update_conversation_execution_preferences(
        &self,
        conversation_id: &str,
        preferences: &ConversationExecutionPreferences,
    ) -> Result<ConversationExecutionPreferences, AppError> {
        validate_execution_preferences(preferences)?;
        let value = serde_json::to_string(preferences)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let changed = transaction.execute(
            "UPDATE conversations
             SET execution_preferences_json = ?2, updated_at = datetime('now')
             WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL",
            params![conversation_id, value],
        )?;
        if changed == 0 {
            return Err(AppError::NotFound(format!(
                "conversación activa {conversation_id}"
            )));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES (
                'conversation.execution_preferences_updated',
                'user',
                ?1,
                json_object(
                    'data_classification', ?2,
                    'strategy', ?3,
                    'preset', ?4,
                    'max_cost_usd', ?5,
                    'long_context', ?6,
                    'priority', ?7
                )
             )",
            params![
                conversation_id,
                preferences.data_classification,
                preferences.strategy,
                preferences.preset,
                preferences.max_cost_usd,
                preferences.long_context,
                preferences.priority
            ],
        )?;
        transaction.commit()?;
        Ok(preferences.clone())
    }

    pub fn task_record(&self, id: &str) -> Result<BrokerTaskRecord, AppError> {
        let connection = self.connect()?;
        connection
            .query_row(
                "SELECT id, remote_task_id, request_json, consecutive_poll_errors
                 FROM broker_tasks WHERE id = ?1",
                params![id],
                |row| {
                    let request_json: String = row.get(2)?;
                    let request = serde_json::from_str(&request_json).map_err(|error| {
                        rusqlite::Error::FromSqlConversionFailure(
                            request_json.len(),
                            rusqlite::types::Type::Text,
                            Box::new(error),
                        )
                    })?;
                    Ok(BrokerTaskRecord {
                        id: row.get(0)?,
                        remote_task_id: row.get(1)?,
                        request,
                        consecutive_poll_errors: row.get(3)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::BrokerContract(format!("tarea local no encontrada: {id}")))
    }

    pub fn recoverable_tasks(&self) -> Result<Vec<BrokerTaskRecord>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id FROM broker_tasks
             WHERE local_state IN (
                'created', 'submitting', 'polling', 'recovery_pending'
             )
             ORDER BY created_at",
        )?;
        let ids = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        ids.into_iter().map(|id| self.task_record(&id)).collect()
    }

    pub fn mark_submitting(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE broker_tasks
             SET local_state = 'submitting', attempt = attempt + 1,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        Ok(())
    }

    pub fn attach_remote_task(&self, id: &str, accepted: &TaskAccepted) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE broker_tasks
             SET remote_task_id = ?2, remote_status = ?3, local_state = 'polling',
                 consecutive_poll_errors = 0, next_poll_at = datetime('now'),
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, accepted.task_id, accepted.status.as_str()],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'remote.accepted', ?2, ?3, datetime('now'))",
            params![
                id,
                accepted.status.as_str(),
                serde_json::to_string(accepted)
                    .map_err(|error| AppError::BrokerContract(error.to_string()))?
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn record_remote_state(&self, id: &str, state: &TaskState) -> Result<(), AppError> {
        let connection = self.connect()?;
        let (previous, request_message_id, response_message_id, conversation_id, request): (
            String,
            Option<String>,
            Option<String>,
            Option<String>,
            Value,
        ) = connection.query_row(
            "SELECT remote_status, request_message_id, response_message_id, conversation_id,
                    request_json
             FROM broker_tasks WHERE id = ?1",
            params![id],
            |row| {
                let request_json: String = row.get(4)?;
                let request = serde_json::from_str(&request_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        request_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                Ok((row.get(0)?, row.get(1)?, row.get(2)?, row.get(3)?, request))
            },
        )?;
        let local_state = if state.status.is_terminal() {
            "terminal"
        } else if state.status.as_str() == "waiting_for_tools" {
            "waiting_for_tools"
        } else {
            "polling"
        };
        let result_json = state
            .result
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let error_json = state
            .error
            .as_ref()
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let progress_json = serde_json::to_string(&state.progress)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let payload_json = serde_json::to_string(state)
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE broker_tasks
             SET remote_status = ?2, local_state = ?3,
                 consecutive_poll_errors = 0, result_json = ?4, error_json = ?5,
                 progress_json = ?6,
                 terminal_at = CASE
                    WHEN ?3 = 'terminal'
                    THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    ELSE NULL
                 END,
                 next_poll_at = CASE WHEN ?3 = 'polling' THEN datetime('now') ELSE NULL END,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![
                id,
                state.status.as_str(),
                local_state,
                result_json,
                error_json,
                progress_json
            ],
        )?;
        let research_phase = state
            .progress
            .get("phase")
            .and_then(Value::as_str)
            .unwrap_or_else(|| state.status.as_str());
        let research_run_exists: bool = transaction.query_row(
            "SELECT EXISTS(SELECT 1 FROM research_runs WHERE broker_task_id = ?1)",
            params![id],
            |row| row.get(0),
        )?;
        if research_run_exists {
            let (run_status, _active_ordinal) = if state.status.is_terminal() {
                (
                    match state.status.as_str() {
                        "completed" => "completed",
                        "cancelled" => "cancelled",
                        _ => "failed",
                    },
                    None,
                )
            } else if matches!(research_phase, "synthesizing" | "verifying") {
                ("synthesizing", Some(2_i64))
            } else if matches!(
                research_phase,
                "queued" | "routing" | "planning" | "resource_planning"
            ) {
                ("planning", Some(0_i64))
            } else {
                ("researching", Some(1_i64))
            };
            transaction.execute(
                "UPDATE research_runs
                 SET status = ?2,
                     updated_at = datetime('now'),
                     completed_at = CASE WHEN ?3 = 1 THEN datetime('now') ELSE NULL END
                 WHERE broker_task_id = ?1",
                params![id, run_status, state.status.is_terminal()],
            )?;
            if state.status.is_terminal() {
                let step_status = match state.status.as_str() {
                    "completed" => "completed",
                    "cancelled" => "cancelled",
                    _ => "failed",
                };
                transaction.execute(
                    "UPDATE research_steps
                     SET status = CASE
                           -- Un paso que ya terminó conserva su desenlace: que
                           -- la investigación acabe bien no convierte en buena
                           -- una fuente que no se pudo abrir. Solo se cierran
                           -- los pasos que se quedaron a medias.
                           WHEN status IN ('completed', 'failed', 'cancelled') THEN status
                           ELSE ?2
                         END,
                         started_at = COALESCE(started_at, datetime('now')),
                         completed_at = COALESCE(completed_at, datetime('now'))
                     WHERE research_run_id = (
                       SELECT id FROM research_runs WHERE broker_task_id = ?1
                     )",
                    params![id, step_status],
                )?;
            }
            // Sin proyección por ordinal: antes se derivaba el estado de cada
            // etapa fija de la fase remota, porque las etapas eran una
            // plantilla. Los pasos reales llevan su propio estado, el de la
            // herramienta que se ejecutó, y sobrescribirlo desde la fase de la
            // tarea daría por «pendiente» una fuente ya abierta.
        }
        if previous != state.status.as_str() {
            transaction.execute(
                "INSERT INTO broker_task_events(
                    broker_task_id, event_type, remote_status, payload_json, occurred_at
                 ) VALUES (?1, 'remote.status_changed', ?2, ?3, datetime('now'))",
                params![id, state.status.as_str(), payload_json],
            )?;
        }
        let request_metadata = request
            .get("content")
            .and_then(|content| content.get("metadata"));
        let request_source_type = request_metadata
            .and_then(|value| value.get("source_type"))
            .and_then(Value::as_str);
        let request_source_id = request_metadata
            .and_then(|value| value.get("source_id"))
            .and_then(Value::as_str);
        if previous != state.status.as_str()
            && state.status.is_terminal()
            && request_source_type == Some("conversation_summary")
        {
            if let Some(summary_id) = request_source_id {
                if state.status.as_str() == "completed" {
                    let markdown = state
                        .result
                        .as_ref()
                        .and_then(assistant_result_text)
                        .map(str::trim)
                        .filter(|value| !value.is_empty())
                        .ok_or_else(|| {
                            AppError::BrokerContract(
                                "el resumen completado no incluye contenido Markdown".to_owned(),
                            )
                        })?;
                    transaction.execute(
                        "UPDATE conversation_summaries
                         SET status = 'draft', draft_text = ?2,
                             updated_at = datetime('now')
                         WHERE id = ?1 AND status = 'generating'",
                        params![summary_id, markdown],
                    )?;
                    transaction.execute(
                        "INSERT INTO audit_events(
                            event_type, actor, conversation_id, payload_json
                         ) SELECT 'summary.draft_ready', 'broker', conversation_id, ?2
                           FROM conversation_summaries WHERE id = ?1",
                        params![
                            summary_id,
                            serde_json::json!({"summary_id": summary_id}).to_string()
                        ],
                    )?;
                } else {
                    transaction.execute(
                        "UPDATE conversation_summaries
                         SET status = ?2, updated_at = datetime('now')
                         WHERE id = ?1 AND status = 'generating'",
                        params![
                            summary_id,
                            if state.status.as_str() == "cancelled" {
                                "cancelled"
                            } else {
                                "failed"
                            }
                        ],
                    )?;
                }
            }
        }
        if state.status.as_str() == "completed"
            && request.get("inference_kind").and_then(Value::as_str) == Some("embedding")
        {
            let metadata = request
                .get("content")
                .and_then(|content| content.get("metadata"));
            let source_type = metadata
                .and_then(|value| value.get("source_type"))
                .and_then(Value::as_str);
            let source_id = metadata
                .and_then(|value| value.get("source_id"))
                .and_then(Value::as_str);
            let content_sha256 = metadata
                .and_then(|value| value.get("content_sha256"))
                .and_then(Value::as_str);
            let vector = state
                .result
                .as_ref()
                .and_then(|result| result.get("embedding"))
                .and_then(Value::as_array);
            if let (Some(source_type), Some(source_id), Some(content_sha256), Some(vector)) =
                (source_type, source_id, content_sha256, vector)
            {
                let values = vector
                    .iter()
                    .map(|value| {
                        value.as_f64().ok_or_else(|| {
                            AppError::BrokerContract(
                                "el embedding contiene un valor no numérico".to_owned(),
                            )
                        })
                    })
                    .collect::<Result<Vec<_>, _>>()?;
                if values.is_empty() {
                    return Err(AppError::BrokerContract(
                        "el embedding completado está vacío".to_owned(),
                    ));
                }
                let mut vector_blob = Vec::with_capacity(values.len() * 8);
                for value in &values {
                    vector_blob.extend_from_slice(&value.to_le_bytes());
                }
                let model_used = state
                    .result
                    .as_ref()
                    .and_then(|result| result.get("model_used"));
                let model = model_used
                    .map(|model| {
                        format!(
                            "{}/{}/{}",
                            model
                                .get("provider")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown"),
                            model
                                .get("deployment")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown"),
                            model
                                .get("model")
                                .and_then(Value::as_str)
                                .unwrap_or("unknown")
                        )
                    })
                    .unwrap_or_else(|| "unknown/unknown/unknown".to_owned());
                let source_is_current = if source_type == "memory" {
                    transaction
                        .query_row(
                            "SELECT content FROM memory_items WHERE id = ?1",
                            params![source_id],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()?
                        .is_some_and(|content| {
                            format!("{:x}", Sha256::digest(content.as_bytes())) == content_sha256
                        })
                } else if source_type == "attachment_chunk" {
                    transaction
                        .query_row(
                            "SELECT content_sha256 FROM attachment_chunks WHERE id = ?1",
                            params![source_id],
                            |row| row.get::<_, String>(0),
                        )
                        .optional()?
                        .is_some_and(|current_sha256| current_sha256 == content_sha256)
                } else {
                    true
                };
                if source_is_current {
                    transaction.execute(
                        "INSERT INTO embedding_records(
                        id, source_type, source_id, chunk_index, model,
                        dimensions, vector_blob, content_sha256
                     ) VALUES (?1, ?2, ?3, 0, ?4, ?5, ?6, ?7)
                     ON CONFLICT(source_type, source_id, chunk_index, model) DO UPDATE SET
                        dimensions = excluded.dimensions,
                        vector_blob = excluded.vector_blob,
                        content_sha256 = excluded.content_sha256,
                        created_at = datetime('now')",
                        params![
                            format!("embedding_{}", Uuid::new_v4().simple()),
                            source_type,
                            source_id,
                            model,
                            values.len() as i64,
                            vector_blob,
                            content_sha256
                        ],
                    )?;
                }
            }
        }
        if state.status.as_str() == "waiting_for_tools" {
            let pending = state
                .result
                .as_ref()
                .and_then(|result| result.get("pending_tool_calls"))
                .and_then(Value::as_array)
                .ok_or_else(|| {
                    AppError::BrokerContract(
                        "waiting_for_tools no incluye pending_tool_calls".to_owned(),
                    )
                })?;
            for call in pending {
                let remote_tool_call_id =
                    call.get("id").and_then(Value::as_str).ok_or_else(|| {
                        AppError::BrokerContract(
                            "una llamada de herramienta no incluye id".to_owned(),
                        )
                    })?;
                let tool_name = call.get("name").and_then(Value::as_str).ok_or_else(|| {
                    AppError::BrokerContract(
                        "una llamada de herramienta no incluye name".to_owned(),
                    )
                })?;
                let arguments = call
                    .get("arguments")
                    .cloned()
                    .unwrap_or_else(|| serde_json::json!({}));
                transaction.execute(
                    "INSERT INTO tool_calls(
                        id, broker_task_id, remote_tool_call_id, tool_name,
                        arguments_json, status
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 'confirmation_required')
                     ON CONFLICT(broker_task_id, remote_tool_call_id) DO UPDATE SET
                        tool_name = excluded.tool_name,
                        arguments_json = excluded.arguments_json,
                        status = CASE
                            WHEN tool_calls.status IN ('requested', 'confirmation_required')
                            THEN 'confirmation_required'
                            ELSE tool_calls.status
                        END",
                    params![
                        format!("toolcall_{}", Uuid::new_v4().simple()),
                        id,
                        remote_tool_call_id,
                        tool_name,
                        arguments.to_string()
                    ],
                )?;
                // El expediente de confirmación nace junto a la llamada, antes de
                // que nadie pueda decidir: así queda constancia de qué se propuso
                // aunque la persona cierre la aplicación sin responder.
                let local_call_id: String = transaction.query_row(
                    "SELECT id FROM tool_calls
                     WHERE broker_task_id = ?1 AND remote_tool_call_id = ?2",
                    params![id, remote_tool_call_id],
                    |row| row.get(0),
                )?;
                let already_recorded: Option<String> = transaction
                    .query_row(
                        "SELECT id FROM confirmation_requests WHERE tool_call_id = ?1",
                        params![local_call_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                if already_recorded.is_none() {
                    let conversation_id: Option<String> = transaction.query_row(
                        "SELECT conversation_id FROM broker_tasks WHERE id = ?1",
                        params![id],
                        |row| row.get(0),
                    )?;
                    let (action_type, resources, disclosure, consequences) =
                        confirmation_blueprint(tool_name, &arguments, conversation_id.as_deref());
                    transaction.execute(
                        "INSERT INTO confirmation_requests(
                            id, action_type, tool_name, resources_json, disclosure_json,
                            consequences, status, tool_call_id, conversation_id
                         ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, 'pending', ?7, ?8)",
                        params![
                            format!("confirm_{}", Uuid::new_v4().simple()),
                            action_type,
                            tool_name,
                            resources.to_string(),
                            disclosure.to_string(),
                            consequences,
                            local_call_id,
                            conversation_id
                        ],
                    )?;
                }
            }
        }
        if previous != state.status.as_str() && state.status.is_terminal() {
            if let Some(message_id) = response_message_id {
                let (message_status, kind, content_text, content_json) =
                    if state.status.as_str() == "completed" {
                        let markdown = state
                            .result
                            .as_ref()
                            .and_then(assistant_result_text)
                            .unwrap_or("La tarea terminó sin contenido Markdown.")
                            .to_owned();
                        ("complete", "markdown", Some(markdown), None)
                    } else {
                        (
                            if state.status.as_str() == "cancelled" {
                                "cancelled"
                            } else {
                                "failed"
                            },
                            "error",
                            None,
                            Some(
                                state
                                    .error
                                    .clone()
                                    .unwrap_or_else(
                                        || serde_json::json!({"status": state.status.as_str()}),
                                    )
                                    .to_string(),
                            ),
                        )
                    };
                transaction.execute(
                    "UPDATE messages SET status = ?2, updated_at = datetime('now')
                     WHERE id = ?1",
                    params![message_id, message_status],
                )?;
                transaction.execute(
                    "INSERT INTO message_parts(
                        id, message_id, ordinal, kind, content_text, content_json
                     ) VALUES (?1, ?2, 0, ?3, ?4, ?5)
                     ON CONFLICT(message_id, ordinal) DO UPDATE SET
                        kind = excluded.kind,
                        content_text = excluded.content_text,
                        content_json = excluded.content_json",
                    params![
                        format!("part_{}", Uuid::new_v4().simple()),
                        message_id,
                        kind,
                        content_text,
                        content_json
                    ],
                )?;
                if state.status.as_str() == "completed" {
                    if let Some(request_message_id) = request_message_id.as_deref() {
                        let sources = {
                            let mut statement = transaction.prepare(
                                "SELECT a.id, a.display_name, a.broker_file_id,
                                        a.media_type, a.size_bytes, ma.ordinal
                                 FROM message_attachments ma
                                 JOIN attachments a ON a.id = ma.attachment_id
                                 WHERE ma.message_id = ?1
                                 ORDER BY ma.ordinal",
                            )?;
                            let rows = statement
                                .query_map(params![request_message_id], |row| {
                                    Ok((
                                        row.get::<_, String>(0)?,
                                        row.get::<_, String>(1)?,
                                        row.get::<_, Option<String>>(2)?,
                                        row.get::<_, Option<String>>(3)?,
                                        row.get::<_, Option<i64>>(4)?,
                                        row.get::<_, i64>(5)?,
                                    ))
                                })?
                                .collect::<Result<Vec<_>, _>>()?;
                            rows
                        };
                        for (
                            attachment_id,
                            title,
                            broker_file_id,
                            media_type,
                            size_bytes,
                            ordinal,
                        ) in sources
                        {
                            let metadata = serde_json::json!({
                                "kind": "broker_file",
                                "broker_file_id": broker_file_id,
                                "media_type": media_type,
                                "size_bytes": size_bytes,
                                "attribution": "turn_attachment"
                            });
                            transaction.execute(
                                "INSERT INTO citations(
                                    id, message_id, ordinal, title,
                                    source_attachment_id, metadata_json
                                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)
                                 ON CONFLICT(message_id, ordinal) DO UPDATE SET
                                    title = excluded.title,
                                    source_attachment_id = excluded.source_attachment_id,
                                    metadata_json = excluded.metadata_json",
                                params![
                                    format!("citation_{}", Uuid::new_v4().simple()),
                                    message_id,
                                    ordinal,
                                    title,
                                    attachment_id,
                                    metadata.to_string()
                                ],
                            )?;
                        }
                    }
                    if request_metadata
                        .and_then(|metadata| metadata.get("workflow_kind"))
                        .and_then(Value::as_str)
                        == Some("deep_research")
                    {
                        let markdown = state
                            .result
                            .as_ref()
                            .and_then(assistant_result_text)
                            .unwrap_or_default();
                        let first_ordinal: i64 = transaction.query_row(
                            "SELECT COALESCE(MAX(ordinal), -1) + 1
                             FROM citations WHERE message_id = ?1",
                            params![message_id],
                            |row| row.get(0),
                        )?;
                        for (offset, (title, url)) in
                            markdown_web_sources(markdown).into_iter().enumerate()
                        {
                            transaction.execute(
                                "INSERT INTO citations(
                                    id, message_id, ordinal, title, url, metadata_json
                                 ) VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
                                params![
                                    format!("citation_{}", Uuid::new_v4().simple()),
                                    message_id,
                                    first_ordinal + offset as i64,
                                    title,
                                    url,
                                    serde_json::json!({
                                        "kind": "web",
                                        "attribution": "deep_research_markdown"
                                    })
                                    .to_string()
                                ],
                            )?;
                        }
                    }
                }
                if let Some(conversation_id) = conversation_id {
                    transaction.execute(
                        "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                        params![conversation_id],
                    )?;
                }
            }
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn record_transport_error(&self, id: &str, message: &str) -> Result<(), AppError> {
        let payload = serde_json::json!({"message": message});
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE broker_tasks
             SET consecutive_poll_errors = consecutive_poll_errors + 1,
                 next_poll_at = datetime('now', '+' ||
                    min(60, (consecutive_poll_errors + 1) * 2) || ' seconds'),
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) SELECT id, 'transport.error', remote_status, ?2, datetime('now')
               FROM broker_tasks WHERE id = ?1",
            params![id, payload.to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn mark_orphaned(&self, id: &str, message: &str) -> Result<(), AppError> {
        let payload = serde_json::json!({"message": message});
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        let (response_message_id, conversation_id): (Option<String>, Option<String>) = transaction
            .query_row(
                "SELECT response_message_id, conversation_id
                 FROM broker_tasks WHERE id = ?1",
                params![id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )?;
        transaction.execute(
            "UPDATE broker_tasks
             SET local_state = 'orphaned', error_json = ?2, next_poll_at = NULL,
                 updated_at = datetime('now')
             WHERE id = ?1",
            params![id, payload.to_string()],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) SELECT id, 'local.orphaned', remote_status, ?2, datetime('now')
               FROM broker_tasks WHERE id = ?1",
            params![id, payload.to_string()],
        )?;
        if let Some(message_id) = response_message_id {
            transaction.execute(
                "UPDATE messages
                 SET status = 'failed', updated_at = datetime('now')
                 WHERE id = ?1",
                params![message_id],
            )?;
            transaction.execute(
                "INSERT INTO message_parts(
                    id, message_id, ordinal, kind, content_json
                 ) VALUES (?1, ?2, 0, 'error', ?3)
                 ON CONFLICT(message_id, ordinal) DO UPDATE SET
                    kind = excluded.kind,
                    content_text = NULL,
                    content_json = excluded.content_json",
                params![
                    format!("part_{}", Uuid::new_v4().simple()),
                    message_id,
                    payload.to_string()
                ],
            )?;
        }
        if let Some(conversation_id) = conversation_id {
            transaction.execute(
                "UPDATE conversations SET updated_at = datetime('now') WHERE id = ?1",
                params![conversation_id],
            )?;
        }
        transaction.commit()?;
        Ok(())
    }

    pub fn pending_tool_calls(&self, local_task_id: &str) -> Result<Vec<ToolCallView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT call.remote_tool_call_id, call.tool_name, call.arguments_json, call.status,
                    request.id, request.action_type, request.tool_name, request.resources_json,
                    request.disclosure_json, request.consequences, request.status,
                    request.requested_at, request.resolved_at
             FROM tool_calls call
             LEFT JOIN confirmation_requests request ON request.tool_call_id = call.id
             WHERE call.broker_task_id = ?1 AND call.status = 'confirmation_required'
             ORDER BY call.requested_at, call.id",
        )?;
        let calls = statement
            .query_map(params![local_task_id], |row| {
                let arguments_json: String = row.get(2)?;
                let arguments = serde_json::from_str(&arguments_json).map_err(|error| {
                    rusqlite::Error::FromSqlConversionFailure(
                        arguments_json.len(),
                        rusqlite::types::Type::Text,
                        Box::new(error),
                    )
                })?;
                let confirmation_id: Option<String> = row.get(4)?;
                let confirmation = match confirmation_id {
                    Some(id) => {
                        let resources_json: String = row.get(7)?;
                        let disclosure_json: String = row.get(8)?;
                        Some(ConfirmationRequestView {
                            id,
                            action_type: row.get(5)?,
                            tool_name: row.get(6)?,
                            resources: serde_json::from_str(&resources_json).unwrap_or(Value::Null),
                            disclosure: serde_json::from_str(&disclosure_json)
                                .unwrap_or(Value::Null),
                            consequences: row.get(9)?,
                            status: row.get(10)?,
                            requested_at: row.get(11)?,
                            resolved_at: row.get(12)?,
                        })
                    }
                    None => None,
                };
                Ok(ToolCallView {
                    tool_call_id: row.get(0)?,
                    name: row.get(1)?,
                    arguments,
                    status: row.get(3)?,
                    confirmation,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(calls)
    }

    pub fn task_conversation_id(&self, local_task_id: &str) -> Result<String, AppError> {
        self.connect()?
            .query_row(
                "SELECT conversation_id FROM broker_tasks WHERE id = ?1",
                params![local_task_id],
                |row| row.get::<_, Option<String>>(0),
            )?
            .ok_or_else(|| AppError::BrokerContract("la tarea no pertenece a un chat".to_owned()))
    }

    pub fn prepare_tool_outcomes(
        &self,
        local_task_id: &str,
        outcomes: &[ToolOutcomeRecord],
    ) -> Result<(), AppError> {
        let expected = self.pending_tool_calls(local_task_id)?;
        let expected_ids: HashSet<&str> = expected
            .iter()
            .map(|call| call.tool_call_id.as_str())
            .collect();
        let provided_ids: HashSet<&str> = outcomes
            .iter()
            .map(|outcome| outcome.tool_call_id.as_str())
            .collect();
        if expected_ids != provided_ids || outcomes.len() != provided_ids.len() {
            return Err(AppError::Validation(
                "debe decidirse exactamente una vez sobre cada herramienta pendiente".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        for outcome in outcomes {
            if !matches!(outcome.status.as_str(), "approved" | "cancelled") {
                return Err(AppError::Validation(
                    "el resultado local de herramienta no es válido".to_owned(),
                ));
            }
            let local_call_id: String = transaction.query_row(
                "SELECT id FROM tool_calls
                 WHERE broker_task_id = ?1 AND remote_tool_call_id = ?2
                   AND status = 'confirmation_required'",
                params![local_task_id, outcome.tool_call_id],
                |row| row.get(0),
            )?;
            // La confirmación se resuelve en la misma transacción que ejecuta la
            // decisión: sin expediente pendiente no hay ejecución posible, y un
            // segundo intento sobre el mismo expediente se rechaza.
            let confirmation_status = if outcome.status == "approved" {
                "allowed_once"
            } else {
                "cancelled"
            };
            let resolved = transaction.execute(
                "UPDATE confirmation_requests
                 SET status = ?2, resolved_at = datetime('now')
                 WHERE tool_call_id = ?1 AND status = 'pending'",
                params![local_call_id, confirmation_status],
            )?;
            if resolved == 0 {
                let existing: Option<String> = transaction
                    .query_row(
                        "SELECT status FROM confirmation_requests WHERE tool_call_id = ?1",
                        params![local_call_id],
                        |row| row.get(0),
                    )
                    .optional()?;
                match existing {
                    Some(status) => {
                        return Err(AppError::Conflict(format!(
                            "esta confirmación ya se resolvió como {status}; \
                             vuelve a abrir la conversación para ver su estado"
                        )));
                    }
                    // Tarea heredada de un esquema anterior al expediente: se deja
                    // constancia de la decisión en lugar de bloquear la respuesta.
                    None => {
                        let conversation_id: Option<String> = transaction.query_row(
                            "SELECT conversation_id FROM broker_tasks WHERE id = ?1",
                            params![local_task_id],
                            |row| row.get(0),
                        )?;
                        let tool_name: String = transaction.query_row(
                            "SELECT tool_name FROM tool_calls WHERE id = ?1",
                            params![local_call_id],
                            |row| row.get(0),
                        )?;
                        let arguments_json: String = transaction.query_row(
                            "SELECT arguments_json FROM tool_calls WHERE id = ?1",
                            params![local_call_id],
                            |row| row.get(0),
                        )?;
                        let arguments: Value =
                            serde_json::from_str(&arguments_json).unwrap_or(Value::Null);
                        let (action_type, resources, disclosure, consequences) =
                            confirmation_blueprint(
                                &tool_name,
                                &arguments,
                                conversation_id.as_deref(),
                            );
                        transaction.execute(
                            "INSERT INTO confirmation_requests(
                                id, action_type, tool_name, resources_json, disclosure_json,
                                consequences, status, requested_at, resolved_at,
                                tool_call_id, conversation_id
                             ) VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, datetime('now'),
                                       datetime('now'), ?8, ?9)",
                            params![
                                format!("confirm_{}", Uuid::new_v4().simple()),
                                action_type,
                                tool_name,
                                resources.to_string(),
                                disclosure.to_string(),
                                consequences,
                                confirmation_status,
                                local_call_id,
                                conversation_id
                            ],
                        )?;
                    }
                }
            }
            transaction.execute(
                "INSERT INTO audit_events(
                    event_type, actor, conversation_id, broker_task_id, payload_json
                 ) VALUES ('confirmation.resolved', 'user',
                           (SELECT conversation_id FROM broker_tasks WHERE id = ?1), ?1, ?2)",
                params![
                    local_task_id,
                    serde_json::json!({
                        "decision": confirmation_status,
                        "tool_call_id": local_call_id
                    })
                    .to_string()
                ],
            )?;
            transaction.execute(
                "UPDATE tool_calls SET status = ?2 WHERE id = ?1",
                params![local_call_id, outcome.status],
            )?;
            transaction.execute(
                "INSERT INTO tool_results(id, tool_call_id, content_text, is_error)
                 VALUES (?1, ?2, ?3, ?4)
                 ON CONFLICT(tool_call_id) DO UPDATE SET
                    content_text = excluded.content_text,
                    is_error = excluded.is_error",
                params![
                    format!("toolresult_{}", Uuid::new_v4().simple()),
                    local_call_id,
                    outcome.content,
                    i64::from(outcome.status == "cancelled")
                ],
            )?;
        }
        transaction.execute(
            "UPDATE broker_tasks
             SET local_state = 'polling', updated_at = datetime('now')
             WHERE id = ?1",
            params![local_task_id],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'local.tool_decisions_prepared', 'waiting_for_tools', ?2, datetime('now'))",
            params![
                local_task_id,
                serde_json::json!({"count": outcomes.len()}).to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn prepared_tool_results(&self, local_task_id: &str) -> Result<Value, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT tc.remote_tool_call_id, tr.content_text
             FROM tool_calls tc
             JOIN tool_results tr ON tr.tool_call_id = tc.id
             WHERE tc.broker_task_id = ?1
               AND tc.status IN ('approved', 'cancelled')
             ORDER BY tc.requested_at, tc.id",
        )?;
        let results = statement
            .query_map(params![local_task_id], |row| {
                Ok(serde_json::json!({
                    "tool_call_id": row.get::<_, String>(0)?,
                    "content": row.get::<_, Option<String>>(1)?.unwrap_or_default()
                }))
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(serde_json::json!({"tool_results": results}))
    }

    pub fn mark_tool_results_submitted(&self, local_task_id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let transaction = connection.unchecked_transaction()?;
        transaction.execute(
            "UPDATE tool_calls
             SET status = 'completed', completed_at = datetime('now')
             WHERE broker_task_id = ?1 AND status IN ('approved', 'cancelled')",
            params![local_task_id],
        )?;
        transaction.execute(
            "INSERT INTO broker_task_events(
                broker_task_id, event_type, remote_status, payload_json, occurred_at
             ) VALUES (?1, 'remote.tool_results_accepted', 'queued', '{}', datetime('now'))",
            params![local_task_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn last_completed_export_hash(
        &self,
        stable_export_id: &str,
        destination_path: &str,
    ) -> Result<Option<String>, AppError> {
        Ok(self
            .connect()?
            .query_row(
                "SELECT destination_hash_after
                 FROM export_records
                 WHERE stable_export_id = ?1 AND destination_path = ?2
                   AND status = 'completed'",
                params![stable_export_id, destination_path],
                |row| row.get(0),
            )
            .optional()?
            .flatten())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn record_export(
        &self,
        source_id: &str,
        stable_export_id: &str,
        destination_path: &str,
        source_hash: &str,
        destination_hash_before: Option<&str>,
        destination_hash_after: Option<&str>,
        status: &str,
        error: Option<&Value>,
    ) -> Result<(), AppError> {
        let error_json = error
            .map(serde_json::to_string)
            .transpose()
            .map_err(|error| AppError::BrokerContract(error.to_string()))?;
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO export_records(
                id, source_type, source_id, stable_export_id, destination_path,
                source_hash, destination_hash_before, destination_hash_after,
                status, error_json
             ) VALUES (?1, 'conversation', ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
             ON CONFLICT(stable_export_id, destination_path) DO UPDATE SET
                source_id = excluded.source_id,
                source_hash = excluded.source_hash,
                destination_hash_before = excluded.destination_hash_before,
                destination_hash_after = excluded.destination_hash_after,
                status = excluded.status,
                error_json = excluded.error_json,
                updated_at = datetime('now')",
            params![
                format!("export_{}", Uuid::new_v4().simple()),
                source_id,
                stable_export_id,
                destination_path,
                source_hash,
                destination_hash_before,
                destination_hash_after,
                status,
                error_json
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES (?1, 'user', ?2, ?3)",
            params![
                format!("export.{status}"),
                source_id,
                serde_json::json!({
                    "stable_export_id": stable_export_id,
                    "destination_path": destination_path,
                    "source_hash": source_hash
                })
                .to_string()
            ],
        )?;
        Ok(())
    }

    pub fn task_snapshot(&self, id: &str) -> Result<LocalTaskSnapshot, AppError> {
        let connection = self.connect()?;
        let mut snapshot = connection
            .query_row(
                "SELECT id, remote_task_id, remote_status, local_state,
                        consecutive_poll_errors, result_json, error_json, updated_at,
                        progress_json,
                        json_extract(request_json, '$.inference_kind'),
                        json_extract(request_json, '$.content.metadata.source_type')
                 FROM broker_tasks WHERE id = ?1",
                params![id],
                |row| {
                    let result_json: Option<String> = row.get(6)?;
                    let error_json: Option<String> = row.get(6)?;
                    let progress_json: String = row.get(8)?;
                    let progress_value: Value =
                        serde_json::from_str(&progress_json).unwrap_or(Value::Null);
                    let inference_kind: Option<String> = row.get(9)?;
                    let source_type: Option<String> = row.get(10)?;
                    let activity = match (inference_kind.as_deref(), source_type.as_deref()) {
                        (Some("chat"), Some("conversation_summary")) => {
                            "Preparando borrador del resumen"
                        }
                        (Some("embedding"), Some("chat_memory_search")) => {
                            "Buscando contexto relacionado"
                        }
                        (Some("embedding"), Some("chat_document_search")) => {
                            "Buscando fragmentos relacionados"
                        }
                        (Some("embedding"), Some("memory_search")) => "Buscando en la memoria",
                        (Some("embedding"), Some("attachment_chunk")) => {
                            "Preparando el índice documental"
                        }
                        (Some("embedding"), _) => "Preparando el índice de memoria",
                        (Some("chat"), _) | (Some("agent"), _) => "Generando respuesta",
                        _ => "Procesando tarea",
                    };
                    Ok(LocalTaskSnapshot {
                        id: row.get(0)?,
                        activity: activity.to_owned(),
                        remote_task_id: row.get(1)?,
                        remote_status: row.get(2)?,
                        local_state: row.get(3)?,
                        consecutive_poll_errors: row.get(4)?,
                        result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                        error: error_json.and_then(|value| serde_json::from_str(&value).ok()),
                        progress: TaskProgressView {
                            phase: progress_value
                                .get("phase")
                                .and_then(Value::as_str)
                                .map(str::to_owned),
                            invocations_completed: progress_value
                                .get("invocations_completed")
                                .and_then(Value::as_i64),
                            invocations_total: progress_value
                                .get("invocations_total")
                                .and_then(Value::as_i64),
                        },
                        pending_tool_calls: Vec::new(),
                        updated_at: row.get(7)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| AppError::BrokerContract(format!("tarea local no encontrada: {id}")))?;
        snapshot.pending_tool_calls = self.pending_tool_calls(id)?;
        Ok(snapshot)
    }

    pub fn list_scheduled_task_templates(
        &self,
    ) -> Result<Vec<ScheduledTaskTemplateView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, name, prompt, schedule_expression, created_at, updated_at
             FROM scheduled_task_templates
             ORDER BY datetime(updated_at) DESC, name COLLATE NOCASE",
        )?;
        let templates = statement
            .query_map([], |row| {
                Ok(ScheduledTaskTemplateView {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_expression: row.get(3)?,
                    created_at: row.get(4)?,
                    updated_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(templates)
    }

    pub fn create_scheduled_task_template(
        &self,
        name: &str,
        prompt: &str,
        schedule_expression: &str,
    ) -> Result<ScheduledTaskTemplateView, AppError> {
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia de la plantilla no es válida".to_owned(),
            ));
        }
        let id = format!("scheduled_template_{}", Uuid::new_v4().simple());
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        transaction.execute(
            "INSERT INTO scheduled_task_templates(
                id, name, prompt, schedule_expression, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![id, name, prompt, schedule_expression],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_template.created', 'user', ?1)",
            params![serde_json::json!({
                "scheduled_template_id": id,
                "schedule_expression": schedule_expression
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.list_scheduled_task_templates()?
            .into_iter()
            .find(|template| template.id == id)
            .ok_or_else(|| AppError::NotFound("plantilla programada recién creada".to_owned()))
    }

    /// Registra un lote de duraciones de una misma métrica y poda las antiguas.
    ///
    /// Insertar y podar en la misma transacción es lo que hace que el límite sea
    /// real: no existe un instante en el que la tabla supere las muestras
    /// conservadas, ni una tarea de mantenimiento que pueda no ejecutarse nunca.
    pub fn record_performance_samples(
        &self,
        metric: &str,
        durations_ms: &[i64],
    ) -> Result<i64, AppError> {
        let metric = PerformanceMetric::parse(metric).ok_or_else(|| {
            AppError::Validation("la métrica de rendimiento no es válida".to_owned())
        })?;
        if durations_ms.is_empty() {
            return Ok(0);
        }
        if durations_ms.len() > metrics::MAX_SAMPLES_PER_CALL {
            return Err(AppError::Validation(
                "demasiadas muestras de rendimiento en una sola llamada".to_owned(),
            ));
        }
        if let Some(invalid) = durations_ms
            .iter()
            .find(|duration| !metrics::is_reportable_sample(**duration))
        {
            return Err(AppError::Validation(format!(
                "la duración {invalid} ms está fuera del rango admitido"
            )));
        }

        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        {
            let mut insert = transaction
                .prepare("INSERT INTO performance_samples(metric, duration_ms) VALUES (?1, ?2)")?;
            for duration in durations_ms {
                insert.execute(params![metric.as_str(), duration])?;
            }
        }
        transaction.execute(
            "DELETE FROM performance_samples
             WHERE metric = ?1
               AND id NOT IN (
                   SELECT id FROM performance_samples
                   WHERE metric = ?1
                   ORDER BY id DESC
                   LIMIT ?2
               )",
            params![metric.as_str(), metrics::MAX_SAMPLES_PER_METRIC],
        )?;
        let retained: i64 = transaction.query_row(
            "SELECT COUNT(*) FROM performance_samples WHERE metric = ?1",
            params![metric.as_str()],
            |row| row.get(0),
        )?;
        transaction.commit()?;
        Ok(retained)
    }

    /// Tareas que aquí se dieron por perdidas pero siguen vivas en el Broker.
    ///
    /// Una tarea queda `orphaned` cuando un error permanente impide seguir
    /// atendiéndola —por ejemplo, si el envío de resultados de herramienta es
    /// rechazado por contrato—. La recuperación las excluye a propósito: no
    /// tiene sentido reintentar algo que no puede mejorar repitiéndolo.
    ///
    /// El problema es el otro lado. Si el Broker la dejó pausada esperando una
    /// herramienta, `waiting_for_tools` **no caduca**: seguiría esperando una
    /// respuesta que ChatyGPT ya no va a enviar. Estas son las que hay que
    /// cerrar explícitamente al arrancar.
    pub fn abandoned_remote_tasks(&self) -> Result<Vec<(String, String)>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, remote_task_id FROM broker_tasks
             WHERE local_state = 'orphaned'
               AND remote_task_id IS NOT NULL
               AND remote_status NOT IN ('completed', 'failed', 'cancelled')
             ORDER BY created_at",
        )?;
        let rows = statement
            .query_map([], |row| Ok((row.get(0)?, row.get(1)?)))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(rows)
    }

    /// Deja constancia de que se cerró una tarea abandonada.
    ///
    /// Se audita porque es trabajo del Broker que ChatyGPT decide descartar sin
    /// preguntar: conviene poder responder después a «¿quién canceló esto?».
    pub fn record_abandoned_cancellation(
        &self,
        local_task_id: &str,
        remote_task_id: &str,
        remote_status: &str,
    ) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             SELECT 'task.abandoned_cancelled', 'chatygpt', conversation_id, ?2
             FROM broker_tasks WHERE id = ?1",
            params![
                local_task_id,
                serde_json::json!({
                    "broker_task_id": local_task_id,
                    "remote_task_id": remote_task_id,
                    "remote_status": remote_status
                })
                .to_string()
            ],
        )?;
        Ok(())
    }

    /// Anota una herramienta ejecutada como paso real de la investigación.
    ///
    /// Sustituye a las tres etapas fijas por lo que de verdad ocurrió: cada
    /// llamada que el modelo pidió, con su parámetro visible —la URL—, su
    /// resultado y su marca de tiempo. El `tool_call_id` es la identidad: el
    /// mismo paso no se registra dos veces aunque una recuperación reejecute
    /// la herramienta.
    ///
    /// `kind` es `research` porque el CHECK de la tabla solo admite las tres
    /// clases originales; el detalle real vive en `objective` y `result_json`.
    pub fn record_research_tool_step(
        &self,
        broker_task_id: &str,
        tool_call_id: &str,
        tool_name: &str,
        argument: &str,
        status: &str,
        result: &Value,
    ) -> Result<(), AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let research_run_id: Option<String> = transaction
            .query_row(
                "SELECT id FROM research_runs WHERE broker_task_id = ?1",
                params![broker_task_id],
                |row| row.get(0),
            )
            .optional()?;
        let Some(research_run_id) = research_run_id else {
            return Ok(());
        };
        // La identidad del paso es la llamada, no su posición: reejecutar la
        // herramienta tras un reinicio actualiza el mismo registro.
        let existing: Option<String> = transaction
            .query_row(
                "SELECT id FROM research_steps
                 WHERE research_run_id = ?1
                   AND json_extract(result_json, '$.tool_call_id') = ?2",
                params![research_run_id, tool_call_id],
                |row| row.get(0),
            )
            .optional()?;
        let mut payload = result.clone();
        payload["tool_call_id"] = serde_json::json!(tool_call_id);
        payload["tool"] = serde_json::json!(tool_name);
        let payload_json = payload.to_string();
        match existing {
            Some(step_id) => {
                transaction.execute(
                    "UPDATE research_steps
                     SET status = ?2,
                         result_json = ?3,
                         completed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                         updated_at = datetime('now')
                     WHERE id = ?1",
                    params![step_id, status, payload_json],
                )?;
            }
            None => {
                let next_ordinal: i64 = transaction.query_row(
                    "SELECT COALESCE(MAX(ordinal), -1) + 1 FROM research_steps
                     WHERE research_run_id = ?1",
                    params![research_run_id],
                    |row| row.get(0),
                )?;
                transaction.execute(
                    "INSERT INTO research_steps(
                        id, research_run_id, ordinal, objective, status,
                        broker_task_id, kind, title, started_at, completed_at,
                        result_json
                     ) VALUES (
                        ?1, ?2, ?3, ?4, ?5, ?6, 'research', ?7,
                        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                        strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                        ?8
                     )",
                    params![
                        format!("research_step_{}", Uuid::new_v4().simple()),
                        research_run_id,
                        next_ordinal,
                        argument,
                        status,
                        broker_task_id,
                        format!("{tool_name}: {argument}"),
                        payload_json
                    ],
                )?;
            }
        }
        transaction.commit()?;
        Ok(())
    }

    /// Informe de rendimiento sobre las muestras conservadas.
    pub fn performance_report(&self) -> Result<PerformanceReportView, AppError> {
        let connection = self.connect()?;
        let mut summaries = Vec::with_capacity(PerformanceMetric::ALL.len());
        let mut total = 0_i64;
        for metric in PerformanceMetric::ALL {
            let mut statement = connection.prepare(
                "SELECT duration_ms FROM performance_samples
                 WHERE metric = ?1
                 ORDER BY id DESC
                 LIMIT ?2",
            )?;
            let durations = statement
                .query_map(
                    params![metric.as_str(), metrics::MAX_SAMPLES_PER_METRIC],
                    |row| row.get::<_, i64>(0),
                )?
                .collect::<Result<Vec<_>, _>>()?;
            let last_recorded_at = connection
                .query_row(
                    "SELECT recorded_at FROM performance_samples
                     WHERE metric = ?1
                     ORDER BY id DESC
                     LIMIT 1",
                    params![metric.as_str()],
                    |row| row.get::<_, String>(0),
                )
                .optional()?;
            total += durations.len() as i64;
            summaries.push(metrics::summarize(metric, &durations, last_recorded_at));
        }
        Ok(PerformanceReportView {
            metrics: summaries,
            sample_limit: metrics::MAX_SAMPLES_PER_METRIC,
            total_samples: total,
        })
    }

    /// Borra todas las mediciones. Exige confirmación y queda auditado.
    pub fn clear_performance_samples(&self, confirmed: bool) -> Result<(), AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "vaciar las mediciones requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let removed = transaction.execute("DELETE FROM performance_samples", [])?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('performance.samples_cleared', 'user', ?1)",
            params![serde_json::json!({ "removed_samples": removed }).to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn delete_scheduled_task_template(
        &self,
        id: &str,
        confirmed: bool,
    ) -> Result<(), AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "eliminar una plantilla requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let name = transaction
            .query_row(
                "SELECT name FROM scheduled_task_templates WHERE id = ?1",
                params![id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| AppError::NotFound("plantilla programada".to_owned()))?;
        transaction.execute(
            "DELETE FROM scheduled_task_templates WHERE id = ?1",
            params![id],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_template.deleted', 'user', ?1)",
            params![serde_json::json!({
                "scheduled_template_id": id,
                "name": name
            })
            .to_string()],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn list_scheduled_tasks(&self) -> Result<Vec<ScheduledTaskView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT st.id, st.name,
                    COALESCE(json_extract(st.payload_json, '$.target_kind'), 'conversation'),
                    json_extract(st.payload_json, '$.conversation_id'),
                    c.title,
                    json_extract(st.payload_json, '$.workflow_id'),
                    w.name,
                    version.version_no,
                    json_extract(st.payload_json, '$.prompt'),
                    st.schedule_expression, st.timezone, st.enabled,
                    st.confirmed_at, st.next_run_at, st.created_at, st.updated_at
             FROM scheduled_tasks st
             LEFT JOIN conversations c
               ON c.id = json_extract(st.payload_json, '$.conversation_id')
             LEFT JOIN workflows w
               ON w.id = json_extract(st.payload_json, '$.workflow_id')
             LEFT JOIN workflow_versions version
               ON version.id = json_extract(st.payload_json, '$.workflow_version_id')
             WHERE (
                    COALESCE(json_extract(st.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                    AND c.id IS NOT NULL AND c.deleted_at IS NULL
               ) OR (
                    json_extract(st.payload_json, '$.target_kind') = 'workflow'
                    AND w.id IS NOT NULL AND w.archived_at IS NULL
               )
             ORDER BY st.created_at DESC",
        )?;
        let mut tasks = statement
            .query_map([], |row| {
                Ok(ScheduledTaskView {
                    id: row.get(0)?,
                    name: row.get(1)?,
                    target_kind: row.get(2)?,
                    conversation_id: row.get(3)?,
                    conversation_title: row.get(4)?,
                    workflow_id: row.get(5)?,
                    workflow_name: row.get(6)?,
                    workflow_version_no: row.get(7)?,
                    prompt: row.get(8)?,
                    schedule_expression: row.get(9)?,
                    timezone: row.get(10)?,
                    enabled: row.get::<_, i64>(11)? != 0,
                    confirmed_at: row.get(12)?,
                    next_run_at: row.get(13)?,
                    created_at: row.get(14)?,
                    updated_at: row.get(15)?,
                    runs: Vec::new(),
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        for task in &mut tasks {
            let mut runs = connection.prepare(
                "SELECT id, due_at, status, broker_task_id, workflow_run_id,
                        attempt, result_json,
                        created_at, updated_at
                 FROM scheduled_runs
                 WHERE scheduled_task_id = ?1
                 ORDER BY datetime(created_at) DESC, attempt DESC
                 LIMIT 10",
            )?;
            task.runs = runs
                .query_map(params![task.id], |row| {
                    let result_json: Option<String> = row.get(6)?;
                    Ok(ScheduledRunView {
                        id: row.get(0)?,
                        due_at: row.get(1)?,
                        status: row.get(2)?,
                        broker_task_id: row.get(3)?,
                        workflow_run_id: row.get(4)?,
                        attempt: row.get(5)?,
                        result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                })?
                .collect::<Result<Vec<_>, _>>()?;
        }
        Ok(tasks)
    }

    pub fn scheduled_history_export_rows(
        &self,
        status_filter: &str,
        period_filter: &str,
    ) -> Result<Vec<ScheduledHistoryExportRow>, AppError> {
        if !matches!(
            status_filter,
            "all" | "active" | "completed" | "failed" | "cancelled"
        ) {
            return Err(AppError::Validation(
                "el filtro de estado del historial no es válido".to_owned(),
            ));
        }
        if !matches!(period_filter, "all" | "today" | "7d" | "30d") {
            return Err(AppError::Validation(
                "el filtro de fecha del historial no es válido".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT task.name, COALESCE(conversation.title, workflow.name),
                    json_extract(task.payload_json, '$.prompt'),
                    task.schedule_expression, task.timezone,
                    run.id, run.due_at, run.status, run.attempt, run.result_json,
                    run.created_at, run.updated_at
             FROM scheduled_runs run
             JOIN scheduled_tasks task ON task.id = run.scheduled_task_id
             LEFT JOIN conversations conversation
               ON conversation.id = json_extract(task.payload_json, '$.conversation_id')
             LEFT JOIN workflows workflow
               ON workflow.id = json_extract(task.payload_json, '$.workflow_id')
             WHERE (
                    (
                        COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                        AND conversation.id IS NOT NULL AND conversation.deleted_at IS NULL
                    ) OR (
                        json_extract(task.payload_json, '$.target_kind') = 'workflow'
                        AND workflow.id IS NOT NULL AND workflow.archived_at IS NULL
                    )
               ) AND (
                    ?1 = 'all'
                    OR (?1 = 'active' AND run.status IN ('claimed', 'running'))
                    OR run.status = ?1
               )
               AND (
                    ?2 = 'all'
                    OR (?2 = 'today'
                        AND date(run.updated_at, 'localtime') = date('now', 'localtime'))
                    OR (?2 = '7d'
                        AND datetime(run.updated_at) >= datetime('now', '-7 days'))
                    OR (?2 = '30d'
                        AND datetime(run.updated_at) >= datetime('now', '-30 days'))
               )
             ORDER BY datetime(run.updated_at) DESC, run.attempt DESC",
        )?;
        let rows = statement
            .query_map(params![status_filter, period_filter], |row| {
                let result_json: Option<String> = row.get(9)?;
                Ok(ScheduledHistoryExportRow {
                    task_name: row.get(0)?,
                    conversation_title: row.get(1)?,
                    prompt: row.get(2)?,
                    schedule_expression: row.get(3)?,
                    timezone: row.get(4)?,
                    run_id: row.get(5)?,
                    due_at: row.get(6)?,
                    status: row.get(7)?,
                    attempt: row.get(8)?,
                    result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                    created_at: row.get(10)?,
                    updated_at: row.get(11)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()
            .map_err(AppError::from)?;
        Ok(rows)
    }

    pub fn scheduled_run_page(
        &self,
        scheduled_task_id: &str,
        status_filter: &str,
        period_filter: &str,
        sort: &str,
        page: i64,
        page_size: i64,
    ) -> Result<ScheduledRunPageView, AppError> {
        if !matches!(
            status_filter,
            "all" | "active" | "completed" | "failed" | "cancelled"
        ) {
            return Err(AppError::Validation(
                "el filtro de estado del historial no es válido".to_owned(),
            ));
        }
        if !matches!(period_filter, "all" | "today" | "7d" | "30d") {
            return Err(AppError::Validation(
                "el filtro de fecha del historial no es válido".to_owned(),
            ));
        }
        if !matches!(sort, "newest" | "oldest") {
            return Err(AppError::Validation(
                "la ordenación del historial no es válida".to_owned(),
            ));
        }
        if page < 1 || !matches!(page_size, 10 | 25 | 50) {
            return Err(AppError::Validation(
                "la página o su tamaño no son válidos".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let exists: bool = connection.query_row(
            "SELECT EXISTS(SELECT 1 FROM scheduled_tasks WHERE id = ?1)",
            params![scheduled_task_id],
            |row| row.get(0),
        )?;
        if !exists {
            return Err(AppError::NotFound("tarea programada".to_owned()));
        }
        let filters = "scheduled_task_id = ?1
            AND (
                ?2 = 'all'
                OR (?2 = 'active' AND status IN ('claimed', 'running'))
                OR status = ?2
            )
            AND (
                ?3 = 'all'
                OR (?3 = 'today'
                    AND date(updated_at, 'localtime') = date('now', 'localtime'))
                OR (?3 = '7d'
                    AND datetime(updated_at) >= datetime('now', '-7 days'))
                OR (?3 = '30d'
                    AND datetime(updated_at) >= datetime('now', '-30 days'))
            )";
        let total: i64 = connection.query_row(
            &format!("SELECT COUNT(*) FROM scheduled_runs WHERE {filters}"),
            params![scheduled_task_id, status_filter, period_filter],
            |row| row.get(0),
        )?;
        let maximum_page = std::cmp::max(1, (total + page_size - 1) / page_size);
        let page = std::cmp::min(page, maximum_page);
        let offset = (page - 1) * page_size;
        let direction = if sort == "oldest" { "ASC" } else { "DESC" };
        let query = format!(
            "SELECT id, due_at, status, broker_task_id, workflow_run_id,
                    attempt, result_json,
                    created_at, updated_at
             FROM scheduled_runs
             WHERE {filters}
             ORDER BY datetime(updated_at) {direction}, attempt {direction}, id {direction}
             LIMIT ?4 OFFSET ?5"
        );
        let mut statement = connection.prepare(&query)?;
        let items = statement
            .query_map(
                params![
                    scheduled_task_id,
                    status_filter,
                    period_filter,
                    page_size,
                    offset
                ],
                |row| {
                    let result_json: Option<String> = row.get(6)?;
                    Ok(ScheduledRunView {
                        id: row.get(0)?,
                        due_at: row.get(1)?,
                        status: row.get(2)?,
                        broker_task_id: row.get(3)?,
                        workflow_run_id: row.get(4)?,
                        attempt: row.get(5)?,
                        result: result_json.and_then(|value| serde_json::from_str(&value).ok()),
                        created_at: row.get(7)?,
                        updated_at: row.get(8)?,
                    })
                },
            )?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(ScheduledRunPageView {
            items,
            total,
            page,
            page_size,
            sort: sort.to_owned(),
        })
    }

    /// Autoriza una carpeta para escritura tras una elección humana explícita.
    ///
    /// Reautorizar una carpeta previamente revocada la reactiva y actualiza su
    /// motivo, sin duplicar la fila.
    pub fn authorize_folder(
        &self,
        folder: &Path,
        display_name: &str,
        purpose: &str,
    ) -> Result<(), AppError> {
        let key = folder_key(folder);
        let connection = self.connect()?;
        connection.execute(
            "INSERT INTO authorized_folders(
                id, canonical_path, display_name, permissions_json
             ) VALUES (?1, ?2, ?3, ?4)
             ON CONFLICT(canonical_path) DO UPDATE SET
                display_name = excluded.display_name,
                permissions_json = excluded.permissions_json,
                granted_at = datetime('now'),
                revoked_at = NULL",
            params![
                format!("folder_{}", Uuid::new_v4().simple()),
                key,
                display_name,
                serde_json::json!({"write": true, "purpose": purpose}).to_string()
            ],
        )?;
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_folder.granted', 'user', ?1)",
            params![serde_json::json!({"purpose": purpose}).to_string()],
        )?;
        Ok(())
    }

    pub fn list_authorized_folders(&self) -> Result<Vec<AuthorizedFolderView>, AppError> {
        let connection = self.connect()?;
        let mut statement = connection.prepare(
            "SELECT id, canonical_path, display_name, permissions_json, granted_at, revoked_at
             FROM authorized_folders
             ORDER BY revoked_at IS NOT NULL, granted_at DESC",
        )?;
        let folders = statement
            .query_map([], |row| {
                let permissions_json: String = row.get(3)?;
                Ok(AuthorizedFolderView {
                    id: row.get(0)?,
                    path: row.get(1)?,
                    display_name: row.get(2)?,
                    permissions: serde_json::from_str(&permissions_json).unwrap_or(Value::Null),
                    granted_at: row.get(4)?,
                    revoked_at: row.get(5)?,
                })
            })?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(folders)
    }

    /// Revoca una carpeta: las exportaciones posteriores exigirán volver a
    /// elegirla en el selector nativo.
    pub fn revoke_authorized_folder(&self, id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        let affected = connection.execute(
            "UPDATE authorized_folders SET revoked_at = datetime('now')
             WHERE id = ?1 AND revoked_at IS NULL",
            params![id],
        )?;
        if affected == 0 {
            return Err(AppError::NotFound(
                "la carpeta autorizada no existe o ya estaba revocada".to_owned(),
            ));
        }
        connection.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('authorized_folder.revoked', 'user', '{}')",
            [],
        )?;
        Ok(())
    }

    /// Indica si un destino cae dentro de una carpeta autorizada y vigente.
    ///
    /// Acepta descendientes —la exportación a Obsidian escribe en subcarpetas de
    /// la bóveda— pero nunca una carpeta hermana con nombre parecido.
    pub fn write_is_authorized(&self, destination: &Path) -> Result<bool, AppError> {
        let target = folder_key(destination);
        let connection = self.connect()?;
        let mut statement = connection
            .prepare("SELECT canonical_path FROM authorized_folders WHERE revoked_at IS NULL")?;
        let authorized = statement
            .query_map([], |row| row.get::<_, String>(0))?
            .collect::<Result<Vec<_>, _>>()?;
        Ok(authorized.iter().any(|folder| {
            target == *folder || target.starts_with(&format!("{folder}{MAIN_SEPARATOR}"))
        }))
    }

    pub fn record_scheduled_history_export(
        &self,
        destination_path: &str,
        destination_hash: &str,
        run_count: usize,
        status_filter: &str,
        period_filter: &str,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_history.exported', 'user', ?1)",
            params![serde_json::json!({
                "destination_path": destination_path,
                "destination_hash": destination_hash,
                "run_count": run_count,
                "status_filter": status_filter,
                "period_filter": period_filter
            })
            .to_string()],
        )?;
        Ok(())
    }

    pub fn record_scheduled_calendar_export(
        &self,
        destination_path: &str,
        destination_hash: &str,
        event_count: usize,
        range_days: u8,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_calendar.exported', 'user', ?1)",
            params![serde_json::json!({
                "destination_path": destination_path,
                "destination_hash": destination_hash,
                "event_count": event_count,
                "range_days": range_days
            })
            .to_string()],
        )?;
        Ok(())
    }

    pub fn record_windows_startup_changed(
        &self,
        enabled: bool,
        credential_protected: bool,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('windows_startup.changed', 'user', ?1)",
            params![serde_json::json!({
                "enabled": enabled,
                "credential_protected": credential_protected,
                "scope": "current_user"
            })
            .to_string()],
        )?;
        Ok(())
    }

    /// Deja constancia de que la credencial cambió, nunca de su valor.
    pub fn record_broker_credential_changed(&self, stored: bool) -> Result<(), AppError> {
        self.connect()?.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('broker_credential.changed', 'user', ?1)",
            params![serde_json::json!({
                "stored": stored,
                "protection": "dpapi_current_user"
            })
            .to_string()],
        )?;
        Ok(())
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_scheduled_task(
        &self,
        name: &str,
        conversation_id: &str,
        prompt: &str,
        due_at: &str,
        timezone: &str,
        schedule_expression: &str,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "activar una tarea programada requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let conversation_exists: bool = transaction.query_row(
            "SELECT EXISTS(
                SELECT 1 FROM conversations
                WHERE id = ?1 AND archived_at IS NULL AND deleted_at IS NULL
             )",
            params![conversation_id],
            |row| row.get(0),
        )?;
        if !conversation_exists {
            return Err(AppError::NotFound(
                "la conversación seleccionada ya no está disponible".to_owned(),
            ));
        }
        let valid_due_at: bool = transaction.query_row(
            "SELECT datetime(?1) IS NOT NULL AND datetime(?1) > datetime('now')",
            params![due_at],
            |row| row.get(0),
        )?;
        if !valid_due_at {
            return Err(AppError::Validation(
                "la fecha programada debe estar en el futuro".to_owned(),
            ));
        }
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia programada no es válida".to_owned(),
            ));
        }
        let id = format!("scheduled_{}", Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "conversation_id": conversation_id,
            "prompt": prompt
        });
        transaction.execute(
            "INSERT INTO scheduled_tasks(
                id, name, schedule_expression, timezone, payload_json,
                enabled, confirmed_at, next_run_at, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, 1,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?6,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                id,
                name,
                schedule_expression,
                timezone,
                payload.to_string(),
                due_at
            ],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_task.created', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": id,
                    "due_at": due_at,
                    "timezone": timezone,
                    "schedule_expression": schedule_expression
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("tarea programada recién creada".to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn create_scheduled_workflow(
        &self,
        name: &str,
        workflow_id: &str,
        input_text: &str,
        due_at: &str,
        timezone: &str,
        schedule_expression: &str,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "activar un flujo programado requiere confirmación explícita".to_owned(),
            ));
        }
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia programada no es válida".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let workflow_version_id = transaction
            .query_row(
                "SELECT published_version_id FROM workflows
             WHERE id = ?1 AND archived_at IS NULL AND published_version_id IS NOT NULL",
                params![workflow_id],
                |row| row.get::<_, String>(0),
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict("publica el flujo antes de programarlo".to_owned())
            })?;
        let valid_due_at: bool = transaction.query_row(
            "SELECT datetime(?1) IS NOT NULL AND datetime(?1) > datetime('now')",
            params![due_at],
            |row| row.get(0),
        )?;
        if !valid_due_at {
            return Err(AppError::Validation(
                "la fecha programada debe estar en el futuro".to_owned(),
            ));
        }
        let id = format!("scheduled_{}", Uuid::new_v4().simple());
        let payload = serde_json::json!({
            "target_kind": "workflow",
            "workflow_id": workflow_id,
            "workflow_version_id": workflow_version_id,
            "prompt": input_text
        });
        transaction.execute(
            "INSERT INTO scheduled_tasks(
                id, name, schedule_expression, timezone, payload_json,
                enabled, confirmed_at, next_run_at, created_at, updated_at
             ) VALUES (
                ?1, ?2, ?3, ?4, ?5, 1,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?6,
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             )",
            params![
                id,
                name,
                schedule_expression,
                timezone,
                payload.to_string(),
                due_at
            ],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, payload_json)
             VALUES ('scheduled_workflow.created', 'user', ?1)",
            params![serde_json::json!({
                "scheduled_task_id": id,
                "workflow_id": workflow_id,
                "due_at": due_at,
                "timezone": timezone,
                "schedule_expression": schedule_expression
            })
            .to_string()],
        )?;
        transaction.commit()?;
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("flujo programado recién creado".to_owned()))
    }

    pub fn set_scheduled_task_enabled(
        &self,
        id: &str,
        enabled: bool,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        if enabled && !confirmed {
            return Err(AppError::Validation(
                "reactivar una tarea programada requiere confirmación explícita".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let changed = connection.execute(
            "UPDATE scheduled_tasks
             SET enabled = ?2,
                 confirmed_at = CASE
                    WHEN ?2 = 1 THEN strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
                    ELSE confirmed_at
                 END,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND (schedule_expression != 'once' OR last_claim_key IS NULL)
               AND next_run_at IS NOT NULL",
            params![id, i64::from(enabled)],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "la tarea ya se ejecutó o dejó de estar disponible".to_owned(),
            ));
        }
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("tarea programada".to_owned()))
    }

    #[allow(clippy::too_many_arguments)]
    pub fn update_scheduled_task(
        &self,
        id: &str,
        name: &str,
        conversation_id: &str,
        prompt: &str,
        due_at: &str,
        timezone: &str,
        schedule_expression: &str,
        confirmed: bool,
    ) -> Result<ScheduledTaskView, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "editar y reactivar una tarea requiere confirmación explícita".to_owned(),
            ));
        }
        if !matches!(schedule_expression, "once" | "daily" | "weekly") {
            return Err(AppError::Validation(
                "la recurrencia programada no es válida".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let valid_due_at: bool = transaction.query_row(
            "SELECT datetime(?1) IS NOT NULL AND datetime(?1) > datetime('now')",
            params![due_at],
            |row| row.get(0),
        )?;
        if !valid_due_at {
            return Err(AppError::Validation(
                "la fecha programada debe estar en el futuro".to_owned(),
            ));
        }
        let payload = serde_json::json!({
            "conversation_id": conversation_id,
            "prompt": prompt
        });
        let changed = transaction.execute(
            "UPDATE scheduled_tasks
             SET name = ?2, payload_json = ?3, next_run_at = ?4, timezone = ?5,
                 schedule_expression = ?6, enabled = 1, last_claim_key = NULL,
                 confirmed_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now'),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND COALESCE(json_extract(payload_json, '$.target_kind'), 'conversation') = 'conversation'
               AND EXISTS(
                    SELECT 1 FROM conversations
                    WHERE id = ?7 AND archived_at IS NULL AND deleted_at IS NULL
               )
               AND NOT EXISTS(
                    SELECT 1 FROM scheduled_runs
                    WHERE scheduled_task_id = ?1 AND status IN ('claimed', 'running')
               )",
            params![
                id,
                name,
                payload.to_string(),
                due_at,
                timezone,
                schedule_expression,
                conversation_id
            ],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "la programación está ejecutándose o la conversación ya no está disponible"
                    .to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_task.updated', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": id,
                    "due_at": due_at,
                    "timezone": timezone,
                    "schedule_expression": schedule_expression
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        self.list_scheduled_tasks()?
            .into_iter()
            .find(|task| task.id == id)
            .ok_or_else(|| AppError::NotFound("tarea programada editada".to_owned()))
    }

    pub fn delete_scheduled_task(&self, id: &str, confirmed: bool) -> Result<(), AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "eliminar una tarea programada requiere confirmación explícita".to_owned(),
            ));
        }
        let connection = self.connect()?;
        let changed = connection.execute(
            "DELETE FROM scheduled_tasks
             WHERE id = ?1
               AND NOT EXISTS(
                 SELECT 1 FROM scheduled_runs
                 WHERE scheduled_task_id = ?1 AND status IN ('claimed', 'running')
               )",
            params![id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "no se puede eliminar una programación que se está ejecutando".to_owned(),
            ));
        }
        Ok(())
    }

    pub fn claim_due_scheduled_task(&self) -> Result<Option<ScheduledClaim>, AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let candidate = transaction
            .query_row(
                "SELECT id, next_run_at, schedule_expression,
                        COALESCE(json_extract(payload_json, '$.target_kind'), 'conversation'),
                        json_extract(payload_json, '$.conversation_id'),
                        json_extract(payload_json, '$.workflow_id'),
                        json_extract(payload_json, '$.workflow_version_id'),
                        json_extract(payload_json, '$.prompt')
                 FROM scheduled_tasks
                 WHERE enabled = 1
                   AND confirmed_at IS NOT NULL
                   AND next_run_at IS NOT NULL
                   AND datetime(next_run_at) <= datetime('now')
                 ORDER BY datetime(next_run_at), created_at
                 LIMIT 1",
                [],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, String>(2)?,
                        row.get::<_, String>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, Option<String>>(5)?,
                        row.get::<_, Option<String>>(6)?,
                        row.get::<_, String>(7)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            scheduled_task_id,
            due_at,
            schedule_expression,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        )) = candidate
        else {
            return Ok(None);
        };
        let claim_key = format!("{scheduled_task_id}:{due_at}");
        let run_id = format!("scheduled_run_{}", Uuid::new_v4().simple());
        let inserted = transaction.execute(
            "INSERT OR IGNORE INTO scheduled_runs(
                id, scheduled_task_id, due_at, claim_key, status, attempt
             ) VALUES (?1, ?2, ?3, ?4, 'claimed', 1)",
            params![run_id, scheduled_task_id, due_at, claim_key],
        )?;
        if inserted == 0 {
            transaction.commit()?;
            return Ok(None);
        }
        let next_run_at = match schedule_expression.as_str() {
            "daily" | "weekly" => {
                let modifier = if schedule_expression == "daily" {
                    "+1 day"
                } else {
                    "+7 days"
                };
                transaction.query_row(
                    "WITH RECURSIVE occurrences(value, step) AS (
                        SELECT ?1, 0
                        UNION ALL
                        SELECT strftime(
                                   '%Y-%m-%dT%H:%M:%fZ',
                                   datetime(value, 'localtime', ?2, 'utc')
                               ),
                               step + 1
                        FROM occurrences
                        WHERE datetime(value) <= datetime('now') AND step < 5000
                     )
                     SELECT value
                     FROM occurrences
                     WHERE datetime(value) > datetime('now')
                     ORDER BY step
                     LIMIT 1",
                    params![due_at, modifier],
                    |row| row.get::<_, String>(0),
                )?
            }
            _ => due_at.clone(),
        };
        transaction.execute(
            "UPDATE scheduled_tasks
             SET enabled = CASE WHEN schedule_expression = 'once' THEN 0 ELSE 1 END,
                 next_run_at = ?3,
                 last_claim_key = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1",
            params![scheduled_task_id, claim_key, next_run_at],
        )?;
        transaction.commit()?;
        Ok(Some(ScheduledClaim {
            run_id,
            scheduled_task_id,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        }))
    }

    pub fn retry_failed_scheduled_run(
        &self,
        run_id: &str,
        confirmed: bool,
    ) -> Result<ScheduledClaim, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "reintentar una ejecución fallida requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source = transaction
            .query_row(
                "SELECT source.scheduled_task_id,
                        COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation'),
                        json_extract(task.payload_json, '$.conversation_id'),
                        json_extract(task.payload_json, '$.workflow_id'),
                        json_extract(task.payload_json, '$.workflow_version_id'),
                        json_extract(task.payload_json, '$.prompt'),
                        COALESCE((
                            SELECT MAX(attempt) FROM scheduled_runs
                            WHERE scheduled_task_id = source.scheduled_task_id
                        ), 0)
                 FROM scheduled_runs source
                 JOIN scheduled_tasks task ON task.id = source.scheduled_task_id
                 WHERE source.id = ?1
                   AND source.status = 'failed'
                   AND (
                       (
                           COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                           AND EXISTS(
                               SELECT 1 FROM conversations conversation
                               WHERE conversation.id = json_extract(task.payload_json, '$.conversation_id')
                                 AND conversation.archived_at IS NULL
                                 AND conversation.deleted_at IS NULL
                           )
                       ) OR (
                           json_extract(task.payload_json, '$.target_kind') = 'workflow'
                           AND EXISTS(
                               SELECT 1 FROM workflows workflow
                               WHERE workflow.id = json_extract(task.payload_json, '$.workflow_id')
                                 AND workflow.archived_at IS NULL
                                 AND workflow.published_version_id IS NOT NULL
                           )
                       )
                   )
                   AND NOT EXISTS(
                       SELECT 1 FROM scheduled_runs active
                       WHERE active.scheduled_task_id = source.scheduled_task_id
                         AND active.status IN ('claimed', 'running')
                   )",
                params![run_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, String>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, Option<String>>(4)?,
                        row.get::<_, String>(5)?,
                        row.get::<_, i64>(6)?,
                    ))
                },
            )
            .optional()?;
        let Some((
            scheduled_task_id,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
            maximum_attempt,
        )) = source
        else {
            return Err(AppError::Conflict(
                "esta ejecución ya no admite reintento o existe otra en curso".to_owned(),
            ));
        };
        let retry_run_id = format!("scheduled_run_{}", Uuid::new_v4().simple());
        let attempt = maximum_attempt + 1;
        let claim_key = format!(
            "{scheduled_task_id}:retry:{attempt}:{}",
            Uuid::new_v4().simple()
        );
        transaction.execute(
            "INSERT INTO scheduled_runs(
                id, scheduled_task_id, due_at, claim_key, status, attempt
             ) VALUES (
                ?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3, 'claimed', ?4
             )",
            params![retry_run_id, scheduled_task_id, claim_key, attempt],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_run.retry_requested', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": scheduled_task_id,
                    "source_run_id": run_id,
                    "retry_run_id": retry_run_id,
                    "attempt": attempt
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(ScheduledClaim {
            run_id: retry_run_id,
            scheduled_task_id,
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        })
    }

    pub fn claim_scheduled_task_now(
        &self,
        scheduled_task_id: &str,
        confirmed: bool,
    ) -> Result<ScheduledClaim, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "ejecutar una programación ahora requiere confirmación explícita".to_owned(),
            ));
        }
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let source = transaction
            .query_row(
                "SELECT COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation'),
                        json_extract(task.payload_json, '$.conversation_id'),
                        json_extract(task.payload_json, '$.workflow_id'),
                        json_extract(task.payload_json, '$.workflow_version_id'),
                        json_extract(task.payload_json, '$.prompt')
                 FROM scheduled_tasks task
                 WHERE task.id = ?1
                   AND (
                       (
                           COALESCE(json_extract(task.payload_json, '$.target_kind'), 'conversation') = 'conversation'
                           AND EXISTS(
                               SELECT 1 FROM conversations conversation
                               WHERE conversation.id = json_extract(task.payload_json, '$.conversation_id')
                                 AND conversation.archived_at IS NULL
                                 AND conversation.deleted_at IS NULL
                           )
                       ) OR (
                           json_extract(task.payload_json, '$.target_kind') = 'workflow'
                           AND EXISTS(
                               SELECT 1 FROM workflows workflow
                               WHERE workflow.id = json_extract(task.payload_json, '$.workflow_id')
                                 AND workflow.archived_at IS NULL
                                 AND workflow.published_version_id IS NOT NULL
                           )
                       )
                   )
                   AND NOT EXISTS(
                       SELECT 1 FROM scheduled_runs active
                       WHERE active.scheduled_task_id = task.id
                         AND active.status IN ('claimed', 'running')
                   )",
                params![scheduled_task_id],
                |row| {
                    Ok((
                        row.get::<_, String>(0)?,
                        row.get::<_, Option<String>>(1)?,
                        row.get::<_, Option<String>>(2)?,
                        row.get::<_, Option<String>>(3)?,
                        row.get::<_, String>(4)?,
                    ))
                },
            )
            .optional()?;
        let Some((target_kind, conversation_id, workflow_id, workflow_version_id, prompt)) = source
        else {
            return Err(AppError::Conflict(
                "la programación ya tiene una ejecución en curso o su conversación no está disponible"
                    .to_owned(),
            ));
        };
        let manual_run_id = format!("scheduled_run_{}", Uuid::new_v4().simple());
        let claim_key = format!("{scheduled_task_id}:manual:{}", Uuid::new_v4().simple());
        transaction.execute(
            "INSERT INTO scheduled_runs(
                id, scheduled_task_id, due_at, claim_key, status, attempt
             ) VALUES (
                ?1, ?2, strftime('%Y-%m-%dT%H:%M:%fZ', 'now'), ?3, 'claimed', 1
             )",
            params![manual_run_id, scheduled_task_id, claim_key],
        )?;
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             VALUES ('scheduled_run.manual_requested', 'user', ?1, ?2)",
            params![
                conversation_id,
                serde_json::json!({
                    "scheduled_task_id": scheduled_task_id,
                    "scheduled_run_id": manual_run_id
                })
                .to_string()
            ],
        )?;
        transaction.commit()?;
        Ok(ScheduledClaim {
            run_id: manual_run_id,
            scheduled_task_id: scheduled_task_id.to_owned(),
            target_kind,
            conversation_id,
            workflow_id,
            workflow_version_id,
            prompt,
        })
    }

    pub fn start_scheduled_run(&self, run_id: &str, broker_task_id: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE scheduled_runs
             SET status = 'running', broker_task_id = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status = 'claimed'",
            params![run_id, broker_task_id],
        )?;
        Ok(())
    }

    pub fn start_scheduled_workflow_run(
        &self,
        run_id: &str,
        workflow_run_id: &str,
    ) -> Result<(), AppError> {
        self.connect()?.execute(
            "UPDATE scheduled_runs
             SET status = 'running', workflow_run_id = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status = 'claimed'",
            params![run_id, workflow_run_id],
        )?;
        Ok(())
    }

    pub fn fail_scheduled_run(&self, run_id: &str, message: &str) -> Result<(), AppError> {
        let connection = self.connect()?;
        connection.execute(
            "UPDATE scheduled_runs
             SET status = 'failed', result_json = ?2,
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1 AND status IN ('claimed', 'running')",
            params![
                run_id,
                serde_json::json!({ "message": message }).to_string()
            ],
        )?;
        Ok(())
    }

    pub fn scheduled_cancellation_target(
        &self,
        run_id: &str,
        confirmed: bool,
    ) -> Result<ScheduledCancellationTarget, AppError> {
        if !confirmed {
            return Err(AppError::Validation(
                "cancelar una ejecución programada requiere confirmación explícita".to_owned(),
            ));
        }
        self.connect()?
            .query_row(
                "SELECT scheduled_task_id, broker_task_id, workflow_run_id
                 FROM scheduled_runs
                 WHERE id = ?1
                   AND status = 'running'
                   AND (broker_task_id IS NOT NULL OR workflow_run_id IS NOT NULL)",
                params![run_id],
                |row| {
                    Ok(ScheduledCancellationTarget {
                        scheduled_task_id: row.get(0)?,
                        broker_task_id: row.get(1)?,
                        workflow_run_id: row.get(2)?,
                    })
                },
            )
            .optional()?
            .ok_or_else(|| {
                AppError::Conflict(
                    "la ejecución ya terminó o todavía no puede cancelarse".to_owned(),
                )
            })
    }

    pub fn finish_scheduled_cancellation(
        &self,
        run_id: &str,
        execution_id: &str,
    ) -> Result<(), AppError> {
        let mut connection = self.connect()?;
        let transaction = connection.transaction_with_behavior(TransactionBehavior::Immediate)?;
        let changed = transaction.execute(
            "UPDATE scheduled_runs
             SET status = 'cancelled',
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE id = ?1
               AND (broker_task_id = ?2 OR workflow_run_id = ?2)
               AND status IN ('running', 'cancelled')",
            params![run_id, execution_id],
        )?;
        if changed == 0 {
            return Err(AppError::Conflict(
                "la ejecución cambió de estado antes de completar la cancelación".to_owned(),
            ));
        }
        transaction.execute(
            "INSERT INTO audit_events(event_type, actor, conversation_id, payload_json)
             SELECT 'scheduled_run.cancelled', 'user',
                    json_extract(task.payload_json, '$.conversation_id'),
                    json_object(
                        'scheduled_task_id', run.scheduled_task_id,
                        'scheduled_run_id', run.id,
                        'broker_task_id', run.broker_task_id
                    )
             FROM scheduled_runs run
             JOIN scheduled_tasks task ON task.id = run.scheduled_task_id
             WHERE run.id = ?1",
            params![run_id],
        )?;
        transaction.commit()?;
        Ok(())
    }

    pub fn reconcile_scheduled_runs(&self) -> Result<usize, AppError> {
        let connection = self.connect()?;
        let broker_runs = connection.execute(
            "UPDATE scheduled_runs
             SET status = (
                    SELECT CASE bt.remote_status
                        WHEN 'completed' THEN 'completed'
                        WHEN 'cancelled' THEN 'cancelled'
                        ELSE 'failed'
                    END
                    FROM broker_tasks bt WHERE bt.id = scheduled_runs.broker_task_id
                 ),
                 result_json = (
                    SELECT COALESCE(bt.result_json, bt.error_json)
                    FROM broker_tasks bt WHERE bt.id = scheduled_runs.broker_task_id
                 ),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE status = 'running'
               AND broker_task_id IS NOT NULL
               AND EXISTS(
                    SELECT 1 FROM broker_tasks bt
                    WHERE bt.id = scheduled_runs.broker_task_id
                      AND bt.remote_status IN ('completed', 'failed', 'cancelled')
               )",
            [],
        )?;
        let workflow_runs = connection.execute(
            "UPDATE scheduled_runs
             SET status = (
                    SELECT CASE workflow.status
                        WHEN 'completed' THEN 'completed'
                        WHEN 'cancelled' THEN 'cancelled'
                        ELSE 'failed'
                    END
                    FROM workflow_runs workflow
                    WHERE workflow.id = scheduled_runs.workflow_run_id
                 ),
                 result_json = (
                    SELECT json_object(
                        'workflow_run_id', workflow.id,
                        'outputs', json(COALESCE(workflow.output_json, '{}')),
                        'error', CASE
                            WHEN workflow.error_json IS NULL THEN NULL
                            ELSE json(workflow.error_json)
                        END
                    )
                    FROM workflow_runs workflow
                    WHERE workflow.id = scheduled_runs.workflow_run_id
                 ),
                 updated_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now')
             WHERE status = 'running'
               AND workflow_run_id IS NOT NULL
               AND EXISTS(
                    SELECT 1 FROM workflow_runs workflow
                    WHERE workflow.id = scheduled_runs.workflow_run_id
                      AND workflow.status IN ('completed', 'partial_failed', 'failed', 'cancelled')
               )",
            [],
        )?;
        Ok(broker_runs + workflow_runs)
    }

    pub fn path(&self) -> &Path {
        &self.path
    }
}

fn validate_execution_preferences(
    preferences: &ConversationExecutionPreferences,
) -> Result<(), AppError> {
    if !matches!(
        preferences.data_classification.as_str(),
        "public" | "internal" | "confidential" | "local_only"
    ) {
        return Err(AppError::Validation(
            "la clasificación de datos no es válida".to_owned(),
        ));
    }
    if !matches!(
        preferences.strategy.as_str(),
        "single" | "auto" | "mixture_of_agents"
    ) {
        return Err(AppError::Validation(
            "la estrategia de ejecución no es válida".to_owned(),
        ));
    }
    if !matches!(preferences.preset.as_str(), "fast" | "slow") {
        return Err(AppError::Validation(
            "la profundidad de análisis no es válida".to_owned(),
        ));
    }
    if !preferences.max_cost_usd.is_finite() || !(0.0..=10.0).contains(&preferences.max_cost_usd) {
        return Err(AppError::Validation(
            "el límite de coste debe estar entre 0 y 10 USD".to_owned(),
        ));
    }
    if !matches!(preferences.long_context.as_str(), "fail" | "map_reduce") {
        return Err(AppError::Validation(
            "el tratamiento de documentos largos no es válido".to_owned(),
        ));
    }
    if preferences.priority > 1000 {
        return Err(AppError::Validation(
            "la prioridad debe estar entre 0 y 1000".to_owned(),
        ));
    }
    Ok(())
}

fn decode_embedding(blob: &[u8], dimensions: i64) -> Result<Vec<f64>, AppError> {
    let expected = usize::try_from(dimensions)
        .ok()
        .and_then(|value| value.checked_mul(8))
        .ok_or_else(|| AppError::BrokerContract("dimensiones de embedding inválidas".to_owned()))?;
    if blob.len() != expected {
        return Err(AppError::BrokerContract(
            "el vector almacenado no coincide con sus dimensiones".to_owned(),
        ));
    }
    Ok(blob
        .chunks_exact(8)
        .map(|chunk| f64::from_le_bytes(chunk.try_into().expect("chunk de ocho bytes")))
        .collect())
}

fn cosine_similarity(left: &[f64], right: &[f64]) -> f64 {
    if left.len() != right.len() || left.is_empty() {
        return f64::NAN;
    }
    let dot = left.iter().zip(right).map(|(a, b)| a * b).sum::<f64>();
    let left_norm = left.iter().map(|value| value * value).sum::<f64>().sqrt();
    let right_norm = right.iter().map(|value| value * value).sum::<f64>().sqrt();
    if left_norm == 0.0 || right_norm == 0.0 {
        f64::NAN
    } else {
        (dot / (left_norm * right_norm)).clamp(-1.0, 1.0)
    }
}

/// Abre el expediente durable de una investigación cuando la petición lo es.
///
/// La decisión se toma leyendo la petición ya construida, no un parámetro
/// aparte: así una investigación abre exactamente el mismo expediente tanto si
/// llega por el camino directo como si llega tras una recuperación semántica, y
/// no puede existir una petición `deep_research` sin sus etapas asociadas.
fn insert_research_run_if_needed(
    transaction: &rusqlite::Transaction<'_>,
    request: &Value,
    conversation_id: &str,
    local_task_id: &str,
    user_text: &str,
) -> Result<(), AppError> {
    if request
        .get("content")
        .and_then(|content| content.get("metadata"))
        .and_then(|metadata| metadata.get("workflow_kind"))
        .and_then(Value::as_str)
        != Some("deep_research")
    {
        return Ok(());
    }
    let research_run_id = format!("research_{}", Uuid::new_v4().simple());
    transaction.execute(
        "INSERT INTO research_runs(
            id, conversation_id, broker_task_id, objective, status
         ) VALUES (?1, ?2, ?3, ?4, 'planning')",
        params![research_run_id, conversation_id, local_task_id, user_text],
    )?;
    // Sin etapas fijas. Antes se insertaban tres —plan, búsqueda, síntesis— que
    // no describían nada: eran una plantilla dibujada antes de que ocurriera
    // nada. Los pasos reales los escribe `record_research_tool_step` conforme
    // el modelo pide herramientas, cada uno con su parámetro y su resultado.
    transaction.execute(
        "INSERT INTO audit_events(
            event_type, actor, conversation_id, payload_json
         ) VALUES ('research.started', 'user', ?1, ?2)",
        params![
            conversation_id,
            serde_json::json!({
                "research_run_id": research_run_id,
                "broker_task_id": local_task_id
            })
            .to_string()
        ],
    )?;
    Ok(())
}

fn lexical_terms(text: &str) -> HashSet<String> {
    text.to_lowercase()
        .split(|character: char| !character.is_alphanumeric())
        .filter(|term| term.chars().count() >= 3)
        .map(str::to_owned)
        .collect()
}

fn normalized_document_query(text: &str) -> String {
    text.to_lowercase()
        .chars()
        .map(|character| match character {
            'á' | 'à' | 'ä' | 'â' => 'a',
            'é' | 'è' | 'ë' | 'ê' => 'e',
            'í' | 'ì' | 'ï' | 'î' => 'i',
            'ó' | 'ò' | 'ö' | 'ô' => 'o',
            'ú' | 'ù' | 'ü' | 'û' => 'u',
            'ñ' => 'n',
            other if other.is_alphanumeric() => other,
            _ => ' ',
        })
        .collect::<String>()
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

fn is_global_document_request(query: &str) -> bool {
    let query = normalized_document_query(query);
    let explicit_global_request = [
        "de que va",
        "de que trata",
        "resumen del libro",
        "resumen del documento",
        "resume el libro",
        "resume este libro",
        "resume el documento",
        "resume este documento",
        "hazme un resumen",
        "vision general",
        "idea principal",
        "ideas principales",
        "estructura del libro",
        "estructura del documento",
        "cuantos capitulos",
        "cuantos temas",
        "what is the book about",
        "what is this book about",
        "what is the document about",
        "summarize the book",
        "summarize this book",
        "summarize the document",
        "document overview",
    ]
    .iter()
    .any(|phrase| query.contains(phrase));
    if explicit_global_request {
        return true;
    }

    let asks_for_summary = query.contains("resumen") || query.contains("summary");
    let narrows_to_part = [
        "capitulo",
        "seccion",
        "apartado",
        "pagina",
        "fragmento",
        "chapter",
        "section",
        "page",
    ]
    .iter()
    .any(|term| query.contains(term));
    asks_for_summary && !narrows_to_part
}

fn global_chunk_role(chunk: &SelectedAttachmentChunk) -> (&'static str, i32) {
    let text = normalized_document_query(&chunk.text);
    if text.contains("table of contents")
        || text.contains("indice general")
        || text.contains("indice de contenidos")
        || text.contains("contenido") && text.contains("capitulo")
    {
        ("Vista global del documento · índice", 1_000)
    } else if text.contains("abstract") || text.contains("sinopsis") || text.contains("resumen") {
        ("Vista global del documento · resumen editorial", 980)
    } else if text.contains("preface")
        || text.contains("foreword")
        || text.contains("prefacio")
        || text.contains("prologo")
    {
        ("Vista global del documento · prefacio", 960)
    } else if text.contains("introduction") || text.contains("introduccion") {
        ("Vista global del documento · introducción", 940)
    } else if text.contains("conclusion")
        || text.contains("conclusiones")
        || text.contains("epilogue")
        || text.contains("epilogo")
    {
        ("Vista global del documento · conclusiones", 920)
    } else if chunk.ordinal == 0 {
        ("Vista global del documento · cabecera", 900)
    } else if chunk.ordinal <= 2 {
        (
            "Vista global del documento · apertura",
            850 - chunk.ordinal as i32,
        )
    } else {
        ("Vista global del documento · muestra representativa", 100)
    }
}

fn select_global_document_chunks(
    candidates: Vec<SelectedAttachmentChunk>,
    maximum_chunks: usize,
    character_budget: usize,
) -> Result<Vec<SelectedAttachmentChunk>, AppError> {
    let mut attachment_order = Vec::new();
    let mut grouped: HashMap<String, Vec<SelectedAttachmentChunk>> = HashMap::new();
    for candidate in candidates {
        if !grouped.contains_key(&candidate.attachment_id) {
            attachment_order.push(candidate.attachment_id.clone());
        }
        grouped
            .entry(candidate.attachment_id.clone())
            .or_default()
            .push(candidate);
    }

    let mut ranked_groups = Vec::new();
    for attachment_id in attachment_order {
        let group = grouped.remove(&attachment_id).unwrap_or_default();
        let group_len = group.len();
        let mut structural = group
            .iter()
            .filter(|chunk| global_chunk_role(chunk).1 > 100)
            .cloned()
            .collect::<Vec<_>>();
        structural.sort_by(|left, right| {
            let (_, left_priority) = global_chunk_role(left);
            let (_, right_priority) = global_chunk_role(right);
            right_priority
                .cmp(&left_priority)
                .then_with(|| left.ordinal.cmp(&right.ordinal))
        });

        // If the converter did not preserve recognizable headings, add samples
        // from the beginning, middle and end instead of pretending that cosine
        // similarity can answer a question about the whole document.
        let mut ranked = structural;
        let mut included = ranked
            .iter()
            .map(|chunk| chunk.id.clone())
            .collect::<HashSet<_>>();
        for ordinal in [
            0,
            group_len / 3,
            group_len.saturating_mul(2) / 3,
            group_len.saturating_sub(1),
        ] {
            if let Some(sample) = group.iter().find(|chunk| chunk.ordinal == ordinal as i64) {
                if included.insert(sample.id.clone()) {
                    ranked.push(sample.clone());
                }
            }
        }
        let mut remaining = group
            .into_iter()
            .filter(|chunk| included.insert(chunk.id.clone()))
            .collect::<Vec<_>>();
        remaining.sort_by_key(|chunk| chunk.ordinal);
        ranked.extend(remaining);
        ranked_groups.push(ranked);
    }

    let mut selected = Vec::new();
    let mut used_characters = 0_usize;
    let mut next_indexes = vec![0_usize; ranked_groups.len()];
    while selected.len() < maximum_chunks {
        let mut progressed = false;
        for (group_index, group) in ranked_groups.iter().enumerate() {
            while let Some(candidate) = group.get(next_indexes[group_index]) {
                next_indexes[group_index] += 1;
                let candidate_characters = candidate.text.chars().count();
                if used_characters.saturating_add(candidate_characters) > character_budget {
                    continue;
                }
                let mut candidate = candidate.clone();
                let (reason, priority) = global_chunk_role(&candidate);
                candidate.reason = reason.to_owned();
                candidate.score = f64::from(priority) / 1_000.0;
                used_characters += candidate_characters;
                selected.push(candidate);
                progressed = true;
                break;
            }
            if selected.len() == maximum_chunks {
                break;
            }
        }
        if !progressed {
            break;
        }
    }
    Ok(selected)
}

#[cfg(test)]
mod tests {
    use super::{
        markdown_web_sources, validated_preferred_model, ContextMessage,
        ConversationExecutionPreferences, CustomGptToolPermissions, Database, ToolOutcomeRecord,
        WorkflowEdge, WorkflowNode, INITIAL_MIGRATION, SCHEMA_VERSION,
    };
    use crate::broker::TaskState;
    use crate::error::AppError;
    use rusqlite::params;
    use serde_json::Value;
    use sha2::{Digest, Sha256};
    use uuid::Uuid;

    fn test_database() -> Database {
        let path = std::env::temp_dir().join(format!(
            "chatygpt-db-test-{}.sqlite",
            Uuid::new_v4().simple()
        ));
        Database::open(path).expect("test database should open")
    }

    fn cleanup(database: &Database) {
        let path = database.path().to_path_buf();
        for candidate in [
            path.clone(),
            path.with_extension("sqlite-wal"),
            path.with_extension("sqlite-shm"),
        ] {
            let _ = std::fs::remove_file(candidate);
        }
    }

    /// Investigación profunda y recuperación semántica ya conviven.
    ///
    /// Antes, activar ambos controles descartaba la recuperación en silencio.
    /// Ahora el plan se congela en la primera etapa, sobrevive a un reinicio y
    /// la segunda etapa abre el mismo expediente de investigación que abriría
    /// el camino directo.
    #[test]
    fn semantic_workflow_carries_a_frozen_research_plan_into_its_second_stage() {
        let database = test_database();
        let conversation = database
            .create_conversation("Investigación con contexto", None)
            .expect("conversation should be created");
        let embedding_request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {"metadata": {
                "source_type": "chat_memory_search",
                "source_id": "research-workflow",
                "content_sha256": "research-hash"
            }}
        });
        let context = vec![ContextMessage {
            message_id: "research-user".to_owned(),
            role: "user".to_owned(),
            text: "Contrasta el informe adjunto con fuentes públicas".to_owned(),
        }];
        let plan = serde_json::json!({ "skills": ["web_search"], "client_tools": ["fetch_url"], "max_iterations": 12 });
        database
            .prepare_semantic_chat_turn(
                "research-workflow",
                &conversation.id,
                "research-user",
                "research-assistant",
                "research-embedding-task",
                "research-embedding-key",
                "Contrasta el informe adjunto con fuentes públicas",
                &embedding_request,
                &context,
                &[],
                false,
                false,
                &ConversationExecutionPreferences::default(),
                Some(&plan),
            )
            .expect("semantic research turn should persist");

        // El plan se recupera intacto, que es lo que hace posible reanudar
        // tras un reinicio sin volver a negociar capacidades con el Broker.
        let workflow = database
            .semantic_chat_workflow_for_task("research-embedding-task")
            .expect("workflow should load")
            .expect("workflow should exist");
        assert_eq!(workflow.research_plan.as_ref(), Some(&plan));
        assert_eq!(workflow.status, "searching");

        // Un turno semántico ordinario sigue sin plan.
        database
            .prepare_semantic_chat_turn(
                "plain-workflow",
                &conversation.id,
                "plain-user",
                "plain-assistant",
                "plain-embedding-task",
                "plain-embedding-key",
                "Resume lo que ya hemos hablado",
                &embedding_request,
                &context,
                &[],
                false,
                false,
                &ConversationExecutionPreferences::default(),
                None,
            )
            .expect("plain semantic turn should persist");
        assert!(database
            .semantic_chat_workflow_for_task("plain-embedding-task")
            .expect("workflow should load")
            .expect("workflow should exist")
            .research_plan
            .is_none());

        // La segunda etapa abre el expediente durable de la investigación.
        let chat_request = serde_json::json!({
            "idempotency_key": "chatygpt:semantic-chat:research-workflow",
            "inference_kind": "chat",
            "content": {
                "prompt": "Ejecuta una investigación profunda y trazable.",
                "metadata": {"workflow_kind": "deep_research"}
            }
        });
        database
            .prepare_semantic_chat_submission(
                "research-workflow",
                "research-chat-task",
                "chatygpt:semantic-chat:research-workflow",
                &chat_request,
                &[],
                &[],
            )
            .expect("second stage should persist");

        let view = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        assert_eq!(view.research_runs.len(), 1);
        assert_eq!(view.research_runs[0].status, "planning");
        // Sin etapas fijas: los pasos aparecen cuando el modelo pide una
        // herramienta, no dibujados de antemano.
        assert_eq!(view.research_runs[0].steps.len(), 0);
        assert_eq!(
            view.research_runs[0].objective,
            "Contrasta el informe adjunto con fuentes públicas"
        );

        let connection = database.connect().expect("connection should open");
        let audited: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events WHERE event_type = 'research.started'",
                [],
                |row| row.get(0),
            )
            .expect("audit count should succeed");
        assert_eq!(audited, 1);
        drop(connection);

        cleanup(&database);
    }

    /// Cada herramienta ejecutada es un paso real, con su parámetro visible.
    ///
    /// Sustituye a las tres etapas fijas: aquellas eran una plantilla dibujada
    /// antes de que ocurriera nada, y decían lo mismo en toda investigación.
    #[test]
    fn executed_tools_become_the_real_research_steps() {
        let database = test_database();
        let conversation = database
            .create_conversation("Investigación con pasos reales", None)
            .expect("conversation should be created");
        let request = serde_json::json!({
            "idempotency_key": "research:1:1",
            "inference_kind": "chat",
            "content": {
                "prompt": "Investiga la normativa",
                "metadata": {"workflow_kind": "deep_research"}
            }
        });
        database
            .prepare_chat_turn_with_project_instruction(
                &conversation.id,
                "msg-user",
                "msg-assistant",
                "local-research",
                "research:1:1",
                "Investiga la normativa",
                &request,
                &[],
                None,
                None,
                &[],
                &[],
                &[],
            )
            .expect("research turn should persist");

        // Al abrirse, el expediente no tiene ningún paso: nada ha ocurrido aún.
        let inicial = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        assert_eq!(inicial.research_runs.len(), 1);
        assert!(inicial.research_runs[0].steps.is_empty());

        database
            .record_research_tool_step(
                "local-research",
                "call_1",
                "fetch_url",
                "https://example.org/normativa",
                "completed",
                &serde_json::json!({"url": "https://example.org/normativa", "truncated": false}),
            )
            .expect("el paso debe registrarse");
        database
            .record_research_tool_step(
                "local-research",
                "call_2",
                "fetch_url",
                "https://example.org/roto",
                "failed",
                &serde_json::json!({"error": "la página respondió HTTP 500"}),
            )
            .expect("un fallo también es un paso");

        let view = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        let steps = &view.research_runs[0].steps;
        assert_eq!(steps.len(), 2);
        // El parámetro con el que se llamó es visible, que es lo que faltaba:
        // «abrí esta URL» en vez de «buscar y contrastar fuentes».
        assert_eq!(steps[0].title, "fetch_url: https://example.org/normativa");
        assert_eq!(steps[0].kind, "research");
        assert_eq!(steps[0].status, "completed");
        // Un fallo se registra como paso, no se omite: el recorrido incluye
        // las fuentes que no se pudieron leer.
        assert_eq!(steps[1].status, "failed");

        // Reejecutar la misma llamada tras un reinicio actualiza el paso, no
        // añade uno nuevo: la identidad es la llamada, no su posición.
        database
            .record_research_tool_step(
                "local-research",
                "call_2",
                "fetch_url",
                "https://example.org/roto",
                "completed",
                &serde_json::json!({"url": "https://example.org/roto"}),
            )
            .expect("el reintento debe actualizar el mismo paso");
        let reintentado = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        assert_eq!(reintentado.research_runs[0].steps.len(), 2);
        assert_eq!(reintentado.research_runs[0].steps[1].status, "completed");

        cleanup(&database);
    }

    #[test]
    fn performance_samples_are_bounded_typed_and_free_of_personal_content() {
        let database = test_database();

        // Una métrica desconocida no llega a tocar la tabla.
        let rejected = database.record_performance_samples("prompt del usuario", &[10]);
        assert!(matches!(rejected, Err(AppError::Validation(_))));
        // Tampoco una duración imposible.
        assert!(matches!(
            database.record_performance_samples("app_start", &[-1]),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            database.record_performance_samples("app_start", &[600_001]),
            Err(AppError::Validation(_))
        ));
        // Ni un lote mayor que el máximo por llamada.
        let oversized = vec![1_i64; 101];
        assert!(matches!(
            database.record_performance_samples("ui_response", &oversized),
            Err(AppError::Validation(_))
        ));

        // La retención es real: 250 muestras dejan exactamente 200 conservadas.
        let durations: Vec<i64> = (1..=250).collect();
        for lote in durations.chunks(50) {
            database
                .record_performance_samples("conversation_open", lote)
                .expect("las muestras válidas deben registrarse");
        }
        let connection = database.connect().expect("connection should open");
        let retained: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM performance_samples WHERE metric = 'conversation_open'",
                [],
                |row| row.get(0),
            )
            .expect("count should succeed");
        assert_eq!(retained, 200);
        // Se conservan las últimas, no las primeras.
        let oldest: i64 = connection
            .query_row(
                "SELECT MIN(duration_ms) FROM performance_samples
                 WHERE metric = 'conversation_open'",
                [],
                |row| row.get(0),
            )
            .expect("min should succeed");
        assert_eq!(oldest, 51);

        // La tabla no tiene ninguna columna capaz de guardar texto libre.
        let columns: Vec<String> = connection
            .prepare("SELECT name FROM pragma_table_info('performance_samples')")
            .expect("pragma should prepare")
            .query_map([], |row| row.get::<_, String>(0))
            .expect("pragma should run")
            .collect::<Result<Vec<_>, _>>()
            .expect("column names should be readable");
        assert_eq!(columns, vec!["id", "metric", "duration_ms", "recorded_at"]);
        drop(connection);

        cleanup(&database);
    }

    #[test]
    fn performance_report_only_judges_metrics_that_were_executed() {
        let database = test_database();

        // Sin ninguna muestra, ninguna métrica obtiene veredicto.
        let empty = database
            .performance_report()
            .expect("el informe vacío debe poder consultarse");
        assert_eq!(empty.metrics.len(), 4);
        assert_eq!(empty.total_samples, 0);
        assert_eq!(empty.sample_limit, 200);
        for summary in &empty.metrics {
            assert_eq!(summary.samples, 0);
            assert_eq!(summary.meets_budget, None);
            assert!(summary.last_recorded_at.is_none());
            assert!(summary.budget_ms > 0);
        }

        // Veinte aperturas rápidas cumplen el objetivo; la búsqueda lenta no.
        database
            .record_performance_samples("conversation_open", &[120; 20])
            .expect("las aperturas deben registrarse");
        database
            .record_performance_samples("conversation_search", &[900; 10])
            .expect("las búsquedas deben registrarse");

        let report = database
            .performance_report()
            .expect("el informe debe poder consultarse");
        assert_eq!(report.total_samples, 30);
        let open = report
            .metrics
            .iter()
            .find(|summary| summary.metric == "conversation_open")
            .expect("la apertura debe figurar");
        assert_eq!(open.samples, 20);
        assert_eq!(open.p95_ms, Some(120));
        assert_eq!(open.meets_budget, Some(true));
        assert!(open.last_recorded_at.is_some());
        let search = report
            .metrics
            .iter()
            .find(|summary| summary.metric == "conversation_search")
            .expect("la búsqueda debe figurar");
        assert_eq!(search.meets_budget, Some(false));
        // Las métricas nunca ejecutadas siguen sin veredicto en el mismo informe.
        let ui = report
            .metrics
            .iter()
            .find(|summary| summary.metric == "ui_response")
            .expect("la respuesta de interfaz debe figurar");
        assert_eq!(ui.meets_budget, None);

        // Vaciar exige confirmación y queda auditado.
        assert!(matches!(
            database.clear_performance_samples(false),
            Err(AppError::Validation(_))
        ));
        database
            .clear_performance_samples(true)
            .expect("vaciar confirmado debe funcionar");
        let cleared = database
            .performance_report()
            .expect("el informe debe seguir consultándose");
        assert_eq!(cleared.total_samples, 0);
        let connection = database.connect().expect("connection should open");
        let audited: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE event_type = 'performance.samples_cleared'",
                [],
                |row| row.get(0),
            )
            .expect("audit count should succeed");
        assert_eq!(audited, 1);
        drop(connection);

        cleanup(&database);
    }

    #[test]
    fn projects_search_and_lifecycle_are_audited() {
        let database = test_database();
        let project = database
            .create_project("TFM", Some("Trabajo final"))
            .expect("project should be created");
        let conversation = database
            .create_conversation("Normativa", Some(&project.id))
            .expect("conversation should be created");

        let connection = database.connect().expect("connection should open");
        connection
            .execute(
                "INSERT INTO messages(
                    id, conversation_id, role, status, sequence_no
                 ) VALUES ('message-search', ?1, 'user', 'complete', 1)",
                params![conversation.id],
            )
            .expect("message should be inserted");
        connection
            .execute(
                "INSERT INTO message_parts(
                    id, message_id, ordinal, kind, content_text
                 ) VALUES (
                    'part-search', 'message-search', 0, 'text',
                    'consulta sobre contratación pública'
                 )",
                [],
            )
            .expect("message part should be inserted");
        drop(connection);

        let results = database
            .search_conversations("contratación", 10)
            .expect("search should succeed");
        assert_eq!(results.len(), 1);
        assert_eq!(results[0].id, conversation.id);
        assert!(database
            .search_conversations("%", 10)
            .expect("wildcard should be treated literally")
            .is_empty());

        database
            .rename_conversation(&conversation.id, "Normativa española")
            .expect("rename should succeed");
        database
            .archive_project(&project.id)
            .expect("archive should succeed");

        let conversation_after = database
            .conversation_summary(&conversation.id)
            .expect("conversation should remain");
        assert!(conversation_after.project_id.is_none());
        assert!(database
            .list_projects()
            .expect("projects should list")
            .is_empty());

        let connection = database.connect().expect("connection should open");
        let audited: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE event_type IN (
                    'project.created', 'conversation.created',
                    'conversation.renamed', 'project.archived'
                 )",
                [],
                |row| row.get(0),
            )
            .expect("audit count should load");
        assert_eq!(audited, 4);
        drop(connection);
        cleanup(&database);
    }

    #[test]
    fn custom_gpt_edits_create_immutable_versions_without_tool_permissions() {
        let database = test_database();
        assert!(matches!(
            database.create_custom_gpt("", None, "Responde con claridad."),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            database.create_custom_gpt("Ayudante", None, "   "),
            Err(AppError::Validation(_))
        ));

        let created = database
            .create_custom_gpt(
                "Ayudante de estudio",
                Some("Explica conceptos técnicos"),
                "Responde paso a paso y define cada término.",
            )
            .expect("custom GPT should be created");
        assert_eq!(created.version_no, 1);
        assert_eq!(
            created.instructions,
            "Responde paso a paso y define cada término."
        );

        let updated = database
            .update_custom_gpt(
                &created.id,
                "Tutor de estudio",
                Some("Explica y comprueba la comprensión"),
                "Primero explica; después formula una pregunta de comprobación.",
            )
            .expect("custom GPT should create a new version");
        assert_eq!(updated.version_no, 2);
        assert_eq!(updated.name, "Tutor de estudio");
        assert_eq!(
            updated.instructions,
            "Primero explica; después formula una pregunta de comprobación."
        );
        let listed = database
            .list_custom_gpts()
            .expect("custom GPTs should list");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].version_no, 2);

        let connection = database.connect().expect("connection should open");
        let versions: Vec<(i64, String)> = {
            let mut statement = connection
                .prepare(
                    "SELECT version_no, configuration_json
                     FROM gpt_versions
                     WHERE custom_gpt_id = ?1
                     ORDER BY version_no",
                )
                .expect("version query should prepare");
            statement
                .query_map(params![created.id], |row| Ok((row.get(0)?, row.get(1)?)))
                .expect("versions should query")
                .collect::<Result<Vec<_>, _>>()
                .expect("versions should collect")
        };
        assert_eq!(versions.len(), 2);
        let first_configuration: Value =
            serde_json::from_str(&versions[0].1).expect("first configuration should be JSON");
        assert_eq!(
            first_configuration["instructions"],
            "Responde paso a paso y define cada término."
        );
        assert_eq!(first_configuration["toolsEnabled"], false);
        let (permission_count, non_denied_count): (i64, i64) = connection
            .query_row(
                "SELECT COUNT(*), SUM(effect != 'deny') FROM gpt_tool_permissions",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("permission matrix should load");
        assert_eq!(permission_count, 4);
        assert_eq!(non_denied_count, 0);
        let feature_enabled: bool = connection
            .query_row(
                "SELECT enabled FROM feature_flags WHERE key = 'custom_gpts'",
                [],
                |row| row.get(0),
            )
            .expect("feature flag should load");
        assert!(feature_enabled);
        let audited: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE event_type IN ('custom_gpt.created', 'custom_gpt.version_created')",
                [],
                |row| row.get(0),
            )
            .expect("audit count should load");
        assert_eq!(audited, 2);
        drop(connection);
        cleanup(&database);
    }

    #[test]
    fn custom_gpt_execution_profile_is_optional_versioned_and_restorable() {
        let database = test_database();
        let profile = ConversationExecutionPreferences {
            data_classification: "confidential".to_owned(),
            strategy: "mixture_of_agents".to_owned(),
            preset: "slow".to_owned(),
            max_cost_usd: 0.75,
            long_context: "fail".to_owned(),
            priority: 50,
        };
        let created = database
            .create_custom_gpt_with_starters(
                "Analista versionado",
                None,
                "Contrasta varias perspectivas.",
                &[],
                &CustomGptToolPermissions::default(),
                None,
                None,
                Some(&profile),
            )
            .expect("profile should be accepted");
        let created_profile = created
            .execution_profile
            .as_ref()
            .expect("the active version should expose its profile");
        assert_eq!(created_profile.strategy, "mixture_of_agents");
        assert_eq!(created_profile.preset, "slow");
        assert_eq!(created_profile.data_classification, "confidential");

        let inherited = database
            .update_custom_gpt_with_starters(
                &created.id,
                "Analista versionado",
                None,
                "Ahora hereda los ajustes del chat.",
                &[],
                &CustomGptToolPermissions::default(),
                None,
                None,
                None,
            )
            .expect("profile can be disabled in a new version");
        assert!(inherited.execution_profile.is_none());

        let history = database
            .list_custom_gpt_versions(&created.id)
            .expect("history should preserve both profiles");
        assert!(history[0].execution_profile.is_none());
        assert_eq!(
            history[1]
                .execution_profile
                .as_ref()
                .map(|value| value.max_cost_usd),
            Some(0.75)
        );
        let restored = database
            .restore_custom_gpt_version(&created.id, &history[1].id, true)
            .expect("old profile should restore as a new version");
        assert_eq!(
            restored
                .execution_profile
                .as_ref()
                .map(|value| value.priority),
            Some(50)
        );

        let invalid = ConversationExecutionPreferences {
            priority: 1001,
            ..ConversationExecutionPreferences::default()
        };
        assert!(matches!(
            database.create_custom_gpt_with_starters(
                "Perfil inválido",
                None,
                "No debe guardarse.",
                &[],
                &CustomGptToolPermissions::default(),
                None,
                None,
                Some(&invalid),
            ),
            Err(AppError::Validation(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn custom_gpt_starters_and_portable_json_round_trip_safely() {
        let database = test_database();
        let permissions = CustomGptToolPermissions {
            run_code: "confirm".to_owned(),
            rename_conversation: "deny".to_owned(),
        };
        let created = database
            .create_custom_gpt_with_starters(
                "Tutor portable",
                Some("Ayuda a estudiar"),
                "Explica con ejemplos.",
                &[
                    " Explícame el tema paso a paso ".to_owned(),
                    "Hazme cinco preguntas".to_owned(),
                    "hazme cinco preguntas".to_owned(),
                ],
                &permissions,
                Some("qwen2.5:14b"),
                None,
                None,
            )
            .expect("custom GPT with starters should be created");
        assert_eq!(
            created.conversation_starters,
            vec![
                "Explícame el tema paso a paso".to_owned(),
                "Hazme cinco preguntas".to_owned()
            ]
        );
        assert_eq!(created.tool_permissions.run_code, "confirm");
        assert_eq!(created.tool_permissions.rename_conversation, "deny");

        let exported = database
            .export_custom_gpt_json(&created.id)
            .expect("custom GPT should export");
        let portable: Value = serde_json::from_str(&exported).expect("export should be JSON");
        assert_eq!(portable["schemaVersion"], 1);
        assert_eq!(
            portable["conversationStarters"].as_array().unwrap().len(),
            2
        );
        assert!(portable.get("id").is_none());
        assert!(portable.get("toolsEnabled").is_none());
        assert!(portable.get("toolPermissions").is_none());

        let imported = database
            .import_custom_gpt_json(&exported)
            .expect("portable GPT should import");
        assert_ne!(imported.id, created.id);
        assert_eq!(imported.name, created.name);
        assert_eq!(
            imported.conversation_starters,
            created.conversation_starters
        );
        assert_eq!(imported.tool_permissions.run_code, "deny");
        assert_eq!(imported.tool_permissions.rename_conversation, "deny");
        assert!(matches!(
            database.import_custom_gpt_json(
                r#"{"schemaVersion":1,"name":"X","instructions":"Y","unexpected":true}"#
            ),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            database.create_custom_gpt_with_starters(
                "Demasiados",
                None,
                "Instrucciones",
                &vec!["Inicio".to_owned(); 7],
                &CustomGptToolPermissions::default(),
                None,
                None,
                None
            ),
            Err(AppError::Validation(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn custom_gpt_history_restores_a_previous_version_without_losing_any() {
        let database = test_database();
        let created = database
            .create_custom_gpt_with_starters(
                "Revisor",
                Some("Revisa textos"),
                "Versión uno de las instrucciones.",
                &["Revisa este texto".to_owned()],
                &CustomGptToolPermissions {
                    run_code: "deny".to_owned(),
                    rename_conversation: "confirm".to_owned(),
                },
                Some("qwen2.5:14b"),
                None,
                None,
            )
            .expect("el GPT debe crearse");
        assert_eq!(created.preferred_model.as_deref(), Some("qwen2.5:14b"));

        database
            .update_custom_gpt_with_starters(
                &created.id,
                "Revisor",
                Some("Revisa textos"),
                "Versión dos de las instrucciones.",
                &[],
                &CustomGptToolPermissions::default(),
                None,
                None,
                None,
            )
            .expect("la edición debe crear otra versión");

        let history = database
            .list_custom_gpt_versions(&created.id)
            .expect("el historial debe cargarse");
        assert_eq!(history.len(), 2);
        assert_eq!(history[0].version_no, 2);
        assert!(history[0].active, "la más reciente es la activa");
        assert!(!history[1].active);
        assert_eq!(history[1].instructions, "Versión uno de las instrucciones.");
        assert_eq!(history[1].preferred_model.as_deref(), Some("qwen2.5:14b"));
        assert_eq!(history[1].tool_permissions.rename_conversation, "confirm");

        // Restaurar exige confirmación explícita.
        assert!(matches!(
            database.restore_custom_gpt_version(&created.id, &history[1].id, false),
            Err(AppError::Validation(_))
        ));
        // Y no tiene sentido restaurar la que ya está activa.
        assert!(matches!(
            database.restore_custom_gpt_version(&created.id, &history[0].id, true),
            Err(AppError::Conflict(_))
        ));

        let restored = database
            .restore_custom_gpt_version(&created.id, &history[1].id, true)
            .expect("la restauración debe funcionar");
        assert_eq!(restored.version_no, 3, "restaurar crea una versión nueva");
        assert_eq!(restored.instructions, "Versión uno de las instrucciones.");
        assert_eq!(restored.preferred_model.as_deref(), Some("qwen2.5:14b"));
        assert_eq!(
            restored.tool_permissions.rename_conversation, "confirm",
            "los permisos de la versión restaurada la acompañan"
        );

        let history = database
            .list_custom_gpt_versions(&created.id)
            .expect("el historial debe recargarse");
        assert_eq!(history.len(), 3, "no se borra ninguna revisión");
        assert_eq!(
            history.iter().filter(|version| version.active).count(),
            1,
            "solo puede haber una versión activa"
        );
        cleanup(&database);
    }

    #[test]
    fn duplicating_a_custom_gpt_never_carries_permissions_or_knowledge() {
        let database = test_database();
        let source = database
            .create_custom_gpt_with_starters(
                "Asistente con permisos",
                None,
                "Instrucciones originales.",
                &["Empieza aquí".to_owned()],
                &CustomGptToolPermissions {
                    run_code: "confirm".to_owned(),
                    rename_conversation: "confirm".to_owned(),
                },
                Some("qwen2.5:14b"),
                None,
                None,
            )
            .expect("el GPT origen debe crearse");
        database
            .create_custom_gpt_memory_item(
                &source.id,
                "Dato reservado del asistente",
                "fact",
                "sensitive",
            )
            .expect("el conocimiento debe guardarse");

        let copy = database
            .duplicate_custom_gpt(&source.id, None)
            .expect("la duplicación debe funcionar");

        assert_ne!(copy.id, source.id);
        assert_eq!(copy.name, "Asistente con permisos (copia)");
        assert_eq!(copy.instructions, source.instructions);
        assert_eq!(copy.conversation_starters, source.conversation_starters);
        assert_eq!(copy.preferred_model.as_deref(), Some("qwen2.5:14b"));
        assert_eq!(copy.version_no, 1, "la copia empieza su propio historial");
        assert_eq!(
            copy.tool_permissions.run_code, "deny",
            "un duplicado nunca hereda permisos concedidos"
        );
        assert_eq!(copy.tool_permissions.rename_conversation, "deny");
        assert!(
            database
                .custom_gpt_knowledge(&copy.id)
                .expect("el conocimiento de la copia debe consultarse")
                .is_empty(),
            "el conocimiento no se copia con el asistente"
        );
        // El original permanece intacto.
        let originals = database
            .custom_gpt_knowledge(&source.id)
            .expect("el conocimiento original debe seguir ahí");
        assert_eq!(originals.len(), 1);
        cleanup(&database);
    }

    #[test]
    fn custom_gpt_icon_is_validated_versioned_portable_and_duplicated() {
        let database = test_database();
        let created = database
            .create_custom_gpt_with_icon(
                "Research helper",
                None,
                Some("research"),
                "Investigate carefully.",
                &[],
                &CustomGptToolPermissions::default(),
                None,
                None,
                None,
            )
            .expect("a catalog icon should be accepted");
        assert_eq!(created.icon_ref, "research");

        let updated = database
            .update_custom_gpt_with_icon(
                &created.id,
                &created.name,
                None,
                Some("code"),
                "Build and verify the solution.",
                &[],
                &CustomGptToolPermissions::default(),
                None,
                None,
                None,
            )
            .expect("changing the icon should create a version");
        assert_eq!(updated.icon_ref, "code");
        assert_eq!(updated.version_no, 2);

        let history = database
            .list_custom_gpt_versions(&created.id)
            .expect("icon history should load");
        assert_eq!(history[0].icon_ref, "code");
        assert_eq!(history[1].icon_ref, "research");

        let exported = database
            .export_custom_gpt_json(&created.id)
            .expect("icon should export");
        let portable: Value = serde_json::from_str(&exported).expect("export should be JSON");
        assert_eq!(portable["iconRef"], "code");
        let imported = database
            .import_custom_gpt_json(&exported)
            .expect("icon should import");
        assert_eq!(imported.icon_ref, "code");

        let duplicate = database
            .duplicate_custom_gpt(&created.id, Some("Research helper copy"))
            .expect("duplicate should retain presentation");
        assert_eq!(duplicate.icon_ref, "code");

        let restored = database
            .restore_custom_gpt_version(&created.id, &history[1].id, true)
            .expect("restoring should retain the historical icon");
        assert_eq!(restored.icon_ref, "research");

        assert!(matches!(
            database.create_custom_gpt_with_icon(
                "Invalid icon",
                None,
                Some("../../icon.png"),
                "Do not save this.",
                &[],
                &CustomGptToolPermissions::default(),
                None,
                None,
                None,
            ),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            database.import_custom_gpt_json(
                r#"{"schemaVersion":1,"name":"Invalid icon","iconRef":"../../icon.png","instructions":"Do not save this."}"#
            ),
            Err(AppError::Validation(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn preferred_model_is_validated_against_the_broker_limit() {
        assert_eq!(
            validated_preferred_model(Some("  qwen2.5:14b  ")).expect("debe normalizarse"),
            Some("qwen2.5:14b".to_owned())
        );
        assert_eq!(
            validated_preferred_model(Some("   ")).expect("vacío es ninguno"),
            None
        );
        assert_eq!(validated_preferred_model(None).expect("sin valor"), None);
        assert!(matches!(
            validated_preferred_model(Some(&"a".repeat(129))),
            Err(AppError::Validation(_))
        ));
        assert!(matches!(
            validated_preferred_model(Some("modelo con espacios")),
            Err(AppError::Validation(_))
        ));
    }

    #[test]
    fn custom_gpt_portable_knowledge_is_explicit_filtered_and_quarantined() {
        let database = test_database();
        let created = database
            .create_custom_gpt(
                "Analista portable",
                Some("Conocimiento transferible"),
                "Responde solo con datos revisados.",
            )
            .expect("custom GPT should be created");
        let (portable_id, _) = database
            .create_custom_gpt_memory_item(
                &created.id,
                "La versión estable es la 3.",
                "fact",
                "normal",
            )
            .expect("portable knowledge should be created");
        database
            .create_custom_gpt_memory_item(
                &created.id,
                "Clave que nunca debe viajar.",
                "instruction",
                "sensitive",
            )
            .expect("sensitive knowledge should be created");
        let (disabled_id, _) = database
            .create_custom_gpt_memory_item(
                &created.id,
                "Borrador todavía sin revisar.",
                "preference",
                "normal",
            )
            .expect("draft knowledge should be created");
        database
            .set_custom_gpt_memory_item_enabled(&created.id, &disabled_id, false)
            .expect("draft knowledge should be disabled");
        database
            .register_custom_gpt_attachment(
                &created.id,
                "C:\\managed\\manual.pdf",
                "manual.pdf",
                Some("application/pdf"),
                42,
                "portable-knowledge-file-hash",
            )
            .expect("private file should be linked");

        let configuration_only = database
            .export_custom_gpt_portable(&created.id, false)
            .expect("configuration should export");
        assert_eq!(configuration_only.included_knowledge, 0);
        let configuration_json: Value =
            serde_json::from_str(&configuration_only.json).expect("export should be JSON");
        assert_eq!(configuration_json["schemaVersion"], 1);
        assert!(configuration_json.get("knowledge").is_none());

        let package = database
            .export_custom_gpt_portable(&created.id, true)
            .expect("knowledge package should export");
        assert_eq!(package.included_knowledge, 1);
        assert_eq!(package.excluded_sensitive, 1);
        assert_eq!(package.excluded_disabled, 1);
        assert_eq!(package.excluded_files, 1);
        assert!(!package.json.contains("Clave que nunca debe viajar."));
        assert!(!package.json.contains("Borrador todavía sin revisar."));
        assert!(!package.json.contains("manual.pdf"));
        assert!(!package.json.contains(&portable_id));
        let package_json: Value =
            serde_json::from_str(&package.json).expect("package should be JSON");
        assert_eq!(package_json["schemaVersion"], 2);
        assert_eq!(package_json["knowledge"].as_array().unwrap().len(), 1);
        assert!(package_json.get("toolPermissions").is_none());

        let imported = database
            .import_custom_gpt_package_json(&package.json)
            .expect("knowledge package should import");
        assert_eq!(imported.imported_knowledge, 1);
        assert!(imported.knowledge_requires_review);
        assert_eq!(imported.custom_gpt.tool_permissions.run_code, "deny");
        assert_eq!(
            imported.custom_gpt.tool_permissions.rename_conversation,
            "deny"
        );
        let imported_knowledge = database
            .custom_gpt_knowledge(&imported.custom_gpt.id)
            .expect("imported knowledge should load");
        assert_eq!(imported_knowledge.len(), 1);
        assert_eq!(imported_knowledge[0].content, "La versión estable es la 3.");
        assert!(!imported_knowledge[0].enabled);
        assert_eq!(imported_knowledge[0].sensitivity, "normal");
        assert!(database
            .list_custom_gpt_files(&imported.custom_gpt.id)
            .expect("imported files should load")
            .is_empty());
        assert!(matches!(
            database.import_custom_gpt_package_json(
                r#"{"schemaVersion":1,"name":"X","instructions":"Y","knowledge":[{"category":"fact","content":"No permitido"}]}"#
            ),
            Err(AppError::Validation(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn conversation_custom_gpt_selection_and_task_version_are_durable() {
        let database = test_database();
        let conversation = database
            .create_conversation("GPT por conversación", None)
            .expect("conversation should be created");
        let gpt = database
            .create_custom_gpt(
                "Analista",
                Some("Primera versión"),
                "Responde usando la versión uno.",
            )
            .expect("custom GPT should be created");
        let selected = database
            .set_conversation_custom_gpt(&conversation.id, Some(&gpt.id))
            .expect("custom GPT should be selected");
        assert_eq!(selected.custom_gpt_id.as_deref(), Some(gpt.id.as_str()));

        let frozen = database
            .custom_gpt_for_conversation(&conversation.id)
            .expect("selection should be readable")
            .expect("custom GPT should be active");
        let request = serde_json::json!({
            "content": {
                "prompt": frozen.instructions,
                "metadata": {
                    "custom_gpt_version_id": frozen.version_id,
                    "custom_gpt_version_no": frozen.version_no
                }
            }
        });
        let context = vec![ContextMessage {
            message_id: "message-gpt-user".to_owned(),
            role: "user".to_owned(),
            text: "Aplica mis instrucciones".to_owned(),
        }];
        database
            .prepare_chat_turn_with_project_instruction(
                &conversation.id,
                "message-gpt-user",
                "message-gpt-assistant",
                "task-gpt-v1",
                "gpt-v1-key",
                "Aplica mis instrucciones",
                &request,
                &context,
                None,
                Some(&frozen),
                &[],
                &[],
                &[],
            )
            .expect("turn should persist the selected version");

        database
            .update_custom_gpt(
                &gpt.id,
                "Analista",
                Some("Segunda versión"),
                "Responde usando la versión dos.",
            )
            .expect("a new active version should be created");
        let active = database
            .custom_gpt_for_conversation(&conversation.id)
            .expect("selection should remain")
            .expect("custom GPT should remain active");
        assert_eq!(active.version_no, 2);
        assert_ne!(active.version_id, frozen.version_id);

        let task_version: Option<String> = database
            .connect()
            .expect("connection should open")
            .query_row(
                "SELECT gpt_version_id FROM broker_tasks WHERE id = 'task-gpt-v1'",
                [],
                |row| row.get(0),
            )
            .expect("task should store its GPT version");
        assert_eq!(task_version.as_deref(), Some(frozen.version_id.as_str()));
        let snapshot = database
            .task_context("task-gpt-v1")
            .expect("task context should be traceable");
        let source = snapshot
            .sources
            .iter()
            .find(|source| source.kind == "custom_gpt")
            .expect("custom GPT should be a context source");
        assert!(source.excerpt.contains("versión 1"));
        assert!(source.excerpt.contains("versión uno"));
        assert!(!source.excerpt.contains("versión dos"));
        assert!(snapshot.strategy.ends_with("+ GPT personal"));

        database
            .set_conversation_custom_gpt(&conversation.id, None)
            .expect("custom GPT should be removable");
        assert!(database
            .custom_gpt_for_conversation(&conversation.id)
            .expect("empty selection should be readable")
            .is_none());
        cleanup(&database);
    }

    #[test]
    fn project_instructions_are_scoped_and_visible_in_the_exact_task_context() {
        let database = test_database();
        let project = database
            .create_project("Investigación", None)
            .expect("project should be created");
        let other_project = database
            .create_project("Otro", None)
            .expect("other project should be created");
        let conversation = database
            .create_conversation("Chat del proyecto", Some(&project.id))
            .expect("conversation should be created");
        let other_conversation = database
            .create_conversation("Chat aislado", Some(&other_project.id))
            .expect("other conversation should be created");

        let updated = database
            .update_project_instructions(
                &project.id,
                Some("Distingue hechos de hipótesis y cita las fuentes."),
            )
            .expect("instructions should persist");
        assert_eq!(
            updated.instructions.as_deref(),
            Some("Distingue hechos de hipótesis y cita las fuentes.")
        );
        let instruction = database
            .project_instruction_for_conversation(&conversation.id)
            .expect("instruction lookup should succeed")
            .expect("project instruction should be available");
        assert!(database
            .project_instruction_for_conversation(&other_conversation.id)
            .expect("isolated lookup should succeed")
            .is_none());

        let context = vec![ContextMessage {
            message_id: "project-instruction-user".to_owned(),
            role: "user".to_owned(),
            text: "Analiza el resultado".to_owned(),
        }];
        database
            .prepare_chat_turn_with_project_instruction(
                &conversation.id,
                "project-instruction-user",
                "project-instruction-assistant",
                "project-instruction-task",
                "project-instruction-key",
                "Analiza el resultado",
                &serde_json::json!({"inference_kind": "chat"}),
                &context,
                Some(&instruction),
                None,
                &[],
                &[],
                &[],
            )
            .expect("turn should retain the project instruction");
        let visible = database
            .task_context("project-instruction-task")
            .expect("task context should load");
        assert!(visible.strategy.contains("instrucciones del proyecto"));
        assert!(visible.sources.iter().any(|source| {
            source.kind == "project_instruction"
                && source.label == "Instrucciones del proyecto"
                && source.excerpt.contains("Distingue hechos")
        }));

        database
            .update_project_instructions(&project.id, None)
            .expect("instructions should be removable");
        assert!(database
            .project_instruction_for_conversation(&conversation.id)
            .expect("cleared lookup should succeed")
            .is_none());
        cleanup(&database);
    }

    #[test]
    fn conversation_with_active_task_cannot_be_hidden() {
        let database = test_database();
        let conversation = database
            .create_conversation("Tarea activa", None)
            .expect("conversation should be created");
        let connection = database.connect().expect("connection should open");
        connection
            .execute(
                "INSERT INTO broker_tasks(
                    id, conversation_id, idempotency_key, request_json,
                    remote_status, local_state
                 ) VALUES (
                    'active-task', ?1, 'active-key', '{}',
                    'generating', 'polling'
                 )",
                params![conversation.id],
            )
            .expect("task should be inserted");
        drop(connection);

        assert!(matches!(
            database.archive_conversation(&conversation.id),
            Err(AppError::Conflict(_))
        ));
        assert!(matches!(
            database.delete_conversation(&conversation.id),
            Err(AppError::Conflict(_))
        ));

        let connection = database.connect().expect("connection should open");
        connection
            .execute(
                "UPDATE broker_tasks
                 SET remote_status = 'completed', local_state = 'terminal'
                 WHERE id = 'active-task'",
                [],
            )
            .expect("task should become terminal");
        drop(connection);

        database
            .delete_conversation(&conversation.id)
            .expect("terminal conversation can be deleted");
        assert!(matches!(
            database.conversation_summary(&conversation.id),
            Err(AppError::NotFound(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn conversation_execution_preferences_are_validated_persisted_and_visible() {
        let database = test_database();
        let conversation = database
            .create_conversation("Opciones", None)
            .expect("conversation should be created");
        assert_eq!(
            database
                .conversation_view(&conversation.id)
                .expect("conversation should load")
                .execution_preferences
                .strategy,
            "single"
        );

        let preferences = ConversationExecutionPreferences {
            data_classification: "confidential".to_owned(),
            strategy: "mixture_of_agents".to_owned(),
            preset: "slow".to_owned(),
            max_cost_usd: 0.5,
            long_context: "fail".to_owned(),
            priority: 25,
        };
        database
            .update_conversation_execution_preferences(&conversation.id, &preferences)
            .expect("valid preferences should persist");
        let reloaded = database
            .conversation_view(&conversation.id)
            .expect("conversation should reload");
        assert_eq!(
            reloaded.execution_preferences.data_classification,
            "confidential"
        );
        assert_eq!(reloaded.execution_preferences.strategy, "mixture_of_agents");
        assert_eq!(reloaded.execution_preferences.preset, "slow");
        assert_eq!(reloaded.execution_preferences.max_cost_usd, 0.5);
        assert_eq!(reloaded.execution_preferences.priority, 25);

        let invalid = ConversationExecutionPreferences {
            max_cost_usd: 25.0,
            ..preferences
        };
        assert!(matches!(
            database.update_conversation_execution_preferences(&conversation.id, &invalid),
            Err(AppError::Validation(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn broker_progress_is_persisted_for_the_visible_task_snapshot() {
        let database = test_database();
        database
            .prepare_broker_task(
                "progress-task",
                "progress-key",
                &serde_json::json!({
                    "inference_kind": "chat",
                    "content": {"metadata": {}}
                }),
            )
            .expect("task should be prepared");
        let state: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-progress",
            "kind": "inference",
            "status": "proposing",
            "request_id": null,
            "created_at": "2026-07-26T10:00:00Z",
            "updated_at": "2026-07-26T10:00:01Z",
            "execution_strategy": "mixture_of_agents",
            "execution_preset": "slow",
            "selection_mode": "auto",
            "progress": {
                "phase": "proposing",
                "invocations_completed": 2,
                "invocations_total": 3
            },
            "result": null,
            "error": null
        }))
        .expect("progress state should parse");
        database
            .record_remote_state("progress-task", &state)
            .expect("progress should persist");

        let snapshot = database
            .task_snapshot("progress-task")
            .expect("snapshot should load");
        assert_eq!(snapshot.progress.phase.as_deref(), Some("proposing"));
        assert_eq!(snapshot.progress.invocations_completed, Some(2));
        assert_eq!(snapshot.progress.invocations_total, Some(3));
        cleanup(&database);
    }

    #[test]
    fn deep_research_run_tracks_durable_steps_and_terminal_sources() {
        let database = test_database();
        let conversation = database
            .create_conversation("Investigación durable", None)
            .expect("conversation should be created");
        let user_message_id = "research-user";
        let assistant_message_id = "research-assistant";
        let task_id = "research-task";
        let objective = "Compara dos marcos regulatorios";
        database
            .prepare_chat_turn(
                &conversation.id,
                user_message_id,
                assistant_message_id,
                task_id,
                "research-idempotency",
                objective,
                &serde_json::json!({
                    "inference_kind": "chat",
                    "content": {
                        "metadata": {"workflow_kind": "deep_research"}
                    }
                }),
                &[ContextMessage {
                    message_id: user_message_id.to_owned(),
                    role: "user".to_owned(),
                    text: objective.to_owned(),
                }],
                &[],
                &[],
                &[],
            )
            .expect("research turn should be prepared");
        let initial = database
            .conversation_view(&conversation.id)
            .expect("research run should load");
        assert_eq!(initial.research_runs.len(), 1);
        assert_eq!(initial.research_runs[0].status, "planning");
        assert_eq!(initial.research_runs[0].steps.len(), 0);

        let synthesizing: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-research",
            "status": "synthesizing",
            "request_id": null,
            "created_at": "2026-07-30T12:00:00Z",
            "updated_at": "2026-07-30T12:00:05Z",
            "execution_strategy": "agent",
            "execution_preset": "slow",
            "selection_mode": "adaptive",
            "progress": {"phase": "synthesizing"},
            "result": null,
            "error": null
        }))
        .expect("synthesizing state should parse");
        database
            .record_remote_state(task_id, &synthesizing)
            .expect("research progress should persist");
        let synthesizing_view = database
            .conversation_view(&conversation.id)
            .expect("research progress should load");
        // La fase remota sigue describiendo el expediente completo, pero ya no
        // inventa el estado de ningún paso: los pasos son las herramientas que
        // se ejecutaron, y no hay ninguna todavía.
        assert_eq!(synthesizing_view.research_runs[0].status, "synthesizing");
        assert!(synthesizing_view.research_runs[0].steps.is_empty());

        let completed: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-research",
            "status": "completed",
            "request_id": null,
            "created_at": "2026-07-30T12:00:00Z",
            "updated_at": "2026-07-30T12:00:10Z",
            "execution_strategy": "agent",
            "execution_preset": "slow",
            "selection_mode": "adaptive",
            "progress": {"phase": "completed"},
            "result": {
                "result_markdown": "Informe con [Fuente A](https://example.com/report#section) y https://example.org/data. Duplicada: https://example.com/report."
            },
            "error": null
        }))
        .expect("completed state should parse");
        database
            .record_remote_state(task_id, &completed)
            .expect("research completion should persist");
        let completed_view = database
            .conversation_view(&conversation.id)
            .expect("completed research should load");
        assert_eq!(completed_view.research_runs[0].status, "completed");
        assert!(completed_view.research_runs[0]
            .steps
            .iter()
            .all(|step| step.status == "completed"));
        assert_eq!(completed_view.research_runs[0].source_count, 2);
        let assistant = completed_view
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .expect("assistant message should load");
        assert_eq!(assistant.sources.len(), 2);
        assert_eq!(assistant.sources[0].title, "Fuente A");
        assert_eq!(
            assistant.sources[0].url.as_deref(),
            Some("https://example.com/report")
        );
        assert_eq!(
            assistant.sources[1].url.as_deref(),
            Some("https://example.org/data")
        );
        cleanup(&database);
    }

    #[test]
    fn markdown_web_sources_are_bounded_deduplicated_and_http_only() {
        let sources = markdown_web_sources(
            "[Informe](https://example.com/a#one) \
             https://example.com/a#two \
             [Correo](mailto:test@example.com) \
             https://user:secret@example.net/private \
             [Datos](http://data.example.org/table).",
        );
        assert_eq!(
            sources,
            vec![
                ("Informe".to_owned(), "https://example.com/a".to_owned()),
                (
                    "Datos".to_owned(),
                    "http://data.example.org/table".to_owned()
                )
            ]
        );
    }

    #[test]
    fn attachment_is_deduplicated_and_reused_across_conversations() {
        let database = test_database();
        assert_eq!(
            database.schema_version().expect("version should load"),
            SCHEMA_VERSION
        );
        let first_conversation = database
            .create_conversation("Primera", None)
            .expect("conversation should be created");
        let second_conversation = database
            .create_conversation("Segunda", None)
            .expect("conversation should be created");
        let first = database
            .register_attachment(
                &first_conversation.id,
                "C:/managed/document.pdf",
                "document.pdf",
                Some("application/pdf"),
                42,
                "abc123",
            )
            .expect("attachment should be registered");
        let second = database
            .register_attachment(
                &second_conversation.id,
                "C:/managed/document.pdf",
                "document.pdf",
                Some("application/pdf"),
                42,
                "abc123",
            )
            .expect("attachment should be reused");
        assert_eq!(first.id, second.id);
        assert_eq!(
            database
                .list_attachments(&first_conversation.id)
                .expect("first attachments should list")
                .len(),
            1
        );
        assert_eq!(
            database
                .list_attachments(&second_conversation.id)
                .expect("second attachments should list")
                .len(),
            1
        );

        database
            .update_attachment_ingestion(
                &first.id,
                "ready",
                Some("broker-file-1"),
                Some("document"),
                Some("test"),
                Some(&serde_json::json!({})),
                None,
            )
            .expect("attachment should become ready");
        let ready = database
            .ready_attachments_for_turn(&second_conversation.id, std::slice::from_ref(&first.id))
            .expect("reused attachment should be ready");
        assert_eq!(ready[0].broker_file_id.as_deref(), Some("broker-file-1"));

        database
            .remove_conversation_attachment(&first_conversation.id, &first.id)
            .expect("first association should be removed");
        assert!(database
            .list_attachments(&first_conversation.id)
            .expect("first attachments should list")
            .is_empty());
        assert_eq!(
            database
                .list_attachments(&second_conversation.id)
                .expect("second association should remain")
                .len(),
            1
        );
        cleanup(&database);
    }

    #[test]
    fn workflow_publication_freezes_gpt_version_and_creates_durable_node_runs() {
        let database = test_database();
        let project = database
            .create_project("Proyecto de revisión", None)
            .expect("project should be created");
        database
            .update_project_instructions(
                &project.id,
                Some("Distingue siempre los hechos de las hipótesis."),
            )
            .expect("project instructions should persist");
        database
            .set_memory_enabled(true)
            .expect("memory should be enabled");
        let (project_memory_id, _) = database
            .create_memory_item(
                "La revisión se entrega en español.",
                "instruction",
                "normal",
                Some(&project.id),
            )
            .expect("project memory should be created");
        let gpt = database
            .create_custom_gpt("Revisor", None, "Revisa el texto con rigor.")
            .expect("custom GPT should be created");
        let (memory_id, _) = database
            .create_custom_gpt_memory_item(
                &gpt.id,
                "Solo responde con evidencia verificable.",
                "instruction",
                "sensitive",
            )
            .expect("custom GPT knowledge should be created");
        let gpt_file = database
            .register_custom_gpt_attachment(
                &gpt.id,
                "C:/managed/guide.pdf",
                "guide.pdf",
                Some("application/pdf"),
                42,
                "workflow-gpt-file",
            )
            .expect("custom GPT file should register");
        database
            .update_attachment_ingestion(
                &gpt_file.id,
                "ready",
                Some("broker-gpt-file"),
                Some("document"),
                Some("test"),
                Some(&serde_json::json!({})),
                None,
            )
            .expect("custom GPT file should become ready");
        let mut workflow = database
            .create_workflow("Revisión en cadena", Some(&project.id))
            .expect("workflow should be created");
        let input_id = workflow.definition.nodes[0].id.clone();
        let result_id = workflow.definition.nodes[1].id.clone();
        let gpt_node_id = "node-reviewer".to_owned();
        workflow.definition.nodes.push(WorkflowNode {
            id: gpt_node_id.clone(),
            kind: "custom_gpt".to_owned(),
            label: "Revisor".to_owned(),
            x: 350.0,
            y: 170.0,
            custom_gpt_id: Some(gpt.id.clone()),
            custom_gpt_version_id: None,
            custom_gpt_name: None,
            custom_gpt_icon_ref: None,
            custom_gpt_instructions: None,
            preferred_model: None,
            execution_profile: None,
            custom_gpt_memory_ids: Vec::new(),
            custom_gpt_attachment_ids: Vec::new(),
            instruction: None,
            attachment_ids: Vec::new(),
        });
        workflow.definition.edges = vec![
            WorkflowEdge {
                id: "edge-in".to_owned(),
                source: input_id.clone(),
                target: gpt_node_id.clone(),
            },
            WorkflowEdge {
                id: "edge-out".to_owned(),
                source: gpt_node_id.clone(),
                target: result_id,
            },
        ];
        database
            .update_workflow(
                &workflow.summary.id,
                &workflow.summary.name,
                None,
                Some(&project.id),
                &workflow.definition,
            )
            .expect("draft should save");
        let published = database
            .publish_workflow(&workflow.summary.id)
            .expect("workflow should publish");
        assert_eq!(published.summary.published_version_no, Some(1));

        let record = database
            .create_workflow_run(&workflow.summary.id, "Texto para revisar")
            .expect("durable run should be created");
        let frozen_gpt = record
            .definition
            .nodes
            .iter()
            .find(|node| node.kind == "custom_gpt")
            .expect("published GPT node should exist");
        let frozen_project = record
            .definition
            .project_context
            .as_ref()
            .expect("published project context should exist");
        assert_eq!(frozen_project.project_id, project.id);
        assert_eq!(
            frozen_project.instructions.as_deref(),
            Some("Distingue siempre los hechos de las hipótesis.")
        );
        assert_eq!(frozen_project.memory_ids, vec![project_memory_id.clone()]);
        assert!(database
            .project_instruction_for_workflow(frozen_project)
            .expect("project instruction should resolve")
            .is_some());
        assert_eq!(
            database
                .project_memories_for_workflow(frozen_project)
                .expect("project memories should resolve")
                .len(),
            1
        );
        assert!(frozen_gpt.custom_gpt_version_id.is_some());
        assert_eq!(frozen_gpt.custom_gpt_icon_ref.as_deref(), Some("spark"));
        assert_eq!(
            frozen_gpt.custom_gpt_instructions.as_deref(),
            Some("Revisa el texto con rigor.")
        );
        assert_eq!(frozen_gpt.custom_gpt_memory_ids, vec![memory_id.clone()]);
        assert_eq!(
            frozen_gpt.custom_gpt_attachment_ids,
            vec![gpt_file.id.clone()]
        );
        assert_eq!(
            database
                .custom_gpt_memories_for_workflow(&gpt.id, &frozen_gpt.custom_gpt_memory_ids)
                .expect("published knowledge should resolve")
                .len(),
            1
        );
        assert_eq!(
            database
                .ready_custom_gpt_attachments_for_workflow(
                    &gpt.id,
                    &frozen_gpt.custom_gpt_attachment_ids,
                )
                .expect("published files should resolve")
                .len(),
            1
        );
        let run = database
            .workflow_run(&record.run_id)
            .expect("run should load");
        assert_eq!(run.node_runs.len(), 3);
        assert!(run.node_runs.iter().all(|node| node.status == "pending"));

        database
            .update_workflow_node_run(
                &record.run_id,
                &input_id,
                "completed",
                Some("Texto para revisar"),
                Some("Texto para revisar"),
                None,
                None,
            )
            .expect("input should complete");
        database
            .update_workflow_node_run(
                &record.run_id,
                &gpt_node_id,
                "failed",
                Some("Texto para revisar"),
                None,
                Some("broker-failed"),
                Some(&serde_json::json!({"message": "fallo"})),
            )
            .expect("GPT node should fail");
        database
            .update_workflow_run_status(
                &record.run_id,
                "failed",
                None,
                Some(&serde_json::json!({"message": "fallo"})),
            )
            .expect("run should fail");
        let retry = database
            .retry_workflow_run(&record.run_id)
            .expect("failed run should be retried");
        let retry_view = database
            .workflow_run(&retry.run_id)
            .expect("retry should load");
        assert_eq!(
            retry_view
                .node_runs
                .iter()
                .find(|node| node.node_id == input_id)
                .expect("input should exist")
                .status,
            "completed",
            "successful upstream work is reused"
        );
        assert_eq!(
            retry_view
                .node_runs
                .iter()
                .find(|node| node.node_id == gpt_node_id)
                .expect("GPT should exist")
                .status,
            "pending",
            "the failed node is executed again"
        );
        database
            .set_custom_gpt_memory_item_enabled(&gpt.id, &memory_id, false)
            .expect("knowledge should be disabled");
        database
            .remove_custom_gpt_file(&gpt.id, &gpt_file.id)
            .expect("file should be removed from the GPT");
        assert!(database
            .custom_gpt_memories_for_workflow(&gpt.id, &frozen_gpt.custom_gpt_memory_ids)
            .expect("revoked knowledge should be ignored")
            .is_empty());
        assert!(database
            .ready_custom_gpt_attachments_for_workflow(
                &gpt.id,
                &frozen_gpt.custom_gpt_attachment_ids,
            )
            .expect("revoked files should be ignored")
            .is_empty());
        database
            .update_project_instructions(&project.id, Some("Nueva instrucción"))
            .expect("project instructions should change");
        database
            .set_memory_item_enabled(&project_memory_id, false)
            .expect("project memory should be disabled");
        assert!(database
            .project_instruction_for_workflow(frozen_project)
            .expect("changed instructions should be revoked")
            .is_none());
        assert!(database
            .project_memories_for_workflow(frozen_project)
            .expect("disabled project memories should be ignored")
            .is_empty());
        cleanup(&database);
    }

    #[test]
    fn attachment_deduplication_respects_the_image_processing_policy() {
        let database = test_database();
        let conversation = database
            .create_conversation("Política de imágenes", None)
            .expect("conversation should be created");

        let text_only = database
            .register_attachment_with_image_policy(
                &conversation.id,
                "C:/managed/book.pdf",
                "book.pdf",
                Some("application/pdf"),
                42,
                "same-book",
                Some(false),
            )
            .expect("text-only attachment should register");
        let rich = database
            .register_attachment_with_image_policy(
                &conversation.id,
                "C:/managed/book.pdf",
                "book.pdf",
                Some("application/pdf"),
                42,
                "same-book",
                Some(true),
            )
            .expect("rich attachment should register separately");

        assert_ne!(text_only.id, rich.id);
        assert_eq!(text_only.describe_images, Some(false));
        assert_eq!(rich.describe_images, Some(true));

        let rich_first = database
            .register_attachment_with_image_policy(
                &conversation.id,
                "C:/managed/other.pdf",
                "other.pdf",
                Some("application/pdf"),
                21,
                "rich-first",
                Some(true),
            )
            .expect("rich attachment should register");
        let text_request = database
            .register_attachment_with_image_policy(
                &conversation.id,
                "C:/managed/other.pdf",
                "other.pdf",
                Some("application/pdf"),
                21,
                "rich-first",
                Some(false),
            )
            .expect("rich attachment may satisfy a text-only request");

        assert_eq!(rich_first.id, text_request.id);
        assert_eq!(text_request.describe_images, Some(true));
        cleanup(&database);
    }

    #[test]
    fn project_file_can_be_reused_without_leaking_into_another_project() {
        let database = test_database();
        let project = database
            .create_project("Proyecto compartido", None)
            .expect("project should be created");
        let other_project = database
            .create_project("Proyecto aislado", None)
            .expect("other project should be created");
        let source_conversation = database
            .create_conversation("Origen", Some(&project.id))
            .expect("source conversation should be created");
        let target_conversation = database
            .create_conversation("Destino", Some(&project.id))
            .expect("target conversation should be created");
        let isolated_conversation = database
            .create_conversation("Aislada", Some(&other_project.id))
            .expect("isolated conversation should be created");
        let attachment = database
            .register_attachment(
                &source_conversation.id,
                "C:/managed/project.pdf",
                "project.pdf",
                Some("application/pdf"),
                42,
                "project-file-sha",
            )
            .expect("attachment should be registered");

        database
            .set_project_file(&source_conversation.id, &attachment.id, true)
            .expect("attachment should be saved to the project");
        assert_eq!(
            database
                .list_project_files(&target_conversation.id)
                .expect("project files should list")[0]
                .id,
            attachment.id
        );
        database
            .use_project_file(&target_conversation.id, &attachment.id)
            .expect("project file should link to the target conversation");
        assert_eq!(
            database
                .list_attachments(&target_conversation.id)
                .expect("target attachments should list")
                .len(),
            1
        );
        assert!(matches!(
            database.use_project_file(&isolated_conversation.id, &attachment.id),
            Err(AppError::NotFound(_))
        ));

        database
            .set_project_file(&source_conversation.id, &attachment.id, false)
            .expect("project association should be removable");
        assert!(database
            .list_project_files(&target_conversation.id)
            .expect("project files should list")
            .is_empty());
        assert_eq!(
            database
                .list_attachments(&target_conversation.id)
                .expect("existing conversation link should remain")
                .len(),
            1
        );
        cleanup(&database);
    }

    #[test]
    fn project_knowledge_overview_composes_only_the_selected_project_sources() {
        let database = test_database();
        let project = database
            .create_project("Proyecto visible", None)
            .expect("project should be created");
        let other_project = database
            .create_project("Proyecto ajeno", None)
            .expect("other project should be created");
        let conversation = database
            .create_conversation("Chat visible", Some(&project.id))
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "C:/managed/visible.pdf",
                "visible.pdf",
                Some("application/pdf"),
                99,
                "visible-project-file",
            )
            .expect("attachment should be registered");
        database
            .set_project_file(&conversation.id, &attachment.id, true)
            .expect("file should be saved to the project");
        let second_conversation = database
            .create_conversation("Segundo chat visible", Some(&project.id))
            .expect("second conversation should be created");
        database
            .use_project_file(&second_conversation.id, &attachment.id)
            .expect("project file should be used by the second conversation");
        database
            .update_project_instructions(&project.id, Some("Cita siempre las fuentes."))
            .expect("instructions should persist");
        let (memory_id, _) = database
            .create_memory_item(
                "La fecha de corte es mensual.",
                "fact",
                "normal",
                Some(&project.id),
            )
            .expect("project memory should be created");
        let (other_memory_id, _) = database
            .create_memory_item(
                "Este recuerdo pertenece a otro proyecto.",
                "fact",
                "normal",
                Some(&other_project.id),
            )
            .expect("other memory should be created");

        let overview = database
            .project_knowledge_overview(&project.id)
            .expect("overview should load");
        assert_eq!(overview.project.id, project.id);
        assert_eq!(
            overview.project.instructions.as_deref(),
            Some("Cita siempre las fuentes.")
        );
        assert_eq!(overview.files.len(), 1);
        assert_eq!(overview.files[0].display_name, "visible.pdf");
        assert_eq!(overview.file_usages.len(), 1);
        assert_eq!(overview.file_usages[0].attachment_id, attachment.id);
        assert_eq!(overview.file_usages[0].conversations.len(), 2);
        assert!(overview.file_usages[0]
            .conversations
            .iter()
            .all(|item| item.project_id.as_deref() == Some(project.id.as_str())));
        assert!(overview.file_usages[0]
            .conversations
            .iter()
            .any(|item| item.id == conversation.id && item.title == "Chat visible"));
        assert!(overview.file_usages[0]
            .conversations
            .iter()
            .any(|item| item.id == second_conversation.id && item.title == "Segundo chat visible"));
        assert_eq!(overview.memories.len(), 1);
        assert_eq!(
            overview.memories[0].content,
            "La fecha de corte es mensual."
        );

        let toggled = database
            .set_project_memory_item_enabled(&project.id, &memory_id, false)
            .expect("project memory should be disabled from the overview");
        assert!(!toggled.memories[0].enabled);
        assert!(matches!(
            database.set_project_memory_item_enabled(&project.id, &other_memory_id, false),
            Err(AppError::NotFound(_))
        ));

        let without_file = database
            .remove_project_file(&project.id, &attachment.id)
            .expect("project file should be removable from the overview");
        assert!(without_file.files.is_empty());
        assert!(without_file.file_usages.is_empty());
        assert_eq!(
            database
                .list_attachments(&conversation.id)
                .expect("conversation attachment should remain")
                .len(),
            1
        );
        assert_eq!(
            database
                .list_attachments(&second_conversation.id)
                .expect("second conversation attachment should remain")
                .len(),
            1
        );
        cleanup(&database);
    }

    #[test]
    fn reattaching_a_failed_file_starts_a_fresh_broker_conversion() {
        let database = test_database();
        let conversation = database
            .create_conversation("PDF grande", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "C:/managed/math-deep.pdf",
                "math-deep.pdf",
                Some("application/pdf"),
                24_629_575,
                "large-pdf-sha",
            )
            .expect("attachment should be registered");
        database
            .update_attachment_ingestion(
                &attachment.id,
                "failed",
                Some("file_old_conversion"),
                Some("document"),
                Some("docling"),
                Some(&serde_json::json!({"pages": 2204})),
                Some(&serde_json::json!({
                    "code": "CONVERSION_FAILED",
                    "message": "max_num_pages limit of 2000"
                })),
            )
            .expect("attachment should fail");

        let reattached = database
            .register_attachment(
                &conversation.id,
                "C:/managed/math-deep.pdf",
                "math-deep.pdf",
                Some("application/pdf"),
                24_629_575,
                "large-pdf-sha",
            )
            .expect("failed attachment should be reattached");

        assert_eq!(reattached.id, attachment.id);
        assert_eq!(reattached.ingestion_status, "local");
        assert_eq!(reattached.broker_file_id, None);
        assert_eq!(reattached.ingestion_error, None);
        cleanup(&database);
    }

    #[test]
    fn existing_schema_one_database_upgrades_without_losing_conversations() {
        let path = std::env::temp_dir().join(format!(
            "chatygpt-db-upgrade-test-{}.sqlite",
            Uuid::new_v4().simple()
        ));
        let connection = rusqlite::Connection::open(&path).expect("legacy database should open");
        connection
            .execute_batch(INITIAL_MIGRATION)
            .expect("initial migration should apply");
        connection
            .pragma_update(None, "user_version", 1)
            .expect("legacy version should be set");
        connection
            .execute(
                "INSERT INTO conversations(id, title) VALUES ('legacy-conversation', 'Legado')",
                [],
            )
            .expect("legacy conversation should exist");
        drop(connection);

        let database = Database::open(&path).expect("database should upgrade");
        assert_eq!(
            database.schema_version().expect("version should load"),
            SCHEMA_VERSION
        );
        assert_eq!(
            database
                .list_conversations()
                .expect("conversations should survive")
                .first()
                .map(|conversation| conversation.id.as_str()),
            Some("legacy-conversation")
        );
        cleanup(&database);
    }

    #[test]
    fn retrying_failed_attachment_discards_terminal_broker_file_id() {
        let database = test_database();
        let conversation = database
            .create_conversation("Adjunto fallido", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "C:/managed/failed.pdf",
                "failed.pdf",
                Some("application/pdf"),
                100,
                "failed-sha",
            )
            .expect("attachment should be registered");
        database
            .update_attachment_ingestion(
                &attachment.id,
                "failed",
                Some("file-terminal-failure"),
                Some("document"),
                Some("docling"),
                Some(&serde_json::json!({"pages": 0})),
                Some(&serde_json::json!({"code": "ENGINE_MISSING"})),
            )
            .expect("attachment should fail");

        database
            .reset_failed_attachment_for_retry(&attachment.id)
            .expect("failed attachment should reset");
        let reset = database
            .attachment_record(&attachment.id)
            .expect("attachment should load");
        assert_eq!(reset.ingestion_status, "local");
        assert!(reset.broker_file_id.is_none());
        assert!(database
            .attachment_view(&attachment.id)
            .expect("attachment view should load")
            .ingestion_error
            .is_none());
        cleanup(&database);
    }

    #[test]
    fn completed_turn_materializes_attachment_sources_on_assistant_message() {
        let database = test_database();
        let conversation = database
            .create_conversation("Pregunta con fuente", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "C:/managed/source.pdf",
                "source.pdf",
                Some("application/pdf"),
                2048,
                "source-sha",
            )
            .expect("attachment should be registered");
        database
            .update_attachment_ingestion(
                &attachment.id,
                "ready",
                Some("broker-source-1"),
                Some("document"),
                Some("docling"),
                Some(&serde_json::json!({"pages": 2})),
                None,
            )
            .expect("attachment should become ready");
        let user_message_id = "message-source-user";
        let assistant_message_id = "message-source-assistant";
        let context = vec![ContextMessage {
            message_id: user_message_id.to_owned(),
            role: "user".to_owned(),
            text: "Resume el documento".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                user_message_id,
                assistant_message_id,
                "local-source-task",
                "source-idempotency-key",
                "Resume el documento",
                &serde_json::json!({}),
                &context,
                &[],
                &[],
                std::slice::from_ref(&attachment.id),
            )
            .expect("turn should be prepared");
        let state: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-source-task",
            "status": "completed",
            "request_id": "request-source",
            "created_at": "2026-07-21T00:00:00Z",
            "updated_at": "2026-07-21T00:00:01Z",
            "execution_strategy": "single",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "result_markdown": "Resumen documentado",
                "model_used": {
                    "provider": "lmstudio",
                    "deployment": "local",
                    "model": "modelo-prueba"
                },
                "consensus": {
                    "synthesized": false,
                    "warnings": ["Se entregó la mejor propuesta disponible"]
                },
                "arbiter_failures": [
                    {"model": "revisor-prueba", "code": "PROVIDER_UNAVAILABLE", "message": "offline"}
                ]
            },
            "error": null
        }))
        .expect("task state should deserialize");
        database
            .record_remote_state("local-source-task", &state)
            .expect("completed state should materialize");
        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE messages
                 SET created_at = '2026-07-21T00:00:00.000Z'
                 WHERE id = ?1",
                params![assistant_message_id],
            )
            .expect("message start time should be fixed");
        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE broker_tasks
                 SET terminal_at = '2026-07-21T00:00:12.500Z'
                 WHERE id = 'local-source-task'",
                [],
            )
            .expect("task finish time should be fixed");

        let view = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        let assistant = view
            .messages
            .iter()
            .find(|message| message.id == assistant_message_id)
            .expect("assistant message should exist");
        assert_eq!(assistant.sources.len(), 1);
        assert_eq!(assistant.sources[0].title, "source.pdf");
        assert_eq!(
            assistant
                .model_used
                .as_ref()
                .map(|model| model.model.as_str()),
            Some("modelo-prueba")
        );
        assert_eq!(assistant.response_duration_ms, Some(12_500));
        assert_eq!(assistant.consensus_synthesized, Some(false));
        assert_eq!(
            assistant.consensus_warnings,
            ["Se entregó la mejor propuesta disponible"]
        );
        assert_eq!(assistant.arbiter_failure_count, 1);
        assert_eq!(
            assistant.sources[0].source_attachment_id.as_deref(),
            Some(attachment.id.as_str())
        );
        cleanup(&database);
    }

    #[test]
    fn waiting_tool_call_is_persisted_and_decisions_are_durable() {
        let database = test_database();
        let conversation = database
            .create_conversation("Herramienta pendiente", None)
            .expect("conversation should be created");
        let context = vec![ContextMessage {
            message_id: "tool-user-message".to_owned(),
            role: "user".to_owned(),
            text: "Renombra este chat".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "tool-user-message",
                "tool-assistant-message",
                "local-tool-task",
                "tool-idempotency-key",
                "Renombra este chat",
                &serde_json::json!({}),
                &context,
                &[],
                &[],
                &[],
            )
            .expect("turn should be prepared");
        let waiting: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-tool-task",
            "status": "waiting_for_tools",
            "request_id": "request-tool",
            "created_at": "2026-07-21T00:00:00Z",
            "updated_at": "2026-07-21T00:00:01Z",
            "execution_strategy": "agent",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "status": "waiting_for_tools",
                "pending_tool_calls": [{
                    "id": "call-rename-1",
                    "name": "rename_conversation",
                    "arguments": {"title": "Título propuesto"}
                }]
            },
            "error": null
        }))
        .expect("waiting state should deserialize");
        database
            .record_remote_state("local-tool-task", &waiting)
            .expect("waiting state should persist");
        let waiting_snapshot = database
            .task_snapshot("local-tool-task")
            .expect("snapshot should load");
        assert_eq!(waiting_snapshot.local_state, "waiting_for_tools");
        assert_eq!(waiting_snapshot.pending_tool_calls.len(), 1);
        assert_eq!(
            waiting_snapshot.pending_tool_calls[0].arguments["title"],
            "Título propuesto"
        );

        database
            .prepare_tool_outcomes(
                "local-tool-task",
                &[ToolOutcomeRecord {
                    tool_call_id: "call-rename-1".to_owned(),
                    status: "approved".to_owned(),
                    content: serde_json::json!({"ok": true}).to_string(),
                }],
            )
            .expect("decision should persist before HTTP");
        let prepared = database
            .prepared_tool_results("local-tool-task")
            .expect("prepared results should load");
        assert_eq!(prepared["tool_results"][0]["tool_call_id"], "call-rename-1");
        assert!(database
            .task_snapshot("local-tool-task")
            .expect("snapshot should load")
            .pending_tool_calls
            .is_empty());
        cleanup(&database);
    }

    #[test]
    fn tool_confirmation_is_disclosed_persisted_and_cannot_be_replayed() {
        let database = test_database();
        let conversation = database
            .create_conversation("Confirmación durable", None)
            .expect("conversation should be created");
        let context = vec![ContextMessage {
            message_id: "confirm-user-message".to_owned(),
            role: "user".to_owned(),
            text: "Renombra este chat".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "confirm-user-message",
                "confirm-assistant-message",
                "local-confirm-task",
                "confirm-idempotency-key",
                "Renombra este chat",
                &serde_json::json!({}),
                &context,
                &[],
                &[],
                &[],
            )
            .expect("turn should be prepared");
        let waiting: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "remote-confirm-task",
            "status": "waiting_for_tools",
            "request_id": "request-confirm",
            "created_at": "2026-08-01T00:00:00Z",
            "updated_at": "2026-08-01T00:00:01Z",
            "execution_strategy": "agent",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "status": "waiting_for_tools",
                "pending_tool_calls": [{
                    "id": "call-confirm-1",
                    "name": "rename_conversation",
                    "arguments": {"title": "Presupuesto de obra"}
                }]
            },
            "error": null
        }))
        .expect("waiting state should deserialize");
        database
            .record_remote_state("local-confirm-task", &waiting)
            .expect("waiting state should persist");

        // El expediente nace pendiente y revela los siete elementos exigidos.
        let pending = database
            .pending_tool_calls("local-confirm-task")
            .expect("pending calls should load");
        let confirmation = pending[0]
            .confirmation
            .as_ref()
            .expect("la llamada debe traer su expediente de confirmación");
        assert_eq!(confirmation.status, "pending");
        assert_eq!(confirmation.action_type, "conversation.rename");
        assert_eq!(
            confirmation.tool_name.as_deref(),
            Some("rename_conversation")
        );
        assert_eq!(confirmation.resources["conversation_id"], conversation.id);
        assert_eq!(
            confirmation.disclosure["data_sent"][0]["value"],
            "Presupuesto de obra"
        );
        assert_eq!(confirmation.disclosure["destination"], "local");
        assert_eq!(confirmation.disclosure["scope"], "one_time");
        assert!(confirmation.consequences.contains("reversible"));
        assert!(confirmation.resolved_at.is_none());

        // Sobrevive a un reinicio: el expediente se lee desde SQLite, no de memoria.
        let reopened = Database::open(database.path())
            .expect("database should reopen without losing the record");
        assert_eq!(
            reopened
                .pending_tool_calls("local-confirm-task")
                .expect("pending calls should reload")[0]
                .confirmation
                .as_ref()
                .expect("el expediente debe seguir ahí")
                .status,
            "pending"
        );

        database
            .prepare_tool_outcomes(
                "local-confirm-task",
                &[ToolOutcomeRecord {
                    tool_call_id: "call-confirm-1".to_owned(),
                    status: "approved".to_owned(),
                    content: serde_json::json!({"ok": true}).to_string(),
                }],
            )
            .expect("decision should persist");

        let connection = database.connect().expect("connection should open");
        let (status, resolved_at): (String, Option<String>) = connection
            .query_row(
                "SELECT status, resolved_at FROM confirmation_requests
                 WHERE conversation_id = ?1",
                params![conversation.id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("resolved confirmation should exist");
        assert_eq!(status, "allowed_once");
        assert!(resolved_at.is_some(), "la resolución debe quedar fechada");

        let audited: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE event_type = 'confirmation.resolved'",
                [],
                |row| row.get(0),
            )
            .expect("audit count should load");
        assert_eq!(audited, 1);

        // Si la interfaz reenviara la misma decisión, el expediente ya resuelto
        // impide una segunda ejecución aunque la llamada vuelva a estar pendiente.
        connection
            .execute(
                "UPDATE tool_calls SET status = 'confirmation_required'
                 WHERE remote_tool_call_id = 'call-confirm-1'",
                [],
            )
            .expect("replay scenario should be forced");
        let replay = database.prepare_tool_outcomes(
            "local-confirm-task",
            &[ToolOutcomeRecord {
                tool_call_id: "call-confirm-1".to_owned(),
                status: "approved".to_owned(),
                content: serde_json::json!({"ok": true}).to_string(),
            }],
        );
        assert!(
            matches!(replay, Err(AppError::Conflict(_))),
            "una confirmación ya resuelta no puede volver a ejecutarse: {replay:?}"
        );
        cleanup(&database);
    }

    #[test]
    fn audit_inspector_exposes_only_safe_presentation_fields() {
        let database = test_database();
        let conversation = database
            .create_conversation("Auditoría segura", None)
            .expect("conversation should be created");
        let secret_path = r"C:\Users\private\Documents\conversation.md";
        let internal_hash = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";
        database
            .record_export(
                &conversation.id,
                "conversation:audit:markdown:v1",
                secret_path,
                internal_hash,
                None,
                Some(internal_hash),
                "completed",
                None,
            )
            .expect("export audit should be recorded");

        let events = database
            .list_audit_events(50)
            .expect("safe audit view should load");
        let serialized = serde_json::to_string(&events).expect("audit view should serialize");
        assert!(!serialized.contains(secret_path));
        assert!(!serialized.contains(internal_hash));
        assert!(events
            .iter()
            .any(|event| event.summary == "Exportación completada"));
        cleanup(&database);
    }

    #[test]
    fn pending_conversation_is_identified_for_visible_startup_recovery() {
        let database = test_database();
        let conversation = database
            .create_conversation("Conversación recuperable", None)
            .expect("conversation should be created");
        let context = vec![ContextMessage {
            message_id: "recovery-user-message".to_owned(),
            role: "user".to_owned(),
            text: "Continúa tras reiniciar".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "recovery-user-message",
                "recovery-assistant-message",
                "recovery-local-task",
                "recovery-idempotency",
                "Continúa tras reiniciar",
                &serde_json::json!({}),
                &context,
                &[],
                &[],
                &[],
            )
            .expect("pending turn should be persisted");

        let candidates = database
            .recovery_candidates()
            .expect("recovery candidates should load");
        assert_eq!(candidates.len(), 1);
        assert_eq!(
            candidates[0].conversation_id.as_deref(),
            Some(conversation.id.as_str())
        );
        assert_eq!(candidates[0].label, "Respuesta pendiente");
        cleanup(&database);
    }

    #[test]
    fn memory_is_opt_in_scoped_and_user_controllable() {
        let database = test_database();
        let general = database
            .create_conversation("Chat general", None)
            .expect("general conversation should exist");
        let project = database
            .create_project("Proyecto memoria", None)
            .expect("project should exist");
        let scoped = database
            .create_conversation("Chat de proyecto", Some(&project.id))
            .expect("scoped conversation should exist");
        database
            .create_memory_item("Responder en español", "preference", "normal", None)
            .expect("global memory should be created");
        database
            .create_memory_item("El proyecto usa Rust", "fact", "normal", Some(&project.id))
            .expect("project memory should be created");

        assert!(database
            .active_memories_for_conversation(&general.id)
            .expect("disabled memory should load")
            .is_empty());
        database
            .set_memory_enabled(true)
            .expect("memory should enable");
        let general_memories = database
            .active_memories_for_conversation(&general.id)
            .expect("global memory should load");
        assert_eq!(general_memories.len(), 1);
        let scoped_memories = database
            .active_memories_for_conversation(&scoped.id)
            .expect("scoped memories should load");
        assert_eq!(scoped_memories.len(), 2);
        let context = vec![ContextMessage {
            message_id: "memory-context-user".to_owned(),
            role: "user".to_owned(),
            text: "Usa mi memoria".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &scoped.id,
                "memory-context-user",
                "memory-context-assistant",
                "memory-context-task",
                "memory-context-key",
                "Usa mi memoria",
                &serde_json::json!({}),
                &context,
                &scoped_memories,
                &[],
                &[],
            )
            .expect("memory context should be traced");
        let connection = database.connect().expect("connection should open");
        let (strategy, memory_sources): (String, i64) = connection
            .query_row(
                "SELECT cs.strategy_version,
                        (SELECT COUNT(*) FROM context_sources src
                         WHERE src.snapshot_id = cs.id AND src.source_type = 'memory')
                 FROM context_snapshots cs
                 WHERE cs.broker_task_id = 'memory-context-task'",
                [],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("memory snapshot should load");
        assert_eq!(strategy, "window-memory-v1");
        assert_eq!(memory_sources, 2);
        drop(connection);

        database
            .set_memory_item_enabled(&general_memories[0].id, false)
            .expect("item should disable");
        assert!(database
            .active_memories_for_conversation(&general.id)
            .expect("disabled item should be omitted")
            .is_empty());
        database
            .delete_memory_item(&general_memories[0].id)
            .expect("item should delete");
        assert_eq!(
            database
                .memory_overview()
                .expect("overview should load")
                .items
                .len(),
            1
        );
        cleanup(&database);
    }

    #[test]
    fn custom_gpt_knowledge_is_private_and_independent_from_global_memory() {
        let database = test_database();
        let gpt_a = database
            .create_custom_gpt("GPT Alfa", None, "Usa conocimiento Alfa.")
            .expect("first custom GPT should exist");
        let gpt_b = database
            .create_custom_gpt("GPT Beta", None, "Usa conocimiento Beta.")
            .expect("second custom GPT should exist");
        let conversation_a = database
            .create_conversation("Chat Alfa", None)
            .expect("first conversation should exist");
        let conversation_b = database
            .create_conversation("Chat Beta", None)
            .expect("second conversation should exist");
        database
            .set_conversation_custom_gpt(&conversation_a.id, Some(&gpt_a.id))
            .expect("first GPT should be selected");
        database
            .set_conversation_custom_gpt(&conversation_b.id, Some(&gpt_b.id))
            .expect("second GPT should be selected");
        let (memory_a_id, _) = database
            .create_custom_gpt_memory_item(&gpt_a.id, "Dato exclusivo de Alfa", "fact", "normal")
            .expect("first GPT knowledge should be created");
        database
            .create_custom_gpt_memory_item(&gpt_b.id, "Dato exclusivo de Beta", "fact", "normal")
            .expect("second GPT knowledge should be created");

        assert!(database
            .memory_overview()
            .expect("global memory should load")
            .items
            .is_empty());
        let memories_a = database
            .active_memories_for_conversation(&conversation_a.id)
            .expect("first GPT knowledge should load while global memory is off");
        assert_eq!(memories_a.len(), 1);
        assert_eq!(memories_a[0].content, "Dato exclusivo de Alfa");
        assert_eq!(
            memories_a[0].custom_gpt_id.as_deref(),
            Some(gpt_a.id.as_str())
        );
        let memories_b = database
            .active_memories_for_conversation(&conversation_b.id)
            .expect("second GPT knowledge should load");
        assert_eq!(memories_b.len(), 1);
        assert_eq!(memories_b[0].content, "Dato exclusivo de Beta");

        database
            .set_custom_gpt_memory_item_enabled(&gpt_a.id, &memory_a_id, false)
            .expect("first GPT knowledge should disable");
        assert!(database
            .active_memories_for_conversation(&conversation_a.id)
            .expect("disabled GPT knowledge should be excluded")
            .is_empty());
        assert_eq!(
            database
                .active_memories_for_conversation(&conversation_b.id)
                .expect("second GPT should remain unaffected")
                .len(),
            1
        );

        cleanup(&database);
    }

    #[test]
    fn custom_gpt_files_follow_the_selected_gpt_without_sticky_chat_links() {
        let database = test_database();
        let gpt_a = database
            .create_custom_gpt("GPT con archivo", None, "Consulta su archivo.")
            .expect("first GPT should exist");
        let gpt_b = database
            .create_custom_gpt("GPT sin archivo", None, "No comparte archivos.")
            .expect("second GPT should exist");
        let conversation = database
            .create_conversation("Chat con archivo de GPT", None)
            .expect("conversation should exist");
        database
            .set_conversation_custom_gpt(&conversation.id, Some(&gpt_a.id))
            .expect("first GPT should be selected");
        let file = database
            .register_custom_gpt_attachment(
                &gpt_a.id,
                "C:/managed/private-gpt.pdf",
                "private-gpt.pdf",
                Some("application/pdf"),
                512,
                "custom-gpt-file-sha",
            )
            .expect("GPT file should be registered");
        database
            .update_attachment_ingestion(
                &file.id,
                "ready",
                Some("broker-custom-gpt-file"),
                Some("document"),
                Some("docling"),
                None,
                None,
            )
            .expect("GPT file should become ready");

        let active = database
            .ready_custom_gpt_file_ids_for_conversation(&conversation.id)
            .expect("selected GPT files should resolve");
        assert_eq!(active, vec![file.id.clone()]);
        assert_eq!(
            database
                .ready_attachments_for_turn(&conversation.id, &active)
                .expect("GPT file should be authorized for the turn")
                .len(),
            1
        );
        database
            .replace_attachment_chunks(
                &file.id,
                &["El archivo privado del GPT contiene el dato Delta.".to_owned()],
            )
            .expect("GPT file chunks should be stored");
        let trace_conversation = database
            .create_conversation("Traza del archivo de GPT", None)
            .expect("trace conversation should exist");
        database
            .set_conversation_custom_gpt(&trace_conversation.id, Some(&gpt_a.id))
            .expect("GPT should be selected for trace");
        let trace_files = database
            .ready_custom_gpt_file_ids_for_conversation(&trace_conversation.id)
            .expect("trace GPT files should resolve");
        let chunks = database
            .select_attachment_chunks(&trace_conversation.id, &trace_files, "dato Delta", 4, 8_000)
            .expect("GPT file chunks should be selectable");
        let frozen_gpt = database
            .custom_gpt_for_conversation(&trace_conversation.id)
            .expect("selected GPT should resolve")
            .expect("selected GPT should exist");
        let context = vec![ContextMessage {
            message_id: "custom-gpt-file-user".to_owned(),
            role: "user".to_owned(),
            text: "¿Cuál es el dato?".to_owned(),
        }];
        database
            .prepare_chat_turn_with_project_instruction(
                &trace_conversation.id,
                "custom-gpt-file-user",
                "custom-gpt-file-assistant",
                "custom-gpt-file-task",
                "custom-gpt-file-key",
                "¿Cuál es el dato?",
                &serde_json::json!({}),
                &context,
                None,
                Some(&frozen_gpt),
                &[],
                &chunks,
                &trace_files,
            )
            .expect("GPT file turn should persist");
        let trace = database
            .task_context("custom-gpt-file-task")
            .expect("GPT file context should be inspectable");
        assert!(trace.sources.iter().any(|source| {
            source.kind == "attachment_chunk"
                && source
                    .reason
                    .contains("Archivo de conocimiento del GPT personal seleccionado")
        }));
        let connection = database.connect().expect("connection should open");
        let sticky_links: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM conversation_attachments
                 WHERE conversation_id = ?1 AND attachment_id = ?2",
                params![conversation.id, file.id],
                |row| row.get(0),
            )
            .expect("chat links should be counted");
        assert_eq!(sticky_links, 0);
        drop(connection);

        database
            .set_conversation_custom_gpt(&conversation.id, Some(&gpt_b.id))
            .expect("second GPT should be selected");
        assert!(database
            .ready_custom_gpt_file_ids_for_conversation(&conversation.id)
            .expect("second GPT files should resolve")
            .is_empty());
        assert!(matches!(
            database.ready_attachments_for_turn(&conversation.id, std::slice::from_ref(&file.id)),
            Err(AppError::Validation(_))
        ));

        database
            .set_conversation_custom_gpt(&conversation.id, Some(&gpt_a.id))
            .expect("first GPT should be selected again");
        assert!(database
            .remove_custom_gpt_file(&gpt_a.id, &file.id)
            .expect("GPT file should be removed")
            .is_empty());
        assert!(database
            .ready_custom_gpt_file_ids_for_conversation(&conversation.id)
            .expect("removed GPT files should resolve")
            .is_empty());
        cleanup(&database);
    }

    #[test]
    fn editing_memory_preserves_or_invalidates_its_index_by_content() {
        let database = test_database();
        let project = database
            .create_project("Memoria editable", None)
            .expect("project should exist");
        let original = "Prefiero respuestas breves";
        let (memory_id, _) = database
            .create_memory_item(original, "preference", "normal", None)
            .expect("memory should exist");
        let original_hash = format!("{:x}", Sha256::digest(original.as_bytes()));
        let connection = database.connect().expect("database should connect");
        connection
            .execute(
                "INSERT INTO embedding_records(
                    id, source_type, source_id, chunk_index, model,
                    dimensions, vector_blob, content_sha256
                 ) VALUES ('editable-embedding', 'memory', ?1, 0, 'nomic', 2, ?2, ?3)",
                params![memory_id, vec![0_u8; 16], original_hash],
            )
            .expect("embedding should exist");
        drop(connection);

        let (content_changed, overview) = database
            .update_memory_item(
                &memory_id,
                original,
                "instruction",
                "sensitive",
                Some(&project.id),
            )
            .expect("metadata-only edit should succeed");
        assert!(!content_changed);
        let item = overview
            .items
            .iter()
            .find(|item| item.id == memory_id)
            .expect("memory should remain visible");
        assert_eq!(item.embedding_status, "ready");
        assert_eq!(item.category, "instruction");
        assert_eq!(item.sensitivity, "sensitive");
        assert_eq!(item.project_id.as_deref(), Some(project.id.as_str()));

        let (content_changed, overview) = database
            .update_memory_item(
                &memory_id,
                "Prefiero respuestas breves con ejemplos",
                "instruction",
                "sensitive",
                Some(&project.id),
            )
            .expect("content edit should succeed");
        assert!(content_changed);
        let item = overview
            .items
            .iter()
            .find(|item| item.id == memory_id)
            .expect("edited memory should remain visible");
        assert_eq!(item.embedding_status, "missing");
        assert_eq!(item.content, "Prefiero respuestas breves con ejemplos");
        cleanup(&database);
    }

    #[test]
    fn stale_embedding_result_cannot_replace_an_edited_memory_index() {
        let database = test_database();
        let original = "Texto anterior";
        let (memory_id, _) = database
            .create_memory_item(original, "fact", "normal", None)
            .expect("memory should exist");
        let original_hash = format!("{:x}", Sha256::digest(original.as_bytes()));
        let request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {
                "prompt": original,
                "metadata": {
                    "source_type": "memory",
                    "source_id": memory_id,
                    "content_sha256": original_hash
                }
            }
        });
        database
            .prepare_broker_task("stale-memory-task", "stale-memory-key", &request)
            .expect("old embedding task should persist");
        database
            .update_memory_item(&memory_id, "Texto corregido", "fact", "normal", None)
            .expect("memory should be edited");
        let completed: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "stale-memory-task",
            "status": "completed",
            "created_at": "2026-07-27T00:00:00Z",
            "updated_at": "2026-07-27T00:00:01Z",
            "execution_strategy": "single",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "inference_kind": "embedding",
                "embedding": [1.0, 0.0],
                "model_used": {
                    "provider": "ollama",
                    "deployment": "local",
                    "model": "nomic"
                }
            },
            "error": null
        }))
        .expect("completed state should deserialize");
        database
            .record_remote_state("stale-memory-task", &completed)
            .expect("stale completion should be recorded safely");

        let connection = database.connect().expect("database should connect");
        let embedding_count: i64 = connection
            .query_row(
                "SELECT COUNT(*) FROM embedding_records
                 WHERE source_type = 'memory' AND source_id = ?1",
                params![memory_id],
                |row| row.get(0),
            )
            .expect("embedding count should load");
        assert_eq!(embedding_count, 0);
        drop(connection);
        let item = database
            .memory_item(&memory_id)
            .expect("memory should load");
        assert_eq!(item.embedding_status, "missing");
        cleanup(&database);
    }

    #[test]
    fn context_inspector_explains_the_sources_used_by_a_chat_turn() {
        let database = test_database();
        let conversation = database
            .create_conversation("Contexto visible", None)
            .expect("conversation should exist");
        database
            .set_memory_enabled(true)
            .expect("memory should enable");
        let (memory_id, _) = database
            .create_memory_item(
                "Prefiero respuestas con ejemplos",
                "preference",
                "normal",
                None,
            )
            .expect("memory should be created");
        let memories = database
            .active_memories_for_conversation(&conversation.id)
            .expect("active memories should load");
        let context = vec![ContextMessage {
            message_id: "context-visible-user".to_owned(),
            role: "user".to_owned(),
            text: "Explícame este concepto".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "context-visible-user",
                "context-visible-assistant",
                "context-visible-task",
                "context-visible-key",
                "Explícame este concepto",
                &serde_json::json!({}),
                &context,
                &memories,
                &[],
                &[],
            )
            .expect("turn should persist its context");

        let snapshot = database
            .task_context("context-visible-task")
            .expect("context should be inspectable");

        assert_eq!(snapshot.strategy, "Ventana reciente + memoria");
        assert!(snapshot.estimated_tokens > 0);
        assert_eq!(snapshot.sources.len(), 2);
        assert_eq!(snapshot.sources[0].kind, "message");
        assert_eq!(snapshot.sources[0].label, "Mensaje actual");
        assert_eq!(snapshot.sources[0].reason, "Petición que acabas de enviar");
        assert_eq!(snapshot.sources[1].kind, "memory");
        assert_eq!(snapshot.sources[1].label, "Recuerdo · Preferencia");
        assert_eq!(
            snapshot.sources[1].reason,
            "Recuerdo activado explícitamente por el usuario"
        );
        assert_eq!(
            snapshot.sources[1].excerpt,
            "Prefiero respuestas con ejemplos"
        );
        assert!(!format!("{snapshot:?}").contains(&memory_id));
        cleanup(&database);
    }

    #[test]
    fn semantic_chat_persists_the_turn_before_requesting_its_query_embedding() {
        let database = test_database();
        let conversation = database
            .create_conversation("Memoria semántica durable", None)
            .expect("conversation should exist");
        let gpt = database
            .create_custom_gpt("Tutor semántico", None, "Instrucciones congeladas.")
            .expect("custom GPT should exist");
        database
            .set_conversation_custom_gpt(&conversation.id, Some(&gpt.id))
            .expect("custom GPT should be selected");
        let custom_gpt = database
            .custom_gpt_for_conversation(&conversation.id)
            .expect("custom GPT lookup should succeed")
            .expect("custom GPT should be active");
        let context = vec![ContextMessage {
            message_id: "semantic-user".to_owned(),
            role: "user".to_owned(),
            text: "¿Cómo prefiero recibir las respuestas?".to_owned(),
        }];
        let request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {
                "prompt": "¿Cómo prefiero recibir las respuestas?",
                "metadata": {
                    "source_type": "chat_memory_search",
                    "source_id": "semantic-workflow",
                    "content_sha256": "semantic-query-hash"
                }
            }
        });

        let task = database
            .prepare_semantic_chat_turn_with_project_instruction(
                "semantic-workflow",
                &conversation.id,
                "semantic-user",
                "semantic-assistant",
                "semantic-embedding-task",
                "semantic-embedding-key",
                "¿Cómo prefiero recibir las respuestas?",
                &request,
                &context,
                None,
                Some(&custom_gpt),
                &[],
                false,
                false,
                &ConversationExecutionPreferences::default(),
                None,
            )
            .expect("turn and semantic search should persist atomically");
        database
            .update_custom_gpt(
                &gpt.id,
                "Tutor semántico",
                None,
                "Instrucciones posteriores.",
            )
            .expect("custom GPT should receive a new active version");

        assert_eq!(task.id, "semantic-embedding-task");
        let snapshot = database
            .task_snapshot("semantic-embedding-task")
            .expect("semantic task should be visible");
        assert_eq!(snapshot.activity, "Buscando contexto relacionado");
        assert!(database
            .semantic_workflow_uses_memory("semantic-workflow")
            .expect("memory scope should load"));
        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE broker_tasks
                 SET request_json = json_set(
                   request_json,
                   '$.content.metadata.source_type',
                   'chat_document_search'
                 )
                 WHERE id = 'semantic-embedding-task'",
                [],
            )
            .expect("workflow scope should change for the regression");
        assert!(!database
            .semantic_workflow_uses_memory("semantic-workflow")
            .expect("document scope should load"));
        let view = database
            .conversation_view(&conversation.id)
            .expect("persisted turn should be visible");
        assert_eq!(view.messages.len(), 2);
        assert_eq!(view.messages[0].status, "complete");
        assert_eq!(
            view.messages[0].text.as_deref(),
            Some("¿Cómo prefiero recibir las respuestas?")
        );
        assert_eq!(view.messages[1].status, "pending");
        assert_eq!(
            view.messages[1].broker_task_id.as_deref(),
            Some("semantic-embedding-task")
        );
        let workflow = database
            .semantic_chat_workflow_for_task("semantic-embedding-task")
            .expect("workflow lookup should succeed")
            .expect("workflow should exist");
        assert_eq!(workflow.id, "semantic-workflow");
        assert_eq!(workflow.status, "searching");
        assert_eq!(workflow.context.len(), 1);
        let frozen_gpt = workflow
            .custom_gpt_context
            .expect("workflow should retain its GPT version");
        assert_eq!(frozen_gpt.version_no, 1);
        assert_eq!(frozen_gpt.instructions, "Instrucciones congeladas.");
        cleanup(&database);
    }

    #[test]
    fn completed_semantic_search_prepares_chat_with_ranked_memory_and_trace() {
        fn completed_embedding(task_id: &str, vector: &[f64]) -> TaskState {
            serde_json::from_value(serde_json::json!({
                "task_id": task_id,
                "status": "completed",
                "request_id": format!("request-{task_id}"),
                "created_at": "2026-07-24T00:00:00Z",
                "updated_at": "2026-07-24T00:00:01Z",
                "execution_strategy": "single",
                "execution_preset": "fast",
                "selection_mode": "automatic",
                "progress": {},
                "result": {
                    "inference_kind": "embedding",
                    "embedding": vector,
                    "model_used": {
                        "provider": "ollama",
                        "deployment": "local",
                        "model": "nomic"
                    }
                },
                "error": null
            }))
            .expect("embedding state should deserialize")
        }

        let database = test_database();
        let conversation = database
            .create_conversation("Selección semántica", None)
            .expect("conversation should exist");
        database
            .set_memory_enabled(true)
            .expect("memory should enable");
        let (memory_id, _) = database
            .create_memory_item("Prefiero respuestas breves", "preference", "normal", None)
            .expect("memory should exist");
        let memory_request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {
                "prompt": "Prefiero respuestas breves",
                "metadata": {
                    "source_type": "memory",
                    "source_id": memory_id,
                    "content_sha256": format!(
                        "{:x}",
                        Sha256::digest("Prefiero respuestas breves".as_bytes())
                    )
                }
            }
        });
        database
            .prepare_broker_task("ranked-memory-task", "ranked-memory-key", &memory_request)
            .expect("memory task should persist");
        database
            .record_remote_state(
                "ranked-memory-task",
                &completed_embedding("ranked-memory-task", &[1.0, 0.0]),
            )
            .expect("memory vector should persist");

        let context = vec![ContextMessage {
            message_id: "ranked-user".to_owned(),
            role: "user".to_owned(),
            text: "Recuérdame cómo prefiero las respuestas".to_owned(),
        }];
        let embedding_request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {"metadata": {
                "source_type": "chat_memory_search",
                "source_id": "ranked-workflow",
                "content_sha256": "query-hash"
            }}
        });
        database
            .prepare_semantic_chat_turn(
                "ranked-workflow",
                &conversation.id,
                "ranked-user",
                "ranked-assistant",
                "ranked-query-task",
                "ranked-query-key",
                "Recuérdame cómo prefiero las respuestas",
                &embedding_request,
                &context,
                &[],
                false,
                false,
                &ConversationExecutionPreferences::default(),
                None,
            )
            .expect("semantic turn should persist");
        database
            .record_remote_state(
                "ranked-query-task",
                &completed_embedding("ranked-query-task", &[1.0, 0.0]),
            )
            .expect("query vector should persist");
        assert_eq!(
            database
                .semantic_chat_workflows_ready_to_continue()
                .expect("recoverable workflows should load"),
            vec!["ranked-query-task".to_owned()]
        );

        let matches = database
            .semantic_memory_matches("ranked-workflow")
            .expect("ranked memories should load");
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].memory.id, memory_id);
        assert_eq!(matches[0].score, 1.0);
        database
            .prepare_semantic_chat_submission(
                "ranked-workflow",
                "ranked-chat-task",
                "ranked-chat-key",
                &serde_json::json!({"inference_kind": "chat"}),
                &matches,
                &[],
            )
            .expect("final chat should be prepared");

        let view = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        assert_eq!(
            view.messages[1].broker_task_id.as_deref(),
            Some("ranked-chat-task")
        );
        let snapshot = database
            .task_context("ranked-chat-task")
            .expect("semantic context should be inspectable");
        assert_eq!(snapshot.strategy, "Ventana reciente + memoria semántica");
        assert_eq!(snapshot.sources[1].score, Some(1.0));
        assert_eq!(snapshot.sources[1].reason, "Coincidencia semántica alta");
        let workflow = database
            .semantic_chat_workflow_for_task("ranked-chat-task")
            .expect("workflow lookup should succeed")
            .expect("workflow should exist");
        assert_eq!(workflow.status, "submitted");
        cleanup(&database);
    }

    #[test]
    fn completed_memory_embedding_is_stored_with_model_and_dimensions() {
        let database = test_database();
        let (memory_id, _) = database
            .create_memory_item("Memoria vectorial", "fact", "normal", None)
            .expect("memory should be created");
        let request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {
                "prompt": "Memoria vectorial",
                "metadata": {
                    "source_type": "memory",
                    "source_id": memory_id,
                    "content_sha256": format!(
                        "{:x}",
                        Sha256::digest("Memoria vectorial".as_bytes())
                    )
                }
            }
        });
        database
            .prepare_broker_task("embedding-local-task", "embedding-key", &request)
            .expect("embedding task should persist");
        database
            .mark_orphaned(
                "embedding-local-task",
                "Broker AI devolvió HTTP 422: contrato inválido",
            )
            .expect("failed submission should be recorded");
        let failed_item = database
            .memory_item(&memory_id)
            .expect("memory should load");
        assert_eq!(failed_item.embedding_status, "failed");
        assert!(failed_item
            .embedding_error
            .as_deref()
            .is_some_and(|error| error.contains("HTTP 422")));
        let completed: TaskState = serde_json::from_value(serde_json::json!({
            "task_id": "embedding-remote-task",
            "status": "completed",
            "request_id": "embedding-request",
            "created_at": "2026-07-22T00:00:00Z",
            "updated_at": "2026-07-22T00:00:01Z",
            "execution_strategy": "single",
            "execution_preset": "fast",
            "selection_mode": "automatic",
            "progress": {},
            "result": {
                "inference_kind": "embedding",
                "embedding": [0.1, 0.2, 0.3],
                "model_used": {
                    "provider": "ollama",
                    "deployment": "local",
                    "model": "nomic-embed-text"
                }
            },
            "error": null
        }))
        .expect("completed embedding state should deserialize");
        database
            .record_remote_state("embedding-local-task", &completed)
            .expect("embedding should materialize");

        let item = database
            .memory_item(&memory_id)
            .expect("memory should load");
        assert_eq!(item.embedding_status, "ready");
        assert_eq!(
            item.embedding_model.as_deref(),
            Some("ollama/local/nomic-embed-text")
        );
        let connection = database.connect().expect("connection should open");
        let (dimensions, bytes): (i64, i64) = connection
            .query_row(
                "SELECT dimensions, length(vector_blob) FROM embedding_records
                 WHERE source_type = 'memory' AND source_id = ?1",
                params![memory_id],
                |row| Ok((row.get(0)?, row.get(1)?)),
            )
            .expect("embedding record should exist");
        assert_eq!(dimensions, 3);
        assert_eq!(bytes, 24);
        drop(connection);
        cleanup(&database);
    }

    #[test]
    fn semantic_memory_search_ranks_compatible_vectors_and_respects_scope() {
        fn completed_embedding(task_id: &str, model: &str, vector: &[f64]) -> TaskState {
            serde_json::from_value(serde_json::json!({
                "task_id": task_id,
                "status": "completed",
                "request_id": format!("request-{task_id}"),
                "created_at": "2026-07-22T00:00:00Z",
                "updated_at": "2026-07-22T00:00:01Z",
                "execution_strategy": "single",
                "execution_preset": "fast",
                "selection_mode": "automatic",
                "progress": {},
                "result": {
                    "inference_kind": "embedding",
                    "embedding": vector,
                    "model_used": {
                        "provider": "ollama",
                        "deployment": "local",
                        "model": model
                    }
                },
                "error": null
            }))
            .expect("embedding state should deserialize")
        }

        fn store_memory_embedding(
            database: &Database,
            memory_id: &str,
            task_id: &str,
            model: &str,
            vector: &[f64],
        ) {
            let content = database
                .memory_item(memory_id)
                .expect("memory should exist")
                .content;
            let request = serde_json::json!({
                "inference_kind": "embedding",
                "content": {
                    "prompt": content,
                    "metadata": {
                        "source_type": "memory",
                        "source_id": memory_id,
                        "content_sha256": format!("{:x}", Sha256::digest(content.as_bytes()))
                    }
                }
            });
            database
                .prepare_broker_task(task_id, &format!("key-{task_id}"), &request)
                .expect("memory embedding task should persist");
            database
                .record_remote_state(task_id, &completed_embedding(task_id, model, vector))
                .expect("memory embedding should materialize");
        }

        let database = test_database();
        let project = database
            .create_project("TFM", None)
            .expect("project should be created");
        let other_project = database
            .create_project("Otro", None)
            .expect("other project should be created");
        let (global_id, _) = database
            .create_memory_item("Prefiero respuestas breves", "preference", "normal", None)
            .expect("global memory should be created");
        let (scoped_id, _) = database
            .create_memory_item(
                "El TFM usa arquitectura durable",
                "fact",
                "normal",
                Some(&project.id),
            )
            .expect("scoped memory should be created");
        let (other_id, _) = database
            .create_memory_item(
                "Recuerdo de otro proyecto",
                "fact",
                "normal",
                Some(&other_project.id),
            )
            .expect("other memory should be created");
        let (different_model_id, _) = database
            .create_memory_item("Modelo incompatible", "fact", "normal", None)
            .expect("incompatible memory should be created");
        store_memory_embedding(&database, &global_id, "task-global", "nomic", &[1.0, 0.0]);
        store_memory_embedding(&database, &scoped_id, "task-scoped", "nomic", &[0.8, 0.2]);
        store_memory_embedding(&database, &other_id, "task-other", "nomic", &[1.0, 0.0]);
        store_memory_embedding(
            &database,
            &different_model_id,
            "task-different-model",
            "other-model",
            &[1.0, 0.0],
        );

        let search_id = "memory-search-test";
        let search_task_id = "memory-search-task";
        let request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {"metadata": {
                "source_type": "memory_search",
                "source_id": search_id,
                "content_sha256": "search-hash"
            }}
        });
        database
            .prepare_memory_search(
                search_id,
                "respuestas concisas",
                Some(&project.id),
                search_task_id,
                "memory-search-key",
                &request,
            )
            .expect("search should persist atomically");
        database
            .record_remote_state(
                search_task_id,
                &completed_embedding(search_task_id, "nomic", &[1.0, 0.0]),
            )
            .expect("search embedding should materialize");

        let search = database
            .memory_search(search_id)
            .expect("search should load");
        assert_eq!(search.status, "completed");
        assert_eq!(search.results.len(), 2);
        assert_eq!(search.results[0].memory_id, global_id);
        assert_eq!(search.results[1].memory_id, scoped_id);
        assert!(search.results[0].score > search.results[1].score);
        assert!(search
            .results
            .iter()
            .all(|result| result.memory_id != other_id && result.memory_id != different_model_id));
        cleanup(&database);
    }

    #[test]
    fn generated_conversation_summary_is_an_inactive_draft() {
        let database = test_database();
        let conversation = database
            .create_conversation("Conversación larga", None)
            .expect("conversation should be created");
        let connection = database.connect().expect("connection should open");
        connection
            .execute(
                "INSERT INTO messages(
                    id, conversation_id, role, status, sequence_no
                 ) VALUES ('summary-source', ?1, 'user', 'complete', 1)",
                params![conversation.id],
            )
            .expect("source message should be inserted");
        connection
            .execute(
                "INSERT INTO message_parts(
                    id, message_id, ordinal, kind, content_text
                 ) VALUES (
                    'summary-source-part', 'summary-source', 0, 'text',
                    'Necesito conservar las decisiones importantes.'
                 )",
                [],
            )
            .expect("source part should be inserted");
        drop(connection);

        let request = serde_json::json!({
            "inference_kind": "chat",
            "content": {"metadata": {
                "source_type": "conversation_summary",
                "source_id": "summary-draft"
            }}
        });
        database
            .prepare_conversation_summary(
                &conversation.id,
                "summary-draft",
                "summary-task",
                "summary-key",
                &request,
                1,
            )
            .expect("summary should persist atomically");
        database
            .record_remote_state(
                "summary-task",
                &serde_json::from_value::<TaskState>(serde_json::json!({
                    "task_id": "remote-summary-task",
                    "status": "completed",
                    "request_id": null,
                    "created_at": "2026-07-24T12:00:00Z",
                    "updated_at": "2026-07-24T12:00:01Z",
                    "execution_strategy": "single",
                    "execution_preset": "fast",
                    "selection_mode": "adaptive",
                    "progress": {},
                    "result": {
                        "result_markdown": "## Decisiones\n\nConservar las decisiones importantes."
                    },
                    "error": null
                }))
                .expect("completed state should be valid"),
            )
            .expect("completed generation should materialize");

        let overview = database
            .conversation_summary_overview(&conversation.id)
            .expect("summary overview should load");
        let candidate = overview.candidate.expect("draft should be visible");
        assert_eq!(candidate.status, "draft");
        assert_eq!(
            candidate.draft_text.as_deref(),
            Some("## Decisiones\n\nConservar las decisiones importantes.")
        );
        assert!(
            overview.active.is_none(),
            "a draft must never become active"
        );
        cleanup(&database);
    }

    #[test]
    fn approved_edited_summary_compacts_context_without_deleting_messages() {
        let database = test_database();
        let conversation = database
            .create_conversation("Decisiones del proyecto", None)
            .expect("conversation should be created");
        let connection = database.connect().expect("connection should open");
        for (id, sequence, role, text) in [
            ("summary-old-user", 1_i64, "user", "Usaremos SQLite."),
            (
                "summary-old-assistant",
                2_i64,
                "assistant",
                "De acuerdo, SQLite será la base local.",
            ),
            (
                "summary-new-user",
                3_i64,
                "user",
                "Además, la interfaz será en español.",
            ),
        ] {
            connection
                .execute(
                    "INSERT INTO messages(
                        id, conversation_id, role, status, sequence_no
                     ) VALUES (?1, ?2, ?3, 'complete', ?4)",
                    params![id, conversation.id, role, sequence],
                )
                .expect("message should be inserted");
            connection
                .execute(
                    "INSERT INTO message_parts(
                        id, message_id, ordinal, kind, content_text
                     ) VALUES (?1, ?2, 0, 'text', ?3)",
                    params![format!("{id}-part"), id, text],
                )
                .expect("message part should be inserted");
        }
        connection
            .execute(
                "INSERT INTO conversation_summaries(
                    id, conversation_id, source_through_sequence,
                    status, draft_text
                 ) VALUES (
                    'summary-editable', ?1, 2, 'draft',
                    'Borrador generado automáticamente.'
                 )",
                params![conversation.id],
            )
            .expect("draft should be inserted");
        drop(connection);

        database
            .update_conversation_summary_draft(
                "summary-editable",
                "Decisión aprobada: usar SQLite como base local.",
            )
            .expect("draft should be editable");
        database
            .approve_conversation_summary("summary-editable")
            .expect("draft should be approved");

        let context = database
            .recent_context(&conversation.id, 12, 12_000)
            .expect("context should load");
        assert_eq!(context.len(), 2);
        assert_eq!(context[0].role, "summary");
        assert_eq!(
            context[0].text,
            "Decisión aprobada: usar SQLite como base local."
        );
        assert_eq!(context[1].message_id, "summary-new-user");

        let conversation_view = database
            .conversation_view(&conversation.id)
            .expect("conversation should load");
        assert_eq!(
            conversation_view.messages.len(),
            3,
            "approving a summary must not delete original messages"
        );

        let mut traced_context = context;
        traced_context.push(ContextMessage {
            message_id: "summary-trace-current".to_owned(),
            role: "user".to_owned(),
            text: "¿Qué decisiones siguen vigentes?".to_owned(),
        });
        database
            .prepare_chat_turn(
                &conversation.id,
                "summary-trace-current",
                "summary-trace-assistant",
                "summary-trace-task",
                "summary-trace-key",
                "¿Qué decisiones siguen vigentes?",
                &serde_json::json!({"inference_kind": "chat"}),
                &traced_context,
                &[],
                &[],
                &[],
            )
            .expect("chat with approved summary should be prepared");
        let trace = database
            .task_context("summary-trace-task")
            .expect("context trace should load");
        assert_eq!(trace.strategy, "Resumen aprobado + ventana reciente");
        assert_eq!(trace.sources[0].kind, "summary");
        assert_eq!(trace.sources[0].label, "Resumen aprobado");
        assert_eq!(
            trace.sources[0].reason,
            "Resumen revisado y aprobado por ti"
        );
        cleanup(&database);
    }

    #[test]
    fn conversation_summary_input_is_bounded_and_leaves_newer_messages_uncovered() {
        fn complete_turn(
            database: &Database,
            conversation_id: &str,
            suffix: &str,
            user_text: &str,
            assistant_text: &str,
        ) {
            let user_message_id = format!("bounded-user-{suffix}");
            let assistant_message_id = format!("bounded-assistant-{suffix}");
            let task_id = format!("bounded-task-{suffix}");
            let context = vec![ContextMessage {
                message_id: user_message_id.clone(),
                role: "user".to_owned(),
                text: user_text.to_owned(),
            }];
            database
                .prepare_chat_turn(
                    conversation_id,
                    &user_message_id,
                    &assistant_message_id,
                    &task_id,
                    &format!("bounded-key-{suffix}"),
                    user_text,
                    &serde_json::json!({"inference_kind": "chat"}),
                    &context,
                    &[],
                    &[],
                    &[],
                )
                .expect("turn should be prepared");
            database
                .record_remote_state(
                    &task_id,
                    &serde_json::from_value::<TaskState>(serde_json::json!({
                        "task_id": format!("remote-{task_id}"),
                        "status": "completed",
                        "request_id": null,
                        "created_at": "2026-07-24T12:00:00Z",
                        "updated_at": "2026-07-24T12:00:01Z",
                        "execution_strategy": "single",
                        "execution_preset": "fast",
                        "selection_mode": "adaptive",
                        "progress": {},
                        "result": {"result_markdown": assistant_text},
                        "error": null
                    }))
                    .expect("completed state should be valid"),
                )
                .expect("turn should complete");
        }

        let database = test_database();
        let conversation = database
            .create_conversation("Historial extenso", None)
            .expect("conversation should be created");
        complete_turn(
            &database,
            &conversation.id,
            "one",
            "AAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBB",
        );
        complete_turn(
            &database,
            &conversation.id,
            "two",
            "CCCCCCCCCCCCCCCCCCCCCCCC",
            "DDDDDDDDDDDDDDDDDDDDDDDD",
        );

        let input = database
            .conversation_summary_input(&conversation.id, 60)
            .expect("bounded summary input should load");

        assert_eq!(input.messages.len(), 2);
        assert_eq!(input.source_through_sequence, 2);
        assert_eq!(input.included_message_count, 2);
        assert_eq!(input.remaining_message_count, 2);
        assert_eq!(input.character_count, 48);
        assert!(input.character_count <= 60);
        cleanup(&database);
    }

    #[test]
    fn next_summary_input_merges_the_approved_summary_with_only_new_messages() {
        fn complete_turn(
            database: &Database,
            conversation_id: &str,
            suffix: &str,
            user_text: &str,
            assistant_text: &str,
        ) {
            let user_message_id = format!("incremental-user-{suffix}");
            let assistant_message_id = format!("incremental-assistant-{suffix}");
            let task_id = format!("incremental-task-{suffix}");
            database
                .prepare_chat_turn(
                    conversation_id,
                    &user_message_id,
                    &assistant_message_id,
                    &task_id,
                    &format!("incremental-key-{suffix}"),
                    user_text,
                    &serde_json::json!({"inference_kind": "chat"}),
                    &[ContextMessage {
                        message_id: user_message_id.clone(),
                        role: "user".to_owned(),
                        text: user_text.to_owned(),
                    }],
                    &[],
                    &[],
                    &[],
                )
                .expect("turn should be prepared");
            database
                .record_remote_state(
                    &task_id,
                    &serde_json::from_value::<TaskState>(serde_json::json!({
                        "task_id": format!("remote-{task_id}"),
                        "status": "completed",
                        "request_id": null,
                        "created_at": "2026-07-24T12:00:00Z",
                        "updated_at": "2026-07-24T12:00:01Z",
                        "execution_strategy": "single",
                        "execution_preset": "fast",
                        "selection_mode": "adaptive",
                        "progress": {},
                        "result": {"result_markdown": assistant_text},
                        "error": null
                    }))
                    .expect("completed state should be valid"),
                )
                .expect("turn should complete");
        }

        let database = test_database();
        let conversation = database
            .create_conversation("Resumen incremental", None)
            .expect("conversation should be created");
        complete_turn(
            &database,
            &conversation.id,
            "old",
            "AAAAAAAAAAAAAAAAAAAAAAAA",
            "BBBBBBBBBBBBBBBBBBBBBBBB",
        );
        complete_turn(
            &database,
            &conversation.id,
            "new",
            "CCCCCCCCCCCCCCCCCCCCCCCC",
            "DDDDDDDDDDDDDDDDDDDDDDDD",
        );

        let request = serde_json::json!({
            "inference_kind": "chat",
            "content": {"metadata": {
                "source_type": "conversation_summary",
                "source_id": "incremental-summary"
            }}
        });
        database
            .prepare_conversation_summary(
                &conversation.id,
                "incremental-summary",
                "incremental-summary-task",
                "incremental-summary-key",
                &request,
                2,
            )
            .expect("summary should be prepared");
        database
            .record_remote_state(
                "incremental-summary-task",
                &serde_json::from_value::<TaskState>(serde_json::json!({
                    "task_id": "remote-incremental-summary",
                    "status": "completed",
                    "request_id": null,
                    "created_at": "2026-07-24T12:00:00Z",
                    "updated_at": "2026-07-24T12:00:01Z",
                    "execution_strategy": "single",
                    "execution_preset": "fast",
                    "selection_mode": "adaptive",
                    "progress": {},
                    "result": {"result_markdown": "Resumen previo base."},
                    "error": null
                }))
                .expect("completed summary state should be valid"),
            )
            .expect("summary draft should materialize");
        database
            .approve_conversation_summary("incremental-summary")
            .expect("summary should be approved");
        let overview = database
            .conversation_summary_overview(&conversation.id)
            .expect("coverage should be visible");
        assert_eq!(overview.total_message_count, 4);
        assert_eq!(overview.active_covered_message_count, 2);
        assert_eq!(overview.remaining_message_count, 2);
        assert_eq!(overview.candidate_covered_message_count, None);

        let input = database
            .conversation_summary_input(&conversation.id, 50)
            .expect("incremental input should load");

        assert_eq!(input.messages.len(), 2);
        assert_eq!(input.messages[0].role, "summary");
        assert_eq!(input.messages[0].text, "Resumen previo base.");
        assert_eq!(input.messages[1].message_id, "incremental-user-new");
        assert_eq!(input.source_through_sequence, 3);
        assert_eq!(input.included_message_count, 1);
        assert_eq!(input.remaining_message_count, 1);
        assert_eq!(input.character_count, 44);
        cleanup(&database);
    }

    #[test]
    fn document_chunk_selection_is_relevant_bounded_and_traceable() {
        let database = test_database();
        let managed_root = std::env::temp_dir().join(format!(
            "chatygpt-document-source-test-{}",
            Uuid::new_v4().simple()
        ));
        let managed_file = managed_root.join("prices.csv");
        std::fs::create_dir_all(&managed_root).expect("managed root should exist");
        std::fs::write(&managed_file, b"date,open,high,low,close")
            .expect("managed file should exist");
        let conversation = database
            .create_conversation("Análisis de precios", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                managed_file.to_str().expect("managed path should be UTF-8"),
                "prices.csv",
                Some("text/csv"),
                9_000_000,
                "prices-hash",
            )
            .expect("attachment should be registered");
        database
            .update_attachment_ingestion(
                &attachment.id,
                "ready",
                Some("broker-prices"),
                Some("document"),
                Some("docling"),
                None,
                None,
            )
            .expect("attachment should become ready");
        database
            .replace_attachment_chunks(
                &attachment.id,
                &[
                    "Introducción general y procedencia del fichero.".to_owned(),
                    "Columnas OHLC de precios. Calcular media y mediana del cierre.".to_owned(),
                    "Notas finales sobre licencias y autores.".to_owned(),
                ],
            )
            .expect("chunks should be stored");

        let selected = database
            .select_attachment_chunks(
                &conversation.id,
                std::slice::from_ref(&attachment.id),
                "calcula la media y mediana de los precios OHLC",
                2,
                80,
            )
            .expect("chunks should be selected");

        assert_eq!(selected.len(), 1);
        assert_eq!(selected[0].attachment_name, "prices.csv");
        assert_eq!(selected[0].ordinal, 1);
        assert!(selected[0].score > 0.0);
        assert_eq!(selected[0].reason, "Coincidencia con la pregunta");
        assert!(
            selected
                .iter()
                .map(|chunk| chunk.text.chars().count())
                .sum::<usize>()
                <= 80
        );

        let context = vec![ContextMessage {
            message_id: "document-user".to_owned(),
            role: "user".to_owned(),
            text: "calcula la media y mediana de los precios OHLC".to_owned(),
        }];
        database
            .prepare_chat_turn(
                &conversation.id,
                "document-user",
                "document-assistant",
                "document-task",
                "document-key",
                "calcula la media y mediana de los precios OHLC",
                &serde_json::json!({"inference_kind": "chat"}),
                &context,
                &[],
                &selected,
                std::slice::from_ref(&attachment.id),
            )
            .expect("turn with document chunks should be prepared");
        let trace = database
            .task_context("document-task")
            .expect("document context should be inspectable");
        assert_eq!(trace.strategy, "Ventana reciente + documentos");
        assert_eq!(trace.sources[1].kind, "attachment_chunk");
        assert_eq!(trace.sources[1].label, "prices.csv · fragmento 2");
        assert_eq!(trace.sources[1].reason, "Coincidencia con la pregunta");
        assert!(trace.sources[1].source_available);
        let source_reference = trace.sources[1]
            .source_reference
            .as_deref()
            .expect("document source should expose an opaque reference");
        let source = database
            .context_source_file("document-task", source_reference)
            .expect("document source should resolve");
        assert_eq!(source.local_path, managed_file.to_string_lossy());
        assert_eq!(source.display_name, "prices.csv");
        assert!(matches!(
            database.context_source_file("another-task", source_reference),
            Err(AppError::NotFound(_))
        ));
        cleanup(&database);
        std::fs::remove_dir_all(managed_root).expect("managed test files should be removed");
    }

    #[test]
    fn global_document_request_prefers_structure_over_cosine_winners() {
        let database = test_database();
        let managed_root = std::env::temp_dir().join(format!(
            "chatygpt-global-document-test-{}",
            Uuid::new_v4().simple()
        ));
        let managed_file = managed_root.join("book.pdf");
        std::fs::create_dir_all(&managed_root).expect("managed root should exist");
        std::fs::write(&managed_file, b"book").expect("managed file should exist");
        let conversation = database
            .create_conversation("Resumen de libro", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                managed_file.to_str().expect("managed path should be UTF-8"),
                "book.pdf",
                Some("application/pdf"),
                4,
                "book-hash",
            )
            .expect("attachment should be registered");
        database
            .replace_attachment_chunks(
                &attachment.id,
                &[
                    "Título y autor de la obra.".to_owned(),
                    "Table of contents. Chapter 1: Origins. Chapter 2: Methods.".to_owned(),
                    "Preface. This book presents the history and foundations of pattern recognition."
                        .to_owned(),
                    "Un detalle aislado acerca de un algoritmo.".to_owned(),
                    "Otro detalle técnico.".to_owned(),
                    "Conclusion. The field combines statistical learning and computation."
                        .to_owned(),
                ],
            )
            .expect("chunks should be stored");

        let selected = database
            .select_attachment_chunks(
                &conversation.id,
                std::slice::from_ref(&attachment.id),
                "Dime de qué va el libro y hazme un resumen",
                4,
                20_000,
            )
            .expect("global view should be selected");

        assert_eq!(selected.len(), 4);
        assert_eq!(selected[0].ordinal, 1);
        assert!(selected[0].reason.contains("índice"));
        assert_eq!(selected[1].ordinal, 2);
        assert!(selected[1].reason.contains("prefacio"));
        assert_eq!(selected[2].ordinal, 5);
        assert!(selected[2].reason.contains("conclusiones"));
        assert_eq!(selected[3].ordinal, 0);
        assert!(selected
            .iter()
            .all(|chunk| chunk.reason.starts_with("Vista global del documento")));
        cleanup(&database);
        std::fs::remove_dir_all(managed_root).expect("managed test files should be removed");
    }

    #[test]
    fn specific_document_request_keeps_relevance_ranking() {
        assert!(!super::is_global_document_request(
            "¿Qué fórmula utiliza el capítulo 7 para la varianza?"
        ));
        assert!(super::is_global_document_request(
            "¿De qué trata este documento?"
        ));
        assert!(super::is_global_document_request(
            "Hazme un resumen del libro"
        ));
        assert!(!super::is_global_document_request(
            "Haz un resumen de la sección sobre regresión"
        ));
    }

    #[test]
    fn attachment_exposes_durable_document_context_progress_and_chunk_count() {
        let database = test_database();
        let conversation = database
            .create_conversation("Contexto documental visible", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "managed/guide.pdf",
                "guide.pdf",
                Some("application/pdf"),
                120_000,
                "guide-hash",
            )
            .expect("attachment should be registered");

        assert_eq!(attachment.context_status, "pending");
        assert_eq!(attachment.chunk_count, 0);
        assert_eq!(attachment.indexed_characters, 0);
        assert_eq!(attachment.semantic_index_status, "unavailable");
        database
            .mark_attachment_context_preparing(&attachment.id)
            .expect("context preparation should start");
        let preparing = database
            .attachment_view(&attachment.id)
            .expect("attachment should be visible");
        assert_eq!(preparing.context_status, "preparing");

        database
            .replace_attachment_chunks(
                &attachment.id,
                &[
                    "Primer fragmento del documento.".to_owned(),
                    "Segundo fragmento del documento.".to_owned(),
                ],
            )
            .expect("chunks should be stored");
        let ready = database
            .attachment_view(&attachment.id)
            .expect("attachment should be visible");
        assert_eq!(ready.context_status, "ready");
        assert_eq!(ready.chunk_count, 2);
        assert_eq!(
            ready.indexed_characters,
            "Primer fragmento del documento.".chars().count() as i64
                + "Segundo fragmento del documento.".chars().count() as i64
        );
        assert_eq!(ready.semantic_indexed_chunks, 0);
        assert_eq!(ready.semantic_index_status, "pending");
        assert!(ready.context_error.is_none());
        cleanup(&database);
    }

    #[test]
    fn document_selection_includes_nearby_context_after_relevant_chunks() {
        let database = test_database();
        let conversation = database
            .create_conversation("Contexto vecino", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "managed/guide.md",
                "guide.md",
                Some("text/markdown"),
                200,
                "guide-neighbor-hash",
            )
            .expect("attachment should be registered");
        database
            .replace_attachment_chunks(
                &attachment.id,
                &[
                    "Capítulo: indicadores estadísticos.".to_owned(),
                    "La mediana del cierre reduce el efecto de valores extremos.".to_owned(),
                    "El apéndice describe las fuentes de datos.".to_owned(),
                ],
            )
            .expect("chunks should be stored");

        let selected = database
            .select_attachment_chunks(
                &conversation.id,
                std::slice::from_ref(&attachment.id),
                "mediana del cierre",
                2,
                500,
            )
            .expect("chunks should be selected");

        assert_eq!(selected.len(), 2);
        assert_eq!(selected[0].ordinal, 1);
        assert_eq!(selected[0].reason, "Coincidencia con la pregunta");
        assert_eq!(selected[1].ordinal, 0);
        assert_eq!(
            selected[1].reason,
            "Contexto próximo al fragmento relevante"
        );
        cleanup(&database);
    }

    #[test]
    fn hybrid_document_selection_uses_compatible_chunk_embeddings() {
        let database = test_database();
        let conversation = database
            .create_conversation("Recuperación híbrida", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "managed/hybrid.md",
                "hybrid.md",
                Some("text/markdown"),
                200,
                "hybrid-hash",
            )
            .expect("attachment should be registered");
        database
            .replace_attachment_chunks(
                &attachment.id,
                &[
                    "Contenido sobre licencias.".to_owned(),
                    "Explicación del cálculo estadístico.".to_owned(),
                ],
            )
            .expect("chunks should be stored");
        let connection = database.connect().expect("database should connect");
        let chunks = {
            let mut statement = connection
                .prepare(
                    "SELECT id, content_sha256 FROM attachment_chunks
                     WHERE attachment_id = ?1 ORDER BY ordinal",
                )
                .expect("chunk query should prepare");
            statement
                .query_map(params![attachment.id], |row| {
                    Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
                })
                .expect("chunks should load")
                .collect::<Result<Vec<_>, _>>()
                .expect("chunks should collect")
        };
        let vector_blob = |values: &[f64]| {
            values
                .iter()
                .flat_map(|value| value.to_le_bytes())
                .collect::<Vec<_>>()
        };
        connection
            .execute(
                "INSERT INTO embedding_records(
                    id, source_type, source_id, chunk_index, model,
                    dimensions, vector_blob, content_sha256
                 ) VALUES
                    ('query-vector', 'chat_memory_search', 'hybrid-query', 0,
                     'ollama/local/nomic', 2, ?1, 'query-hash'),
                    ('chunk-vector-0', 'attachment_chunk', ?2, 0,
                     'ollama/local/nomic', 2, ?3, ?4),
                    ('chunk-vector-1', 'attachment_chunk', ?5, 0,
                     'ollama/local/nomic', 2, ?6, ?7)",
                params![
                    vector_blob(&[1.0, 0.0]),
                    &chunks[0].0,
                    vector_blob(&[0.0, 1.0]),
                    &chunks[0].1,
                    &chunks[1].0,
                    vector_blob(&[0.95, 0.05]),
                    &chunks[1].1
                ],
            )
            .expect("vectors should persist");
        drop(connection);

        let selected = database
            .select_attachment_chunks_hybrid(
                &conversation.id,
                std::slice::from_ref(&attachment.id),
                "consulta sin coincidencias literales",
                2,
                500,
                "hybrid-query",
            )
            .expect("hybrid selection should succeed");

        assert_eq!(selected[0].ordinal, 1);
        assert_eq!(selected[0].reason, "Coincidencia semántica");
        let view = database
            .attachment_view(&attachment.id)
            .expect("attachment view should load");
        assert_eq!(view.semantic_indexed_chunks, 2);
        assert_eq!(view.semantic_index_status, "ready");
        assert_eq!(
            view.semantic_index_model.as_deref(),
            Some("ollama/local/nomic")
        );
        cleanup(&database);
    }

    #[test]
    fn document_embedding_queue_is_sequential_and_skips_failed_chunks() {
        let database = test_database();
        let conversation = database
            .create_conversation("Cola semántica", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "managed/queue.md",
                "queue.md",
                Some("text/markdown"),
                100,
                "queue-hash",
            )
            .expect("attachment should be registered");
        database
            .replace_attachment_chunks(
                &attachment.id,
                &[
                    "Primer fragmento.".to_owned(),
                    "Segundo fragmento.".to_owned(),
                ],
            )
            .expect("chunks should be stored");
        let first = database
            .next_attachment_chunk_for_embedding(&attachment.id, false)
            .expect("queue should load")
            .expect("first chunk should be available");
        let request = serde_json::json!({
            "inference_kind": "embedding",
            "content": {"metadata": {
                "source_type": "attachment_chunk",
                "source_id": first.id.clone(),
                "content_sha256": first.content_sha256.clone()
            }}
        });
        database
            .prepare_broker_task("queue-task", "queue-key", &request)
            .expect("active task should persist");
        assert!(database
            .next_attachment_chunk_for_embedding(&attachment.id, false)
            .expect("queue should load")
            .is_none());
        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE broker_tasks
                 SET local_state = 'terminal', remote_status = 'failed'
                 WHERE id = 'queue-task'",
                [],
            )
            .expect("task should fail");

        let next = database
            .next_attachment_chunk_for_embedding(&attachment.id, false)
            .expect("queue should load")
            .expect("second chunk should remain available");
        assert_ne!(next.id, first.id);
        let retry = database
            .next_attachment_chunk_for_embedding(&attachment.id, true)
            .expect("retry queue should load")
            .expect("failed chunk should be retryable");
        assert_eq!(retry.id, first.id);
        cleanup(&database);
    }

    #[test]
    fn document_context_failure_does_not_invalidate_upload_and_can_be_retried() {
        let database = test_database();
        let conversation = database
            .create_conversation("Reintento de contexto", None)
            .expect("conversation should be created");
        let attachment = database
            .register_attachment(
                &conversation.id,
                "managed/manual.pdf",
                "manual.pdf",
                Some("application/pdf"),
                240_000,
                "manual-hash",
            )
            .expect("attachment should be registered");
        database
            .update_attachment_ingestion(
                &attachment.id,
                "ready",
                Some("broker-manual"),
                Some("document"),
                Some("docling"),
                None,
                None,
            )
            .expect("upload should be ready");
        database
            .record_attachment_context_failure(
                &attachment.id,
                &serde_json::json!({"message": "falló la descarga del Markdown"}),
            )
            .expect("context failure should be recorded");

        let failed = database
            .attachment_view(&attachment.id)
            .expect("attachment should remain visible");
        assert_eq!(failed.ingestion_status, "ready");
        assert_eq!(failed.context_status, "failed");
        assert_eq!(
            failed.context_error,
            Some(serde_json::json!({"message": "falló la descarga del Markdown"}))
        );

        database
            .reset_attachment_context_for_retry(&attachment.id)
            .expect("context retry should be accepted");
        let pending = database
            .attachment_view(&attachment.id)
            .expect("attachment should remain visible");
        assert_eq!(pending.ingestion_status, "ready");
        assert_eq!(pending.context_status, "pending");
        assert!(pending.context_error.is_none());
        cleanup(&database);
    }

    #[test]
    fn scheduled_task_templates_are_durable_reusable_and_audited() {
        let database = test_database();
        let template = database
            .create_scheduled_task_template(
                "Resumen semanal",
                "Resume los avances y bloqueos.",
                "weekly",
            )
            .expect("template should be created");
        assert_eq!(template.name, "Resumen semanal");
        assert_eq!(template.schedule_expression, "weekly");

        let listed = database
            .list_scheduled_task_templates()
            .expect("templates should be listed");
        assert_eq!(listed.len(), 1);
        assert_eq!(listed[0].prompt, "Resume los avances y bloqueos.");
        assert!(matches!(
            database.delete_scheduled_task_template(&template.id, false),
            Err(AppError::Validation(_))
        ));
        database
            .delete_scheduled_task_template(&template.id, true)
            .expect("confirmed deletion should succeed");
        assert!(database
            .list_scheduled_task_templates()
            .expect("templates should be listed")
            .is_empty());

        let events: Vec<String> = database
            .connect()
            .expect("database should connect")
            .prepare(
                "SELECT event_type FROM audit_events
                 WHERE event_type LIKE 'scheduled_template.%'
                 ORDER BY id",
            )
            .expect("audit query should prepare")
            .query_map([], |row| row.get(0))
            .expect("audit query should run")
            .collect::<Result<_, _>>()
            .expect("audit events should collect");
        assert_eq!(
            events,
            vec!["scheduled_template.created", "scheduled_template.deleted"]
        );
        cleanup(&database);
    }

    #[test]
    fn manual_scheduled_run_preserves_the_future_schedule_and_blocks_overlap() {
        let database = test_database();
        let conversation = database
            .create_conversation("Seguimiento manual", None)
            .expect("conversation should be created");
        let scheduled = database
            .create_scheduled_task(
                "Informe diario",
                &conversation.id,
                "Resume las novedades.",
                "2099-01-01T10:00:00.000Z",
                "Atlantic/Canary",
                "daily",
                true,
            )
            .expect("schedule should be created");
        assert!(matches!(
            database.claim_scheduled_task_now(&scheduled.id, false),
            Err(AppError::Validation(_))
        ));

        let manual = database
            .claim_scheduled_task_now(&scheduled.id, true)
            .expect("manual run should be claimed");
        assert_eq!(manual.scheduled_task_id, scheduled.id);
        assert_eq!(manual.conversation_id, Some(conversation.id));
        assert!(matches!(
            database.claim_scheduled_task_now(&scheduled.id, true),
            Err(AppError::Conflict(_))
        ));

        let listed = database
            .list_scheduled_tasks()
            .expect("schedule should remain visible");
        assert!(listed[0].enabled);
        assert_eq!(listed[0].next_run_at, scheduled.next_run_at);
        assert_eq!(listed[0].runs[0].id, manual.run_id);
        assert_eq!(listed[0].runs[0].status, "claimed");
        let audited: bool = database
            .connect()
            .expect("database should connect")
            .query_row(
                "SELECT EXISTS(
                    SELECT 1 FROM audit_events
                    WHERE event_type = 'scheduled_run.manual_requested'
                )",
                [],
                |row| row.get(0),
            )
            .expect("audit event should be queryable");
        assert!(audited);
        cleanup(&database);
    }

    #[test]
    fn published_workflow_can_be_scheduled_claimed_and_reconciled() {
        let database = test_database();
        let workflow = database
            .create_workflow("Informe encadenado", None)
            .expect("workflow should be created");
        database
            .publish_workflow(&workflow.summary.id)
            .expect("workflow should publish");
        assert!(matches!(
            database.create_scheduled_workflow(
                "Informe nocturno",
                &workflow.summary.id,
                "Resume la actividad de hoy.",
                "2099-01-01T22:00:00.000Z",
                "Atlantic/Canary",
                "daily",
                false,
            ),
            Err(AppError::Validation(_))
        ));
        let scheduled = database
            .create_scheduled_workflow(
                "Informe nocturno",
                &workflow.summary.id,
                "Resume la actividad de hoy.",
                "2099-01-01T22:00:00.000Z",
                "Atlantic/Canary",
                "daily",
                true,
            )
            .expect("published workflow should be scheduled");
        assert_eq!(scheduled.target_kind, "workflow");
        assert_eq!(
            scheduled.workflow_id.as_deref(),
            Some(workflow.summary.id.as_str())
        );
        assert_eq!(
            scheduled.workflow_name.as_deref(),
            Some("Informe encadenado")
        );
        assert_eq!(scheduled.workflow_version_no, Some(1));
        assert!(scheduled.conversation_id.is_none());
        database
            .publish_workflow(&workflow.summary.id)
            .expect("a later workflow version should publish");

        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE scheduled_tasks
                 SET next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 minute')
                 WHERE id = ?1",
                params![scheduled.id],
            )
            .expect("workflow schedule should become due");
        let claim = database
            .claim_due_scheduled_task()
            .expect("claim should succeed")
            .expect("workflow schedule should be due");
        assert_eq!(claim.target_kind, "workflow");
        assert_eq!(
            claim.workflow_id.as_deref(),
            Some(workflow.summary.id.as_str())
        );
        assert!(claim.workflow_version_id.is_some());
        assert_eq!(claim.prompt, "Resume la actividad de hoy.");

        let workflow_run = database
            .create_workflow_run_from_version(
                &workflow.summary.id,
                claim
                    .workflow_version_id
                    .as_deref()
                    .expect("version should be frozen"),
                &claim.prompt,
            )
            .expect("workflow run should be durable");
        assert_eq!(
            database
                .workflow_run(&workflow_run.run_id)
                .expect("workflow run should load")
                .version_no,
            1,
            "the schedule must keep the version confirmed by the user"
        );
        database
            .start_scheduled_workflow_run(&claim.run_id, &workflow_run.run_id)
            .expect("scheduled run should link to workflow run");
        database
            .update_workflow_run_status(
                &workflow_run.run_id,
                "completed",
                Some(&serde_json::json!({"Resultado": "Informe listo"})),
                None,
            )
            .expect("workflow should complete");
        assert_eq!(
            database
                .reconcile_scheduled_runs()
                .expect("scheduler should reconcile"),
            1
        );
        let reloaded = database
            .list_scheduled_tasks()
            .expect("schedule should reload");
        assert_eq!(reloaded[0].runs[0].status, "completed");
        assert_eq!(
            reloaded[0].runs[0].workflow_run_id.as_deref(),
            Some(workflow_run.run_id.as_str())
        );
        assert_eq!(
            reloaded[0].runs[0]
                .result
                .as_ref()
                .and_then(|value| value.pointer("/outputs/Resultado"))
                .and_then(serde_json::Value::as_str),
            Some("Informe listo")
        );
        cleanup(&database);
    }

    #[test]
    fn scheduled_history_is_filtered_sorted_and_paginated_in_sqlite() {
        let database = test_database();
        let conversation = database
            .create_conversation("Historial extenso", None)
            .expect("conversation should be created");
        let scheduled = database
            .create_scheduled_task(
                "Informe histórico",
                &conversation.id,
                "Resume el periodo.",
                "2099-01-01T10:00:00.000Z",
                "Atlantic/Canary",
                "weekly",
                true,
            )
            .expect("schedule should be created");
        let connection = database.connect().expect("database should connect");
        for index in 0..23 {
            let status = if index % 2 == 0 {
                "completed"
            } else {
                "failed"
            };
            let timestamp = format!("2026-07-{:02}T10:00:00.000Z", index + 1);
            connection
                .execute(
                    "INSERT INTO scheduled_runs(
                        id, scheduled_task_id, due_at, claim_key, status, attempt,
                        result_json, created_at, updated_at
                     ) VALUES (?1, ?2, ?3, ?4, ?5, 1, '{}', ?3, ?3)",
                    params![
                        format!("history-run-{index:02}"),
                        scheduled.id,
                        timestamp,
                        format!("history-claim-{index:02}"),
                        status
                    ],
                )
                .expect("history row should persist");
        }
        drop(connection);

        let newest_page = database
            .scheduled_run_page(&scheduled.id, "all", "all", "newest", 2, 10)
            .expect("second page should load");
        assert_eq!(newest_page.total, 23);
        assert_eq!(newest_page.page, 2);
        assert_eq!(newest_page.items.len(), 10);
        assert_eq!(newest_page.items[0].id, "history-run-12");
        assert_eq!(newest_page.items[9].id, "history-run-03");

        let oldest_last_page = database
            .scheduled_run_page(&scheduled.id, "all", "all", "oldest", 3, 10)
            .expect("last page should load");
        assert_eq!(oldest_last_page.items.len(), 3);
        assert_eq!(oldest_last_page.items[0].id, "history-run-20");
        assert_eq!(oldest_last_page.items[2].id, "history-run-22");

        let failed = database
            .scheduled_run_page(&scheduled.id, "failed", "all", "newest", 1, 10)
            .expect("failed history should load");
        assert_eq!(failed.total, 11);
        assert!(failed.items.iter().all(|run| run.status == "failed"));
        assert!(matches!(
            database.scheduled_run_page(&scheduled.id, "all", "all", "newest", 1, 11),
            Err(AppError::Validation(_))
        ));
        cleanup(&database);
    }

    #[test]
    fn scheduled_task_is_confirmed_pauseable_and_claimed_exactly_once() {
        let database = test_database();
        let conversation = database
            .create_conversation("Informe programado", None)
            .expect("conversation should be created");
        let scheduled = database
            .create_scheduled_task(
                "Preparar informe",
                &conversation.id,
                "Resume la actividad pendiente.",
                "2099-01-01T10:00:00.000Z",
                "Atlantic/Canary",
                "once",
                true,
            )
            .expect("schedule should be created");
        assert!(scheduled.enabled);
        assert_eq!(scheduled.conversation_id, Some(conversation.id.clone()));
        let updated = database
            .update_scheduled_task(
                &scheduled.id,
                "Preparar informe revisado",
                &conversation.id,
                "Resume la actividad pendiente con tres puntos.",
                "2099-01-02T11:00:00.000Z",
                "Atlantic/Canary",
                "once",
                true,
            )
            .expect("schedule should be editable before running");
        assert_eq!(updated.name, "Preparar informe revisado");
        assert_eq!(
            updated.prompt,
            "Resume la actividad pendiente con tres puntos."
        );

        let paused = database
            .set_scheduled_task_enabled(&scheduled.id, false, false)
            .expect("schedule should pause without confirmation");
        assert!(!paused.enabled);
        let active = database
            .set_scheduled_task_enabled(&scheduled.id, true, true)
            .expect("schedule should reactivate with confirmation");
        assert!(active.enabled);

        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE scheduled_tasks SET next_run_at = '2000-01-01T00:00:00.000Z'
                 WHERE id = ?1",
                params![scheduled.id],
            )
            .expect("schedule should become due");
        let claim = database
            .claim_due_scheduled_task()
            .expect("claim should succeed")
            .expect("due schedule should be claimed");
        assert_eq!(claim.conversation_id, Some(conversation.id));
        assert_eq!(
            claim.prompt,
            "Resume la actividad pendiente con tres puntos."
        );
        assert!(database
            .claim_due_scheduled_task()
            .expect("second claim should be safe")
            .is_none());

        let listed = database
            .list_scheduled_tasks()
            .expect("schedule should remain visible");
        assert_eq!(listed[0].runs.len(), 1);
        assert_eq!(listed[0].runs[0].status, "claimed");
        assert!(!listed[0].enabled);
        cleanup(&database);
    }

    #[test]
    fn recurring_schedule_advances_before_it_can_be_claimed_again() {
        let database = test_database();
        let conversation = database
            .create_conversation("Seguimiento diario", None)
            .expect("conversation should be created");
        let scheduled = database
            .create_scheduled_task(
                "Seguimiento",
                &conversation.id,
                "Resume las novedades.",
                "2099-01-01T10:00:00.000Z",
                "Atlantic/Canary",
                "daily",
                true,
            )
            .expect("recurring schedule should be created");
        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE scheduled_tasks
                 SET next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')
                 WHERE id = ?1",
                params![scheduled.id],
            )
            .expect("schedule should become due");
        database
            .claim_due_scheduled_task()
            .expect("claim should succeed")
            .expect("recurring schedule should be claimed");
        assert!(database
            .claim_due_scheduled_task()
            .expect("second immediate claim should be safe")
            .is_none());
        let listed = database
            .list_scheduled_tasks()
            .expect("schedule should remain visible");
        assert!(listed[0].enabled);
        assert_eq!(listed[0].schedule_expression, "daily");
        assert_eq!(listed[0].runs.len(), 1);
        let next_run_at = listed[0]
            .next_run_at
            .as_deref()
            .expect("recurring schedule should have a next run");
        let is_future: bool = database
            .connect()
            .expect("database should connect")
            .query_row(
                "SELECT datetime(?1) > datetime('now')",
                params![next_run_at],
                |row| row.get(0),
            )
            .expect("next run should be comparable");
        assert!(is_future);
        cleanup(&database);
    }

    #[test]
    fn failed_scheduled_run_can_be_retried_without_losing_history() {
        let database = test_database();
        let conversation = database
            .create_conversation("Informe recuperable", None)
            .expect("conversation should be created");
        let scheduled = database
            .create_scheduled_task(
                "Informe",
                &conversation.id,
                "Prepara el informe.",
                "2099-01-01T10:00:00.000Z",
                "Atlantic/Canary",
                "once",
                true,
            )
            .expect("schedule should be created");
        database
            .connect()
            .expect("database should connect")
            .execute(
                "UPDATE scheduled_tasks SET next_run_at = '2000-01-01T00:00:00.000Z'
                 WHERE id = ?1",
                params![scheduled.id],
            )
            .expect("schedule should become due");
        let first = database
            .claim_due_scheduled_task()
            .expect("claim should succeed")
            .expect("due schedule should be claimed");
        database
            .fail_scheduled_run(&first.run_id, "Broker temporalmente no disponible")
            .expect("run should fail");
        assert!(matches!(
            database.retry_failed_scheduled_run(&first.run_id, false),
            Err(AppError::Validation(_))
        ));

        let retry = database
            .retry_failed_scheduled_run(&first.run_id, true)
            .expect("failed run should be retried");
        assert_eq!(retry.scheduled_task_id, scheduled.id);
        assert_ne!(retry.run_id, first.run_id);
        assert!(matches!(
            database.retry_failed_scheduled_run(&first.run_id, true),
            Err(AppError::Conflict(_))
        ));

        let listed = database
            .list_scheduled_tasks()
            .expect("schedule should remain visible");
        assert_eq!(listed[0].runs.len(), 2);
        assert_eq!(listed[0].runs[0].status, "claimed");
        assert_eq!(listed[0].runs[0].attempt, 2);
        assert_eq!(listed[0].runs[1].status, "failed");
        assert_eq!(listed[0].runs[1].attempt, 1);
        cleanup(&database);
    }

    #[test]
    fn running_scheduled_run_can_be_cancelled_without_pausing_recurrence() {
        let database = test_database();
        let conversation = database
            .create_conversation("Seguimiento cancelable", None)
            .expect("conversation should be created");
        let scheduled = database
            .create_scheduled_task(
                "Seguimiento",
                &conversation.id,
                "Resume las novedades.",
                "2099-01-01T10:00:00.000Z",
                "Atlantic/Canary",
                "daily",
                true,
            )
            .expect("recurring schedule should be created");
        let connection = database.connect().expect("database should connect");
        connection
            .execute(
                "UPDATE scheduled_tasks
                 SET next_run_at = strftime('%Y-%m-%dT%H:%M:%fZ', 'now', '-1 hour')
                 WHERE id = ?1",
                params![scheduled.id],
            )
            .expect("schedule should become due");
        connection
            .execute(
                "INSERT INTO broker_tasks(
                    id, idempotency_key, request_json, remote_status, local_state
                 ) VALUES ('cancel-local-task', 'cancel-key', '{}', 'generating', 'polling')",
                [],
            )
            .expect("broker task should be stored");
        drop(connection);
        let claim = database
            .claim_due_scheduled_task()
            .expect("claim should succeed")
            .expect("schedule should be claimed");
        database
            .start_scheduled_run(&claim.run_id, "cancel-local-task")
            .expect("scheduled run should start");

        assert!(matches!(
            database.scheduled_cancellation_target(&claim.run_id, false),
            Err(AppError::Validation(_))
        ));
        let target = database
            .scheduled_cancellation_target(&claim.run_id, true)
            .expect("running run should expose its local task");
        assert_eq!(target.broker_task_id.as_deref(), Some("cancel-local-task"));
        database
            .finish_scheduled_cancellation(
                &claim.run_id,
                target
                    .broker_task_id
                    .as_deref()
                    .expect("broker task should exist"),
            )
            .expect("cancellation should be persisted");

        let listed = database
            .list_scheduled_tasks()
            .expect("schedule should remain visible");
        assert!(listed[0].enabled);
        assert_eq!(listed[0].runs[0].status, "cancelled");
        assert!(matches!(
            database.scheduled_cancellation_target(&claim.run_id, true),
            Err(AppError::Conflict(_))
        ));
        let audited: i64 = database
            .connect()
            .expect("database should connect")
            .query_row(
                "SELECT COUNT(*) FROM audit_events
                 WHERE event_type = 'scheduled_run.cancelled'",
                [],
                |row| row.get(0),
            )
            .expect("audit should be readable");
        assert_eq!(audited, 1);
        cleanup(&database);
    }
}
