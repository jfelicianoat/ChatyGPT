use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum TaskStatus {
    Queued,
    Routing,
    Planning,
    ResourcePlanning,
    Chunking,
    Generating,
    Proposing,
    Evaluating,
    Debating,
    Synthesizing,
    Verifying,
    WaitingForTools,
    Completed,
    Failed,
    Cancelled,
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
            Self::Chunking => "chunking",
            Self::Generating => "generating",
            Self::Proposing => "proposing",
            Self::Evaluating => "evaluating",
            Self::Debating => "debating",
            Self::Synthesizing => "synthesizing",
            Self::Verifying => "verifying",
            Self::WaitingForTools => "waiting_for_tools",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Cancelled => "cancelled",
        }
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
    pub status: TaskStatus,
    pub request_id: Option<String>,
    pub created_at: String,
    pub updated_at: String,
    pub execution_strategy: String,
    pub execution_preset: String,
    pub selection_mode: String,
    #[serde(default)]
    pub progress: Value,
    pub result: Option<Value>,
    pub error: Option<Value>,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct BrokerCapabilities {
    pub contract_version: String,
    #[serde(default)]
    pub strategies: Vec<String>,
    #[serde(default)]
    pub agent_skills: Vec<String>,
    #[serde(default)]
    pub sandbox_run_code: bool,
    #[serde(default)]
    pub file_ingestion: bool,
    #[serde(default)]
    pub client_tool_passthrough: bool,
}
