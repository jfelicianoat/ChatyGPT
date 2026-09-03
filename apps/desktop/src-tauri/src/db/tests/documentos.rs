//! Seleccion de fragmentos de documento, embeddings y reintentos.

use super::comunes::{cleanup, test_database};
use crate::db::ContextMessage;
use crate::error::AppError;
use rusqlite::params;
use uuid::Uuid;

#[test]
fn document_chunk_selection_is_relevant_bounded_and_traceable() {
    let database = test_database();
    let managed_root = std::env::temp_dir().join(format!(
        "chatygpt-document-source-test-{}",
        Uuid::new_v4().simple()
    ));
    let managed_file = managed_root.join("prices.csv");
    std::fs::create_dir_all(&managed_root).expect("managed root should exist");
    std::fs::write(&managed_file, b"date,open,high,low,close").expect("managed file should exist");
    let conversation = database
        .create_conversation("Análisis de precios", None)
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            managed_file.to_str().expect("managed path should be UTF-8"),
            "prices.csv",
            Some("text/csv"),
            9_000_000,
            "prices-hash",
        )
        .expect("attachment should be registered");
    database
        .update_attachment_ingestion(
            &attachment.id,
            "ready",
            Some("broker-prices"),
            Some("document"),
            Some("docling"),
            None,
            None,
        )
        .expect("attachment should become ready");
    database
        .replace_attachment_chunks(
            &attachment.id,
            &[
                "Introducción general y procedencia del fichero.".to_owned(),
                "Columnas OHLC de precios. Calcular media y mediana del cierre.".to_owned(),
                "Notas finales sobre licencias y autores.".to_owned(),
            ],
        )
        .expect("chunks should be stored");

    let selected = database
        .select_attachment_chunks(
            &conversation.id,
            std::slice::from_ref(&attachment.id),
            "calcula la media y mediana de los precios OHLC",
            2,
            80,
        )
        .expect("chunks should be selected");

    assert_eq!(selected.len(), 1);
    assert_eq!(selected[0].attachment_name, "prices.csv");
    assert_eq!(selected[0].ordinal, 1);
    assert!(selected[0].score > 0.0);
    assert_eq!(selected[0].reason, "Coincidencia con la pregunta");
    assert!(
        selected
            .iter()
            .map(|chunk| chunk.text.chars().count())
            .sum::<usize>()
            <= 80
    );

    let context = vec![ContextMessage {
        message_id: "document-user".to_owned(),
        role: "user".to_owned(),
        text: "calcula la media y mediana de los precios OHLC".to_owned(),
    }];
    database
        .prepare_chat_turn(
            &conversation.id,
            "document-user",
            "document-assistant",
            "document-task",
            "document-key",
            "calcula la media y mediana de los precios OHLC",
            &serde_json::json!({"inference_kind": "chat"}),
            &context,
            &[],
            &selected,
            std::slice::from_ref(&attachment.id),
        )
        .expect("turn with document chunks should be prepared");
    let trace = database
        .task_context("document-task")
        .expect("document context should be inspectable");
    assert_eq!(trace.strategy, "Ventana reciente + documentos");
    assert_eq!(trace.sources[1].kind, "attachment_chunk");
    assert_eq!(trace.sources[1].label, "prices.csv · fragmento 2");
    assert_eq!(trace.sources[1].reason, "Coincidencia con la pregunta");
    assert!(trace.sources[1].source_available);
    let source_reference = trace.sources[1]
        .source_reference
        .as_deref()
        .expect("document source should expose an opaque reference");
    let source = database
        .context_source_file("document-task", source_reference)
        .expect("document source should resolve");
    assert_eq!(source.local_path, managed_file.to_string_lossy());
    assert_eq!(source.display_name, "prices.csv");
    assert!(matches!(
        database.context_source_file("another-task", source_reference),
        Err(AppError::NotFound(_))
    ));
    cleanup(&database);
    std::fs::remove_dir_all(managed_root).expect("managed test files should be removed");
}

