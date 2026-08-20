//! Cliente del servicio de Athena.
//!
//! Vive detrás del núcleo Rust y **nunca** se expone a React: la interfaz habla
//! con órdenes de Tauri y recibe proyecciones, sin ver jamás la URL ni el token.
//! Esa asimetría es intencionada — un token en el `webview` es un token que
//! cualquier extensión del navegador puede leer.
//!
//! Athena sigue siendo la fuente de verdad del estado de un run. Aquí no se
//! guarda estado autoritativo: lo que hay es una referencia y, como mucho, una
//! proyección que puede tirarse y volver a pedirse.
//!
//! Notas de contrato:
//!
//! - El servicio publica `runs`, `approvals`, `results` y el flujo de eventos.
//!   **No expone una API de tareas**, así que aquí no se inventa una; el
//!   progreso se deriva de los eventos y de la instantánea.
//! - Los intents viajan por una conexión distinta del flujo SSE, de modo que el
//!   cliente que controla el run se identifica devolviendo el `subscriber_id`
//!   que el marco `state` le entregó.

// El cliente está completo y probado, pero todavía no lo consume ninguna orden
// de Tauri: la capa de órdenes obliga a decidir antes si el progreso llega a
// React por eventos de Tauri o por sondeo, y esta aplicación hoy solo sondea.
// Este permiso desaparece en cuanto exista esa capa; no debe sobrevivir a ella.
#![allow(dead_code, unused_imports)]

mod area;
mod contracts;
mod events;
#[cfg(test)]
pub mod simulated;
mod supervisor;

use std::sync::{Arc, RwLock};
use std::time::Duration;

use reqwest::{header::HeaderValue, Client, Method, Response, StatusCode};
use serde::Serialize;
use serde_json::Value;
use url::Url;

pub use area::{AreaAthena, EstadoAreaAthena, EstadoServicio};
pub use contracts::{
    Checkpoint, DecisionPermiso, EstadoRun, EventoRuntime, InstantaneaRun, ListadoRuns,
    MarcoEstado, MensajeFlujo, ModoCapacidad, PermisoPendiente, ReferenciaArtefacto, ResumenRun,
    RunCreado, SaludServicio, SolicitudRun, WIRE_VERSION_SOPORTADA,
};
pub use events::{FlujoEventos, OpcionesReconexion};
pub use supervisor::{
    ArgumentoVista, ArtefactoVista, ComprobacionVista, ErrorVista, EstadoTarea, FaseRun,
    PermisoVista, ProyeccionRun, TareaVista, UsoHerramienta,
};

use crate::error::AppError;
use crate::logging;

/// Puerto por defecto del servicio local de Athena.
pub const URL_ATHENA_POR_DEFECTO: &str = "http://127.0.0.1:8770";

/// Tiempo máximo de una petición corriente. El flujo de eventos no lo usa: es
/// una conexión larga por definición y morir a los treinta segundos sería lo
/// contrario de lo que hace falta.
const TIEMPO_PETICION: Duration = Duration::from_secs(30);

/// Tiempo máximo para abrir la conexión, separado del anterior: distinguir
/// "no está" de "va lento" es lo que permite decidir si merece la pena arrancar
/// el servicio uno mismo.
const TIEMPO_CONEXION: Duration = Duration::from_secs(5);

/// Opciones de un run. Los valores por defecto **preguntan**.
#[derive(Debug, Clone)]
pub struct OpcionesRun {
    pub escrituras: ModoCapacidad,
    pub ejecucion: ModoCapacidad,
    pub max_iteraciones: u32,
    pub max_ciclos_reparacion: u32,
    pub tiempo_sesion_segundos: f64,
}

impl Default for OpcionesRun {
    fn default() -> Self {
        Self {
            escrituras: ModoCapacidad::Ask,
            ejecucion: ModoCapacidad::Ask,
            max_iteraciones: 12,
            max_ciclos_reparacion: 2,
            tiempo_sesion_segundos: 900.0,
        }
    }
}

/// Traduce un fallo de transporte registrando su clase, nunca su texto.
///
/// El mensaje de `reqwest` puede incluir la URL completa y, con ella, el puerto
/// y el token si viajara en la ruta. En el registro solo queda la operación.
fn fallo_transporte(operacion: &str, error: impl std::fmt::Display) -> AppError {
    logging::warn(
        "athena.transport_failed",
        None,
        &[("operation", logging::code(operacion))],
    );
    AppError::AthenaTransport(error.to_string())
}

