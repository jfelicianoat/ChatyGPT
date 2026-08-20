//! Flujo de eventos de un run, sobre SSE.
//!
//! La reconexión es *instantánea y luego cola*: al conectar, Athena manda el
//! estado completo del run y después los eventos vivos. Por eso perder eventos
//! durante un corte no es un problema de corrección — la instantánea siguiente
//! los sustituye — y por eso este cliente no necesita un registro de eventos ni
//! `Last-Event-ID`.
//!
//! El `subscriber_id` que llega en el marco inicial no es decorativo: los
//! intents viajan por otra conexión, así que es lo único que prueba que quien
//! aprueba un permiso es quien controla el run.

use std::time::Duration;

use reqwest::{Method, StatusCode};
use tokio::time::sleep;

use super::contracts::{EventoRuntime, MarcoEstado, MensajeFlujo};
use super::AthenaClient;
use crate::error::AppError;
use crate::logging;

/// Cuánto esperar entre intentos de reconexión.
#[derive(Debug, Clone)]
pub struct OpcionesReconexion {
    pub espera_inicial: Duration,
    pub espera_maxima: Duration,
    /// `None` reintenta mientras el run siga vivo; útil en producción, malo en
    /// una prueba que debe terminar.
    pub intentos_maximos: Option<u32>,
}

impl Default for OpcionesReconexion {
    fn default() -> Self {
        Self {
            espera_inicial: Duration::from_millis(500),
            espera_maxima: Duration::from_secs(10),
            intentos_maximos: None,
        }
    }
}

/// Lector del flujo de eventos de un run.
pub struct FlujoEventos {
    cliente: AthenaClient,
    run_id: String,
    controlar: bool,
    opciones: OpcionesReconexion,
    /// Identidad que el servicio asignó en la última conexión.
    suscriptor: Option<String>,
}

impl FlujoEventos {
    pub(crate) fn nuevo(cliente: AthenaClient, run_id: &str, controlar: bool) -> Self {
        Self {
            cliente,
            run_id: run_id.to_owned(),
            controlar,
            opciones: OpcionesReconexion::default(),
            suscriptor: None,
        }
    }

    pub fn con_reconexion(mut self, opciones: OpcionesReconexion) -> Self {
        self.opciones = opciones;
        self
    }

    /// Identidad del suscriptor, disponible tras el primer marco de estado.
    pub fn suscriptor(&self) -> Option<&str> {
        self.suscriptor.as_deref()
    }

    /// Escucha el flujo entregando cada mensaje al manejador.
    ///
    /// El manejador decide cuándo parar devolviendo `false`; así quien llama
    /// puede terminar en el evento final sin que este módulo tenga que conocer
    /// el vocabulario de eventos.
    pub async fn escuchar<F>(&mut self, mut manejador: F) -> Result<(), AppError>
    where
        F: FnMut(MensajeFlujo) -> bool,
    {
        let mut espera = self.opciones.espera_inicial;
        let mut intentos: u32 = 0;
        loop {
            match self.conectar_y_leer(&mut manejador).await {
                Ok(Continuacion::Terminado) => return Ok(()),
                Ok(Continuacion::Cortado) => {}
                Err(AppError::AthenaUnauthorized) => {
                    // Reintentar con un token inválido solo repite el rechazo.
                    return Err(AppError::AthenaUnauthorized);
                }
                Err(AppError::NotFound(mensaje)) => return Err(AppError::NotFound(mensaje)),
                Err(error) => {
                    logging::warn(
                        "athena.stream_failed",
                        None,
                        &[("run", logging::id(&self.run_id))],
                    );
                    if self.agotados(intentos) {
                        return Err(error);
                    }
                }
            }
            intentos += 1;
            if self.agotados(intentos) {
                return Ok(());
            }
            logging::info(
                "athena.stream_reconnecting",
                None,
                &[
                    ("run", logging::id(&self.run_id)),
                    ("attempt", logging::count(i64::from(intentos))),
                ],
            );
            sleep(espera).await;
            espera = (espera * 2).min(self.opciones.espera_maxima);
        }
    }

