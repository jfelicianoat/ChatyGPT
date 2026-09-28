mod contracts;
#[cfg(test)]
pub mod simulated;

use std::sync::{Arc, RwLock};
use std::time::Instant;

use reqwest::{header::HeaderValue, Client, StatusCode};
use serde::Serialize;
use serde_json::Value;
use std::collections::HashMap;
use url::Url;

#[cfg_attr(not(test), allow(unused_imports))]
pub use contracts::{
    final_artifact, BrokerCapabilities, FileAccepted, FileState, InvocationTelemetry, TaskAccepted,
    TaskArtifact, TaskState, TaskStatus,
};

use crate::error::AppError;
use crate::logging;
use crate::secrets;

const DEFAULT_BROKER_BASE_URL: &str = "http://192.168.1.52:8765";
const MAX_ARTIFACT_BYTES: usize = 20 * 1024 * 1024;
const MAX_CONVERTED_TEXT_BYTES: usize = 64 * 1024 * 1024;
/// Presupuesto de una respuesta JSON del Broker (H18).
///
/// El estado de una tarea lleva su resultado entero; 32 MB cubren con holgura
/// una respuesta larga y cortan una respuesta defectuosa antes de que llene la
/// memoria. El tiempo de espera no es un presupuesto de bytes.
pub(crate) const MAX_JSON_BYTES: usize = 32 * 1024 * 1024;

/// Convierte el token en cabecera HTTP sin filtrar su contenido al error.
fn header_token(value: &str) -> Result<HeaderValue, AppError> {
    HeaderValue::from_str(value)
        .map_err(|_| AppError::BrokerContract("token administrativo inválido".to_owned()))
}

/// Traduce un fallo de transporte y lo registra por su clase, nunca por su texto.
///
/// El mensaje de `reqwest` puede incluir la URL completa; en el registro solo
/// queda la operación afectada.
fn transport_failure(operation: &str, error: impl std::fmt::Display) -> AppError {
    logging::warn(
        "broker.transport_failed",
        None,
        &[("operation", logging::code(operation))],
    );
    AppError::BrokerTransport(error.to_string())
}

async fn read_body_limited(
    operation: &str,
    mut response: reqwest::Response,
    limit: usize,
    description: &str,
) -> Result<(StatusCode, Vec<u8>), AppError> {
    let status = response.status();
    if response
        .content_length()
        .is_some_and(|length| length > limit as u64)
    {
        return Err(AppError::BrokerContract(format!(
            "{description} supera el límite local de {} MB",
            limit / (1024 * 1024)
        )));
    }
    let mut bytes = Vec::with_capacity(limit.min(64 * 1024));
    while let Some(chunk) = response
        .chunk()
        .await
        .map_err(|error| transport_failure(operation, error))?
    {
        if bytes.len().saturating_add(chunk.len()) > limit {
            return Err(AppError::BrokerContract(format!(
                "{description} supera el límite local de {} MB",
                limit / (1024 * 1024)
            )));
        }
        bytes.extend_from_slice(&chunk);
    }
    Ok((status, bytes))
}

