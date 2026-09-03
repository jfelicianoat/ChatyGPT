//! Busqueda semantica de memoria y su traza en el turno.

use super::comunes::{cleanup, test_database};
use crate::broker::TaskState;
use crate::db::{ContextMessage, ConversationExecutionPreferences, Database};
use rusqlite::params;
use sha2::{Digest, Sha256};

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