/// Extrae la parte accionable de un rechazo, sin arrastrar el cuerpo entero.
fn mensaje_rechazo(bytes: &[u8]) -> (String, String) {
    match serde_json::from_slice::<contracts::CuerpoError>(bytes) {
        Ok(cuerpo) => (cuerpo.error.code, cuerpo.error.message),
        Err(_) => (
            String::from("unparseable"),
            String::from_utf8_lossy(bytes).chars().take(200).collect(),
        ),
    }
}

#[derive(Clone)]
pub struct AthenaClient {
    base_url: Url,
    http: Client,
    /// Token compartido y recargable: Athena lo regenera en cada arranque, así
    /// que rotarlo no puede obligar a reiniciar ChatyGPT.
    token: Arc<RwLock<Option<HeaderValue>>>,
}

impl AthenaClient {
    pub fn for_base_url(base_url: &str) -> Result<Self, AppError> {
        let url =
            Url::parse(base_url).map_err(|error| AppError::InvalidAthenaUrl(error.to_string()))?;
        if !matches!(url.scheme(), "http" | "https") {
            return Err(AppError::InvalidAthenaUrl(
                "el esquema debe ser http o https".to_owned(),
            ));
        }
        let http = Client::builder()
            .connect_timeout(TIEMPO_CONEXION)
            .timeout(TIEMPO_PETICION)
            .build()
            .map_err(|error| AppError::AthenaTransport(error.to_string()))?;
        Ok(Self {
            base_url: url,
            http,
            token: Arc::new(RwLock::new(None)),
        })
    }

    pub fn base_url(&self) -> String {
        self.base_url.to_string()
    }

    /// Sustituye el token sin reconstruir el cliente.
    pub fn replace_token(&self, token: Option<&str>) -> Result<(), AppError> {
        let cabecera = match token {
            Some(valor) if !valor.is_empty() => Some(
                HeaderValue::from_str(&format!("Bearer {valor}"))
                    .map_err(|_| AppError::AthenaContract("token de Athena inválido".to_owned()))?,
            ),
            _ => None,
        };
        let mut guardado = self
            .token
            .write()
            .map_err(|_| AppError::AthenaContract("token ilegible".to_owned()))?;
        *guardado = cabecera;
        Ok(())
    }

    pub(crate) fn token_actual(&self) -> Option<HeaderValue> {
        self.token.read().ok().and_then(|valor| valor.clone())
    }

    pub(crate) fn url_de(&self, ruta: &str) -> Result<Url, AppError> {
        self.base_url
            .join(ruta)
            .map_err(|error| AppError::InvalidAthenaUrl(error.to_string()))
    }

    // -- transporte -------------------------------------------------------

    async fn enviar(
        &self,
        metodo: Method,
        ruta: &str,
        operacion: &str,
        cuerpo: Option<&impl Serialize>,
        suscriptor: Option<&str>,
    ) -> Result<Response, AppError> {
        let url = self.url_de(ruta)?;
        let mut peticion = self.http.request(metodo, url);
        if let Some(cabecera) = self.token_actual() {
            peticion = peticion.header(reqwest::header::AUTHORIZATION, cabecera);
        }
        if let Some(identificador) = suscriptor {
            // Prueba de que quien manda la orden es quien controla el run.
            peticion = peticion.header("X-Athena-Subscriber", identificador);
        }
        if let Some(datos) = cuerpo {
            peticion = peticion.json(datos);
        }
        peticion
            .send()
            .await
            .map_err(|error| fallo_transporte(operacion, error))
    }

    /// Convierte el estado HTTP en un error tipado, para que quien llame pueda
    /// distinguir "no está" de "expiró" de "no mandas tú".
    async fn interpretar(respuesta: Response, operacion: &str) -> Result<Response, AppError> {
        let estado = respuesta.status();
        if estado.is_success() {
            return Ok(respuesta);
        }
        let bytes = respuesta.bytes().await.unwrap_or_default();
        let (codigo, mensaje) = mensaje_rechazo(&bytes);
        logging::warn(
            "athena.rejected",
            None,
            &[
                ("operation", logging::code(operacion)),
                ("status", logging::count(i64::from(estado.as_u16()))),
                ("code", logging::code(&codigo)),
            ],
        );
        Err(match estado {
            StatusCode::UNAUTHORIZED => AppError::AthenaUnauthorized,
            StatusCode::FORBIDDEN if codigo == "not_controller" => AppError::AthenaNotController,
            // Una petición que ya no está no es un fallo del usuario: caducó, el
            // run terminó, o alguien contestó antes. Merece su propio error para
            // que la interfaz lo explique en vez de alarmar.
            StatusCode::NOT_FOUND if operacion.contains("approval") => AppError::AthenaRequestGone,
            StatusCode::CONFLICT if codigo == "already_resolved" => AppError::AthenaAlreadyResolved,
            StatusCode::NOT_FOUND => AppError::NotFound(mensaje),
            StatusCode::GONE => AppError::AthenaArtifactExpired(mensaje),
            StatusCode::CONFLICT => AppError::Conflict(mensaje),
            StatusCode::BAD_REQUEST => AppError::Validation(mensaje),
            otro => AppError::AthenaResponse {
                status: otro.as_u16(),
                message: mensaje,
            },
        })
    }

