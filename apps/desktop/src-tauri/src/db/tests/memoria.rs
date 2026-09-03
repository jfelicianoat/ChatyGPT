//! Memoria: ambito, control del usuario e invalidacion del indice.

use super::comunes::{cleanup, test_database};
use crate::broker::TaskState;
use crate::db::ContextMessage;
use rusqlite::params;
use sha2::{Digest, Sha256};

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