#[test]
fn global_document_request_prefers_structure_over_cosine_winners() {
    let database = test_database();
    let managed_root = std::env::temp_dir().join(format!(
        "chatygpt-global-document-test-{}",
        Uuid::new_v4().simple()
    ));
    let managed_file = managed_root.join("book.pdf");
    std::fs::create_dir_all(&managed_root).expect("managed root should exist");
    std::fs::write(&managed_file, b"book").expect("managed file should exist");
    let conversation = database
        .create_conversation("Resumen de libro", None)
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            managed_file.to_str().expect("managed path should be UTF-8"),
            "book.pdf",
            Some("application/pdf"),
            4,
            "book-hash",
        )
        .expect("attachment should be registered");
    database
        .replace_attachment_chunks(
            &attachment.id,
            &[
                "Título y autor de la obra.".to_owned(),
                "Table of contents. Chapter 1: Origins. Chapter 2: Methods.".to_owned(),
                "Preface. This book presents the history and foundations of pattern recognition."
                    .to_owned(),
                "Un detalle aislado acerca de un algoritmo.".to_owned(),
                "Otro detalle técnico.".to_owned(),
                "Conclusion. The field combines statistical learning and computation.".to_owned(),
            ],
        )
        .expect("chunks should be stored");

    let selected = database
        .select_attachment_chunks(
            &conversation.id,
            std::slice::from_ref(&attachment.id),
            "Dime de qué va el libro y hazme un resumen",
            4,
            20_000,
        )
        .expect("global view should be selected");

    assert_eq!(selected.len(), 4);
    assert_eq!(selected[0].ordinal, 1);
    assert!(selected[0].reason.contains("índice"));
    assert_eq!(selected[1].ordinal, 2);
    assert!(selected[1].reason.contains("prefacio"));
    assert_eq!(selected[2].ordinal, 5);
    assert!(selected[2].reason.contains("conclusiones"));
    assert_eq!(selected[3].ordinal, 0);
    assert!(selected
        .iter()
        .all(|chunk| chunk.reason.starts_with("Vista global del documento")));
    cleanup(&database);
    std::fs::remove_dir_all(managed_root).expect("managed test files should be removed");
}

#[test]
fn specific_document_request_keeps_relevance_ranking() {
    assert!(!crate::db::is_global_document_request(
        "¿Qué fórmula utiliza el capítulo 7 para la varianza?"
    ));
    assert!(crate::db::is_global_document_request(
        "¿De qué trata este documento?"
    ));
    assert!(crate::db::is_global_document_request(
        "Hazme un resumen del libro"
    ));
    assert!(!crate::db::is_global_document_request(
        "Haz un resumen de la sección sobre regresión"
    ));
}

#[test]
fn attachment_exposes_durable_document_context_progress_and_chunk_count() {
    let database = test_database();
    let conversation = database
        .create_conversation("Contexto documental visible", None)
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            "managed/guide.pdf",
            "guide.pdf",
            Some("application/pdf"),
            120_000,
            "guide-hash",
        )
        .expect("attachment should be registered");

    assert_eq!(attachment.context_status, "pending");
    assert_eq!(attachment.chunk_count, 0);
    assert_eq!(attachment.indexed_characters, 0);
    assert_eq!(attachment.semantic_index_status, "unavailable");
    database
        .mark_attachment_context_preparing(&attachment.id)
        .expect("context preparation should start");
    let preparing = database
        .attachment_view(&attachment.id)
        .expect("attachment should be visible");
    assert_eq!(preparing.context_status, "preparing");

    database
        .replace_attachment_chunks(
            &attachment.id,
            &[
                "Primer fragmento del documento.".to_owned(),
                "Segundo fragmento del documento.".to_owned(),
            ],
        )
        .expect("chunks should be stored");
    let ready = database
        .attachment_view(&attachment.id)
        .expect("attachment should be visible");
    assert_eq!(ready.context_status, "ready");
    assert_eq!(ready.chunk_count, 2);
    assert_eq!(
        ready.indexed_characters,
        "Primer fragmento del documento.".chars().count() as i64
            + "Segundo fragmento del documento.".chars().count() as i64
    );
    assert_eq!(ready.semantic_indexed_chunks, 0);
    assert_eq!(ready.semantic_index_status, "pending");
    assert!(ready.context_error.is_none());
    cleanup(&database);
}

