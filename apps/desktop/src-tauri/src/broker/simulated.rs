//! Broker AI simulado para pruebas de integración locales.
//!
//! Los caminos de envío, sondeo, reintento y recuperación solo se ejercitan de
//! verdad contra un servidor: son bucles asíncronos que reaccionan a códigos
//! HTTP y a transiciones de estado, y ninguna prueba unitaria sobre funciones
//! puras demuestra que terminan donde deben. Este módulo levanta un servidor
//! HTTP real en `127.0.0.1` con un puerto efímero y responde según un guion.
//!
//! Es deliberadamente mínimo y sin dependencias nuevas: habla HTTP/1.1 con
//! `Connection: close`, que es todo lo que el cliente necesita. Lo que aporta no
//! es fidelidad de servidor sino dos cosas que las pruebas necesitan y un Broker
//! real no da: **respuestas programables** —incluidos fallos transitorios y
//! permanentes que serían imposibles de provocar a voluntad— y un **registro de
//! peticiones** con el que comprobar que un reintento no duplica una tarea.

use std::collections::{HashMap, VecDeque};
use std::io::{BufRead, BufReader, Read, Write};
use std::net::{TcpListener, TcpStream};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::{Arc, Mutex};
use std::time::{Duration, Instant};

use serde_json::{json, Value};

/// Respuesta programada para una ruta.
#[derive(Debug, Clone)]
pub struct ScriptedResponse {
    pub status: u16,
    pub body: Vec<u8>,
    pub content_type: &'static str,
}

impl ScriptedResponse {
    pub fn ok(body: Value) -> Self {
        Self {
            status: 200,
            body: body.to_string().into_bytes(),
            content_type: "application/json",
        }
    }

    pub fn accepted(body: Value) -> Self {
        Self {
            status: 202,
            body: body.to_string().into_bytes(),
            content_type: "application/json",
        }
    }

    /// Cuerpo de texto plano, como el Markdown convertido de un adjunto.
    pub fn text(body: &str) -> Self {
        Self {
            status: 200,
            body: body.as_bytes().to_vec(),
            content_type: "text/markdown; charset=utf-8",
        }
    }

    /// Cuerpo arbitrario: sirve para comprobar qué ocurre con datos no UTF-8.
    pub fn bytes(body: Vec<u8>) -> Self {
        Self {
            status: 200,
            body,
            content_type: "application/octet-stream",
        }
    }

    /// Respuesta con éxito pero cuerpo que no cumple el contrato esperado.
    pub fn malformed() -> Self {
        Self {
            status: 200,
            body: b"{esto no es JSON}".to_vec(),
            content_type: "application/json",
        }
    }

    /// Fallo transitorio: el cliente debe reintentar sin dar la tarea por perdida.
    pub fn transient() -> Self {
        Self {
            status: 503,
            body: json!({"detail": "el Broker está saturado"})
                .to_string()
                .into_bytes(),
            content_type: "application/json",
        }
    }

    /// Fallo permanente de contrato: reintentar no puede arreglarlo.
    pub fn permanent() -> Self {
        Self {
            status: 422,
            body: json!({"detail": "la petición no cumple el contrato"})
                .to_string()
                .into_bytes(),
            content_type: "application/json",
        }
    }

    /// Respuesta con un código arbitrario y cuerpo vacío.
    pub fn status(status: u16) -> Self {
        Self {
            status,
            body: Vec::new(),
            content_type: "application/json",
        }
    }
}

/// Petición recibida, para comprobar duplicados, idempotencia y cabeceras.
#[derive(Debug, Clone)]
pub struct RecordedRequest {
    pub method: String,
    pub path: String,
    /// Cuerpo interpretado como JSON, o `Null` si no lo era (por ejemplo,
    /// multipart).
    pub body: Value,
    /// Cuerpo en bruto, como texto con pérdidas: permite comprobar que un
    /// multipart lleva el nombre y el contenido del archivo.
    pub raw_body: String,
    /// Cabeceras en minúsculas. Sirve para comprobar que el token viaja.
    pub headers: HashMap<String, String>,
}

