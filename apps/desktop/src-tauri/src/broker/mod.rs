mod contracts;

use std::sync::{Arc, RwLock};
use std::time::Instant;

use reqwest::{header::HeaderValue, Client, StatusCode};
use serde::Serialize;
use serde_json::Value;
use url::Url;

pub use contracts::{BrokerCapabilities, FileAccepted, FileState, TaskAccepted, TaskState};

use crate::error::AppError;
use crate::logging;
use crate::secrets;

const DEFAULT_BROKER_BASE_URL: &str = "http://192.168.1.52:8765";

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

#[derive(Clone)]
pub struct BrokerClient {
    base_url: Url,
    http: Client,
    /// Token compartido y recargable: rotarlo no obliga a reiniciar la aplicación.
    admin_token: Arc<RwLock<Option<HeaderValue>>>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerDiagnostic {
    pub reachable: bool,
    pub ready: bool,
    pub base_url: String,
    pub contract_version: Option<String>,
    pub strategies: Vec<String>,
    pub presets: Value,
    pub derived_data_boundary: Option<bool>,
    pub work_lanes: Vec<String>,
    pub agent_skills: Vec<String>,
    pub sandbox_run_code: Option<bool>,
    pub file_ingestion: Option<bool>,
    pub long_context_map_reduce: Option<bool>,
    pub max_active_workflows: Option<u64>,
    pub latency_ms: u128,
    pub message: String,
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
            .user_agent(concat!("ChatyGPT/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Ok(Self {
            base_url,
            http,
            admin_token: Arc::new(RwLock::new(admin_token)),
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
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| transport_failure(operation, error))?;
        if !status.is_success() {
            let message = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|body| body.get("detail").cloned())
                .map(|detail| detail.to_string())
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
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

    pub async fn capabilities(&self) -> Result<BrokerCapabilities, AppError> {
        let response = self
            .authorize(self.http.get(self.endpoint("/api/v1/capabilities")?))
            .send()
            .await
            .map_err(|error| transport_failure("capabilities", error))?;
        Self::decode("capabilities", response).await
    }

    pub async fn create_task(&self, request: &Value) -> Result<TaskAccepted, AppError> {
        let response = self
            .authorize(
                self.http
                    .post(self.endpoint("/api/v1/tasks")?)
                    .json(request),
            )
            .send()
            .await
            .map_err(|error| transport_failure("create_task", error))?;
        Self::decode("create_task", response).await
    }

    pub async fn upload_file(
        &self,
        path: &std::path::Path,
        filename: &str,
        media_type: Option<&str>,
        size_bytes: u64,
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
            let form = reqwest::blocking::multipart::Form::new().part("file", part);
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
            let bytes = response
                .bytes()
                .map_err(|error| transport_failure("upload_file", error))?;
            if !status.is_success() {
                let message = serde_json::from_slice::<Value>(&bytes)
                    .ok()
                    .and_then(|body| body.get("detail").cloned())
                    .map(|detail| detail.to_string())
                    .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
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
        let url = match Url::parse(location) {
            Ok(url) if matches!(url.scheme(), "http" | "https") => url,
            Ok(_) => {
                return Err(AppError::InvalidBrokerUrl(
                    "la URL del texto convertido no usa HTTP o HTTPS".to_owned(),
                ))
            }
            Err(url::ParseError::RelativeUrlWithoutBase) => self.endpoint(location)?,
            Err(error) => return Err(AppError::InvalidBrokerUrl(error.to_string())),
        };
        let response = self
            .authorize(self.http.get(url))
            .send()
            .await
            .map_err(|error| transport_failure("download_text", error))?;
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| transport_failure("download_text", error))?;
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
        if bytes.len() > 64 * 1024 * 1024 {
            return Err(AppError::BrokerContract(
                "el texto convertido supera el límite local de 64 MB".to_owned(),
            ));
        }
        String::from_utf8(bytes.to_vec())
            .map_err(|_| AppError::BrokerContract("el texto convertido no es UTF-8".to_owned()))
    }

    pub async fn get_task(&self, task_id: &str) -> Result<TaskState, AppError> {
        let path = format!("/api/v1/tasks/{task_id}");
        let response = self
            .authorize(self.http.get(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("get_task", error))?;
        Self::decode("get_task", response).await
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<TaskState, AppError> {
        let path = format!("/api/v1/tasks/{task_id}");
        let response = self
            .authorize(self.http.delete(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| transport_failure("cancel_task", error))?;
        Self::decode("cancel_task", response).await
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
        Self::decode("submit_tool_results", response).await
    }

    /// Diagnostica el Broker y deja constancia del resultado, no de su mensaje.
    pub async fn diagnose(&self) -> BrokerDiagnostic {
        let diagnostic = self.probe().await;
        logging::info(
            "broker.diagnosed",
            None,
            &[
                ("reachable", logging::flag(diagnostic.reachable)),
                ("ready", logging::flag(diagnostic.ready)),
                ("latency_ms", logging::millis(diagnostic.latency_ms)),
            ],
        );
        diagnostic
    }

    async fn probe(&self) -> BrokerDiagnostic {
        let started = Instant::now();
        let readiness_url = match self.endpoint("/health/ready") {
            Ok(url) => url,
            Err(error) => {
                return BrokerDiagnostic {
                    reachable: false,
                    ready: false,
                    base_url: self.base_url.to_string(),
                    contract_version: None,
                    strategies: vec![],
                    presets: Value::Null,
                    derived_data_boundary: None,
                    work_lanes: vec![],
                    agent_skills: vec![],
                    sandbox_run_code: None,
                    file_ingestion: None,
                    long_context_map_reduce: None,
                    max_active_workflows: None,
                    latency_ms: started.elapsed().as_millis(),
                    message: error.to_string(),
                };
            }
        };
        let readiness = self.http.get(readiness_url).send().await;
        let latency_ms = started.elapsed().as_millis();
        match readiness {
            Ok(response) if response.status().is_success() => match self.capabilities().await {
                Ok(capabilities) => BrokerDiagnostic {
                    reachable: true,
                    ready: true,
                    base_url: self.base_url.to_string(),
                    contract_version: Some(capabilities.contract_version),
                    strategies: capabilities.strategies,
                    presets: capabilities.presets,
                    derived_data_boundary: Some(capabilities.derived_data_boundary),
                    work_lanes: capabilities.work_lanes,
                    agent_skills: capabilities.agent_skills,
                    sandbox_run_code: Some(capabilities.sandbox_run_code),
                    file_ingestion: Some(capabilities.file_ingestion),
                    long_context_map_reduce: Some(capabilities.long_context_map_reduce),
                    max_active_workflows: capabilities.max_active_workflows,
                    latency_ms,
                    message: "Broker AI está listo".to_owned(),
                },
                Err(error) => BrokerDiagnostic {
                    reachable: true,
                    ready: false,
                    base_url: self.base_url.to_string(),
                    contract_version: None,
                    strategies: vec![],
                    presets: Value::Null,
                    derived_data_boundary: None,
                    work_lanes: vec![],
                    agent_skills: vec![],
                    sandbox_run_code: None,
                    file_ingestion: None,
                    long_context_map_reduce: None,
                    max_active_workflows: None,
                    latency_ms,
                    message: format!("Broker accesible, capacidades no verificadas: {error}"),
                },
            },
            Ok(response) => BrokerDiagnostic {
                reachable: true,
                ready: false,
                base_url: self.base_url.to_string(),
                contract_version: None,
                strategies: vec![],
                presets: Value::Null,
                derived_data_boundary: None,
                work_lanes: vec![],
                agent_skills: vec![],
                sandbox_run_code: None,
                file_ingestion: None,
                long_context_map_reduce: None,
                max_active_workflows: None,
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
                base_url: self.base_url.to_string(),
                contract_version: None,
                strategies: vec![],
                presets: Value::Null,
                derived_data_boundary: None,
                work_lanes: vec![],
                agent_skills: vec![],
                sandbox_run_code: None,
                file_ingestion: None,
                long_context_map_reduce: None,
                max_active_workflows: None,
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
            initial_ms: 750,
            maximum_ms: 15_000,
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
        ((base as i64) * (10_000 + bounded_jitter) / 10_000).max(100) as u64
    }
}

#[cfg(test)]
mod tests {
    use super::PollPolicy;

    #[test]
    fn polling_is_bounded_and_backed_off() {
        let policy = PollPolicy::default();
        assert_eq!(policy.delay_ms(0, 0), 750);
        assert_eq!(policy.delay_ms(2, 0), 3_000);
        assert_eq!(policy.delay_ms(30, 0), 15_000);
        assert_eq!(policy.delay_ms(30, 1_500), 17_250);
    }
}
