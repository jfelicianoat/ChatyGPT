//! Investigacion profunda: plan congelado, pasos reales y fuentes web.

use super::comunes::{cleanup, test_database};
use crate::broker::TaskState;
use crate::db::{markdown_web_sources, ContextMessage, ConversationExecutionPreferences};

/// Investigación profunda y recuperación semántica ya conviven.
///
/// Antes, activar ambos controles descartaba la recuperación en silencio.
/// Ahora el plan se congela en la primera etapa, sobrevive a un reinicio y
/// la segunda etapa abre el mismo expediente de investigación que abriría
/// el camino directo.
#[test]
fn semantic_workflow_carries_a_frozen_research_plan_into_its_second_stage() {
    let database = test_database();
    let conversation = database
        .create_conversation("Investigación con contexto", None)
        .expect("conversation should be created");
    let embedding_request = serde_json::json!({
        "inference_kind": "embedding",
        "content": {"metadata": {
            "source_type": "chat_memory_search",
            "source_id": "research-workflow",
            "content_sha256": "research-hash"
        }}
    });
    let context = vec![ContextMessage {
        message_id: "research-user".to_owned(),
        role: "user".to_owned(),
        text: "Contrasta el informe adjunto con fuentes públicas".to_owned(),
    }];
    let plan = serde_json::json!({ "skills": ["web_search"], "client_tools": ["fetch_url"], "max_iterations": 12 });
    database
        .prepare_semantic_chat_turn(
            "research-workflow",
            &conversation.id,
            "research-user",
            "research-assistant",
            "research-embedding-task",
            "research-embedding-key",
            "Contrasta el informe adjunto con fuentes públicas",
            &embedding_request,
            &context,
            &[],
            false,
            false,
            &ConversationExecutionPreferences::default(),
            Some(&plan),
        )
        .expect("semantic research turn should persist");

    // El plan se recupera intacto, que es lo que hace posible reanudar
    // tras un reinicio sin volver a negociar capacidades con el Broker.
    let workflow = database
        .semantic_chat_workflow_for_task("research-embedding-task")
        .expect("workflow should load")
        .expect("workflow should exist");
    assert_eq!(workflow.research_plan.as_ref(), Some(&plan));
    assert_eq!(workflow.status, "searching");

    // Un turno semántico ordinario sigue sin plan.
    database
        .prepare_semantic_chat_turn(
            "plain-workflow",
            &conversation.id,
            "plain-user",
            "plain-assistant",
            "plain-embedding-task",
            "plain-embedding-key",
            "Resume lo que ya hemos hablado",
            &embedding_request,
            &context,
            &[],
            false,
            false,
            &ConversationExecutionPreferences::default(),
            None,
        )
        .expect("plain semantic turn should persist");
    assert!(database
        .semantic_chat_workflow_for_task("plain-embedding-task")
        .expect("workflow should load")
        .expect("workflow should exist")
        .research_plan
        .is_none());

    // La segunda etapa abre el expediente durable de la investigación.
    let chat_request = serde_json::json!({
        "idempotency_key": "chatygpt:semantic-chat:research-workflow",
        "inference_kind": "chat",
        "content": {
            "prompt": "Ejecuta una investigación profunda y trazable.",
            "metadata": {"workflow_kind": "deep_research"}
        }
    });
    database
        .prepare_semantic_chat_submission(
            "research-workflow",
            "research-chat-task",
            "chatygpt:semantic-chat:research-workflow",
            &chat_request,
            &[],
            &[],
        )
        .expect("second stage should persist");

    let view = database
        .conversation_view(&conversation.id)
        .expect("conversation should load");
    assert_eq!(view.research_runs.len(), 1);
    assert_eq!(view.research_runs[0].status, "planning");
    // Sin etapas fijas: los pasos aparecen cuando el modelo pide una
    // herramienta, no dibujados de antemano.
    assert_eq!(view.research_runs[0].steps.len(), 0);
    assert_eq!(
        view.research_runs[0].objective,
        "Contrasta el informe adjunto con fuentes públicas"
    );

    let connection = database.connect().expect("connection should open");
    let audited: i64 = connection
        .query_row(
            "SELECT COUNT(*) FROM audit_events WHERE event_type = 'research.started'",
            [],
            |row| row.get(0),
        )
        .expect("audit count should succeed");
    assert_eq!(audited, 1);
    drop(connection);

    cleanup(&database);
}

