//! AI OS Core como director: pide permiso antes de mandar trabajo a Broker AI.
//!
//! Core es el plano de control del ecosistema del TFM. ChatyGPT no lo necesita:
//! sin `CHATYGPT_CORE_URL` todo sigue igual. Con él, `BrokerClient::create_task`
//! evalúa antes la política `chatygpt:broker` y solo envía con un ALLOW
//! ejecutable:
//!
//! - denegado: `AppError::CoreDenied`, permanente (la tarea queda huérfana con el
//!   motivo de Core y no se reintenta sola);
//! - Core no contesta: `AppError::BrokerTransport`, que se reintenta como
//!   cualquier caída de red. **No se envía nada** mientras tanto: quien configuró
//!   un director no quiere que se le salte cuando se cae;
//! - cada tarea deja recibos en Core con su clave de idempotencia (`accepted` al
//!   enviarla y el estado terminal al verla terminar).

use std::collections::HashMap;
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use reqwest::{Client, StatusCode};
use serde_json::{json, Value};
use url::Url;

use crate::error::AppError;

pub const POLICY_SCOPE: &str = "chatygpt:broker";
/// Cómo se presenta ChatyGPT ante Core (debe coincidir con su token por app).
pub const PRINCIPAL: &str = "chatygpt";
/// Un salto para el monitor de Core nunca debe hacer esperar al turno.
const HOP_TIMEOUT: Duration = Duration::from_secs(3);

/// El flujo del monitor de Core para un turno: su clave de idempotencia, en un
/// formato que Core acepta (`[A-Za-z0-9_.:-]`, 128 caracteres como mucho).
pub fn flow_id_for(operation_id: &str) -> String {
    let flow: String = operation_id
        .chars()
        .map(|c| {
            if c.is_ascii_alphanumeric() || "_.:-".contains(c) {
                c
            } else {
                '_'
            }
        })
        .take(128)
        .collect();
    if flow.is_empty() {
        "chatygpt".to_owned()
    } else {
        flow
    }
}

#[derive(Clone)]
pub struct CoreGate {
    base_url: Url,
    token: Option<String>,
    http: Client,
    /// Cuándo salió cada turno hacia el Broker (latencia en el monitor de Core).
    sent_at: Arc<Mutex<HashMap<String, Instant>>>,
}

