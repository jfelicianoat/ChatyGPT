mod contracts;

use std::time::Instant;

use reqwest::{header::HeaderValue, Client, StatusCode};
use serde::Serialize;
use serde_json::Value;
use url::Url;

pub use contracts::{BrokerCapabilities, TaskAccepted, TaskState};

use crate::error::AppError;

#[derive(Clone)]
pub struct BrokerClient {
    base_url: Url,
    http: Client,
    admin_token: Option<HeaderValue>,
}

#[derive(Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerDiagnostic {
    pub reachable: bool,
    pub ready: bool,
    pub base_url: String,
    pub contract_version: Option<String>,
    pub strategies: Vec<String>,
    pub sandbox_run_code: Option<bool>,
    pub file_ingestion: Option<bool>,
    pub latency_ms: u128,
    pub message: String,
}

impl BrokerClient {
    pub fn from_environment() -> Result<Self, AppError> {
        let raw_url = std::env::var("CHATYGPT_BROKER_BASE_URL")
            .unwrap_or_else(|_| "http://127.0.0.1:8765".to_owned());
        let mut base_url = Url::parse(&raw_url)
            .map_err(|error| AppError::InvalidBrokerUrl(error.to_string()))?;
        if !matches!(base_url.scheme(), "http" | "https") {
            return Err(AppError::InvalidBrokerUrl(
                "solo se admiten esquemas http y https".to_owned(),
            ));
        }
        if !base_url.path().ends_with('/') {
            base_url.set_path(&format!("{}/", base_url.path()));
        }
        let admin_token = std::env::var("AI_BROKER_ADMIN_TOKEN")
            .ok()
            .filter(|value| !value.is_empty())
            .map(|value| {
                HeaderValue::from_str(&value)
                    .map_err(|_| AppError::BrokerContract("token administrativo inválido".to_owned()))
            })
            .transpose()?;
        let http = Client::builder()
            .connect_timeout(std::time::Duration::from_secs(3))
            .timeout(std::time::Duration::from_secs(10))
            .user_agent(concat!("ChatyGPT/", env!("CARGO_PKG_VERSION")))
            .build()
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Ok(Self { base_url, http, admin_token })
    }

    fn endpoint(&self, path: &str) -> Result<Url, AppError> {
        self.base_url
            .join(path.trim_start_matches('/'))
            .map_err(|error| AppError::InvalidBrokerUrl(error.to_string()))
    }

    fn authorize(&self, request: reqwest::RequestBuilder) -> reqwest::RequestBuilder {
        match &self.admin_token {
            Some(token) => request.header("x-admin-token", token),
            None => request,
        }
    }

    async fn decode<T: serde::de::DeserializeOwned>(
        response: reqwest::Response,
    ) -> Result<T, AppError> {
        let status = response.status();
        let bytes = response
            .bytes()
            .await
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        if !status.is_success() {
            let message = serde_json::from_slice::<Value>(&bytes)
                .ok()
                .and_then(|body| body.get("detail").cloned())
                .map(|detail| detail.to_string())
                .unwrap_or_else(|| String::from_utf8_lossy(&bytes).into_owned());
            return Err(AppError::BrokerResponse {
                status: status.as_u16(),
                message,
            });
        }
        serde_json::from_slice(&bytes)
            .map_err(|error| AppError::BrokerContract(error.to_string()))
    }

    pub async fn capabilities(&self) -> Result<BrokerCapabilities, AppError> {
        let response = self
            .authorize(self.http.get(self.endpoint("/api/v1/capabilities")?))
            .send()
            .await
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Self::decode(response).await
    }

    pub async fn create_task(&self, request: &Value) -> Result<TaskAccepted, AppError> {
        let response = self
            .authorize(self.http.post(self.endpoint("/api/v1/tasks")?).json(request))
            .send()
            .await
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Self::decode(response).await
    }

    pub async fn get_task(&self, task_id: &str) -> Result<TaskState, AppError> {
        let path = format!("/api/v1/tasks/{task_id}");
        let response = self
            .authorize(self.http.get(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Self::decode(response).await
    }

    pub async fn cancel_task(&self, task_id: &str) -> Result<TaskState, AppError> {
        let path = format!("/api/v1/tasks/{task_id}");
        let response = self
            .authorize(self.http.delete(self.endpoint(&path)?))
            .send()
            .await
            .map_err(|error| AppError::BrokerTransport(error.to_string()))?;
        Self::decode(response).await
    }

    pub async fn diagnose(&self) -> BrokerDiagnostic {
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
                    sandbox_run_code: None,
                    file_ingestion: None,
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
                    sandbox_run_code: Some(capabilities.sandbox_run_code),
                    file_ingestion: Some(capabilities.file_ingestion),
                    latency_ms,
                    message: "Broker AI está listo".to_owned(),
                },
                Err(error) => BrokerDiagnostic {
                    reachable: true,
                    ready: false,
                    base_url: self.base_url.to_string(),
                    contract_version: None,
                    strategies: vec![],
                    sandbox_run_code: None,
                    file_ingestion: None,
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
                sandbox_run_code: None,
                file_ingestion: None,
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
                sandbox_run_code: None,
                file_ingestion: None,
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
        Self { initial_ms: 750, maximum_ms: 15_000 }
    }
}

impl PollPolicy {
    pub fn delay_ms(&self, unchanged_polls: u32, jitter_basis_points: i32) -> u64 {
        let exponent = unchanged_polls.min(6);
        let base = self.initial_ms.saturating_mul(1_u64 << exponent).min(self.maximum_ms);
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
