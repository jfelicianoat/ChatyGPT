//! Conversaciones: seleccion de GPT, preferencias, ocultado y recuperacion.

use super::comunes::{cleanup, test_database};
use crate::broker::TaskState;
use crate::db::{
    ContextMessage, ConversationExecutionPreferences, Database, INITIAL_MIGRATION, SCHEMA_VERSION,
};
use crate::error::AppError;
use rusqlite::params;
use uuid::Uuid;

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