    fn agotados(&self, intentos: u32) -> bool {
        matches!(self.opciones.intentos_maximos, Some(limite) if intentos >= limite)
    }

    async fn conectar_y_leer<F>(&mut self, manejador: &mut F) -> Result<Continuacion, AppError>
    where
        F: FnMut(MensajeFlujo) -> bool,
    {
        let ruta = if self.controlar {
            format!("/v1/runs/{}/events?control=1", self.run_id)
        } else {
            format!("/v1/runs/{}/events", self.run_id)
        };
        let url = self.cliente.url_de(&ruta)?;
        let mut peticion = self
            .cliente
            .http
            .request(Method::GET, url)
            // Sin límite total: un flujo de eventos vive tanto como el run.
            .timeout(Duration::from_secs(60 * 60 * 24));
        if let Some(cabecera) = self.cliente.token_actual() {
            peticion = peticion.header(reqwest::header::AUTHORIZATION, cabecera);
        }
        let respuesta = peticion.send().await.map_err(|error| {
            logging::warn(
                "athena.stream_transport_failed",
                None,
                &[("run", logging::id(&self.run_id))],
            );
            AppError::AthenaTransport(error.to_string())
        })?;

        match respuesta.status() {
            StatusCode::OK => {}
            StatusCode::UNAUTHORIZED => return Err(AppError::AthenaUnauthorized),
            StatusCode::NOT_FOUND => {
                return Err(AppError::NotFound(format!(
                    "el run {} no existe o ya terminó",
                    self.run_id
                )))
            }
            otro => {
                return Err(AppError::AthenaResponse {
                    status: otro.as_u16(),
                    message: "el flujo de eventos fue rechazado".to_owned(),
                })
            }
        }

        let mut respuesta = respuesta;
        let mut pendiente = String::new();
        while let Some(trozo) = respuesta
            .chunk()
            .await
            .map_err(|error| AppError::AthenaTransport(error.to_string()))?
        {
            pendiente.push_str(&String::from_utf8_lossy(&trozo));
            while let Some(corte) = pendiente.find("\n\n") {
                let marco = pendiente[..corte].to_owned();
                pendiente.drain(..corte + 2);
                let Some(mensaje) = self.interpretar_marco(&marco) else {
                    continue;
                };
                if !manejador(mensaje) {
                    return Ok(Continuacion::Terminado);
                }
            }
        }
        Ok(Continuacion::Cortado)
    }

    /// Convierte un marco SSE en un mensaje tipado.
    ///
    /// Un marco que no se entiende se descarta con un aviso en lugar de tumbar
    /// el flujo: que Athena publique un evento nuevo no debe dejar la interfaz
    /// a oscuras.
    fn interpretar_marco(&mut self, marco: &str) -> Option<MensajeFlujo> {
        let datos: String = marco
            .lines()
            .filter_map(|linea| linea.strip_prefix("data: "))
            .collect::<Vec<_>>()
            .join("\n");
        if datos.is_empty() {
            return None;
        }
        // El marco de estado se distingue por llevar el identificador de
        // suscriptor; los eventos llevan `event_id`.
        if datos.contains("\"subscriber_id\"") {
            match serde_json::from_str::<MarcoEstado>(&datos) {
                Ok(estado) => {
                    self.suscriptor = Some(estado.subscriber_id.clone());
                    return Some(MensajeFlujo::Estado(Box::new(estado)));
                }
                Err(_) => {
                    logging::warn(
                        "athena.state_frame_unreadable",
                        None,
                        &[("run", logging::id(&self.run_id))],
                    );
                    return None;
                }
            }
        }
        match serde_json::from_str::<EventoRuntime>(&datos) {
            Ok(evento) => Some(MensajeFlujo::Evento(Box::new(evento))),
            Err(_) => {
                logging::warn(
                    "athena.event_unreadable",
                    None,
                    &[("run", logging::id(&self.run_id))],
                );
                None
            }
        }
    }
}

enum Continuacion {
    /// El manejador dijo que ya no quiere más.
    Terminado,
    /// El servidor cerró la conexión; procede reconectar.
    Cortado,
}