/// Cada herramienta ejecutada es un paso real, con su parámetro visible.
///
/// Sustituye a las tres etapas fijas: aquellas eran una plantilla dibujada
/// antes de que ocurriera nada, y decían lo mismo en toda investigación.
#[test]
fn executed_tools_become_the_real_research_steps() {
    let database = test_database();
    let conversation = database
        .create_conversation("Investigación con pasos reales", None)
        .expect("conversation should be created");
    let request = serde_json::json!({
        "idempotency_key": "research:1:1",
        "inference_kind": "chat",
        "content": {
            "prompt": "Investiga la normativa",
            "metadata": {"workflow_kind": "deep_research"}
        }
    });
    database
        .prepare_chat_turn_with_project_instruction(
            &conversation.id,
            "msg-user",
            "msg-assistant",
            "local-research",
            "research:1:1",
            "Investiga la normativa",
            &request,
            &[],
            None,
            None,
            &[],
            &[],
            &[],
        )
        .expect("research turn should persist");

    // Al abrirse, el expediente no tiene ningún paso: nada ha ocurrido aún.
    let inicial = database
        .conversation_view(&conversation.id)
        .expect("conversation should load");
    assert_eq!(inicial.research_runs.len(), 1);
    assert!(inicial.research_runs[0].steps.is_empty());

    database
        .record_research_tool_step(
            "local-research",
            "call_1",
            "fetch_url",
            "https://example.org/normativa",
            "completed",
            &serde_json::json!({"url": "https://example.org/normativa", "truncated": false}),
        )
        .expect("el paso debe registrarse");
    database
        .record_research_tool_step(
            "local-research",
            "call_2",
            "fetch_url",
            "https://example.org/roto",
            "failed",
            &serde_json::json!({"error": "la página respondió HTTP 500"}),
        )
        .expect("un fallo también es un paso");

    let view = database
        .conversation_view(&conversation.id)
        .expect("conversation should load");
    let steps = &view.research_runs[0].steps;
    assert_eq!(steps.len(), 2);
    // El parámetro con el que se llamó es visible, que es lo que faltaba:
    // «abrí esta URL» en vez de «buscar y contrastar fuentes».
    assert_eq!(steps[0].title, "fetch_url: https://example.org/normativa");
    assert_eq!(steps[0].kind, "research");
    assert_eq!(steps[0].status, "completed");
    // Un fallo se registra como paso, no se omite: el recorrido incluye
    // las fuentes que no se pudieron leer.
    assert_eq!(steps[1].status, "failed");

    // Reejecutar la misma llamada tras un reinicio actualiza el paso, no
    // añade uno nuevo: la identidad es la llamada, no su posición.
    database
        .record_research_tool_step(
            "local-research",
            "call_2",
            "fetch_url",
            "https://example.org/roto",
            "completed",
            &serde_json::json!({"url": "https://example.org/roto"}),
        )
        .expect("el reintento debe actualizar el mismo paso");
    let reintentado = database
        .conversation_view(&conversation.id)
        .expect("conversation should load");
    assert_eq!(reintentado.research_runs[0].steps.len(), 2);
    assert_eq!(reintentado.research_runs[0].steps[1].status, "completed");

    cleanup(&database);
}

