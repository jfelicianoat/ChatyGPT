use std::collections::{HashMap, HashSet};
use std::fs;
use std::hash::{DefaultHasher, Hash, Hasher};
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use uuid::Uuid;

use crate::broker::{BrokerCapabilities, BrokerClient, PollPolicy};
use crate::db::{
    AttachmentRecord, BrokerTaskRecord, ConversationExecutionPreferences,
    ConversationSummaryOverview, CustomGptContext, Database, LocalTaskSnapshot, MemoryItemView,
    MemorySearchView, ProjectInstructionContext, SelectedAttachmentChunk, ToolOutcomeRecord,
};
use crate::error::AppError;
use crate::logging;

mod herramientas;
mod investigacion;
mod peticiones;
mod recuperacion;

pub(crate) use herramientas::*;
pub(crate) use investigacion::*;
use peticiones::*;
pub(crate) use recuperacion::*;

#[derive(Debug, Clone, Default)]
struct ChatExecutionOptions {
    tools_enabled: bool,
    sandbox_enabled: bool,
    execution_preferences: ConversationExecutionPreferences,
}

const SUMMARY_INPUT_CHARACTER_BUDGET: usize = 48_000;
const DOCUMENT_CONTEXT_CHUNK_LIMIT: usize = 8;
const DOCUMENT_CONTEXT_CHARACTER_BUDGET: usize = 24_000;
#[derive(Debug, Clone, PartialEq, Eq)]
enum DocumentIndexDependency {
    Group(String),
    Tasks(Vec<String>),
}