#[derive(Default)]
struct SimulatedState {
    /// Guion por ruta: cada llamada consume la siguiente respuesta.
    scripted: HashMap<String, VecDeque<ScriptedResponse>>,
    /// Respuesta usada cuando el guion de esa ruta se agota.
    fallback: HashMap<String, ScriptedResponse>,
    /// Cambios de estado provocados por una llamada previa.
    ///
    /// Modelan la causalidad real del Broker: una tarea pasa a completada
    /// *porque* recibió los resultados de una herramienta, no porque haya
    /// transcurrido un tiempo. Sin esto, una prueba dependería de qué llega
    /// antes y sería intermitente.
    transitions: HashMap<String, Vec<(String, ScriptedResponse)>>,
    requests: Vec<RecordedRequest>,
}

pub struct SimulatedBroker {
    base_url: String,
    state: Arc<Mutex<SimulatedState>>,
    shutdown: Arc<AtomicBool>,
}

impl SimulatedBroker {
    /// Arranca el servidor en un puerto efímero de loopback.
    pub fn start() -> Self {
        let listener = TcpListener::bind("127.0.0.1:0").expect("el simulador debe poder escuchar");
        let address = listener
            .local_addr()
            .expect("el simulador debe tener dirección");
        listener
            .set_nonblocking(true)
            .expect("el simulador debe poder aceptar sin bloquear");
        let state = Arc::new(Mutex::new(SimulatedState::default()));
        let shutdown = Arc::new(AtomicBool::new(false));
        let thread_state = Arc::clone(&state);
        let thread_shutdown = Arc::clone(&shutdown);
        std::thread::spawn(move || {
            while !thread_shutdown.load(Ordering::Relaxed) {
                match listener.accept() {
                    Ok((stream, _)) => {
                        let connection_state = Arc::clone(&thread_state);
                        // Un hilo por conexión: el cliente puede sondear una
                        // tarea mientras consulta capacidades, y una cola
                        // secuencial convertiría eso en un bloqueo mutuo.
                        std::thread::spawn(move || {
                            let _ = serve_connection(stream, connection_state);
                        });
                    }
                    Err(ref error) if error.kind() == std::io::ErrorKind::WouldBlock => {
                        std::thread::sleep(Duration::from_millis(5));
                    }
                    Err(_) => break,
                }
            }
        });
        Self {
            base_url: format!("http://{address}"),
            state,
            shutdown,
        }
    }

    pub fn base_url(&self) -> &str {
        &self.base_url
    }

    /// Añade una respuesta al final del guion de una ruta.
    pub fn script(&self, route: &str, response: ScriptedResponse) -> &Self {
        self.state
            .lock()
            .expect("el estado del simulador debe estar disponible")
            .scripted
            .entry(route.to_owned())
            .or_default()
            .push_back(response);
        self
    }

    /// Fija la respuesta que se repetirá cuando el guion se agote.
    pub fn always(&self, route: &str, response: ScriptedResponse) -> &Self {
        self.state
            .lock()
            .expect("el estado del simulador debe estar disponible")
            .fallback
            .insert(route.to_owned(), response);
        self
    }

    /// Tras recibir `trigger`, la ruta `target` pasa a responder `response`.
    ///
    /// Expresa la causalidad del Broker: es la llamada la que cambia el estado
    /// de la tarea, no el paso del tiempo.
    pub fn after(&self, trigger: &str, target: &str, response: ScriptedResponse) -> &Self {
        self.state
            .lock()
            .expect("el estado del simulador debe estar disponible")
            .transitions
            .entry(trigger.to_owned())
            .or_default()
            .push((target.to_owned(), response));
        self
    }

    pub fn requests(&self) -> Vec<RecordedRequest> {
        self.state
            .lock()
            .expect("el estado del simulador debe estar disponible")
            .requests
            .clone()
    }

    /// Peticiones recibidas para una ruta concreta.
    pub fn requests_to(&self, method: &str, path: &str) -> Vec<RecordedRequest> {
        self.requests()
            .into_iter()
            .filter(|request| request.method == method && request.path == path)
            .collect()
    }

    /// Espera a que una condición se cumpla, con límite. Devuelve si ocurrió.
    ///
    /// Los bucles de sondeo son asíncronos y deliberadamente espaciados, así que
    /// una prueba no puede afirmar nada inmediatamente después de lanzarlos.
    pub fn wait_until(timeout: Duration, mut condition: impl FnMut() -> bool) -> bool {
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            if condition() {
                return true;
            }
            std::thread::sleep(Duration::from_millis(20));
        }
        condition()
    }
}

impl Drop for SimulatedBroker {
    fn drop(&mut self) {
        self.shutdown.store(true, Ordering::Relaxed);
    }
}