    async fn leer_json<T: serde::de::DeserializeOwned>(
        respuesta: Response,
        operacion: &str,
    ) -> Result<T, AppError> {
        let bytes = respuesta
            .bytes()
            .await
            .map_err(|error| fallo_transporte(operacion, error))?;
        serde_json::from_slice(&bytes).map_err(|error| {
            logging::warn(
                "athena.contract_mismatch",
                None,
                &[("operation", logging::code(operacion))],
            );
            AppError::AthenaContract(error.to_string())
        })
    }

    // -- operaciones ------------------------------------------------------

    /// Comprueba que el servicio responde y habla una versión que entendemos.
    ///
    /// No requiere token: sirve justo para decidir si hay que acoplarse a un
    /// servicio existente o arrancar uno propio.
    pub async fn salud(&self) -> Result<SaludServicio, AppError> {
        let respuesta = self
            .enviar(Method::GET, "/v1/health", "health", None::<&Value>, None)
            .await?;
        let respuesta = Self::interpretar(respuesta, "health").await?;
        let salud: SaludServicio = Self::leer_json(respuesta, "health").await?;
        if salud.wire_version != WIRE_VERSION_SOPORTADA {
            logging::warn(
                "athena.wire_version_mismatch",
                None,
                &[("version", logging::count(i64::from(salud.wire_version)))],
            );
            return Err(AppError::AthenaContract(format!(
                "el servicio habla la versión {} y este cliente la {}",
                salud.wire_version, WIRE_VERSION_SOPORTADA
            )));
        }
        Ok(salud)
    }

    /// Abre un run. Devuelve solo cuando ya es direccionable.
    pub async fn crear_run(
        &self,
        objetivo: &str,
        workspace: &str,
        opciones: &OpcionesRun,
    ) -> Result<RunCreado, AppError> {
        if objetivo.trim().is_empty() {
            return Err(AppError::Validation(
                "el objetivo no puede estar vacío".to_owned(),
            ));
        }
        if workspace.trim().is_empty() {
            return Err(AppError::Validation(
                "falta la carpeta de trabajo".to_owned(),
            ));
        }
        let solicitud = SolicitudRun {
            objective: objetivo.to_owned(),
            workspace: workspace.to_owned(),
            writes: opciones.escrituras.como_texto(),
            ejecucion: opciones.ejecucion.como_texto(),
            max_iterations: opciones.max_iteraciones,
            max_repair_cycles: opciones.max_ciclos_reparacion,
            session_timeout_seconds: opciones.tiempo_sesion_segundos,
        };
        let respuesta = self
            .enviar(
                Method::POST,
                "/v1/runs",
                "create_run",
                Some(&solicitud),
                None,
            )
            .await?;
        let respuesta = Self::interpretar(respuesta, "create_run").await?;
        let creado: RunCreado = Self::leer_json(respuesta, "create_run").await?;
        logging::info(
            "athena.run_created",
            None,
            &[
                ("run", logging::id(&creado.run_id)),
                ("writes", logging::code(&creado.writes)),
                ("exec", logging::code(&creado.ejecucion)),
            ],
        );
        Ok(creado)
    }

    /// Lee la instantánea de un run. Es la fuente de verdad; lo que guarde
    /// ChatyGPT es caché.
    pub async fn leer_run(&self, run_id: &str) -> Result<InstantaneaRun, AppError> {
        let ruta = format!("/v1/runs/{run_id}");
        let respuesta = self
            .enviar(Method::GET, &ruta, "read_run", None::<&Value>, None)
            .await?;
        let respuesta = Self::interpretar(respuesta, "read_run").await?;
        Self::leer_json(respuesta, "read_run").await
    }

    /// Lista runs, opcionalmente filtrando por estado.
    pub async fn listar_runs(&self, estado: Option<&str>) -> Result<Vec<ResumenRun>, AppError> {
        let ruta = match estado {
            Some(valor) => format!("/v1/runs?status={valor}"),
            None => "/v1/runs".to_owned(),
        };
        let respuesta = self
            .enviar(Method::GET, &ruta, "list_runs", None::<&Value>, None)
            .await?;
        let respuesta = Self::interpretar(respuesta, "list_runs").await?;
        let listado: ListadoRuns = Self::leer_json(respuesta, "list_runs").await?;
        Ok(listado.runs)
    }

