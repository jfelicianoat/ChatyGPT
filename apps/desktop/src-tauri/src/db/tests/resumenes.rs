//! Resumenes de conversacion: borrador, aprobacion y compactado.

use super::comunes::{cleanup, test_database};
use crate::broker::TaskState;
use crate::db::{ContextMessage, Database};
use rusqlite::params;

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