/// Normaliza la ruta a una clave de guion estable.
///
/// Los identificadores remotos los inventa el propio guion, de modo que las
/// pruebas programan `/api/v1/tasks/{id}` sin conocerlos de antemano.
fn route_key(method: &str, path: &str) -> String {
    let path = path.split('?').next().unwrap_or(path);
    if let Some(rest) = path.strip_prefix("/api/v1/tasks/") {
        if let Some(identifier) = rest.strip_suffix("/tool_results") {
            if !identifier.is_empty() && !identifier.contains('/') {
                return format!("{method} /api/v1/tasks/{{id}}/tool_results");
            }
        }
        // Solo se sustituye un segmento único: `/tasks/x/y` no es una tarea y
        // debe poder programarse por su ruta literal.
        if !rest.is_empty() && !rest.contains('/') {
            return format!("{method} /api/v1/tasks/{{id}}");
        }
    }
    if let Some(rest) = path.strip_prefix("/api/v1/files/") {
        if !rest.is_empty() && !rest.contains('/') {
            return format!("{method} /api/v1/files/{{id}}");
        }
    }
    format!("{method} {path}")
}

fn serve_connection(
    mut stream: TcpStream,
    state: Arc<Mutex<SimulatedState>>,
) -> std::io::Result<()> {
    // El listener acepta sin bloquear para poder apagarse, pero el socket
    // aceptado hereda ese modo en Windows: sin esto, leer la petición devuelve
    // `WouldBlock` en cuanto los bytes no han llegado todavía y la conexión se
    // cierra sin responder, lo que aparece como un fallo intermitente de red.
    stream.set_nonblocking(false)?;
    stream.set_read_timeout(Some(Duration::from_secs(5)))?;
    let mut reader = BufReader::new(stream.try_clone()?);
    let mut request_line = String::new();
    if reader.read_line(&mut request_line)? == 0 {
        return Ok(());
    }
    let mut parts = request_line.split_whitespace();
    let method = parts.next().unwrap_or_default().to_owned();
    let path = parts.next().unwrap_or_default().to_owned();

    let mut headers = HashMap::new();
    loop {
        let mut header = String::new();
        if reader.read_line(&mut header)? == 0 {
            break;
        }
        let header = header.trim_end();
        if header.is_empty() {
            break;
        }
        if let Some((name, value)) = header.split_once(':') {
            headers.insert(name.trim().to_ascii_lowercase(), value.trim().to_owned());
        }
    }
    let content_length = headers
        .get("content-length")
        .and_then(|value| value.parse::<usize>().ok())
        .unwrap_or(0);
    let mut body = vec![0_u8; content_length];
    if content_length > 0 {
        reader.read_exact(&mut body)?;
    }

    let key = route_key(&method, &path);
    let response = {
        let mut state = state
            .lock()
            .expect("el estado del simulador debe estar disponible");
        state.requests.push(RecordedRequest {
            method: method.clone(),
            path: path.clone(),
            body: serde_json::from_slice(&body).unwrap_or(Value::Null),
            raw_body: String::from_utf8_lossy(&body).into_owned(),
            headers,
        });
        let response = state
            .scripted
            .get_mut(&key)
            .and_then(VecDeque::pop_front)
            .or_else(|| state.fallback.get(&key).cloned())
            .unwrap_or_else(|| ScriptedResponse {
                status: 404,
                body: json!({"detail": format!("ruta no programada: {key}")})
                    .to_string()
                    .into_bytes(),
                content_type: "application/json",
            });
        // La transición se aplica después de resolver la respuesta: quien
        // dispara el cambio recibe todavía la respuesta de esta llamada.
        if let Some(transitions) = state.transitions.remove(&key) {
            for (target, target_response) in transitions {
                state.scripted.remove(&target);
                state.fallback.insert(target, target_response);
            }
        }
        response
    };

    // La cabecera se escribe aparte del cuerpo porque el cuerpo puede no ser
    // texto válido: `download_text` debe poder recibir bytes no UTF-8.
    let head = format!(
        "HTTP/1.1 {} SIMULADO\r\nContent-Type: {}\r\nContent-Length: {}\r\nConnection: close\r\n\r\n",
        response.status,
        response.content_type,
        response.body.len()
    );
    stream.write_all(head.as_bytes())?;
    stream.write_all(&response.body)?;
    stream.flush()
}