#[test]
fn deep_research_run_tracks_durable_steps_and_terminal_sources() {
    let database = test_database();
    let conversation = database
        .create_conversation("Investigación durable", None)
        .expect("conversation should be created");
    let user_message_id = "research-user";
    let assistant_message_id = "research-assistant";
    let task_id = "research-task";
    let objective = "Compara dos marcos regulatorios";
    database
        .prepare_chat_turn(
            &conversation.id,
            user_message_id,
            assistant_message_id,
            task_id,
            "research-idempotency",
            objective,
            &serde_json::json!({
                "inference_kind": "chat",
                "content": {
                    "metadata": {"workflow_kind": "deep_research"}
                }
            }),
            &[ContextMessage {
                message_id: user_message_id.to_owned(),
                role: "user".to_owned(),
                text: objective.to_owned(),
            }],
            &[],
            &[],
            &[],
        )
        .expect("research turn should be prepared");
    let initial = database
        .conversation_view(&conversation.id)
        .expect("research run should load");
    assert_eq!(initial.research_runs.len(), 1);
    assert_eq!(initial.research_runs[0].status, "planning");
    assert_eq!(initial.research_runs[0].steps.len(), 0);

    let synthesizing: TaskState = serde_json::from_value(serde_json::json!({
        "task_id": "remote-research",
        "status": "synthesizing",
        "request_id": null,
        "created_at": "2026-07-30T12:00:00Z",
        "updated_at": "2026-07-30T12:00:05Z",
        "execution_strategy": "agent",
        "execution_preset": "slow",
        "selection_mode": "adaptive",
        "progress": {"phase": "synthesizing"},
        "result": null,
        "error": null
    }))
    .expect("synthesizing state should parse");
    database
        .record_remote_state(task_id, &synthesizing)
        .expect("research progress should persist");
    let synthesizing_view = database
        .conversation_view(&conversation.id)
        .expect("research progress should load");
    // La fase remota sigue describiendo el expediente completo, pero ya no
    // inventa el estado de ningún paso: los pasos son las herramientas que
    // se ejecutaron, y no hay ninguna todavía.
    assert_eq!(synthesizing_view.research_runs[0].status, "synthesizing");
    assert!(synthesizing_view.research_runs[0].steps.is_empty());

    let completed: TaskState = serde_json::from_value(serde_json::json!({
        "task_id": "remote-research",
        "status": "completed",
        "request_id": null,
        "created_at": "2026-07-30T12:00:00Z",
        "updated_at": "2026-07-30T12:00:10Z",
        "execution_strategy": "agent",
        "execution_preset": "slow",
        "selection_mode": "adaptive",
        "progress": {"phase": "completed"},
        "result": {
            "result_markdown": "Informe con [Fuente A](https://example.com/report#section) y https://example.org/data. Duplicada: https://example.com/report."
        },
        "error": null
    }))
    .expect("completed state should parse");
    database
        .record_remote_state(task_id, &completed)
        .expect("research completion should persist");
    let completed_view = database
        .conversation_view(&conversation.id)
        .expect("completed research should load");
    assert_eq!(completed_view.research_runs[0].status, "completed");
    assert!(completed_view.research_runs[0]
        .steps
        .iter()
        .all(|step| step.status == "completed"));
    assert_eq!(completed_view.research_runs[0].source_count, 2);
    let assistant = completed_view
        .messages
        .iter()
        .find(|message| message.id == assistant_message_id)
        .expect("assistant message should load");
    assert_eq!(assistant.sources.len(), 2);
    assert_eq!(assistant.sources[0].title, "Fuente A");
    assert_eq!(
        assistant.sources[0].url.as_deref(),
        Some("https://example.com/report")
    );
    assert_eq!(
        assistant.sources[1].url.as_deref(),
        Some("https://example.org/data")
    );
    cleanup(&database);
}

#[test]
fn markdown_web_sources_are_bounded_deduplicated_and_http_only() {
    let sources = markdown_web_sources(
        "[Informe](https://example.com/a#one) \
         https://example.com/a#two \
         [Correo](mailto:test@example.com) \
         https://user:secret@example.net/private \
         [Datos](http://data.example.org/table).",
    );
    assert_eq!(
        sources,
        vec![
            ("Informe".to_owned(), "https://example.com/a".to_owned()),
            (
                "Datos".to_owned(),
                "http://data.example.org/table".to_owned()
            )
        ]
    );
}
