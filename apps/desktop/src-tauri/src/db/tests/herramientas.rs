//! Herramientas del modelo: espera, confirmacion y no repeticion.

use super::comunes::{cleanup, test_database};
use crate::broker::TaskState;
use crate::db::{ContextMessage, Database, ToolOutcomeRecord};
use crate::error::AppError;
use rusqlite::params;

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
    let reopened =
        Database::open(database.path()).expect("database should reopen without losing the record");
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