impl CoreGate {
    pub fn new(base_url: &str, token: Option<String>) -> Result<Self, AppError> {
        let mut base_url = Url::parse(base_url).map_err(|error| {
            AppError::Validation(format!("CHATYGPT_CORE_URL no es válida: {error}"))
        })?;
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(AppError::Validation(
                "CHATYGPT_CORE_URL debe usar http o https".to_owned(),
            ));
        }
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Ok(Self {
            base_url,
            token: token.filter(|value| !value.trim().is_empty()),
            http,
            sent_at: Arc::default(),
        })
    }

    /// `CHATYGPT_CORE_URL` (y su token `CHATYGPT_CORE_TOKEN`), o `None`.
    pub fn from_env() -> Result<Option<Self>, AppError> {
        match std::env::var("CHATYGPT_CORE_URL") {
            Ok(url) if !url.trim().is_empty() => {
                Self::new(url.trim(), std::env::var("CHATYGPT_CORE_TOKEN").ok()).map(Some)
            }
            _ => Ok(None),
        }
    }

    /// Error si Core no da un ALLOW ejecutable: entonces no se envía nada.
    pub async fn authorize(&self, request: &Value) -> Result<(), AppError> {
        let classification = request
            .pointer("/risk/data_classification")
            .and_then(Value::as_str)
            // Conversaciones de personas reales: sin declarar, lo prudente
            .unwrap_or("confidential");
        let key = request
            .get("idempotency_key")
            .and_then(Value::as_str)
            .unwrap_or("");
        let body = json!({
            "scope": POLICY_SCOPE,
            "principal_id": PRINCIPAL,
            "resource_id": key,
            "context": {
                "operation_type": request.get("inference_kind").and_then(Value::as_str).unwrap_or("chat"),
                "data_classification": classification,
                "flow_id": flow_id_for(key),
            },
        });
        let decision = self.post("api/v1/policies/evaluate", &body).await?;
        let authorization = decision
            .get("authorization")
            .and_then(Value::as_str)
            .unwrap_or("INDETERMINATE");
        if authorization == "ALLOW" && decision.get("executable") == Some(&Value::Bool(true)) {
            return Ok(());
        }
        let missing = decision
            .get("unknown_inputs")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(Value::as_str)
                    .collect::<Vec<_>>()
                    .join(", ")
            })
            .filter(|text| !text.is_empty());
        Err(AppError::CoreDenied(match (authorization, missing) {
            ("REQUIRE_APPROVAL", _) => format!(
                "AI OS Core pide una aprobación humana antes de enviar esto (políticas de «{POLICY_SCOPE}»)"
            ),
            ("INDETERMINATE", missing) => format!(
                "AI OS Core no puede decidir (le falta: {}); por seguridad no se envía",
                missing.unwrap_or_else(|| "algún dato".to_owned())
            ),
            (other, _) => format!(
                "AI OS Core no autoriza este envío ({other}); revisa las políticas de «{POLICY_SCOPE}»"
            ),
        }))
    }

    /// Un recibo perdido no tumba la tarea: Core verá el siguiente estado.
    ///
    /// También cuenta el viaje al monitor de Core: enviar es la petición al
    /// Broker y el estado terminal, su respuesta.
    pub async fn receipt(&self, operation_id: &str, status: &str, task_id: &str) {
        let flow = flow_id_for(operation_id);
        let body = json!({
            "operation_id": operation_id,
            "receiver_service": "ai_broker",
            "status": status,
            "evidence": { "task_id": task_id, "flow_id": flow },
        });
        let _ = self.post("api/v1/operations/receipts", &body).await;
        let mut details = json!({ "task_id": task_id });
        let kind = match status {
            "accepted" => {
                if let Ok(mut sent) = self.sent_at.lock() {
                    sent.insert(operation_id.to_owned(), Instant::now());
                }
                "request"
            }
            "succeeded" | "failed_terminal" | "cancelled" => {
                let began = self
                    .sent_at
                    .lock()
                    .ok()
                    .and_then(|mut sent| sent.remove(operation_id));
                if let Some(began) = began {
                    details["latency_ms"] = json!(began.elapsed().as_millis() as u64);
                }
                details["status"] = json!(status);
                if status == "succeeded" {
                    "response"
                } else {
                    "error"
                }
            }
            _ => return,
        };
        self.hop(&flow, kind, status, details).await;
    }

    /// Enviar y olvidar: el monitor es secundario y jamás para un turno.
    pub async fn hop(&self, flow_id: &str, kind: &str, summary: &str, details: Value) {
        self.hop_to(flow_id, "ai_broker", kind, summary, details)
            .await;
    }

    /// Un salto hacia cualquier pieza (Athena, el Broker…), también enviar y olvidar.
    pub async fn hop_to(
        &self,
        flow_id: &str,
        target: &str,
        kind: &str,
        summary: &str,
        details: Value,
    ) {
        // Core sólo admite un objeto: un `null` haría que rechazara el salto con 422
        let details = if details.is_object() {
            details
        } else {
            json!({})
        };
        let body = json!({
            "flow_id": flow_id,
            "source_system": PRINCIPAL,
            "target_system": target,
            "kind": kind,
            "summary": summary.chars().take(200).collect::<String>(),
            "details": details,
        });
        let _ = self
            .post_within("api/v1/flows/hops", &body, Some(HOP_TIMEOUT))
            .await;
    }

    async fn post(&self, path: &str, body: &Value) -> Result<Value, AppError> {
        self.post_within(path, body, None).await
    }

    async fn post_within(
        &self,
        path: &str,
        body: &Value,
        timeout: Option<Duration>,
    ) -> Result<Value, AppError> {
        let url = self
            .base_url
            .join(path)
            .map_err(|error| AppError::Validation(error.to_string()))?;
        let mut builder = self
            .http
            .post(url)
            .header("X-Principal-Id", PRINCIPAL)
            .json(body);
        if let Some(timeout) = timeout {
            builder = builder.timeout(timeout);
        }
        if let Some(token) = &self.token {
            builder = builder.bearer_auth(token.trim());
        }
        let unavailable = |detail: String| {
            AppError::BrokerTransport(format!(
                "AI OS Core no contesta ({detail}); por seguridad no se ha enviado nada a Broker AI"
            ))
        };
        let response = builder
            .send()
            .await
            .map_err(|error| unavailable(crate::error::describe_network_failure(&error)))?;
        let status = response.status();
        if status.is_client_error() || status.is_server_error() {
            return Err(unavailable(format!("HTTP {}", status.as_u16())));
        }
        if status == StatusCode::NO_CONTENT {
            return Ok(Value::Null);
        }
        response
            .json::<Value>()
            .await
            .map_err(|error| unavailable(error.to_string()))
    }
}