#[test]
fn document_selection_includes_nearby_context_after_relevant_chunks() {
    let database = test_database();
    let conversation = database
        .create_conversation("Contexto vecino", None)
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            "managed/guide.md",
            "guide.md",
            Some("text/markdown"),
            200,
            "guide-neighbor-hash",
        )
        .expect("attachment should be registered");
    database
        .replace_attachment_chunks(
            &attachment.id,
            &[
                "Capítulo: indicadores estadísticos.".to_owned(),
                "La mediana del cierre reduce el efecto de valores extremos.".to_owned(),
                "El apéndice describe las fuentes de datos.".to_owned(),
            ],
        )
        .expect("chunks should be stored");

    let selected = database
        .select_attachment_chunks(
            &conversation.id,
            std::slice::from_ref(&attachment.id),
            "mediana del cierre",
            2,
            500,
        )
        .expect("chunks should be selected");

    assert_eq!(selected.len(), 2);
    assert_eq!(selected[0].ordinal, 1);
    assert_eq!(selected[0].reason, "Coincidencia con la pregunta");
    assert_eq!(selected[1].ordinal, 0);
    assert_eq!(
        selected[1].reason,
        "Contexto próximo al fragmento relevante"
    );
    cleanup(&database);
}

#[test]
fn hybrid_document_selection_uses_compatible_chunk_embeddings() {
    let database = test_database();
    let conversation = database
        .create_conversation("Recuperación híbrida", None)
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            "managed/hybrid.md",
            "hybrid.md",
            Some("text/markdown"),
            200,
            "hybrid-hash",
        )
        .expect("attachment should be registered");
    database
        .replace_attachment_chunks(
            &attachment.id,
            &[
                "Contenido sobre licencias.".to_owned(),
                "Explicación del cálculo estadístico.".to_owned(),
            ],
        )
        .expect("chunks should be stored");
    let connection = database.connect().expect("database should connect");
    let chunks = {
        let mut statement = connection
            .prepare(
                "SELECT id, content_sha256 FROM attachment_chunks
                 WHERE attachment_id = ?1 ORDER BY ordinal",
            )
            .expect("chunk query should prepare");
        statement
            .query_map(params![attachment.id], |row| {
                Ok((row.get::<_, String>(0)?, row.get::<_, String>(1)?))
            })
            .expect("chunks should load")
            .collect::<Result<Vec<_>, _>>()
            .expect("chunks should collect")
    };
    let vector_blob = |values: &[f64]| {
        values
            .iter()
            .flat_map(|value| value.to_le_bytes())
            .collect::<Vec<_>>()
    };
    connection
        .execute(
            "INSERT INTO embedding_records(
                id, source_type, source_id, chunk_index, model,
                dimensions, vector_blob, content_sha256
             ) VALUES
                ('query-vector', 'chat_memory_search', 'hybrid-query', 0,
                 'ollama/local/nomic', 2, ?1, 'query-hash'),
                ('chunk-vector-0', 'attachment_chunk', ?2, 0,
                 'ollama/local/nomic', 2, ?3, ?4),
                ('chunk-vector-1', 'attachment_chunk', ?5, 0,
                 'ollama/local/nomic', 2, ?6, ?7)",
            params![
                vector_blob(&[1.0, 0.0]),
                &chunks[0].0,
                vector_blob(&[0.0, 1.0]),
                &chunks[0].1,
                &chunks[1].0,
                vector_blob(&[0.95, 0.05]),
                &chunks[1].1
            ],
        )
        .expect("vectors should persist");
    drop(connection);

    let selected = database
        .select_attachment_chunks_hybrid(
            &conversation.id,
            std::slice::from_ref(&attachment.id),
            "consulta sin coincidencias literales",
            2,
            500,
            "hybrid-query",
        )
        .expect("hybrid selection should succeed");

    assert_eq!(selected[0].ordinal, 1);
    assert_eq!(selected[0].reason, "Coincidencia semántica");
    let view = database
        .attachment_view(&attachment.id)
        .expect("attachment view should load");
    assert_eq!(view.semantic_indexed_chunks, 2);
    assert_eq!(view.semantic_index_status, "ready");
    assert_eq!(
        view.semantic_index_model.as_deref(),
        Some("ollama/local/nomic")
    );
    cleanup(&database);
}

