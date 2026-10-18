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
    #[error("la URL del servicio de Athena no es válida: {0}")]
    InvalidAthenaUrl(String),
    #[error("el servicio de Athena no está accesible: {0}")]
    AthenaTransport(String),
    #[error("el servicio de Athena devolvió HTTP {status}: {message}")]
    AthenaResponse { status: u16, message: String },
    #[error("el servicio de Athena devolvió un contrato inesperado: {0}")]
    AthenaContract(String),
    #[error("el servicio de Athena rechazó la credencial")]
    AthenaUnauthorized,
    #[error("otro cliente controla este run; solo él puede aprobar")]
    AthenaNotController,
    #[error("esa petición de permiso ya se respondió")]
    AthenaAlreadyResolved,
    #[error("la petición de permiso ya no está activa: caducó o el run terminó")]
    AthenaRequestGone,
    #[error("el resultado ya no está disponible: {0}")]
    AthenaArtifactExpired(String),
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

/// Fallo de red explicado en español (auditoría 14-oct, INC-09).
///
/// El texto de `reqwest` («error sending request for url (…)») está en inglés,
/// repite la URL completa y no dice qué hacer. Aquí queda solo el destino y lo
/// que la persona puede comprobar.
pub fn describe_network_failure(error: &reqwest::Error) -> String {
    let destination = error
        .url()
        .and_then(|url| {
            url.host_str().map(|host| match url.port() {
                Some(port) => format!("{host}:{port}"),
                None => host.to_owned(),
            })
        })
        .unwrap_or_else(|| "el servicio".to_owned());
    if error.is_timeout() {
        format!("{destination} no respondió a tiempo; puede estar ocupado o la red ir lenta")
    } else if error.is_connect() {
        format!("no se pudo conectar con {destination}; comprueba que el servicio está arrancado y que la dirección es correcta")
    } else {
        format!("se cortó la comunicación con {destination}; vuelve a intentarlo")
    }
}

#[cfg(test)]
mod tests {
    use super::describe_network_failure;

    #[test]
    fn a_refused_connection_is_explained_without_library_jargon() {
        // Puerto 9 (discard) cerrado en local: la conexión se rechaza al momento.
        let error = reqwest::blocking::Client::new()
            .get("http://127.0.0.1:9/v1/profiles")
            .send()
            .expect_err("no hay nada escuchando");
        let message = describe_network_failure(&error);
        assert!(message.contains("127.0.0.1:9"), "{message}");
        assert!(message.contains("arrancado"), "{message}");
        assert!(!message.contains("error sending request"), "{message}");
        assert!(!message.contains("/v1/profiles"), "{message}");
    }
}
