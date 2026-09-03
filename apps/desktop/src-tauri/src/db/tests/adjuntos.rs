//! Adjuntos: deduplicado, politica de imagenes, reintentos y fuentes.

use super::comunes::{cleanup, test_database};
use crate::broker::TaskState;
use crate::db::{ContextMessage, SCHEMA_VERSION};
use rusqlite::params;

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
            ],
            "warnings": ["Una dependencia falló; la tarea continuó"],
            "agent": {
                "citations": {
                    "cited": 2,
                    "unsupported": ["https://example.invalid/no-consultada"]
                }
            }
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
        assistant.execution_warnings,
        ["Una dependencia falló; la tarea continuó"]
    );
    assert_eq!(
        assistant.unsupported_citation_urls,
        ["https://example.invalid/no-consultada"]
    );
    assert_eq!(
        assistant.sources[0].source_attachment_id.as_deref(),
        Some(attachment.id.as_str())
    );
    cleanup(&database);
}
