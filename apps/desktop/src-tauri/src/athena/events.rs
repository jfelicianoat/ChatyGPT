//! Flujo de eventos de un run, sobre SSE.
//!
//! Hay dos formas de volver tras un corte y este cliente usa las dos, en orden.
//!
//! Si sabe por dónde iba, manda `Last-Event-ID` y Athena le pone al día evento
//! a evento: conserva lo que ya había derivado en vez de tirarlo. Si estuvo
//! fuera más tiempo del que cabe en el búfer del servidor —o si es la primera
//! conexión— recibe la instantánea completa, que es lo que siempre hizo y sigue
//! siendo la red de seguridad.
//!
//! La instantánea basta para la corrección; el `Last-Event-ID` es lo que evita
//! reconstruir toda la vista por dos segundos sin red.
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

const MAX_SSE_BUFFER_BYTES: usize = 1_048_576;
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
    /// Último evento entregado al manejador. Es el punto por el que se pide
    /// reanudar; sin él una reconexión cuesta una resincronización entera.
    ultimo_evento: Option<String>,
}

impl FlujoEventos {
    pub(crate) fn nuevo(cliente: AthenaClient, run_id: &str, controlar: bool) -> Self {
        Self {
            cliente,
            run_id: run_id.to_owned(),
            controlar,
            opciones: OpcionesReconexion::default(),
            suscriptor: None,
            ultimo_evento: None,
        }
    }

    pub fn con_reconexion(mut self, opciones: OpcionesReconexion) -> Self {
        self.opciones = opciones;
        self
    }

    /// Identidad del suscriptor, disponible tras el primer marco de estado.
    ///
    /// El área lee el suscriptor de la proyección, no del flujo, porque la
    /// proyección sobrevive a la tarea que escucha. Esto queda como lectura
    /// directa para las pruebas y para quien use el flujo sin área.
    #[allow(dead_code)]
    pub fn suscriptor(&self) -> Option<&str> {
        self.suscriptor.as_deref()
    }

    /// Punto por el que se reanudaría ahora mismo. Misma razón que arriba.
    #[allow(dead_code)]
    pub fn ultimo_evento(&self) -> Option<&str> {
        self.ultimo_evento.as_deref()
    }

    /// Reanuda desde un punto conocido, por ejemplo tras reiniciar ChatyGPT.
    pub fn desde_evento(mut self, event_id: Option<String>) -> Self {
        self.ultimo_evento = event_id;
        self
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
        if let Some(ultimo) = &self.ultimo_evento {
            // La cabecera que define la especificación de SSE. Athena responde
            // con `resumed: true` y sólo lo perdido, o con una instantánea si el
            // punto ya se cayó de su ventana.
            peticion = peticion.header("Last-Event-ID", ultimo.clone());
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
        let mut pendiente = Vec::new();
        while let Some(trozo) = respuesta
            .chunk()
            .await
            .map_err(|error| AppError::AthenaTransport(error.to_string()))?
        {
            pendiente.extend_from_slice(&trozo);
            if pendiente.len() > MAX_SSE_BUFFER_BYTES {
                return Err(AppError::AthenaContract(
                    "el flujo SSE superó el límite local sin cerrar un evento".to_owned(),
                ));
            }
            while let Some((fin_marco, fin_separador)) = siguiente_marco_sse(&pendiente) {
                let bytes = pendiente[..fin_marco].to_vec();
                pendiente.drain(..fin_separador);
                let marco = match String::from_utf8(bytes) {
                    Ok(marco) => marco,
                    Err(_) => {
                        logging::warn(
                            "athena.sse_frame_invalid_utf8",
                            None,
                            &[("run", logging::id(&self.run_id))],
                        );
                        continue;
                    }
                };
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
        let datos = datos_sse(marco);
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
            Ok(evento) => {
                // Se anota aquí, no en el manejador: el punto de reanudación es
                // lo último que este cliente *entendió*, y un marco ilegible no
                // debe mover el marcador hacia adelante.
                self.ultimo_evento = Some(evento.event_id.clone());
                Some(MensajeFlujo::Evento(Box::new(evento)))
            }
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

fn datos_sse(marco: &str) -> String {
    marco
        .replace("\r\n", "\n")
        .replace('\r', "\n")
        .lines()
        .filter_map(|linea| {
            linea
                .strip_prefix("data:")
                .map(|valor| valor.strip_prefix(' ').unwrap_or(valor))
        })
        .collect::<Vec<_>>()
        .join("\n")
}

fn siguiente_marco_sse(buffer: &[u8]) -> Option<(usize, usize)> {
    let mut inicio_linea = 0_usize;
    let mut indice = 0_usize;
    while indice < buffer.len() {
        let largo_salto = match buffer[indice] {
            b'\n' => 1,
            b'\r' if buffer.get(indice + 1) == Some(&b'\n') => 2,
            b'\r' => 1,
            _ => {
                indice += 1;
                continue;
            }
        };
        let siguiente = indice + largo_salto;
        if indice == inicio_linea {
            return Some((inicio_linea, siguiente));
        }
        inicio_linea = siguiente;
        indice = siguiente;
    }
    None
}

enum Continuacion {
    /// El manejador dijo que ya no quiere más.
    Terminado,
    /// El servidor cerró la conexión; procede reconectar.
    Cortado,
}

#[cfg(test)]
mod parser_tests {
    use super::{datos_sse, siguiente_marco_sse};

    #[test]
    fn sse_frames_accept_crlf_no_space_and_split_utf8() {
        let completo = "event: event\r\ndata:{\"text\":\"café\"}\r\n\r\n".as_bytes();
        let corte_utf8 = completo
            .windows(2)
            .position(|bytes| bytes == [0xc3, 0xa9])
            .expect("the frame should contain a multibyte character")
            + 1;
        let mut pendiente = Vec::new();
        pendiente.extend_from_slice(&completo[..corte_utf8]);
        assert!(siguiente_marco_sse(&pendiente).is_none());
        pendiente.extend_from_slice(&completo[corte_utf8..]);
        let (fin, separador) = siguiente_marco_sse(&pendiente).expect("frame should complete");
        let marco = String::from_utf8(pendiente[..fin].to_vec()).expect("UTF-8 should survive");
        assert!(marco.contains("data:{\"text\":\"café\"}"));
        assert_eq!(datos_sse(&marco), "{\"text\":\"café\"}");
        assert_eq!(separador, completo.len());
    }
}