    /// Runs que quedaron a medias cuando el runtime murió.
    ///
    /// Se listan aparte porque no deben mostrarse como terminados: necesitan una
    /// decisión, no un resumen.
    pub async fn runs_por_recuperar(&self) -> Result<Vec<ResumenRun>, AppError> {
        self.listar_runs(Some("recovery_pending")).await
    }

    /// Pide la cancelación de un run. Athena propaga la señal hacia dentro.
    pub async fn cancelar_run(&self, run_id: &str) -> Result<(), AppError> {
        let ruta = format!("/v1/runs/{run_id}/cancel");
        let respuesta = self
            .enviar(Method::POST, &ruta, "cancel_run", None::<&Value>, None)
            .await?;
        Self::interpretar(respuesta, "cancel_run").await?;
        logging::info(
            "athena.run_cancelled",
            None,
            &[("run", logging::id(run_id))],
        );
        Ok(())
    }

    /// Reanuda un run interrumpido a partir de su memoria de trabajo.
    pub async fn reanudar_run(&self, run_id: &str, workspace: &str) -> Result<(), AppError> {
        #[derive(Serialize)]
        struct Cuerpo<'a> {
            workspace: &'a str,
        }
        let ruta = format!("/v1/runs/{run_id}/resume");
        let respuesta = self
            .enviar(
                Method::POST,
                &ruta,
                "resume_run",
                Some(&Cuerpo { workspace }),
                None,
            )
            .await?;
        Self::interpretar(respuesta, "resume_run").await?;
        logging::info("athena.run_resumed", None, &[("run", logging::id(run_id))]);
        Ok(())
    }

    /// Confirma que la petición de permiso está delante de una persona.
    ///
    /// Hasta este momento corre el reloj corto de entrega; a partir de él, el
    /// largo de decisión. Sin este aviso, una red lenta se comería el tiempo de
    /// pensar del usuario.
    pub async fn confirmar_recepcion_permiso(
        &self,
        run_id: &str,
        request_id: &str,
        suscriptor: &str,
    ) -> Result<PermisoPendiente, AppError> {
        let ruta = format!("/v1/runs/{run_id}/approvals/{request_id}/ack");
        let respuesta = self
            .enviar(
                Method::POST,
                &ruta,
                "ack_approval",
                None::<&Value>,
                Some(suscriptor),
            )
            .await?;
        let respuesta = Self::interpretar(respuesta, "ack_approval").await?;
        Self::leer_json(respuesta, "ack_approval").await
    }

    /// Responde a una petición de permiso. La respuesta es de un solo uso.
    pub async fn resolver_permiso(
        &self,
        run_id: &str,
        request_id: &str,
        decision: DecisionPermiso,
        suscriptor: &str,
    ) -> Result<(), AppError> {
        #[derive(Serialize)]
        struct Cuerpo<'a> {
            decision: &'a str,
        }
        let ruta = format!("/v1/runs/{run_id}/approvals/{request_id}");
        let respuesta = self
            .enviar(
                Method::POST,
                &ruta,
                "resolve_approval",
                Some(&Cuerpo {
                    decision: decision.como_texto(),
                }),
                Some(suscriptor),
            )
            .await?;
        Self::interpretar(respuesta, "resolve_approval").await?;
        logging::info(
            "athena.approval_resolved",
            None,
            &[
                ("run", logging::id(run_id)),
                ("decision", logging::code(decision.como_texto())),
            ],
        );
        Ok(())
    }

    /// Descarga un resultado externalizado.
    ///
    /// Devuelve `AthenaArtifactExpired` cuando la referencia ya pasó su ventana
    /// de retención: es preferible a devolver un cuerpo vacío que parecería un
    /// artefacto legítimo.
    pub async fn descargar_artefacto(&self, clave: &str) -> Result<String, AppError> {
        let ruta = format!("/v1/results/{clave}");
        let respuesta = self
            .enviar(Method::GET, &ruta, "fetch_artifact", None::<&Value>, None)
            .await?;
        let respuesta = Self::interpretar(respuesta, "fetch_artifact").await?;
        respuesta
            .text()
            .await
            .map_err(|error| fallo_transporte("fetch_artifact", error))
    }

    /// Abre el flujo de eventos de un run.
    pub fn flujo_eventos(&self, run_id: &str, controlar: bool) -> FlujoEventos {
        FlujoEventos::nuevo(self.clone(), run_id, controlar)
    }
}

#[cfg(test)]
mod pruebas;
#[cfg(test)]
mod pruebas_supervisor;
