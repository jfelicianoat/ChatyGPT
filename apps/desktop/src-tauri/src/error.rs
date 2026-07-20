use serde::Serialize;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum AppError {
    #[error("no se pudo preparar el directorio de datos: {0}")]
    DataDirectory(String),
    #[error("falló la persistencia local: {0}")]
    Database(#[from] rusqlite::Error),
    #[error("la URL de Broker AI no es válida: {0}")]
    InvalidBrokerUrl(String),
    #[error("Broker AI no está accesible: {0}")]
    BrokerTransport(String),
    #[error("Broker AI devolvió HTTP {status}: {message}")]
    BrokerResponse { status: u16, message: String },
    #[error("Broker AI devolvió un contrato inesperado: {0}")]
    BrokerContract(String),
    #[error("datos no válidos: {0}")]
    Validation(String),
    #[error("no encontrado: {0}")]
    NotFound(String),
    #[error("la operación no puede realizarse ahora: {0}")]
    Conflict(String),
}

impl Serialize for AppError {
    fn serialize<S>(&self, serializer: S) -> Result<S::Ok, S::Error>
    where
        S: serde::Serializer,
    {
        serializer.serialize_str(&self.to_string())
    }
}