/// Estado del recibo para un estado terminal del Broker.
pub fn terminal_receipt(status: &str) -> &'static str {
    match status {
        "completed" => "succeeded",
        "cancelled" => "cancelled",
        _ => "failed_terminal",
    }
}

#[cfg(test)]
mod tests {
    use super::super::simulated::{
        accepted_task, completed_chat_result, task_state, ScriptedResponse, SimulatedBroker,
    };
    use super::super::BrokerClient;
    use crate::error::AppError;
    use serde_json::json;

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(future)
    }

    fn request() -> serde_json::Value {
        json!({
            "idempotency_key": "chatygpt:turno-1",
            "inference_kind": "chat",
            "content": {"prompt": "hola"},
            "risk": {"data_classification": "confidential"},
        })
    }

    fn core_with(decision: &str) -> SimulatedBroker {
        let core = SimulatedBroker::start();
        core.always(
            "POST /api/v1/policies/evaluate",
            ScriptedResponse::ok(json!({
                "authorization": decision,
                "executable": decision == "ALLOW",
                "unknown_inputs": [],
            })),
        );
        core.always(
            "POST /api/v1/operations/receipts",
            ScriptedResponse::ok(json!({})),
        );
        core.always("POST /api/v1/flows/hops", ScriptedResponse::ok(json!({})));
        core
    }

    fn finished_broker() -> SimulatedBroker {
        let broker = SimulatedBroker::start();
        broker.always(
            "POST /api/v1/tasks",
            ScriptedResponse::accepted(accepted_task("task-1")),
        );
        broker.always(
            "GET /api/v1/tasks/{id}",
            ScriptedResponse::ok(task_state(
                "task-1",
                "completed",
                Some(completed_chat_result("hola")),
            )),
        );
        broker
    }

    fn client(broker: &SimulatedBroker, core_url: &str) -> BrokerClient {
        BrokerClient::for_base_url(broker.base_url())
            .and_then(|client| client.with_core(core_url, Some("tok-chaty")))
            .expect("el cliente debe construirse")
    }

    #[test]
    fn without_allow_nothing_reaches_the_broker() {
        for decision in ["DENY", "REQUIRE_APPROVAL", "INDETERMINATE"] {
            let broker = SimulatedBroker::start();
            let core = core_with(decision);
            let error = block_on(client(&broker, core.base_url()).create_task(&request()))
                .expect_err("Core no autoriza");
            assert!(
                matches!(error, AppError::CoreDenied(_)),
                "{decision}: {error}"
            );
            assert!(broker.requests_to("POST", "/api/v1/tasks").is_empty());
            let evaluation = core.requests_to("POST", "/api/v1/policies/evaluate");
            assert_eq!(evaluation.len(), 1);
            let sent = &evaluation[0];
            assert_eq!(sent.headers["x-principal-id"], "chatygpt");
            assert_eq!(sent.headers["authorization"], "Bearer tok-chaty");
            assert_eq!(sent.body["scope"], "chatygpt:broker");
            assert_eq!(sent.body["context"]["data_classification"], "confidential");
        }
    }

    #[test]
    fn core_down_is_retryable_and_sends_nothing() {
        let broker = SimulatedBroker::start();
        let error = block_on(client(&broker, "http://127.0.0.1:9").create_task(&request()))
            .expect_err("Core caído");
        assert!(
            matches!(&error, AppError::BrokerTransport(text) if text.contains("AI OS Core")),
            "{error}"
        );
        assert!(broker.requests_to("POST", "/api/v1/tasks").is_empty());
    }

    #[test]
    fn allow_sends_and_leaves_receipts() {
        let broker = SimulatedBroker::start();
        broker.always(
            "POST /api/v1/tasks",
            ScriptedResponse::accepted(accepted_task("task-1")),
        );
        broker.always(
            "GET /api/v1/tasks/{id}",
            ScriptedResponse::ok(task_state(
                "task-1",
                "completed",
                Some(completed_chat_result("hola")),
            )),
        );
        let core = core_with("ALLOW");
        let chaty = client(&broker, core.base_url());
        let accepted = block_on(chaty.create_task(&request())).expect("con ALLOW se envía");
        block_on(chaty.get_task(&accepted.task_id)).expect("estado legible");
        assert_eq!(broker.requests_to("POST", "/api/v1/tasks").len(), 1);
        let receipts = core.requests_to("POST", "/api/v1/operations/receipts");
        let statuses: Vec<_> = receipts.iter().map(|r| r.body["status"].clone()).collect();
        assert_eq!(statuses, [json!("accepted"), json!("succeeded")]);
        assert!(receipts
            .iter()
            .all(|r| r.body["operation_id"] == "chatygpt:turno-1"));
    }

    #[test]
    fn the_turn_is_seen_in_the_core_monitor() {
        // ChatyGPT 0.4.3: autorización, recibos y saltos comparten el flujo del turno
        let broker = finished_broker();
        let core = core_with("ALLOW");
        let chaty = client(&broker, core.base_url());
        let accepted = block_on(chaty.create_task(&request())).expect("con ALLOW se envía");
        block_on(chaty.get_task(&accepted.task_id)).expect("estado legible");
        let evaluation = core.requests_to("POST", "/api/v1/policies/evaluate");
        assert_eq!(evaluation[0].body["context"]["flow_id"], "chatygpt:turno-1");
        let receipts = core.requests_to("POST", "/api/v1/operations/receipts");
        assert!(receipts
            .iter()
            .all(|r| r.body["evidence"]["flow_id"] == "chatygpt:turno-1"));
        let hops = core.requests_to("POST", "/api/v1/flows/hops");
        let kinds: Vec<_> = hops.iter().map(|h| h.body["kind"].clone()).collect();
        assert_eq!(kinds, [json!("request"), json!("response")]);
        assert!(hops.iter().all(|h| h.body["flow_id"] == "chatygpt:turno-1"
            && h.body["source_system"] == "chatygpt"
            && h.body["target_system"] == "ai_broker"));
        assert!(hops[1].body["details"]["latency_ms"].is_u64());
    }

    #[test]
    fn a_monitor_that_is_down_never_stops_the_turn() {
        let broker = finished_broker();
        let core = core_with("ALLOW");
        core.always("POST /api/v1/flows/hops", ScriptedResponse::status(500));
        let chaty = client(&broker, core.base_url());
        let accepted =
            block_on(chaty.create_task(&request())).expect("el monitor no frena el envío");
        let state = block_on(chaty.get_task(&accepted.task_id)).expect("ni la lectura");
        assert!(state.status.is_terminal());
    }

    #[test]
    fn flow_ids_are_always_acceptable_to_core() {
        assert_eq!(
            super::flow_id_for("chatygpt:turno 1/ñ"),
            "chatygpt:turno_1__"
        );
        assert_eq!(super::flow_id_for(&"x".repeat(500)).len(), 128);
        assert_eq!(super::flow_id_for(""), "chatygpt");
    }

    #[test]
    fn terminal_states_map_to_core_receipts() {
        assert_eq!(super::terminal_receipt("failed"), "failed_terminal");
        assert_eq!(super::terminal_receipt("completed"), "succeeded");
    }
}
