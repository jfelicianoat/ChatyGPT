//! Vistas de conversacion, adjuntos y workflows.

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::*;

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
    pub execution_warnings: Vec<String>,
    pub unsupported_citation_urls: Vec<String>,
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
    #[serde(default = "default_custom_gpt_context_profile")]
    pub context_profile: String,
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