#[test]
fn document_embedding_batch_is_complete_and_retries_only_failed_chunks() {
    let database = test_database();
    let conversation = database
        .create_conversation("Cola semántica", None)
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            "managed/queue.md",
            "queue.md",
            Some("text/markdown"),
            100,
            "queue-hash",
        )
        .expect("attachment should be registered");
    database
        .replace_attachment_chunks(
            &attachment.id,
            &[
                "Primer fragmento.".to_owned(),
                "Segundo fragmento.".to_owned(),
            ],
        )
        .expect("chunks should be stored");
    let complete_batch = database
        .attachment_chunks_for_embedding(&attachment.id, false)
        .expect("the complete batch should load before submission");
    assert_eq!(complete_batch.len(), 2);
    let first = database
        .next_attachment_chunk_for_embedding(&attachment.id, false)
        .expect("queue should load")
        .expect("first chunk should be available");
    let request = serde_json::json!({
        "inference_kind": "embedding",
        "content": {"metadata": {
            "source_type": "attachment_chunk",
            "source_id": first.id.clone(),
            "content_sha256": first.content_sha256.clone()
        }}
    });
    database
        .prepare_broker_task("queue-task", "queue-key", &request)
        .expect("active task should persist");
    assert!(database
        .next_attachment_chunk_for_embedding(&attachment.id, false)
        .expect("queue should load")
        .is_none());
    database
        .connect()
        .expect("database should connect")
        .execute(
            "UPDATE broker_tasks
             SET local_state = 'terminal', remote_status = 'failed'
             WHERE id = 'queue-task'",
            [],
        )
        .expect("task should fail");

    let next = database
        .next_attachment_chunk_for_embedding(&attachment.id, false)
        .expect("queue should load")
        .expect("second chunk should remain available");
    assert_ne!(next.id, first.id);
    let retry = database
        .next_attachment_chunk_for_embedding(&attachment.id, true)
        .expect("retry queue should load")
        .expect("failed chunk should be retryable");
    assert_eq!(retry.id, first.id);
    cleanup(&database);
}

#[test]
fn document_context_failure_does_not_invalidate_upload_and_can_be_retried() {
    let database = test_database();
    let conversation = database
        .create_conversation("Reintento de contexto", None)
        .expect("conversation should be created");
    let attachment = database
        .register_attachment(
            &conversation.id,
            "managed/manual.pdf",
            "manual.pdf",
            Some("application/pdf"),
            240_000,
            "manual-hash",
        )
        .expect("attachment should be registered");
    database
        .update_attachment_ingestion(
            &attachment.id,
            "ready",
            Some("broker-manual"),
            Some("document"),
            Some("docling"),
            None,
            None,
        )
        .expect("upload should be ready");
    database
        .record_attachment_context_failure(
            &attachment.id,
            &serde_json::json!({"message": "falló la descarga del Markdown"}),
        )
        .expect("context failure should be recorded");

    let failed = database
        .attachment_view(&attachment.id)
        .expect("attachment should remain visible");
    assert_eq!(failed.ingestion_status, "ready");
    assert_eq!(failed.context_status, "failed");
    assert_eq!(
        failed.context_error,
        Some(serde_json::json!({"message": "falló la descarga del Markdown"}))
    );

    database
        .reset_attachment_context_for_retry(&attachment.id)
        .expect("context retry should be accepted");
    let pending = database
        .attachment_view(&attachment.id)
        .expect("attachment should remain visible");
    assert_eq!(pending.ingestion_status, "ready");
    assert_eq!(pending.context_status, "pending");
    assert!(pending.context_error.is_none());
    cleanup(&database);
}