/// Cuerpo de `POST /api/v1/files` aceptado por el Broker.
pub fn accepted_file(file_id: &str, filename: &str, size_bytes: i64, sha256: &str) -> Value {
    json!({
        "file_id": file_id,
        "status": "received",
        "filename": filename,
        "size_bytes": size_bytes,
        "sha256": sha256,
        "created": true,
        "status_url": format!("/api/v1/files/{file_id}")
    })
}

/// Estado de un archivo, opcionalmente con la ubicación de su Markdown.
pub fn file_state(file_id: &str, status: &str, markdown_url: Option<&str>) -> Value {
    json!({
        "file_id": file_id,
        "status": status,
        "filename": "informe.pdf",
        "kind": "document",
        "engine": "docling",
        "size_bytes": 2_048,
        "sha256": "a".repeat(64),
        "meta": {"pages": 3},
        "error": null,
        "created_at": "2026-08-04T10:00:00Z",
        "updated_at": "2026-08-04T10:00:09Z",
        "markdown_url": markdown_url
    })
}

/// Cuerpo de `POST /api/v1/tasks` aceptado por el Broker.
pub fn accepted_task(task_id: &str) -> Value {
    json!({
        "task_id": task_id,
        "status": "queued",
        "execution_strategy": "single",
        "execution_preset": "fast",
        "selection_mode": "auto",
        "status_url": format!("/api/v1/tasks/{task_id}"),
        "cancel_url": format!("/api/v1/tasks/{task_id}")
    })
}

/// Estado de tarea en cualquier fase, con resultado opcional.
pub fn task_state(task_id: &str, status: &str, result: Option<Value>) -> Value {
    json!({
        "task_id": task_id,
        "kind": "inference",
        "status": status,
        "request_id": format!("request_{task_id}"),
        "created_at": "2026-08-04T10:00:00Z",
        "updated_at": "2026-08-04T10:00:05Z",
        "execution_strategy": "single",
        "execution_preset": "fast",
        "selection_mode": "auto",
        "progress": {"phase": status, "invocations_completed": 0, "invocations_total": 1},
        "result": result,
        "error": null
    })
}

/// Estado terminal fallido, con el error tal y como lo publica el Broker.
pub fn failed_task_state(task_id: &str, message: &str) -> Value {
    json!({
        "task_id": task_id,
        "kind": "inference",
        "status": "failed",
        "request_id": format!("request_{task_id}"),
        "created_at": "2026-08-04T10:00:00Z",
        "updated_at": "2026-08-04T10:00:05Z",
        "execution_strategy": "single",
        "execution_preset": "fast",
        "selection_mode": "auto",
        "progress": {"phase": "failed", "invocations_completed": 0, "invocations_total": 1},
        "result": null,
        "error": {"code": "PROVIDER_UNAVAILABLE", "message": message, "retryable": true}
    })
}

/// Tarea detenida a la espera de una decisión sobre una herramienta.
pub fn waiting_for_tools_state(task_id: &str, tool_call_id: &str, tool_name: &str) -> Value {
    json!({
        "task_id": task_id,
        "kind": "inference",
        "status": "waiting_for_tools",
        "request_id": format!("request_{task_id}"),
        "created_at": "2026-08-04T10:00:00Z",
        "updated_at": "2026-08-04T10:00:05Z",
        "execution_strategy": "agent",
        "execution_preset": "fast",
        "selection_mode": "auto",
        "progress": {
            "phase": "waiting_for_tools",
            "invocations_completed": 1,
            "invocations_total": 1,
            "agent_iteration": 1,
            "agent_max_iterations": 6
        },
        "result": {
            "status": "waiting_for_tools",
            "pending_tool_calls": [{
                "id": tool_call_id,
                "name": tool_name,
                "arguments": {"title": "Consulta sobre normativa"}
            }]
        },
        "error": null
    })
}

/// Resultado completo tal y como lo devuelve el Broker en una respuesta de chat.
///
/// La clave es `result_markdown`, que es la que ChatyGPT materializa como
/// mensaje del asistente; usarla aquí mantiene el simulador atado al contrato
/// real en lugar de a una forma inventada para la prueba.
pub fn completed_chat_result(text: &str) -> Value {
    json!({
        "result_markdown": text,
        "provider": "ollama",
        "model": "qwen2.5:7b",
        "usage": {"total_tokens": 128}
    })
}