fn document_embedding_group(attachment_id: &str) -> String {
    let fingerprint = format!("{:x}", Sha256::digest(attachment_id.as_bytes()));
    format!("chatygpt-index-{}", &fingerprint[..32])
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
struct CustomGptContextBudget {
    recent_messages: usize,
    recent_characters: usize,
    memory_items: usize,
    memory_characters: usize,
    document_chunks: usize,
    document_characters: usize,
}

fn custom_gpt_context_budget(context: Option<&CustomGptContext>) -> CustomGptContextBudget {
    match context.map(|item| item.context_profile.as_str()) {
        Some("focused") => CustomGptContextBudget {
            recent_messages: 6,
            recent_characters: 6_000,
            memory_items: 5,
            memory_characters: 2_000,
            document_chunks: 4,
            document_characters: 12_000,
        },
        Some("broad") => CustomGptContextBudget {
            recent_messages: 20,
            recent_characters: 24_000,
            memory_items: 30,
            memory_characters: 16_000,
            document_chunks: 12,
            document_characters: 48_000,
        },
        _ => CustomGptContextBudget {
            recent_messages: 12,
            recent_characters: 12_000,
            memory_items: 20,
            memory_characters: 8_000,
            document_chunks: DOCUMENT_CONTEXT_CHUNK_LIMIT,
            document_characters: DOCUMENT_CONTEXT_CHARACTER_BUDGET,
        },
    }
}

pub async fn start_smoke_task(
    database: Database,
    broker: BrokerClient,
) -> Result<LocalTaskSnapshot, AppError> {
    let local_id = format!("local_{}", Uuid::new_v4().simple());
    let idempotency_key = format!("chatygpt:phase0:{}", Uuid::new_v4());
    let request = smoke_request(&idempotency_key);
    let record = database.prepare_broker_task(&local_id, &idempotency_key, &request)?;
    let snapshot = database.task_snapshot(&local_id)?;
    spawn_submission_and_poll(database, broker, record);
    Ok(snapshot)
}

pub fn start_memory_embedding(
    database: Database,
    broker: BrokerClient,
    memory_id: &str,
    content: &str,
    force_reindex: bool,
) -> Result<LocalTaskSnapshot, AppError> {
    let content_sha256 = format!("{:x}", Sha256::digest(content.as_bytes()));
    let local_id = format!("local_{}", Uuid::new_v4().simple());
    let idempotency_key = if force_reindex {
        format!(
            "chatygpt:memory-embedding:{memory_id}:{content_sha256}:retry:{}",
            Uuid::new_v4()
        )
    } else {
        format!("chatygpt:memory-embedding:{memory_id}:{content_sha256}")
    };
    let request = memory_embedding_request(&idempotency_key, memory_id, content, &content_sha256);
    let record = database.prepare_broker_task(&local_id, &idempotency_key, &request)?;
    let snapshot = database.task_snapshot(&local_id)?;
    spawn_submission_and_poll(database, broker, record);
    Ok(snapshot)
}

pub fn start_attachment_semantic_index(
    database: Database,
    broker: BrokerClient,
    attachment_id: &str,
    retry_failed: bool,
    dependencies_enabled: bool,
) -> Result<Option<LocalTaskSnapshot>, AppError> {
    let chunks = database.attachment_chunks_for_embedding(attachment_id, retry_failed)?;
    if chunks.is_empty() {
        return Ok(None);
    }
    // Primero se persiste el lote entero. Solo después se lanza la primera
    // petición HTTP, para que una pregunta nunca observe 12 de 259 tareas.
    let mut records = Vec::with_capacity(chunks.len());
    for chunk in chunks {
        let local_id = format!("local_{}", Uuid::new_v4().simple());
        let idempotency_key = if retry_failed {
            format!(
                "chatygpt:attachment-chunk-embedding:{}:{}:retry:{}",
                chunk.id,
                chunk.content_sha256,
                Uuid::new_v4()
            )
        } else {
            format!(
                "chatygpt:attachment-chunk-embedding:{}:{}",
                chunk.id, chunk.content_sha256
            )
        };
        let mut request = embedding_request(
            &idempotency_key,
            "attachment_chunk",
            &chunk.id,
            &chunk.text,
            &chunk.content_sha256,
        );
        if dependencies_enabled {
            request["group"] = json!(document_embedding_group(attachment_id));
        }
        records.push(database.prepare_broker_task(&local_id, &idempotency_key, &request)?);
    }
    let snapshot = database.task_snapshot(&records[0].id)?;
    for record in records {
        spawn_submission_and_poll(database.clone(), broker.clone(), record);
    }
    Ok(Some(snapshot))
}

async fn ensure_attachment_embeddings_are_enqueued(
    database: &Database,
    broker: &BrokerClient,
    attachment_ids: &[String],
) -> Result<Option<DocumentIndexDependency>, AppError> {
    for attachment_id in attachment_ids {
        start_attachment_semantic_index(
            database.clone(),
            broker.clone(),
            attachment_id,
            false,
            true,
        )?;
    }
    // Las tareas pueden haberse creado por la ingesta en segundo plano. No se
    // considera listo el lote hasta que todas tienen identidad remota; así el
    // grupo existe completo antes de enviar la pregunta dependiente.
    let records = database.attachment_embedding_tasks(attachment_ids)?;
    if records.is_empty() {
        return Ok(None);
    }
    let single_document_group =
        (attachment_ids.len() == 1).then(|| document_embedding_group(&attachment_ids[0]));
    let can_use_group = single_document_group.as_ref().is_some_and(|expected| {
        records.iter().all(|record| {
            record
                .request
                .get("group")
                .and_then(serde_json::Value::as_str)
                == Some(expected)
        })
    });
    let mut remote_ids = Vec::with_capacity(records.len());
    for record in records {
        let local_id = record.id.clone();
        let needs_submission = record.remote_task_id.is_none();
        submit_or_resume(database.clone(), broker.clone(), record).await?;
        if needs_submission {
            spawn_polling(database.clone(), broker.clone(), local_id.clone());
        }
        let remote_id = database
            .task_record(&local_id)?
            .remote_task_id
            .ok_or_else(|| {
                AppError::BrokerContract(
                    "una tarea de indexación no recibió identificador remoto".to_owned(),
                )
            })?;
        remote_ids.push(remote_id);
    }
    remote_ids.sort();
    remote_ids.dedup();
    if can_use_group {
        return Ok(single_document_group.map(DocumentIndexDependency::Group));
    }
    if remote_ids.len() <= 64 {
        return Ok(Some(DocumentIndexDependency::Tasks(remote_ids)));
    }
    Err(AppError::Conflict(
        "Hay varios documentos con más de 64 fragmentos todavía indexándose. Espera a que indiquen Índice preparado antes de enviar una pregunta conjunta"
            .to_owned(),
    ))
}

pub fn start_memory_search(
    database: Database,
    broker: BrokerClient,
    query: &str,
    project_id: Option<&str>,
) -> Result<MemorySearchView, AppError> {
    let search_id = format!("memory_search_{}", Uuid::new_v4().simple());
    let task_id = format!("local_{}", Uuid::new_v4().simple());
    let content_sha256 = format!("{:x}", Sha256::digest(query.as_bytes()));
    let idempotency_key = format!("chatygpt:memory-search:{search_id}:{content_sha256}");
    let request = embedding_request(
        &idempotency_key,
        "memory_search",
        &search_id,
        query,
        &content_sha256,
    );
    let record = database.prepare_memory_search(
        &search_id,
        query,
        project_id,
        &task_id,
        &idempotency_key,
        &request,
    )?;
    spawn_submission_and_poll(database.clone(), broker, record);
    database.memory_search(&search_id)
}

#[allow(clippy::too_many_arguments)]
pub async fn start_chat_turn(
    database: Database,
    broker: BrokerClient,
    conversation_id: &str,
    user_text: &str,
    attachment_ids: &[String],
    tools_enabled: bool,
    sandbox_enabled: bool,
    semantic_memory_enabled: bool,
    research_mode: bool,
) -> Result<LocalTaskSnapshot, AppError> {
    let user_text = user_text.trim();
    if user_text.is_empty() {
        return Err(AppError::BrokerContract(
            "el mensaje no puede estar vacío".to_owned(),
        ));
    }
    if user_text.chars().count() > 200_000 {
        return Err(AppError::BrokerContract(
            "el mensaje supera el límite de 200.000 caracteres".to_owned(),
        ));
    }
    if attachment_ids.len() > 20 {
        return Err(AppError::BrokerContract(
            "no se pueden enviar más de 20 adjuntos en un turno".to_owned(),
        ));
    }
    let mut effective_attachment_ids = attachment_ids.to_vec();
    for attachment_id in database.ready_custom_gpt_file_ids_for_conversation(conversation_id)? {
        if !effective_attachment_ids.contains(&attachment_id) {
            effective_attachment_ids.push(attachment_id);
        }
    }
    if effective_attachment_ids.len() > 20 {
        return Err(AppError::Conflict(
            "los adjuntos del turno y los archivos del GPT superan juntos el límite de 20"
                .to_owned(),
        ));
    }
    let attachment_ids = effective_attachment_ids.as_slice();
    let execution_preferences = database.conversation_execution_preferences(conversation_id)?;
    let custom_gpt_context = database.custom_gpt_for_conversation(conversation_id)?;
    let context_budget = custom_gpt_context_budget(custom_gpt_context.as_ref());
    let attachments = database.ready_attachments_for_turn(conversation_id, attachment_ids)?;
    let capabilities = if sandbox_enabled || research_mode || !attachments.is_empty() {
        match broker.capabilities().await {
            Ok(capabilities) => Some(capabilities),
            Err(error) => {
                logging::warn(
                    "broker.capabilities_unverified_for_turn",
                    None,
                    &[("error_kind", logging::error_kind(&error))],
                );
                None
            }
        }
    } else {
        None
    };
    let has_tabular_attachment = attachments.iter().any(is_tabular_attachment);
    if has_tabular_attachment && !sandbox_enabled {
        return Err(AppError::Conflict(
            "los archivos CSV, TSV y Excel necesitan Código aislado para poder analizarlos"
                .to_owned(),
        ));
    }
    if sandbox_enabled {
        if custom_gpt_context.as_ref().is_some_and(|custom_gpt| {
            !custom_gpt
                .tool_permissions
                .requires_confirmation("run_code")
        }) {
            return Err(AppError::Conflict(
                "la versión seleccionada del GPT mantiene Código aislado denegado".to_owned(),
            ));
        }
        match capabilities.as_ref() {
            Some(capabilities) => validate_sandbox_capability(capabilities)?,
            None => logging::warn(
                "broker.capabilities_unverified_for_sandbox",
                None,
                &[("fallback", logging::code("broker_validation"))],
            ),
        }
    }
    let document_index_dependency = if capabilities
        .as_ref()
        .is_some_and(|capabilities| capabilities.task_dependencies)
        && !attachments.is_empty()
    {
        ensure_attachment_embeddings_are_enqueued(&database, &broker, attachment_ids).await?
    } else {
        None
    };
    let document_chunks = database.select_attachment_chunks(
        conversation_id,
        attachment_ids,
        user_text,
        context_budget.document_chunks,
        context_budget.document_characters,
    )?;
    let user_message_id = format!("msg_{}", Uuid::new_v4().simple());
    let assistant_message_id = format!("msg_{}", Uuid::new_v4().simple());
    let mut context = database.recent_context(
        conversation_id,
        context_budget.recent_messages,
        context_budget.recent_characters,
    )?;
    context.push(crate::db::ContextMessage {
        message_id: user_message_id.clone(),
        role: "user".to_owned(),
        text: user_text.to_owned(),
    });
    let project_instruction = database.project_instruction_for_conversation(conversation_id)?;
    let semantic_documents_available = database.attachments_have_semantic_index(attachment_ids)?;
    // El plan se decide antes de persistir nada: si el Broker no anuncia las
    // herramientas necesarias, el turno se rechaza sin dejar un mensaje a medias.
    let research_plan = if research_mode {
        Some(match capabilities.as_ref() {
            Some(capabilities) => deep_research_plan(capabilities)?,
            None => {
                logging::warn(
                    "broker.capabilities_unverified_for_research",
                    None,
                    &[("fallback", logging::code("broker_validation"))],
                );
                unverified_deep_research_plan()
            }
        })
    } else {
        None
    };
    let research_plan_value = research_plan
        .as_ref()
        .map(serde_json::to_value)
        .transpose()
        .map_err(|error| AppError::BrokerContract(error.to_string()))?;
    if semantic_memory_enabled || semantic_documents_available {
        let workflow_id = format!("semantic_chat_{}", Uuid::new_v4().simple());
        let local_task_id = format!("local_{}", Uuid::new_v4().simple());
        let content_sha256 = format!("{:x}", Sha256::digest(user_text.as_bytes()));
        let idempotency_key =
            format!("chatygpt:semantic-chat-search:{workflow_id}:{content_sha256}");
        let request = embedding_request(
            &idempotency_key,
            if semantic_memory_enabled {
                "chat_memory_search"
            } else {
                "chat_document_search"
            },
            &workflow_id,
            user_text,
            &content_sha256,
        );
        let request = apply_document_index_dependency(request, document_index_dependency.as_ref());
        let record = database.prepare_semantic_chat_turn_with_project_instruction(
            &workflow_id,
            conversation_id,
            &user_message_id,
            &assistant_message_id,
            &local_task_id,
            &idempotency_key,
            user_text,
            &request,
            &context,
            project_instruction.as_ref(),
            custom_gpt_context.as_ref(),
            attachment_ids,
            tools_enabled,
            sandbox_enabled,
            &execution_preferences,
            research_plan_value.as_ref(),
        )?;
        let snapshot = database.task_snapshot(&local_task_id)?;
        spawn_submission_and_poll(database, broker, record);
        return Ok(snapshot);
    }

    let memories = database.active_memories_for_conversation_with_limits(
        conversation_id,
        context_budget.memory_items,
        context_budget.memory_characters,
    )?;
    let local_task_id = format!("local_{}", Uuid::new_v4().simple());
    let idempotency_key = format!("chatygpt:turn:{}", Uuid::new_v4());
    let mut request = chat_request_with_project_instruction(
        conversation_id,
        &idempotency_key,
        user_text,
        &context,
        &attachments,
        &document_chunks,
        &memories,
        project_instruction.as_ref(),
        custom_gpt_context.as_ref(),
        ChatExecutionOptions {
            tools_enabled,
            sandbox_enabled,
            execution_preferences,
        },
    )?;
    if let Some(plan) = research_plan.as_ref() {
        request = apply_deep_research_plan(request, plan)?;
    }
    request = apply_document_index_dependency(request, document_index_dependency.as_ref());
    let record = database.prepare_chat_turn_with_project_instruction(
        conversation_id,
        &user_message_id,
        &assistant_message_id,
        &local_task_id,
        &idempotency_key,
        user_text,
        &request,
        &context,
        project_instruction.as_ref(),
        custom_gpt_context.as_ref(),
        &memories,
        &document_chunks,
        attachment_ids,
    )?;
    let snapshot = database.task_snapshot(&local_task_id)?;
    spawn_submission_and_poll(database, broker, record);
    Ok(snapshot)
}

#[cfg(test)]
mod tests;