#[derive(Clone)]
pub struct BrokerClient {
    base_url: Url,
    http: Client,
    /// Token compartido y recargable: rotarlo no obliga a reiniciar la aplicación.
    admin_token: Arc<RwLock<Option<HeaderValue>>>,
    /// Lo último que anunció el Broker en `/api/v1/capabilities`.
    ///
    /// Gobierna qué campos opcionales del contrato 2.10 pueden viajar en una
    /// petición: la validación del Broker es estricta (`extra="forbid"`), así
    /// que un campo que ese Broker no conoce **no se ignora** — hace fallar la
    /// petición entera con un 422 y se lleva por delante el mensaje del usuario.
    capabilities: Arc<RwLock<Option<BrokerCapabilities>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerDiagnostic {
    pub reachable: bool,
    pub ready: bool,
    pub capabilities_verified: bool,
    pub base_url: String,
    pub contract_version: Option<String>,
    pub strategies: Vec<String>,
    pub presets: Value,
    pub derived_data_boundary: Option<bool>,
    pub work_lanes: Vec<String>,
    pub agent_skills: Vec<String>,
    pub agent_skills_egress: Vec<String>,
    pub task_dependencies: Option<bool>,
    pub sandbox_run_code: Option<bool>,
    pub file_ingestion: Option<bool>,
    pub ingestion_formats: HashMap<String, Vec<String>>,
    pub long_context_map_reduce: Option<bool>,
    pub max_active_workflows: Option<u64>,
    /// Contrato 2.10 (8.4): si se puede exigir que solo el modelo que responde
    /// vea el contenido. `false` no es un fallo del Broker: significa que una
    /// conversación marcada confidencial puede acabar vista por otro modelo
    /// local bajo el mismo `task_id`, y quien la marcó merece saberlo.
    pub content_exclusivity: Option<bool>,
    /// Contrato 2.10 (8.1 y 8.5): si este Broker puede demostrar CÓMO ejecutó
    /// —separar sus propias llamadas de las de la tarea y acusar recibo de la
    /// poda del prompt—, no solo qué respondió.
    pub demonstrable_execution: Option<bool>,
    /// Veredicto de `/api/v1/auth/check` sobre la credencial guardada (H20):
    /// `valid`, `rejected`, `not_required`, `backend_unavailable` o `unknown`.
    /// `/health` y `/capabilities` responden a cualquiera y no prueban nada.
    pub credential: Option<String>,
    /// Adónde viajan de verdad prompts y resultados, y si van cifrados (H04).
    pub destination: Option<crate::politica::BrokerDestination>,
    pub latency_ms: u128,
    pub message: String,
}

/// Extrae únicamente la parte accionable y no sensible de un rechazo HTTP.
///
/// Los errores 2.8 publican `code`, `message` y, para 422, una lista `fields`.
/// No incluimos `input`: puede contener el prompt o datos del usuario.
fn rejection_message(status: StatusCode, bytes: &[u8]) -> String {
    let Ok(body) = serde_json::from_slice::<Value>(bytes) else {
        return String::from_utf8_lossy(bytes).into_owned();
    };
    let detail = body.get("detail").unwrap_or(&body);
    let code = detail
        .get("code")
        .and_then(Value::as_str)
        .or_else(|| body.get("code").and_then(Value::as_str));
    let message = detail
        .get("message")
        .and_then(Value::as_str)
        .or_else(|| body.get("message").and_then(Value::as_str))
        .or_else(|| detail.as_str());
    let fields = detail
        .get("fields")
        .or_else(|| body.get("fields"))
        .and_then(Value::as_array)
        .map(|items| {
            items
                .iter()
                .filter_map(|item| {
                    let location = item.get("loc").and_then(Value::as_array).map(|parts| {
                        parts
                            .iter()
                            .filter_map(|part| {
                                part.as_str()
                                    .map(str::to_owned)
                                    .or_else(|| part.as_i64().map(|value| value.to_string()))
                            })
                            .collect::<Vec<_>>()
                            .join(".")
                    });
                    let reason = item.get("msg").and_then(Value::as_str);
                    match (location.filter(|value| !value.is_empty()), reason) {
                        (Some(location), Some(reason)) => Some(format!("{location}: {reason}")),
                        (Some(location), None) => Some(location),
                        (None, Some(reason)) => Some(reason.to_owned()),
                        (None, None) => None,
                    }
                })
                .take(4)
                .collect::<Vec<_>>()
        })
        .filter(|items| !items.is_empty());

    let friendly = match code {
        Some("ADMIN_AUTH_REQUIRED") if matches!(status, StatusCode::UNAUTHORIZED | StatusCode::FORBIDDEN) => {
            Some("La credencial de Broker AI ya no es válida. Actualízala en Inicio → Credencial del Broker; las tareas remotas siguen guardadas.")
        }
        Some("ADMIN_AUTH_BACKEND_UNAVAILABLE") => Some(
            "Broker AI no puede acceder a su almacén de credenciales. Revisa el servicio o el llavero del sistema; introducir otro token no resolverá este fallo.",
        ),
        _ => None,
    };
    let mut text = friendly
        .or(message)
        .unwrap_or("Broker AI rechazó la operación")
        .to_owned();
    if let Some(code) = code {
        text = format!("{code}: {text}");
    }
    if let Some(fields) = fields {
        text.push_str(" · ");
        text.push_str(&fields.join("; "));
    }
    text
}

impl BrokerClient {
    /// Construye el cliente resolviendo la credencial desde el almacén protegido
    /// y, solo si allí no hay nada, desde el entorno.
    pub fn bootstrap(data_dir: &std::path::Path) -> Result<Self, AppError> {
        let raw_url = std::env::var("CHATYGPT_BROKER_BASE_URL")
            .unwrap_or_else(|_| DEFAULT_BROKER_BASE_URL.to_owned());
        let mut base_url =
            Url::parse(&raw_url).map_err(|error| AppError::InvalidBrokerUrl(error.to_string()))?;
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(AppError::InvalidBrokerUrl(
                "solo se admiten esquemas http y https".to_owned(),
            ));
        }
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let admin_token = secrets::resolve_broker_token(data_dir)
            .map(|value| header_token(&value))
            .transpose()?;
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(10))
            .redirect(reqwest::redirect::Policy::none())
            .user_agent(concat!("ChatyGPT/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Ok(Self {
            base_url,
            http,
            admin_token: Arc::new(RwLock::new(admin_token)),
            capabilities: Arc::new(RwLock::new(None)),
        })
    }

    /// Cliente apuntado a una URL concreta, sin tocar el almacén de credenciales.
    ///
    /// Existe únicamente para las pruebas de integración contra el Broker
    /// simulado: `bootstrap` resuelve la URL del entorno y la credencial de
    /// DPAPI, que ninguna prueba debe depender de tener configurados. Los
    /// tiempos de espera son cortos porque el servidor está en loopback.
    #[cfg(test)]
    pub fn for_base_url(base_url: &str) -> Result<Self, AppError> {
        let mut base_url =
            Url::parse(base_url).map_err(|error| AppError::InvalidBrokerUrl(error.to_string()))?;
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(2))
            .timeout(std::time::Duration::from_secs(5))
            .redirect(reqwest::redirect::Policy::none())
            .build()
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Ok(Self {
            base_url,
            http,
            admin_token: Arc::new(RwLock::new(None)),
            capabilities: Arc::new(RwLock::new(None)),
        })
    }

    pub fn base_url(&self) -> String {
        self.base_url.as_str().trim_end_matches('/').to_owned()
    }

    /// Sustituye la credencial en caliente tras guardarla o retirarla.
    pub fn replace_admin_token(&self, token: Option<&str>) -> Result<(), AppError> {
        let header = token.map(header_token).transpose()?;
        let mut guard = self
            .admin_token
            .write()
            .map_err(|_| AppError::Conflict("la credencial está en uso".to_owned()))?;
        *guard = header;
        Ok(())
    }

    fn current_token(&self) -> Option<HeaderValue> {
        self.admin_token.read().ok().and_then(|token| token.clone())
    }

    fn endpoint(&self, path: &str) -> Result<Url, AppError> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|error| AppError::InvalidBrokerUrl(error.to_string()))
    }

    fn resource_url(&self, location: &str) -> Result<Url, AppError> {
        let url = match Url::parse(location) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => url,
            Ok(_) => {
                return Err(AppError::InvalidBrokerUrl(
                    "la URL del recurso no usa HTTP o HTTPS".to_owned(),
                ))
            }
            Err(url::ParseError::RelativeUrlWithoutBase) => self.endpoint(location)?,
            Err(error) => return Err(AppError::InvalidBrokerUrl(error.to_string())),
        };
        let same_origin = url.scheme() == self.base_url.scheme()
            && url
                .host_str()
                .zip(self.base_url.host_str())
                .is_some_and(|(left, right)| left.eq_ignore_ascii_case(right))
            && url.port_or_known_default() == self.base_url.port_or_known_default();
        if !same_origin || !url.username().is_empty() || url.password().is_some() {
            return Err(AppError::InvalidBrokerUrl(
                "el recurso del Broker apunta a un origen distinto".to_owned(),
            ));
        }
        Ok(url)
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match self.current_token() {
            Some(token) => request.header("x-admin-token", token),
            None => request,
        }
    }

    async fn decode<T: serde::de::DeserializeOwned>(
        operation: &str,
        response: reqwest::Response,
    ) -> Result<T, AppError> {
        let (status, bytes) = read_body_limited(
            operation,
            response,
            MAX_JSON_BYTES,
            "La respuesta de Broker AI",
        )
        .await?;
        if !status.is_success() {
            let message = rejection_message(status, &bytes);
            // Se registra el código HTTP, no el detalle: puede citar el contenido enviado.
            logging::warn(
                "broker.response_rejected",
                None,
                &[
                    ("operation", logging::code(operation)),
                    ("status", logging::count(i64::from(status.as_u16()))),
                ],
            );
            return Err(AppError::BrokerResponse {
                status: status.as_u16(),
                message,
            });
        }
        serde_json::from_slice(&bytes).map_err(|error| {
            logging::error(
                "broker.contract_mismatch",
                None,
                &[("operation", logging::code(operation))],
            );
            AppError::BrokerContract(error.to_string())
        })
    }

    async fn decode_task_state(
        operation: &str,
        expected_task_id: &str,
        response: reqwest::Response,
    ) -> Result<TaskState, AppError> {
        let value = Self::decode::<Value>(operation, response).await?;
        TaskState::from_contract_value(value, expected_task_id).map_err(|message| {
            logging::error(
                "broker.task_state_contract_mismatch",
                None,
                &[("operation", logging::code(operation))],
            );
            AppError::BrokerContract(message)
        })
    }

    pub async fn capabilities(&self) -> Result<BrokerCapabilities, AppError> {
        let response = self
            .authorize(self.http.get(self.endpoint("/api/v1/capabilities")?))
            .send()
            .await
            .map_err(|error| transport_failure("capabilities", error))?;
        let capabilities: BrokerCapabilities = Self::decode("capabilities", response).await?;
        if let Ok(mut guard) = self.capabilities.write() {
            *guard = Some(capabilities.clone());
        }
        Ok(capabilities)
    }

    /// Lo que prometía el Broker la última vez que se negoció. `None` mientras
    /// no se haya negociado nada: entonces no se supone ninguna capacidad.
    pub fn capabilities_snapshot(&self) -> Option<BrokerCapabilities> {
        self.capabilities
            .read()
            .ok()
            .and_then(|guard| guard.clone())
    }

    /// Si se puede exigir que solo el modelo que responde vea el contenido.
    ///
    /// Sin negociación previa devuelve `false`, que es la respuesta prudente:
    /// no pedir la garantía deja la tarea sin ella, pero pedirla a un Broker
    /// que no la ofrece deja al usuario sin respuesta (Client_API.md, 5.1).
    pub fn content_exclusivity_available(&self) -> bool {
        self.capabilities_snapshot()
            .map(|capabilities| capabilities.auxiliary_invocations_optout)
            .unwrap_or(false)
    }

    pub async fn create_task(&self, request: &Value) -> Result<TaskAccepted, AppError> {
        let request = self.strip_unsupported_fields(request);
        let response = self
            .authorize(
                self.http
                    .post(self.endpoint("/api/v1/tasks")?)
                    .json(&request),
            )
            .send()
            .await
            .map_err(|error| transport_failure("create_task", error))?;
        Self::decode("create_task", response).await
    }

    /// Quita los campos que este Broker no anuncia (Client_API.md, 5.1).
    ///
    /// Su validación es estricta: un campo que no exista en su contrato hace
    /// fallar la petición con 422, no se ignora. Perder la garantía adicional
    /// es malo; perder el mensaje del usuario por haberla pedido es peor.
    fn strip_unsupported_fields(&self, request: &Value) -> Value {
        if request.get("auxiliary_invocations").is_none() || self.content_exclusivity_available() {
            return request.clone();
        }
        let mut filtered = request.clone();
        if let Some(object) = filtered.as_object_mut() {
            object.remove("auxiliary_invocations");
        }
        filtered
    }

    /// Telemetría por invocación (contrato 2.10, 8.1).
    ///
    /// Bajo un mismo `task_id` conviven las llamadas que ejecutan la tarea y
    /// las que el Broker lanza por su cuenta para medir su catálogo. Quien
    /// necesite separarlas debe usar `InvocationTelemetry::is_contractual`,
    /// nunca una lista negra de nombres de rol.
    pub async fn invocations(&self, task_id: &str) -> Result<Vec<InvocationTelemetry>, AppError> {
        let path = format!("/api/v1/tasks/{task_id}/invocations");
        let response = self
            .authorize(self.http.get(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("invocations", error))?;
        let payload: Value = Self::decode("invocations", response).await?;
        Ok(Self::items(&payload))
    }

    /// Ficheros que produjo la tarea (contrato 2.10, 8.3).
    ///
    /// Es la vía canónica para recoger el entregable: viene tipado y con su
    /// `sha256`. Aquí están además las **imágenes** que devuelve un modelo
    /// (`image_output`), que no viajan en `result` porque el resultado JSON se
    /// lee entero en cada sondeo del estado y un PNG en base64 dentro lo
    /// convertiría en megabytes por lectura.
    pub async fn artifacts(&self, task_id: &str) -> Result<Vec<TaskArtifact>, AppError> {
        let path = format!("/api/v1/tasks/{task_id}/artifacts");
        let response = self
            .authorize(self.http.get(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("artifacts", error))?;
        let payload: Value = Self::decode("artifacts", response).await?;
        Ok(Self::items(&payload))
    }

    /// Bytes de un artefacto (contrato 2.10, 8.3).
    ///
    /// Un `410` no es un `404`: la fila existe y el fichero ya no —lo podó la
    /// retención del Broker—. Existió y se borró a propósito, que no es lo
    /// mismo que no haber existido nunca, y el mensaje tiene que decirlo.
    pub async fn download_artifact(
        &self,
        task_id: &str,
        artifact_id: &str,
    ) -> Result<Vec<u8>, AppError> {
        let path = format!("/api/v1/tasks/{task_id}/artifacts/{artifact_id}");
        let response = self
            .authorize(self.http.get(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("download_artifact", error))?;
        let (status, bytes) = read_body_limited(
            "download_artifact",
            response,
            MAX_ARTIFACT_BYTES,
            "el artefacto",
        )
        .await?;
        if status == StatusCode::GONE {
            return Err(AppError::BrokerContract(
                "el artefacto existió y ya se ha borrado (retención del Broker)".to_owned(),
            ));
        }
        if !status.is_success() {
            return Err(AppError::BrokerResponse {
                status: status.as_u16(),
                message: rejection_message(status, &bytes),
            });
        }
        Ok(bytes)
    }

    /// `{task_id, items[]}`: lo que no encaja se descarta en vez de tumbar la
    /// lectura entera. El contrato crece y una entrada nueva no es un error.
    fn items<T: serde::de::DeserializeOwned>(payload: &Value) -> Vec<T> {
        payload
            .get("items")
            .and_then(Value::as_array)
            .map(|items| {
                items
                    .iter()
                    .filter_map(|item| serde_json::from_value(item.clone()).ok())
                    .collect()
            })
            .unwrap_or_default()
    }

    pub async fn upload_file(
        &self,
        path: &std::path::Path,
        filename: &str,
        media_type: Option<&str>,
        size_bytes: u64,
        describe_images: Option<bool>,
    ) -> Result<FileAccepted, AppError> {
        let path = path.to_path_buf();
        let filename = filename.to_owned();
        let media_type = media_type.map(str::to_owned);
        let endpoint = self.endpoint("/api/v1/files")?;
        let admin_token = self.current_token();
        tauri::async_runtime::spawn_blocking(move || {
            let file = std::fs::File::open(path)
                .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
            let mut part = reqwest::blocking::multipart::Part::reader_with_length(file, size_bytes)
                .file_name(filename);
            if let Some(media_type) = media_type {
                part = part
                    .mime_str(&media_type)
                    .map_err(|error| AppError::BrokerContract(error.to_string()))?;
            }
            let mut form = reqwest::blocking::multipart::Form::new().part("file", part);
            if let Some(describe_images) = describe_images {
                form = form.text("describe_images", describe_images.to_string());
            }
            let client = reqwest::blocking::Client::builder()
                .connect_timeout(std::time::Duration::from_secs(3))
                .timeout(std::time::Duration::from_secs(600))
                .user_agent(concat!("ChatyGPT/", env!("CARGO_PKG_VERSION")))
                .build()
                .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
            let mut request = client.post(endpoint).multipart(form);
            if let Some(token) = admin_token {
                request = request.header("x-admin-token", token);
            }
            let response = request
                .send()
                .map_err(|error| transport_failure("upload_file", error))?;
            let status = response.status();
            let mut bytes = Vec::new();
            {
                use std::io::Read;
                response
                    .take((MAX_JSON_BYTES + 1) as u64)
                    .read_to_end(&mut bytes)
                    .map_err(|error| transport_failure("upload_file", error))?;
            }
            if bytes.len() > MAX_JSON_BYTES {
                return Err(AppError::BrokerContract(
                    "la respuesta de Broker AI a la subida supera el límite local".to_owned(),
                ));
            }
            if !status.is_success() {
                let message = rejection_message(status, &bytes);
                logging::warn(
                    "broker.response_rejected",
                    None,
                    &[
                        ("operation", logging::code("upload_file")),
                        ("status", logging::count(i64::from(status.as_u16()))),
                    ],
                );
                return Err(AppError::BrokerResponse {
                    status: status.as_u16(),
                    message,
                });
            }
            serde_json::from_slice(&bytes)
                .map_err(|error| AppError::BrokerContract(error.to_string()))
        })
        .await
        .map_err(|error| AppError::BrokerTransport(error.to_string()))?
    }

    pub async fn get_file(&self, file_id: &str) -> Result<FileState, AppError> {
        let path = format!("/api/v1/files/{file_id}");
        let response = self
            .authorize(self.http.get(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("get_file", error))?;
        Self::decode("get_file", response).await
    }

    pub async fn download_text(&self, location: &str) -> Result<String, AppError> {
        let url = self.resource_url(location)?;
        let response = self
            .authorize(self.http.get(url))
            .send()
            .await
            .map_err(|error| transport_failure("download_text", error))?;
        let (status, bytes) = read_body_limited(
            "download_text",
            response,
            MAX_CONVERTED_TEXT_BYTES,
            "el texto convertido",
        )
        .await?;
        if !status.is_success() {
            logging::warn(
                "broker.response_rejected",
                None,
                &[
                    ("operation", logging::code("download_text")),
                    ("status", logging::count(i64::from(status.as_u16()))),
                ],
            );
            return Err(AppError::BrokerResponse {
                status: status.as_u16(),
                message: String::from_utf8_lossy(&bytes).into_owned(),
            });
        }
        String::from_utf8(bytes)
            .map_err(|_| AppError::BrokerContract("el texto convertido no es UTF-8".to_owned()))
    }

    pub async fn get_task(&self, task_id: &str) -> Result<TaskState, AppError> {
        let path = format!("/api/v1/tasks/{task_id}");
        let response = self
            .authorize(self.http.get(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("get_task", error))?;
        Self::decode_task_state("get_task", task_id, response).await
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<TaskState, AppError> {
        let path = format!("/api/v1/tasks/{task_id}");
        let response = self
            .authorize(self.http.delete(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("cancel_task", error))?;
        Self::decode_task_state("cancel_task", task_id, response).await
    }

    pub async fn submit_tool_results(
        &self,
        task_id: &str,
        tool_results: &Value,
    ) -> Result<TaskState, AppError> {
        let path = format!("/api/v1/tasks/{task_id}/tool_results");
        let response = self
            .authorize(self.http.post(self.endpoint(&path)?).json(tool_results))
            .send()
            .await
            .map_err(|error| transport_failure("submit_tool_results", error))?;
        Self::decode_task_state("submit_tool_results", task_id, response).await
    }

    /// Diagnostica el Broker y deja constancia del resultado, no de su mensaje.
    pub async fn diagnose(&self) -> BrokerDiagnostic {
        let mut diagnostic = self.probe().await;
        diagnostic.destination = Some(crate::politica::broker_destination(&self.base_url()));
        if diagnostic.reachable {
            let credential = self.check_credential().await;
            match credential.as_str() {
                "rejected" => {
                    diagnostic.ready = false;
                    diagnostic.message = "Broker AI rechaza la credencial guardada: el Broker \
                        cambia su token en cada arranque. Introduce el token actual en Ajustes."
                        .to_owned();
                }
                "backend_unavailable" => {
                    diagnostic.ready = false;
                    diagnostic.message = "Broker AI no puede comprobar credenciales ahora \
                        (falla su llavero del sistema); reintentar con otro token no lo arregla."
                        .to_owned();
                }
                "not_required" => {
                    diagnostic.message.push_str(
                        ". Este Broker no exige credencial: cualquiera en su red puede usarlo.",
                    );
                }
                _ => {}
            }
            diagnostic.credential = Some(credential);
        }
        let diagnostic = diagnostic;
        logging::info(
            "broker.diagnosed",
            None,
            &[
                ("reachable", logging::flag(diagnostic.reachable)),
                ("ready", logging::flag(diagnostic.ready)),
                (
                    "capabilities_verified",
                    logging::flag(diagnostic.capabilities_verified),
                ),
                ("latency_ms", logging::millis(diagnostic.latency_ms)),
            ],
        );
        diagnostic
    }

    /// Comprueba la credencial contra el único endpoint que la prueba.
    ///
    /// La misma pregunta que hacen el lanzador y el inicio con Windows: los
    /// tres caminos dan el mismo veredicto para el mismo token.
    pub async fn check_credential(&self) -> String {
        let Ok(url) = self.endpoint("/api/v1/auth/check") else {
            return "unknown".to_owned();
        };
        let response = match self.authorize(self.http.get(url)).send().await {
            Ok(response) => response,
            Err(_) => return "unknown".to_owned(),
        };
        match response.status().as_u16() {
            200 => {
                let body = read_body_limited("auth_check", response, 64 * 1024, "La respuesta")
                    .await
                    .ok()
                    .and_then(|(_, bytes)| serde_json::from_slice::<Value>(&bytes).ok());
                match body.and_then(|value| value.get("auth_required").and_then(Value::as_bool)) {
                    Some(false) => "not_required",
                    Some(true) => "valid",
                    None => "unknown",
                }
            }
            401 | 403 => "rejected",
            503 => "backend_unavailable",
            _ => "unknown",
        }
        .to_owned()
    }

    async fn probe(&self) -> BrokerDiagnostic {
        let started = Instant::now();
        let readiness_url = match self.endpoint("/health/ready") {
            Ok(url) => url,
            Err(error) => {
                return BrokerDiagnostic {
                    reachable: false,
                    ready: false,
                    capabilities_verified: false,
                    base_url: self.base_url.to_string(),
                    contract_version: None,
                    strategies: vec![],
                    presets: Value::Null,
                    derived_data_boundary: None,
                    work_lanes: vec![],
                    agent_skills: vec![],
                    agent_skills_egress: vec![],
                    task_dependencies: None,
                    sandbox_run_code: None,
                    file_ingestion: None,
                    ingestion_formats: HashMap::new(),
                    long_context_map_reduce: None,
                    max_active_workflows: None,
                    content_exclusivity: None,
                    demonstrable_execution: None,
                    credential: None,
                    destination: None,
                    latency_ms: started.elapsed().as_millis(),
                    message: error.to_string(),
                };
            }
        };
        let readiness = self.http.get(readiness_url).send().await;
        let latency_ms = started.elapsed().as_millis();
        match readiness {
            Ok(response) if response.status().is_success() => match self.capabilities().await {
                Ok(capabilities) => {
                    // Se resuelven antes de mover los campos al diagnóstico:
                    // ambas son preguntas sobre el conjunto, no sobre un campo.
                    let content_exclusivity = capabilities.content_exclusivity_available();
                    let demonstrable_execution =
                        capabilities.invocation_contract && capabilities.prompt_compression_echo;
                    BrokerDiagnostic {
                        reachable: true,
                        ready: true,
                        capabilities_verified: true,
                        base_url: self.base_url.to_string(),
                        contract_version: Some(capabilities.contract_version),
                        strategies: capabilities.strategies,
                        presets: capabilities.presets,
                        derived_data_boundary: Some(capabilities.derived_data_boundary),
                        work_lanes: capabilities.work_lanes,
                        agent_skills: capabilities.agent_skills,
                        agent_skills_egress: capabilities.agent_skills_egress,
                        task_dependencies: Some(capabilities.task_dependencies),
                        sandbox_run_code: Some(capabilities.sandbox_run_code),
                        file_ingestion: Some(capabilities.file_ingestion),
                        ingestion_formats: capabilities.ingestion_formats,
                        long_context_map_reduce: Some(capabilities.long_context_map_reduce),
                        max_active_workflows: capabilities.max_active_workflows,
                        content_exclusivity: Some(content_exclusivity),
                        demonstrable_execution: Some(demonstrable_execution),
                        credential: None,
                        destination: None,
                        latency_ms,
                        message: "Broker AI está listo".to_owned(),
                    }
                }
                Err(error) => BrokerDiagnostic {
                    reachable: true,
                    // La sonda de salud sí ha confirmado que el Broker puede
                    // trabajar. Un fallo de lectura de capacidades es una
                    // advertencia, no la prueba de que una función no exista.
                    ready: true,
                    capabilities_verified: false,
                    base_url: self.base_url.to_string(),
                    contract_version: None,
                    strategies: vec![],
                    presets: Value::Null,
                    derived_data_boundary: None,
                    work_lanes: vec![],
                    agent_skills: vec![],
                    agent_skills_egress: vec![],
                    task_dependencies: None,
                    sandbox_run_code: None,
                    file_ingestion: None,
                    ingestion_formats: HashMap::new(),
                    long_context_map_reduce: None,
                    max_active_workflows: None,
                    content_exclusivity: None,
                    demonstrable_execution: None,
                    credential: None,
                    destination: None,
                    latency_ms,
                    message: format!(
                        "Broker AI está listo, pero tiene capacidades no verificadas: {error}"
                    ),
                },
            },
            Ok(response) => BrokerDiagnostic {
                reachable: true,
                ready: false,
                capabilities_verified: false,
                base_url: self.base_url.to_string(),
                contract_version: None,
                strategies: vec![],
                presets: Value::Null,
                derived_data_boundary: None,
                work_lanes: vec![],
                agent_skills: vec![],
                agent_skills_egress: vec![],
                task_dependencies: None,
                sandbox_run_code: None,
                file_ingestion: None,
                ingestion_formats: HashMap::new(),
                long_context_map_reduce: None,
                max_active_workflows: None,
                content_exclusivity: None,
                demonstrable_execution: None,
                credential: None,
                destination: None,
                latency_ms,
                message: if response.status() == StatusCode::SERVICE_UNAVAILABLE {
                    "Broker AI responde, pero no está listo".to_owned()
                } else {
                    format!("Broker AI respondió con HTTP {}", response.status())
                },
            },
            Err(error) => BrokerDiagnostic {
                reachable: false,
                ready: false,
                capabilities_verified: false,
                base_url: self.base_url.to_string(),
                contract_version: None,
                strategies: vec![],
                presets: Value::Null,
                derived_data_boundary: None,
                work_lanes: vec![],
                agent_skills: vec![],
                agent_skills_egress: vec![],
                task_dependencies: None,
                sandbox_run_code: None,
                file_ingestion: None,
                ingestion_formats: HashMap::new(),
                long_context_map_reduce: None,
                max_active_workflows: None,
                content_exclusivity: None,
                demonstrable_execution: None,
                credential: None,
                destination: None,
                latency_ms,
                message: format!("Broker AI no está accesible: {error}"),
            },
        }
    }
}

#[derive(Debug, Clone)]
pub struct PollPolicy {
    pub initial_ms: u64,
    pub maximum_ms: u64,
}

impl Default for PollPolicy {
    fn default() -> Self {
        Self {
            // Contrato 2.9: el intervalo recomendado para tareas es 2–5 s.
            initial_ms: 2_000,
            maximum_ms: 5_000,
        }
    }
}

impl PollPolicy {
    pub fn delay_ms(&self, unchanged_polls: u32, jitter_basis_points: i32) -> u64 {
        let exponent = unchanged_polls.min(6);
        let base = self
            .initial_ms
            .saturating_mul(1_u64 << exponent)
            .min(self.maximum_ms);
        let bounded_jitter = jitter_basis_points.clamp(-1_500, 1_500) as i64;
        (((base as i64) * (10_000 + bounded_jitter) / 10_000).max(100) as u64).min(self.maximum_ms)
    }
}

#[cfg(test)]
mod tests {
    use super::simulated::{
        accepted_file, accepted_task, file_state, task_state, ScriptedResponse, SimulatedBroker,
    };
    use super::{
        final_artifact, AppError, BrokerCapabilities, BrokerClient, InvocationTelemetry,
        PollPolicy, MAX_ARTIFACT_BYTES,
    };
    use serde_json::{json, Value};

    fn block_on<T>(future: impl std::future::Future<Output = T>) -> T {
        tauri::async_runtime::block_on(future)
    }

    fn client_for(simulated: &SimulatedBroker) -> BrokerClient {
        BrokerClient::for_base_url(simulated.base_url()).expect("el cliente debe construirse")
    }

    #[test]
    fn polling_is_bounded_and_backed_off() {
        let policy = PollPolicy::default();
        assert_eq!(policy.delay_ms(0, 0), 2_000);
        assert_eq!(policy.delay_ms(1, 0), 4_000);
        assert_eq!(policy.delay_ms(30, 0), 5_000);
        assert_eq!(policy.delay_ms(30, 1_500), 5_000);
    }

    /// El diagnóstico distingue los tres estados que la interfaz debe mostrar.
    ///
    /// «No accesible», «accesible pero no listo» y «listo» no son matices: la
    /// aplicación decide con ellos si deja enviar un mensaje.
    #[test]
    fn diagnosis_separates_unreachable_not_ready_and_ready() {
        // Listo: sonda de salud correcta y capacidades legibles.
        let ready = SimulatedBroker::start();
        ready.always(
            "GET /health/ready",
            ScriptedResponse::ok(json!({"ready": true})),
        );
        ready.always(
            "GET /api/v1/capabilities",
            ScriptedResponse::ok(json!({
                "contract_version": "2.8",
                "derived_data_boundary": true,
                "work_lanes": ["inference", "ingestion"],
                "strategies": ["single", "agent"],
                "agent_skills": ["web_search"],
                "agent_skills_egress": ["web_search", "fetch_url"],
                "task_dependencies": true,
                "sandbox_run_code": true,
                "file_ingestion": true,
                "ingestion_formats": {"pdf": [".pdf"], "text": [".txt", ".md"]},
                "long_context_map_reduce": true,
                "max_active_workflows": 2
            })),
        );
        let diagnostic = block_on(client_for(&ready).diagnose());
        assert!(diagnostic.reachable && diagnostic.ready);
        assert_eq!(diagnostic.contract_version.as_deref(), Some("2.8"));
        assert!(diagnostic.capabilities_verified);
        assert_eq!(diagnostic.strategies, ["single", "agent"]);
        assert_eq!(diagnostic.ingestion_formats["pdf"], [".pdf"]);
        assert_eq!(diagnostic.sandbox_run_code, Some(true));
        assert_eq!(diagnostic.task_dependencies, Some(true));
        assert_eq!(diagnostic.agent_skills_egress, ["web_search", "fetch_url"]);
        assert_eq!(diagnostic.max_active_workflows, Some(2));
        assert_eq!(diagnostic.message, "Broker AI está listo");

        // Accesible pero arrancando: responde 503 a la sonda de salud.
        let starting = SimulatedBroker::start();
        starting.always("GET /health/ready", ScriptedResponse::status(503));
        let diagnostic = block_on(client_for(&starting).diagnose());
        assert!(diagnostic.reachable);
        assert!(!diagnostic.ready);
        assert_eq!(diagnostic.message, "Broker AI responde, pero no está listo");

        // Sano pero con capacidades ilegibles: puede trabajar, aunque la UI no
        // debe inventar qué capacidades tiene.
        let mismatched = SimulatedBroker::start();
        mismatched.always(
            "GET /health/ready",
            ScriptedResponse::ok(json!({"ready": true})),
        );
        mismatched.always("GET /api/v1/capabilities", ScriptedResponse::malformed());
        let diagnostic = block_on(client_for(&mismatched).diagnose());
        assert!(diagnostic.reachable);
        assert!(diagnostic.ready, "la sonda de salud sí lo declaró listo");
        assert!(!diagnostic.capabilities_verified);
        assert!(diagnostic.message.contains("capacidades no verificadas"));

        // No accesible: puerto cerrado. El simulador se apaga al soltarlo.
        let closed_url = {
            let temporary = SimulatedBroker::start();
            temporary.base_url().to_owned()
        };
        let unreachable = BrokerClient::for_base_url(&closed_url).expect("cliente construible");
        let diagnostic = block_on(unreachable.diagnose());
        assert!(!diagnostic.reachable);
        assert!(!diagnostic.ready);
    }

    /// Un cuerpo con éxito pero fuera de contrato no se acepta como válido.
    #[test]
    fn responses_are_classified_as_contract_or_response_errors() {
        let simulated = SimulatedBroker::start();
        simulated.script("GET /api/v1/capabilities", ScriptedResponse::malformed());
        simulated.always("GET /api/v1/capabilities", ScriptedResponse::permanent());
        let client = client_for(&simulated);

        // HTTP 200 con cuerpo ilegible: es un fallo de contrato, no de red.
        let error = block_on(client.capabilities()).expect_err("un cuerpo ilegible debe fallar");
        assert!(matches!(error, AppError::BrokerContract(_)));

        // HTTP 422: se conserva el código y el detalle publicado por el Broker.
        let error = block_on(client.capabilities()).expect_err("un 422 debe fallar");
        match error {
            AppError::BrokerResponse { status, message } => {
                assert_eq!(status, 422);
                assert!(message.contains("no cumple el contrato"));
            }
            other => panic!("se esperaba una respuesta rechazada, no {other:?}"),
        }
    }

    #[test]
    fn task_state_is_validated_before_polling_can_use_it() {
        let simulated = SimulatedBroker::start();
        simulated.script(
            "GET /api/v1/tasks/{id}",
            ScriptedResponse::ok(task_state("task-valid", "generating", None)),
        );
        simulated.script(
            "GET /api/v1/tasks/{id}",
            ScriptedResponse::ok(json!({
                "task_id": "task-invalid",
                "kind": "inference",
                "status": "waiting_for_tools",
                "request_id": "request-invalid",
                "created_at": "2026-08-14T10:00:00Z",
                "updated_at": "2026-08-14T10:00:01Z",
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
                "result": {"status": "waiting_for_tools"},
                "error": null
            })),
        );
        let client = client_for(&simulated);

        let valid = block_on(client.get_task("task-valid"))
            .expect("un estado completo debe llegar al polling");
        assert_eq!(valid.task_id, "task-valid");

        let error = block_on(client.get_task("task-invalid"))
            .expect_err("un estado sin llamadas pendientes no debe llegar al polling");
        assert!(matches!(error, AppError::BrokerContract(_)));
        assert!(error.to_string().contains("pending_tool_calls"));
    }

    #[test]
    fn contract_errors_expose_safe_field_paths_and_authentication_guidance() {
        let fields = super::rejection_message(
            reqwest::StatusCode::UNPROCESSABLE_ENTITY,
            serde_json::to_string(&json!({
                "code": "CONTRACT_VALIDATION_FAILED",
                "message": "Request does not satisfy Broker contract v1",
                "fields": [{
                    "loc": ["body", "execution", "scheduling"],
                    "msg": "preset fast only supports sequential",
                    "input": "contenido que no debe mostrarse"
                }]
            }))
            .expect("json")
            .as_bytes(),
        );
        assert!(fields.contains("body.execution.scheduling"));
        assert!(fields.contains("preset fast only supports sequential"));
        assert!(!fields.contains("contenido que no debe mostrarse"));

        let auth = super::rejection_message(
            reqwest::StatusCode::FORBIDDEN,
            br#"{"code":"ADMIN_AUTH_REQUIRED","message":"forbidden"}"#,
        );
        assert!(auth.contains("Inicio"));
        assert!(auth.contains("tareas remotas siguen guardadas"));
    }

    /// La credencial viaja en la cabecera y solo cuando existe.
    #[test]
    fn the_admin_token_travels_only_after_it_is_configured() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/capabilities",
            ScriptedResponse::ok(json!({"contract_version": "2.8"})),
        );
        let client = client_for(&simulated);

        block_on(client.capabilities()).expect("sin token la consulta sigue siendo válida");
        let anonymous = &simulated.requests_to("GET", "/api/v1/capabilities")[0];
        assert!(
            !anonymous.headers.contains_key("x-admin-token"),
            "sin credencial no debe enviarse una cabecera vacía"
        );

        client
            .replace_admin_token(Some("token-de-prueba"))
            .expect("la credencial debe poder fijarse en caliente");
        block_on(client.capabilities()).expect("con token la consulta debe funcionar");
        let authorized = &simulated.requests_to("GET", "/api/v1/capabilities")[1];
        assert_eq!(
            authorized.headers.get("x-admin-token").map(String::as_str),
            Some("token-de-prueba")
        );

        // Retirarla deja de enviarla sin reiniciar la aplicación.
        client
            .replace_admin_token(None)
            .expect("la credencial debe poder retirarse");
        block_on(client.capabilities()).expect("sin token vuelve a ser anónima");
        assert!(!simulated.requests_to("GET", "/api/v1/capabilities")[2]
            .headers
            .contains_key("x-admin-token"));

        // Un token con caracteres imposibles en una cabecera se rechaza antes
        // de salir a la red, y sin citar su contenido en el error.
        let rejected = client
            .replace_admin_token(Some("token\ncon salto"))
            .expect_err("un token inválido debe rechazarse");
        assert!(matches!(rejected, AppError::BrokerContract(_)));
        assert!(!rejected.to_string().contains("con salto"));
    }

    /// La subida envía el archivo real como multipart y lee su estado después.
    #[test]
    fn file_upload_sends_the_real_content_and_reads_its_state() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "POST /api/v1/files",
            ScriptedResponse::ok(accepted_file("file-1", "informe.pdf", 21, &"a".repeat(64))),
        );
        simulated.always(
            "GET /api/v1/files/{id}",
            ScriptedResponse::ok(file_state(
                "file-1",
                "ready",
                Some("/api/v1/files/file-1/markdown"),
            )),
        );

        let path = std::env::temp_dir().join(format!(
            "chatygpt-upload-{}.pdf",
            uuid::Uuid::new_v4().simple()
        ));
        let contenido = b"contenido del informe";
        std::fs::write(&path, contenido).expect("el archivo de prueba debe escribirse");

        let client = client_for(&simulated);
        let accepted = block_on(client.upload_file(
            &path,
            "informe.pdf",
            Some("application/pdf"),
            contenido.len() as u64,
            Some(false),
        ))
        .expect("la subida debe aceptarse");
        assert_eq!(accepted.file_id, "file-1");
        assert!(accepted.created);

        // El multipart lleva el nombre declarado y el contenido real del archivo.
        let upload = &simulated.requests_to("POST", "/api/v1/files")[0];
        assert!(upload.raw_body.contains("informe.pdf"));
        assert!(upload.raw_body.contains("contenido del informe"));
        assert!(upload.raw_body.contains("application/pdf"));
        assert!(upload.raw_body.contains("describe_images"));
        assert!(upload.raw_body.contains("false"));
        assert!(upload
            .headers
            .get("content-type")
            .is_some_and(|value| value.starts_with("multipart/form-data")));

        let state = block_on(client.get_file("file-1")).expect("el estado debe leerse");
        assert_eq!(state.status, "ready");
        assert_eq!(
            state.markdown_url.as_deref(),
            Some("/api/v1/files/file-1/markdown")
        );

        let _ = std::fs::remove_file(&path);
    }

    /// Un fallo al subir conserva el código HTTP y no se disfraza de éxito.
    #[test]
    fn a_rejected_upload_keeps_the_broker_status() {
        let simulated = SimulatedBroker::start();
        simulated.always("POST /api/v1/files", ScriptedResponse::permanent());

        let path = std::env::temp_dir().join(format!(
            "chatygpt-upload-rejected-{}.txt",
            uuid::Uuid::new_v4().simple()
        ));
        std::fs::write(&path, b"da igual").expect("el archivo de prueba debe escribirse");

        let error = block_on(client_for(&simulated).upload_file(&path, "nota.txt", None, 8, None))
            .expect_err("un 422 al subir debe fallar");
        match error {
            AppError::BrokerResponse { status, .. } => assert_eq!(status, 422),
            other => panic!("se esperaba una respuesta rechazada, no {other:?}"),
        }

        let _ = std::fs::remove_file(&path);
    }

    /// La descarga del Markdown convertido acepta rutas relativas y absolutas
    /// del Broker, y rechaza otros orígenes y lo que no sea texto HTTP.
    #[test]
    fn converted_markdown_download_is_bounded_to_http_and_utf8() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/files/file-1/markdown",
            ScriptedResponse::text("# Informe\n\nContenido convertido."),
        );
        simulated.always(
            "GET /api/v1/files/roto/markdown",
            ScriptedResponse::status(500),
        );
        // Bytes que no forman UTF-8 válido: no pueden convertirse en Markdown.
        simulated.always(
            "GET /api/v1/files/binario/markdown",
            ScriptedResponse::bytes(vec![0xff, 0xfe, 0x00]),
        );
        let client = client_for(&simulated);

        // Ruta relativa: se resuelve contra la base del Broker.
        let markdown = block_on(client.download_text("/api/v1/files/file-1/markdown"))
            .expect("el Markdown debe descargarse");
        assert!(markdown.contains("Contenido convertido"));

        // URL absoluta al mismo servidor: igualmente válida.
        let absolute = format!("{}/api/v1/files/file-1/markdown", simulated.base_url());
        assert_eq!(
            block_on(client.download_text(&absolute)).expect("la URL absoluta debe funcionar"),
            markdown
        );

        let foreign = SimulatedBroker::start();
        client
            .replace_admin_token(Some("token-sintetico"))
            .expect("token de prueba válido");
        let foreign_url = format!("{}/foreign", foreign.base_url());
        let error = block_on(client.download_text(&foreign_url))
            .expect_err("un origen ajeno al Broker debe rechazarse");
        assert!(matches!(error, AppError::InvalidBrokerUrl(_)));
        assert!(foreign.requests().is_empty(), "no debe abrirse la conexión");

        // Un esquema que no es web se rechaza antes de tocar la red.
        let error = block_on(client.download_text("file:///C:/Windows/System32/config/SAM"))
            .expect_err("un esquema no web debe rechazarse");
        assert!(matches!(error, AppError::InvalidBrokerUrl(_)));

        // Un error del servidor no se convierte en un documento vacío.
        let error = block_on(client.download_text("/api/v1/files/roto/markdown"))
            .expect_err("un 500 debe fallar");
        assert!(matches!(
            error,
            AppError::BrokerResponse { status: 500, .. }
        ));

        // Bytes no UTF-8 se rechazan en lugar de guardarse con pérdidas.
        let error = block_on(client.download_text("/api/v1/files/binario/markdown"))
            .expect_err("un cuerpo no UTF-8 debe rechazarse");
        assert!(matches!(error, AppError::BrokerContract(_)));
    }

    #[test]
    fn artifact_download_stops_at_the_local_size_limit() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/tasks/task-210/artifacts/art-big",
            ScriptedResponse::bytes(vec![0_u8; MAX_ARTIFACT_BYTES + 1]),
        );
        let client = client_for(&simulated);

        let error = block_on(client.download_artifact("task-210", "art-big"))
            .expect_err("un artefacto excesivo debe rechazarse antes de almacenarlo");
        assert!(error.to_string().contains("20 MB"), "mensaje: {error}");
    }

    /// Solo se admiten esquemas web al construir el cliente.
    #[test]
    fn only_web_schemes_are_accepted_as_base_url() {
        assert!(BrokerClient::for_base_url("no-es-una-url").is_err());
        // La base sin barra final se normaliza para que `join` no pierda ruta.
        let client = BrokerClient::for_base_url("http://127.0.0.1:9/api")
            .expect("una base http debe aceptarse");
        assert_eq!(client.base_url(), "http://127.0.0.1:9/api");
    }

    // ----------------------------------------------------------------------
    // Contrato 2.10: ejecución demostrable (Client_API.md, 8.1/8.3/8.4)
    // ----------------------------------------------------------------------

    fn capabilities_2_10() -> serde_json::Value {
        serde_json::json!({
            "contract_version": "2.10",
            "strategies": ["single", "mixture_of_agents", "agent"],
            "auxiliary_invocations": true,
            "auxiliary_invocations_optout": true,
            "invocation_contract": true,
            "prompt_compression_echo": true,
            "invocation_telemetry": true,
            "task_artifacts": true,
            "canonical_artifacts": true
        })
    }

    #[test]
    fn an_absent_auxiliary_invocations_flag_is_not_a_promise_that_there_are_none() {
        // Un Broker anterior al 2.10 las hacía sin anunciarlas, así que su
        // ausencia no puede leerse como `false` (Client_API.md, 8.4).
        let capabilities: BrokerCapabilities =
            serde_json::from_value(serde_json::json!({"contract_version": "2.9"}))
                .expect("capacidades 2.9");

        assert!(capabilities.auxiliary_invocations);
        assert!(!capabilities.auxiliary_invocations_optout);
        assert!(!capabilities.content_exclusivity_available());
    }

    #[test]
    fn content_exclusivity_reaches_a_broker_that_offers_the_optout() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/capabilities",
            ScriptedResponse::ok(capabilities_2_10()),
        );
        simulated.always(
            "POST /api/v1/tasks",
            ScriptedResponse::accepted(accepted_task("task-210")),
        );
        let client = client_for(&simulated);

        block_on(async {
            client.capabilities().await.expect("capacidades");
            client
                .create_task(&serde_json::json!({
                    "idempotency_key": "chatygpt:test:210",
                    "content": {"prompt": "hola"},
                    "auxiliary_invocations": false
                }))
                .await
                .expect("tarea aceptada");
        });

        let sent = simulated.requests_to("POST", "/api/v1/tasks");
        let body: &Value = &sent[0].body;
        assert_eq!(body["auxiliary_invocations"], serde_json::json!(false));
    }

    #[test]
    fn content_exclusivity_is_dropped_rather_than_losing_the_users_message() {
        // La validación del Broker es estricta: un campo que no exista en su
        // contrato hace fallar la petición entera con 422, no se ignora.
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/capabilities",
            ScriptedResponse::ok(serde_json::json!({"contract_version": "2.9"})),
        );
        simulated.always(
            "POST /api/v1/tasks",
            ScriptedResponse::accepted(accepted_task("task-29")),
        );
        let client = client_for(&simulated);

        block_on(async {
            client.capabilities().await.expect("capacidades");
            client
                .create_task(&serde_json::json!({
                    "idempotency_key": "chatygpt:test:29",
                    "content": {"prompt": "hola"},
                    "auxiliary_invocations": false
                }))
                .await
                .expect("tarea aceptada");
        });

        let sent = simulated.requests_to("POST", "/api/v1/tasks");
        let body: &Value = &sent[0].body;
        assert!(body.get("auxiliary_invocations").is_none());
    }

    #[test]
    fn nothing_from_210_travels_before_capabilities_are_negotiated() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "POST /api/v1/tasks",
            ScriptedResponse::accepted(accepted_task("task-x")),
        );
        let client = client_for(&simulated);

        block_on(async {
            client
                .create_task(&serde_json::json!({
                    "idempotency_key": "chatygpt:test:sin-negociar",
                    "content": {"prompt": "hola"},
                    "auxiliary_invocations": false
                }))
                .await
                .expect("tarea aceptada");
        });

        let sent = simulated.requests_to("POST", "/api/v1/tasks");
        let body: &Value = &sent[0].body;
        assert!(body.get("auxiliary_invocations").is_none());
    }

    #[test]
    fn the_brokers_own_calls_are_separated_from_the_ones_that_were_asked_for() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/tasks/task-210/invocations",
            ScriptedResponse::ok(serde_json::json!({
                "task_id": "task-210",
                "items": [
                    {"invocation_id": "inv-1", "role": "single", "status": "completed",
                     "contractual": true,
                     "prompt_compression": {"requested": "off", "effective": "off"}},
                    {"invocation_id": "inv-2", "role": "shadow_probe", "status": "completed",
                     "contractual": false,
                     "prompt_compression": {"requested": "off", "effective": "off"}}
                ]
            })),
        );
        let client = client_for(&simulated);

        let items = block_on(async { client.invocations("task-210").await.expect("telemetría") });

        assert_eq!(items.len(), 2);
        let contractual: Vec<&InvocationTelemetry> =
            items.iter().filter(|item| item.is_contractual()).collect();
        assert_eq!(contractual.len(), 1);
        assert_eq!(contractual[0].invocation_id, "inv-1");
    }

    #[test]
    fn without_the_boolean_only_the_role_name_is_left() {
        // Contra un Broker 2.9 no hay alternativa, y por eso el 2.10 lo añadió.
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/tasks/task-29/invocations",
            ScriptedResponse::ok(serde_json::json!({
                "task_id": "task-29",
                "items": [
                    {"invocation_id": "inv-1", "role": "single", "status": "completed"},
                    {"invocation_id": "inv-2", "role": "shadow_probe", "status": "completed"}
                ]
            })),
        );
        let client = client_for(&simulated);

        let items = block_on(async { client.invocations("task-29").await.expect("telemetría") });

        assert_eq!(items.iter().filter(|item| item.is_contractual()).count(), 1);
    }

    #[test]
    fn the_deliverable_is_the_final_artifact_and_not_the_first_one() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/tasks/task-210/artifacts",
            ScriptedResponse::ok(serde_json::json!({
                "task_id": "task-210",
                "items": [
                    {"artifact_id": "art-img", "artifact_type": "image_output",
                     "filename": "image_01.png", "sha256": "0", "available": true, "final": false},
                    {"artifact_id": "art-out", "artifact_type": "single_output",
                     "filename": "final.md", "sha256": "abc", "available": true, "final": true}
                ]
            })),
        );
        let client = client_for(&simulated);

        let items = block_on(async { client.artifacts("task-210").await.expect("artefactos") });

        let deliverable = final_artifact(&items).expect("entregable marcado");
        assert_eq!(deliverable.artifact_id, "art-out");
        assert_eq!(deliverable.sha256.as_deref(), Some("abc"));
    }

    #[test]
    fn an_artifact_the_broker_already_pruned_says_so_instead_of_not_found() {
        // Un 410 no es un 404: existió y se borró a propósito (8.3).
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /api/v1/tasks/task-210/artifacts/art-gone",
            ScriptedResponse::status(410),
        );
        let client = client_for(&simulated);

        let error = block_on(async {
            client
                .download_artifact("task-210", "art-gone")
                .await
                .expect_err("el artefacto ya no está")
        });

        assert!(error.to_string().contains("retención"), "mensaje: {error}");
    }

    #[test]
    fn the_diagnosis_says_whether_privacy_can_be_demanded() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /health/ready",
            ScriptedResponse::ok(serde_json::json!({"ready": true})),
        );
        simulated.always(
            "GET /api/v1/capabilities",
            ScriptedResponse::ok(capabilities_2_10()),
        );
        let client = client_for(&simulated);

        let diagnostic = block_on(async { client.diagnose().await });

        assert_eq!(diagnostic.content_exclusivity, Some(true));
        assert_eq!(diagnostic.demonstrable_execution, Some(true));
    }

    #[test]
    fn an_unreadable_capabilities_response_leaves_the_210_answers_unknown() {
        // `None` es «no consta», que no es `false`: un fallo de lectura no
        // demuestra que el Broker no sepa hacerlo (Client_API.md, 2).
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /health/ready",
            ScriptedResponse::ok(serde_json::json!({"ready": true})),
        );
        simulated.always("GET /api/v1/capabilities", ScriptedResponse::transient());
        let client = client_for(&simulated);

        let diagnostic = block_on(async { client.diagnose().await });

        assert!(diagnostic.ready);
        assert_eq!(diagnostic.content_exclusivity, None);
        assert_eq!(diagnostic.demonstrable_execution, None);
    }

    /// H18: una respuesta por trozos que no acaba nunca se corta al superar el
    /// presupuesto, sin acumularla entera ni esperar a que termine.
    #[test]
    fn an_endless_response_is_cut_at_the_local_budget() {
        let (base_url, written) = crate::pruebas_red::endless_chunked_server(96);
        let client = BrokerClient::for_base_url(&base_url).expect("cliente");
        let error = block_on(client.get_task("sin-fin")).expect_err("debe cortarse");
        assert!(error.to_string().contains("supera el límite"), "{error}");
        std::thread::sleep(std::time::Duration::from_millis(200));
        let sent = written.load(std::sync::atomic::Ordering::SeqCst);
        assert!(
            sent < 64,
            "el cliente debía cerrar cerca de los 32 MB; el servidor escribió {sent} MB"
        );
    }

    /// H20: el diagnóstico prueba la credencial con `/api/v1/auth/check`, como
    /// el lanzador; un token caducado ya no aparece como «Broker AI está listo».
    #[test]
    fn diagnosis_reports_a_rejected_credential_instead_of_ready() {
        let simulated = SimulatedBroker::start();
        simulated.always(
            "GET /health/ready",
            ScriptedResponse::ok(json!({"status": "ready"})),
        );
        simulated.always("GET /api/v1/auth/check", ScriptedResponse::status(403));
        let client = BrokerClient::for_base_url(simulated.base_url()).expect("cliente");
        let diagnostic = block_on(client.diagnose());
        assert!(diagnostic.reachable);
        assert!(!diagnostic.ready);
        assert_eq!(diagnostic.credential.as_deref(), Some("rejected"));
        assert!(
            diagnostic.message.contains("token actual"),
            "{}",
            diagnostic.message
        );
        let destination = diagnostic.destination.expect("destino");
        assert!(destination.local_machine);

        let open = SimulatedBroker::start();
        open.always(
            "GET /health/ready",
            ScriptedResponse::ok(json!({"status": "ready"})),
        );
        open.always(
            "GET /api/v1/auth/check",
            ScriptedResponse::ok(json!({"authenticated": true, "auth_required": false})),
        );
        let client = BrokerClient::for_base_url(open.base_url()).expect("cliente");
        let diagnostic = block_on(client.diagnose());
        assert_eq!(diagnostic.credential.as_deref(), Some("not_required"));
        assert!(diagnostic.message.contains("no exige credencial"));
    }
}
