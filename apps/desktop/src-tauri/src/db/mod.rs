//! Persistencia en SQLite, repartida por dominio.
//!
//! Este modulo tiene los tipos, las migraciones y el `Database`; cada dominio
//! abre su propio `impl Database` en su fichero. Rust permite varios bloques
//! inherentes dentro del crate, asi que quien llama no nota el reparto y cada
//! consulta vive junto a las que se tocan con ella.

use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf, MAIN_SEPARATOR};

use rusqlite::{params, Connection, OpenFlags, OptionalExtension, TransactionBehavior};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
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
const REMOTE_OPERATION_START_METRIC_MIGRATION: &str =
    include_str!("../../migrations/0022_remote_operation_start_metric.sql");
const ATHENA_RUNS_MIGRATION: &str = include_str!("../../migrations/0023_athena_runs.sql");
const RECOVER_NON_TERMINAL_TASKS: &str =
    include_str!("../../queries/recover_non_terminal_tasks.sql");
pub const SCHEMA_VERSION: i64 = 23;

#[derive(Clone)]
pub struct Database {
    path: PathBuf,
}

mod adjuntos;
mod adjuntos_estado;
mod adjuntos_indice;
mod apertura;
mod auditoria;
mod contexto;
mod conversaciones;
mod gpts;
mod gpts_portabilidad;
mod memoria;
mod metricas;
mod permisos;
mod programacion;
mod programacion_catalogo;
mod programacion_ejecucion;
mod proyectos;
mod resumenes;
mod semantica;
mod semantica_envio;
mod tareas;
mod tareas_herramientas;
mod tareas_investigacion;
mod tipos;
mod turnos;
mod utilidades;
mod workflows;
mod workflows_contexto;
mod workflows_ejecucion;

pub(crate) use tipos::*;
pub(crate) use utilidades::*;

#[cfg(test)]
mod tests;
