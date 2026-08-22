//! Estado del área de Athena dentro del núcleo.
//!
//! Guarda una proyección por run y la alimenta **solo** desde el flujo de
//! eventos del runtime. La interfaz pregunta por la proyección; no la calcula,
//! no la corrige y no rellena huecos por su cuenta.
//!
//! El transporte hacia React es el mismo que usa el resto de ChatyGPT —una
//! orden de Tauri que se consulta— en lugar de eventos de Tauri, que serían un
//! segundo modelo de concurrencia en una aplicación cuyas pruebas asumen
//! sondeo. Lo que importa del requisito «dirigido por eventos» se cumple igual:
//! lo que se sondea es una proyección construida a partir de eventos de Athena.

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use serde::Serialize;

use super::contracts::{EventoHistorico, MensajeFlujo, ObjetivoRun, ResumenHistoria};
use super::supervisor::{PermisoVista, ProyeccionRun};
use super::{AthenaClient, OpcionesReconexion, OpcionesRun, RevisionObjetivo};
use crate::error::AppError;
use crate::logging;

/// Estado de la conexión con el servicio, tal y como lo ve la interfaz.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoServicio {
    /// Todavía no se ha comprobado. Nada lo construye porque el estado siempre
    /// se calcula tras preguntar; existe para que la interfaz pueda representar
    /// "aún no lo sé" sin inventarse un cuarto valor por su cuenta.
    #[allow(dead_code)]
    Desconocido,
    /// Responde, habla un contrato que entendemos y nos conoce.
    Conectado,
    /// Responde, pero todavía no le hemos dado credencial.
    ///
    /// Distinto de `CredencialInvalida`: aquí no falta configuración de Athena, falta un
    /// paso que la persona no ha dado, y decirle que su credencial no vale la mandaría a
    /// revisar una que no existe.
    SinCredencial,
    /// Responde, tenemos credencial y la rechaza.
    ///
    /// Es el caso que antes se contaba como «conectado»: Athena viva, aplicación
    /// anunciándose lista, y cada operación devolviendo 401 sin que nada lo avisara.
    CredencialInvalida,
    /// No responde: los runs de Athena quedan deshabilitados, el chat normal no.
    NoDisponible,
    /// Responde pero con un contrato que este cliente no sabe leer.
    Incompatible,
}

/// Un run terminado, tal y como consta en el registro duradero de Athena.
///
/// Tres cosas y no una: la proyección (lo mismo que se vio en vivo, reconstruido con el
/// mismo lector), el resumen que hace Athena de sus propios hechos, y los hechos en
/// bruto. El resumen no se recalcula aquí: quien escribe los hechos es quien mejor sabe
/// leerlos, y dos lectores acabarían discrepando sin que nadie supiera cuál miente.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HistoriaVista {
    pub proyeccion: ProyeccionRun,
    pub resumen: ResumenHistoria,
    pub hechos: Vec<HechoHistorico>,
}

/// Un hecho del registro, listo para enseñar en una línea.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HechoHistorico {
    /// Su sitio en el orden. Lo asigna el registro, no quien publica.
    pub secuencia: u64,
    pub nombre: String,
    pub cuando: String,
    /// Quién lo hizo: el run, o el delegado que lo hizo por él.
    pub actor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tarea: Option<String>,
    /// Cierto cuando lo hizo un delegado y no el propio run.
    pub delegado: bool,
}

impl HechoHistorico {
    fn desde(evento: &EventoHistorico) -> Self {
        Self {
            secuencia: evento.seq,
            nombre: evento.name.clone(),
            cuando: evento.occurred_at.clone(),
            actor: evento.provenance.actor.clone(),
            tarea: evento.provenance.task_id.clone(),
            delegado: evento.provenance.delegated,
        }
    }
}

/// Lo que la interfaz necesita saber del servicio. Nunca incluye el token.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstadoAreaAthena {
    pub estado: EstadoServicio,
    pub url_base: String,
    /// Cierto cuando hay credencial guardada. El valor no sale de Rust.
    pub credencial_configurada: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub version_contrato: Option<u32>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detalle: Option<String>,
    pub runs_activos: usize,
}

/// Guarda las proyecciones vivas y las alimenta desde los flujos.
#[derive(Clone)]
pub struct AreaAthena {
    cliente: AthenaClient,
    runs: Arc<Mutex<HashMap<String, ProyeccionRun>>>,
}

impl AreaAthena {
    pub fn nueva(cliente: AthenaClient) -> Self {
        Self {
            cliente,
            runs: Arc::new(Mutex::new(HashMap::new())),
        }
    }

    pub fn cliente(&self) -> &AthenaClient {
        &self.cliente
    }

    /// Comprueba el servicio. No propaga el error: la interfaz necesita un
    /// estado que enseñar, no una excepción, y que Athena no esté no es un
    /// fallo de ChatyGPT.
    pub async fn estado(&self, credencial_configurada: bool) -> EstadoAreaAthena {
        let activos = self.runs.lock().map(|mapa| mapa.len()).unwrap_or(0);
        match self.cliente.salud().await {
            Ok(salud) => {
                // Vivo. Que además nos conozca es otra pregunta y se hace aparte; sin
                // preguntarla, una credencial caducada se presentaba como «conectado».
                let (estado, detalle) = if !credencial_configurada {
                    (
                        EstadoServicio::SinCredencial,
                        Some("Falta la credencial del servicio de Athena.".to_owned()),
                    )
                } else {
                    match self.cliente.credencial_valida().await {
                        Ok(true) => (EstadoServicio::Conectado, None),
                        Ok(false) => (
                            EstadoServicio::CredencialInvalida,
                            Some(
                                "Athena está disponible pero rechaza la credencial \
                                 guardada. Vuelve a vincularla."
                                    .to_owned(),
                            ),
                        ),
                        // La comprobación no pudo hacerse: se informa de que está vivo y
                        // se dice por qué no se sabe más, en vez de acusar a la credencial.
                        Err(error) => (EstadoServicio::Conectado, Some(error.to_string())),
                    }
                };
                EstadoAreaAthena {
                    estado,
                    url_base: self.cliente.base_url(),
                    credencial_configurada,
                    version_contrato: Some(salud.wire_version),
                    detalle,
                    runs_activos: activos,
                }
            }
            Err(AppError::AthenaContract(detalle)) => EstadoAreaAthena {
                estado: EstadoServicio::Incompatible,
                url_base: self.cliente.base_url(),
                credencial_configurada,
                version_contrato: None,
                detalle: Some(detalle),
                runs_activos: activos,
            },
            Err(error) => EstadoAreaAthena {
                estado: EstadoServicio::NoDisponible,
                url_base: self.cliente.base_url(),
                credencial_configurada,
                version_contrato: None,
                detalle: Some(error.to_string()),
                runs_activos: activos,
            },
        }
    }

    /// Proyección de un run, si el área la sigue.
    pub fn proyeccion(&self, run_id: &str) -> Option<ProyeccionRun> {
        self.runs
            .lock()
            .ok()
            .and_then(|mapa| mapa.get(run_id).cloned())
    }

    /// Todas las proyecciones vivas. La usa la vista de grafo cuando existe; se
    /// conserva marcada en vez de borrarse porque su ausencia obligaría a exponer
    /// el mapa interno, que es peor.
    #[allow(dead_code)]
    pub fn proyecciones(&self) -> Vec<ProyeccionRun> {
        self.runs
            .lock()
            .map(|mapa| mapa.values().cloned().collect())
            .unwrap_or_default()
    }

    #[allow(dead_code)]
    pub fn olvidar(&self, run_id: &str) {
        if let Ok(mut mapa) = self.runs.lock() {
            mapa.remove(run_id);
        }
    }

    /// Lee una petición de permiso concreta antes de responderla.
    pub fn permiso(&self, run_id: &str, request_id: &str) -> Option<PermisoVista> {
        self.runs.lock().ok().and_then(|mapa| {
            mapa.get(run_id)
                .and_then(|vista| vista.permiso(request_id).cloned())
        })
    }

    /// Retira una petición al responderla, para que un segundo clic no llegue a salir.
    pub fn retirar_permiso(&self, run_id: &str, request_id: &str) -> bool {
        self.runs
            .lock()
            .ok()
            .and_then(|mut mapa| {
                mapa.get_mut(run_id)
                    .map(|vista| vista.retirar_permiso(request_id))
            })
            .unwrap_or(false)
    }

    /// Vuelve a seguir un run recordado de una sesión anterior.
    ///
    /// Sembrar la proyección antes de conectar evita que el área aparezca vacía
    /// mientras llega la primera instantánea, y conserva objetivo y carpeta, que
    /// son lo único que ChatyGPT guarda por su cuenta.
    pub fn readoptar(&self, run_id: &str, objetivo: &str, carpeta: &str) {
        if let Ok(mut mapa) = self.runs.lock() {
            mapa.entry(run_id.to_owned())
                .or_insert_with(|| ProyeccionRun::nueva(run_id, objetivo, carpeta));
        }
        self.seguir(run_id);
    }

    /// Abre un run y empieza a seguirlo.
    pub async fn abrir(
        &self,
        objetivo: &str,
        carpeta: &str,
        opciones: &OpcionesRun,
    ) -> Result<String, AppError> {
        let creado = self.cliente.crear_run(objetivo, carpeta, opciones).await?;
        let run_id = creado.run_id.clone();
        if let Ok(mut mapa) = self.runs.lock() {
            let mut vista = ProyeccionRun::nueva(&run_id, objetivo, carpeta);
            // El perfil se fija aquí y en ningún sitio más. Athena todavía no lo devuelve
            // en la respuesta, así que se prefiere el suyo cuando lo mande y se conserva
            // el pedido mientras no lo haga.
            vista.perfil_solicitado = if creado.profile.is_empty() {
                opciones.perfil.clone()
            } else {
                creado.profile.clone()
            };
            // Un run recién creado va por la primera revisión de su encargo: lo dice el
            // contrato de `GoalBoard`, que empieza en 1, no una suposición de aquí.
            vista.objetivo_revision = 1;
            vista.workspace_id = creado.workspace_id.clone();
            mapa.insert(run_id.clone(), vista);
        }
        self.seguir(&run_id);
        Ok(run_id)
    }

    /// Sigue un run existente, por ejemplo tras reanudarlo.
    pub fn seguir(&self, run_id: &str) {
        let cliente = self.cliente.clone();
        let runs = Arc::clone(&self.runs);
        let identificador = run_id.to_owned();
        // Si ya se siguió este run antes, se retoma por donde se quedó en vez de
        // pedir una resincronización entera: lo que la vista había derivado sigue
        // valiendo, y Athena sólo manda lo que falta.
        let desde = self.runs.lock().ok().and_then(|mapa| {
            mapa.get(run_id)
                .and_then(|vista| vista.ultimo_evento.clone())
        });
        tokio::spawn(async move {
            let mut flujo = cliente
                .flujo_eventos(&identificador, true)
                .con_reconexion(OpcionesReconexion::default())
                .desde_evento(desde);
            let resultado = flujo
                .escuchar(|mensaje| aplicar(&runs, &identificador, &mensaje))
                .await;
            if let Ok(mut mapa) = runs.lock() {
                if let Some(vista) = mapa.get_mut(&identificador) {
                    vista.conectado = false;
                }
            }
            if resultado.is_err() {
                logging::warn(
                    "athena.area_stream_ended",
                    None,
                    &[("run", logging::id(&identificador))],
                );
            }
        });
    }

    /// Refresca la proyección desde la instantánea, sin esperar a un evento.
    ///
    /// Athena manda el estado completo al conectar, pero una vista abierta
    /// mucho después de que el run terminara no recibiría nada; esto la pone al
    /// día con la fuente de verdad.
    pub async fn refrescar(&self, run_id: &str) -> Result<ProyeccionRun, AppError> {
        let instantanea = self.cliente.leer_run(run_id).await?;
        let mut mapa = self
            .runs
            .lock()
            .map_err(|_| AppError::AthenaContract("estado del área ilegible".to_owned()))?;
        let vista = mapa
            .entry(run_id.to_owned())
            .or_insert_with(|| ProyeccionRun::nueva(run_id, &instantanea.objective, ""));
        vista.adoptar_instantanea(&instantanea);
        Ok(vista.clone())
    }

    /// Reconstruye un run terminado a partir del registro duradero de Athena.
    ///
    /// **No es una segunda interpretación.** Los hechos se pasan por la misma
    /// `ProyeccionRun` que alimenta la vista en vivo, así que un run releído se lee igual
    /// que se leyó cuando pasaba. Escribir aquí un segundo lector garantizaría que
    /// antes o después los dos dijeran cosas distintas del mismo run, y no habría forma
    /// de saber cuál miente.
    ///
    /// La instantánea va primero y los hechos después: la instantánea es el estado que
    /// Athena persistió —verificación, artefactos, ficheros— y los hechos añaden el
    /// orden y la atribución que la instantánea no guarda.
    pub async fn historia(&self, run_id: &str) -> Result<HistoriaVista, AppError> {
        let historia = self.cliente.leer_historia(run_id).await?;
        let instantanea = self.cliente.leer_run(run_id).await.ok();
        let objetivo = instantanea
            .as_ref()
            .map(|valor| valor.objective.clone())
            .unwrap_or_default();
        let mut vista = ProyeccionRun::nueva(run_id, &objetivo, "");
        if let Some(instantanea) = &instantanea {
            vista.adoptar_instantanea(instantanea);
        }
        for hecho in &historia.events {
            vista.aplicar(&MensajeFlujo::Evento(Box::new(hecho.como_evento())));
        }
        // El run ya no está vivo: decir lo contrario invitaría a esperar cambios que no
        // van a llegar, y a ofrecer acciones que no harían nada.
        vista.conectado = false;
        Ok(HistoriaVista {
            proyeccion: vista,
            resumen: historia.summary,
            hechos: historia.events.iter().map(HechoHistorico::desde).collect(),
        })
    }

    /// Lee el encargo vigente de Athena y lo fija en la proyección.
    ///
    /// La instantánea no trae la revisión, así que sin esto ChatyGPT no tiene sobre qué
    /// decir que escribe. Se llama al abrir un run y al recuperarse de un conflicto.
    pub async fn objetivo(&self, run_id: &str) -> Result<ObjetivoRun, AppError> {
        let objetivo = self.cliente.leer_objetivo(run_id).await?;
        if let Ok(mut mapa) = self.runs.lock() {
            if let Some(vista) = mapa.get_mut(run_id) {
                vista.adoptar_objetivo(&objetivo.text, objetivo.revision, &objetivo.reason);
            }
        }
        Ok(objetivo)
    }

    /// Revisa el encargo sobre la revisión que esta vista conoce.
    ///
    /// La revisión no la elige quien llama: la pone la proyección, que es quien la
    /// mantiene al día con los eventos. Dejar que la interfaz mandase un número suyo
    /// permitiría escribir sobre una revisión que nunca vio.
    ///
    /// Ante un conflicto **no se reintenta**. Se deja la vista con el encargo vigente y
    /// se devuelve el conflicto: repetir con la revisión nueva es una decisión de quien
    /// escribió, no una recuperación automática — el encargo de otro puede ser
    /// incompatible con el suyo, y pisarlo en silencio es justo lo que ADR-029 impide.
    pub async fn revisar_objetivo(
        &self,
        run_id: &str,
        objetivo: &str,
        motivo: &str,
    ) -> Result<RevisionObjetivo, AppError> {
        let conocida = self
            .runs
            .lock()
            .ok()
            .and_then(|mapa| mapa.get(run_id).map(|vista| vista.objetivo_revision))
            .unwrap_or(0);
        // Cero es «no lo sé», no «la primera». Se pregunta antes de escribir en vez de
        // mandar un uno inventado que Athena rechazaría con un conflicto engañoso.
        let base = if conocida == 0 {
            self.objetivo(run_id).await?.revision
        } else {
            conocida
        };
        let resultado = self
            .cliente
            .revisar_objetivo(run_id, objetivo, base, motivo)
            .await?;
        if let Ok(mut mapa) = self.runs.lock() {
            if let Some(vista) = mapa.get_mut(run_id) {
                match &resultado {
                    // Aceptada: se anota la revisión nueva, pero el encargo mostrado no
                    // cambia todavía. Lo cambia `goal.revised`, cuando el bucle lo
                    // recoge de verdad — antes de eso, el agente sigue con el anterior.
                    RevisionObjetivo::Aceptada { objetivo } => {
                        vista.objetivo_revision = objetivo.revision;
                    }
                    RevisionObjetivo::Conflicto { vigente } => {
                        vista.adoptar_objetivo(&vigente.text, vigente.revision, &vigente.reason);
                    }
                }
            }
        }
        Ok(resultado)
    }

    /// Devuelve la vista después de contrastarla con la instantánea durable.
    ///
    /// El flujo SSE reduce la latencia, pero no es una fuente suficiente para
    /// responder a una consulta: una conexión puede seguir abierta justo
    /// cuando el runtime ya persistió el cierre. Consultar siempre confirma el
    /// estado con Athena, de modo que perder el último evento no deja un run
    /// eternamente en marcha en la interfaz.
    pub async fn consultar(&self, run_id: &str) -> Result<ProyeccionRun, AppError> {
        self.refrescar(run_id).await
    }
}

/// Aplica un mensaje del flujo a la proyección. Devuelve `false` cuando ya no
/// hay nada más que escuchar.
fn aplicar(
    runs: &Arc<Mutex<HashMap<String, ProyeccionRun>>>,
    run_id: &str,
    mensaje: &MensajeFlujo,
) -> bool {
    let Ok(mut mapa) = runs.lock() else {
        return false;
    };
    let vista = mapa
        .entry(run_id.to_owned())
        .or_insert_with(|| ProyeccionRun::nueva(run_id, "", ""));
    vista.aplicar(mensaje);
    // El run terminó: seguir escuchando solo mantendría una conexión abierta
    // para no recibir nada.
    !matches!(mensaje, MensajeFlujo::Evento(evento) if evento.es_final())
}

#[cfg(test)]
mod pruebas {
    use super::*;
    use crate::athena::simulated::{AthenaSimulado, GuionFlujo, RespuestaGuion};
    use serde_json::json;

    fn area(simulado: &AthenaSimulado) -> AreaAthena {
        let cliente = AthenaClient::for_base_url(&simulado.url_base()).expect("url válida");
        cliente.replace_token(Some("t")).expect("token válido");
        AreaAthena::nueva(cliente)
    }

    fn en_runtime<F: std::future::Future>(futuro: F) -> F::Output {
        tokio::runtime::Builder::new_current_thread()
            .enable_all()
            .build()
            .expect("runtime")
            .block_on(futuro)
    }

    #[test]
    fn la_historia_se_reconstruye_con_el_mismo_lector_que_la_vista_en_vivo() {
        // Los hechos duraderos se pasan por la misma `ProyeccionRun` que alimenta la
        // vista mientras el run pasa, asi que un run releido se lee igual que se leyo.
        // Un segundo lector garantizaria que antes o despues los dos contaran cosas
        // distintas del mismo run, y no habria forma de saber cual miente.
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/history",
            RespuestaGuion::ok(json!({
                "run_id": "run-1",
                "events": [
                    {"seq": 1, "event_id": "e1", "name": "agent.started", "version": 1,
                     "correlation_id": null, "occurred_at": "2026-08-22T10:00:00+00:00",
                     "provenance": {"run_id": "run-1", "session_id": "run-1",
                                    "actor": "root", "task_id": null, "delegated": false},
                     "payload": {}},
                    {"seq": 2, "event_id": "e2", "name": "subagent.started", "version": 1,
                     "correlation_id": "sub-1", "occurred_at": "2026-08-22T10:00:05+00:00",
                     "provenance": {"run_id": "run-1", "session_id": "T01",
                                    "actor": "root", "task_id": "T01", "delegated": false},
                     "payload": {"role": "explorer", "session_id": "sub-1",
                                 "parent_session_id": "T01", "provider": "native",
                                 "max_follow_ups": 2}},
                    {"seq": 3, "event_id": "e3", "name": "file.changed", "version": 1,
                     "correlation_id": null, "occurred_at": "2026-08-22T10:00:09+00:00",
                     "provenance": {"run_id": "run-1", "session_id": "sub-1",
                                    "actor": "explorer", "task_id": "T01",
                                    "delegated": true},
                     "payload": {"path": "calc.py"}},
                    {"seq": 4, "event_id": "e4", "name": "agent.completed", "version": 1,
                     "correlation_id": null, "occurred_at": "2026-08-22T10:01:00+00:00",
                     "provenance": {"run_id": "run-1", "session_id": "run-1",
                                    "actor": "root", "task_id": null, "delegated": false},
                     "payload": {"repair_cycles": 0}}
                ],
                "summary": {"status": "completed", "executed_as": "hierarchical",
                            "tasks": {"T01": "completed"},
                            "delegates": {"sub-1": "explorer"},
                            "verification": "passed", "permission_requests": 1},
            })),
        );
        simulado.responder(
            "/v1/runs/run-1",
            RespuestaGuion::ok(json!({
                "run_id": "run-1", "workspace_id": "ws-1", "status": "completed",
                "resumable": false, "degraded": false, "objective": "Arreglar calc.add",
                "created_at": "2026-08-22T10:00:00+00:00",
                "updated_at": "2026-08-22T10:01:00+00:00",
                "working_memory": {"objective": "Arreglar calc.add", "files_modified": []},
                "verification": {"status": "passed", "summary": "Todo pasa"},
                "tool_references": [], "checkpoints": [],
            })),
        );

        let historia = en_runtime(area(&simulado).historia("run-1")).expect("historia legible");

        assert_eq!(historia.resumen.status, "completed");
        assert_eq!(historia.resumen.executed_as, "hierarchical");
        // El delegado consta como delegado, no como una tarea mas del plan.
        assert_eq!(historia.proyeccion.delegados.len(), 1);
        assert_eq!(historia.proyeccion.delegados[0].proveedor, "native");
        // Y lo que hizo se le atribuye a el: con la raiz en su lugar, todo pareceria del
        // padre, que es justo lo que la procedencia existe para evitar.
        assert_eq!(
            historia.proyeccion.delegados[0].ficheros,
            vec!["calc.py".to_owned()]
        );
        assert!(historia.proyeccion.ficheros_modificados.is_empty());
        assert!(
            !historia.proyeccion.conectado,
            "un run releido no esta vivo"
        );
        assert_eq!(historia.hechos.len(), 4);
        assert!(historia.hechos[2].delegado);
        assert_eq!(historia.hechos[2].actor, "explorer");
    }

    #[test]
    fn un_run_del_que_no_consta_historia_no_se_enseña_como_run_vacio() {
        // Athena responde 404 a proposito: o no existio, o es anterior al registro.
        // Un 200 con lista vacia haria pasar la ausencia de historia por historia
        // completa, que es la clase de mentira que este proyecto persigue.
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/history",
            RespuestaGuion::error(404, "not_found", "No durable history for run-9"),
        );

        let resultado = en_runtime(area(&simulado).historia("run-9"));

        assert!(matches!(resultado, Err(AppError::NotFound(_))));
    }

    #[test]
    fn salud_200_con_credencial_invalida_no_es_estar_conectado() {
        // El caso reproducido contra el servicio real: Athena viva, `/v1/health` público
        // devolviendo 200, y la credencial guardada rechazada con 401. Antes de esto, la
        // aplicación decía «Conectado ✓ credencial configurada ✓» y luego fallaba cada
        // operación sin haber avisado de nada.
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/health",
            RespuestaGuion::ok(json!({"status": "ok", "wire_version": 1, "runs": 0})),
        );
        simulado.responder(
            "/v1/auth/check",
            RespuestaGuion::error(401, "unauthorized", "credencial no válida"),
        );

        let estado = en_runtime(area(&simulado).estado(true));

        assert_eq!(estado.estado, EstadoServicio::CredencialInvalida);
        // Sigue habiendo credencial guardada: lo que falla es que valga, y son cosas
        // distintas para quien tiene que arreglarlo.
        assert!(estado.credencial_configurada);
        assert!(estado
            .detalle
            .as_deref()
            .is_some_and(|texto| texto.contains("vincular")));
    }

    #[test]
    fn salud_200_sin_credencial_pide_credencial_y_no_culpa_a_ninguna() {
        // Sin credencial no hay nada que comprobar, y decir que «no vale» mandaría a la
        // persona a revisar una que no ha llegado a existir.
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/health",
            RespuestaGuion::ok(json!({"status": "ok", "wire_version": 1, "runs": 0})),
        );

        let estado = en_runtime(area(&simulado).estado(false));

        assert_eq!(estado.estado, EstadoServicio::SinCredencial);
        assert!(!estado.credencial_configurada);
    }

    #[test]
    fn salud_200_con_credencial_valida_si_es_estar_conectado() {
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/health",
            RespuestaGuion::ok(json!({"status": "ok", "wire_version": 1, "runs": 0})),
        );
        simulado.responder(
            "/v1/auth/check",
            RespuestaGuion::ok(json!({"authenticated": true, "wire_version": 1})),
        );

        let estado = en_runtime(area(&simulado).estado(true));

        assert_eq!(estado.estado, EstadoServicio::Conectado);
        assert!(estado.detalle.is_none());
    }

    #[test]
    fn una_comprobacion_que_no_se_puede_hacer_no_acusa_a_la_credencial() {
        // El servicio contesta a `/v1/health` y se rompe al comprobar. No se sabe si la
        // credencial vale, así que no se dice que no valga: se informa de que está vivo y
        // de por qué no se sabe más.
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/health",
            RespuestaGuion::ok(json!({"status": "ok", "wire_version": 1, "runs": 0})),
        );
        simulado.responder(
            "/v1/auth/check",
            RespuestaGuion::error(500, "internal", "algo se rompió dentro"),
        );

        let estado = en_runtime(area(&simulado).estado(true));

        assert_eq!(estado.estado, EstadoServicio::Conectado);
        assert!(
            estado.detalle.is_some(),
            "debería decir por qué no se sabe más"
        );
    }

    #[test]
    fn un_servicio_caido_da_estado_no_disponible_en_vez_de_error() {
        // Que Athena no esté no puede romper la aplicación: solo deshabilita su
        // área. El chat normal sigue yendo al Broker.
        let cliente = AthenaClient::for_base_url("http://127.0.0.1:1").expect("url válida");
        let area = AreaAthena::nueva(cliente);

        let estado = en_runtime(area.estado(false));

        assert_eq!(estado.estado, EstadoServicio::NoDisponible);
        assert!(!estado.credencial_configurada);
        assert!(estado.detalle.is_some());
    }

    #[test]
    fn un_contrato_incompatible_se_distingue_de_un_servicio_caido() {
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/health",
            RespuestaGuion::ok(json!({"status": "ok", "wire_version": 99, "runs": 0})),
        );

        let estado = en_runtime(area(&simulado).estado(true));

        assert_eq!(estado.estado, EstadoServicio::Incompatible);
        assert!(estado.credencial_configurada);
    }

    #[test]
    fn el_estado_no_expone_nunca_el_token() {
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/health",
            RespuestaGuion::ok(json!({"status": "ok", "wire_version": 1, "runs": 0})),
        );
        let area = area(&simulado);

        let estado = en_runtime(area.estado(true));
        let serializado = serde_json::to_string(&estado).expect("serializable");

        assert!(!serializado.contains("\"t\""));
        assert!(!serializado.to_lowercase().contains("token"));
    }

    #[test]
    fn seguir_un_run_construye_la_proyeccion_desde_sus_eventos() {
        let simulado = AthenaSimulado::arrancar();
        simulado.emitir(GuionFlujo {
            marcos: vec![
                GuionFlujo::marco_estado(
                    "sus-1",
                    true,
                    json!({
                        "run_id": "run-1", "workspace_id": "ws", "status": "running",
                        "resumable": false, "degraded": false, "objective": "Arreglar",
                        "created_at": "", "updated_at": "",
                        "working_memory": {"objective": "Arreglar", "files_modified": []},
                        "verification": {}, "tool_references": [], "checkpoints": []
                    }),
                ),
                GuionFlujo::marco_evento(
                    "tool.started",
                    "run-1",
                    json!({"tool_name": "read_file"}),
                ),
                GuionFlujo::marco_evento("agent.completed", "run-1", json!({"repair_cycles": 0})),
            ],
            cortar_al_final: true,
            retardo: None,
        });
        let area = area(&simulado);

        en_runtime(async {
            area.seguir("run-1");
            for _ in 0..100 {
                if area
                    .proyeccion("run-1")
                    .and_then(|vista| vista.fase)
                    .is_some_and(|fase| fase.es_terminal())
                {
                    break;
                }
                tokio::time::sleep(std::time::Duration::from_millis(20)).await;
            }
        });

        let vista = area.proyeccion("run-1").expect("proyección seguida");
        assert!(vista.fase.expect("fase").es_terminal());
        assert_eq!(vista.herramientas[0].nombre, "read_file");
        assert_eq!(vista.suscriptor.as_deref(), Some("sus-1"));
    }

    #[test]
    fn refrescar_pone_la_proyeccion_al_dia_con_la_fuente_de_verdad() {
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/runs/run-1",
            RespuestaGuion::ok(json!({
                "run_id": "run-1", "workspace_id": "ws", "status": "completed",
                "resumable": false, "degraded": false, "objective": "Arreglar",
                "created_at": "", "updated_at": "",
                "working_memory": {"objective": "Arreglar", "files_modified": ["calc.py"]},
                "verification": {"status": "passed", "summary": "Todo pasa"},
                "tool_references": [], "checkpoints": []
            })),
        );
        let area = area(&simulado);

        let vista = en_runtime(area.refrescar("run-1")).expect("refrescada");

        assert_eq!(vista.ficheros_modificados, vec!["calc.py".to_owned()]);
        assert_eq!(vista.verificacion.as_deref(), Some("passed"));
        assert!(area.proyeccion("run-1").is_some(), "queda guardada");
    }

    #[test]
    fn consultar_no_confia_en_una_conexion_que_conserva_un_estado_obsoleto() {
        let simulado = AthenaSimulado::arrancar();
        simulado.responder(
            "/v1/runs/run-1",
            RespuestaGuion::ok(json!({
                "run_id": "run-1", "workspace_id": "ws", "status": "failed",
                "resumable": false, "degraded": false, "objective": "Crear un archivo",
                "created_at": "", "updated_at": "",
                "working_memory": {
                    "objective": "Crear un archivo",
                    "files_modified": [],
                    "errors": [{
                        "code": "model_permanent_error",
                        "message": "El modelo devolvio texto en vez de una decision de herramienta",
                        "recovery_action": "abort"
                    }]
                },
                "verification": {}, "tool_references": [], "checkpoints": []
            })),
        );
        let area = area(&simulado);
        let mut obsoleta = ProyeccionRun::nueva("run-1", "Crear un archivo", "C:\\repo");
        obsoleta.conectado = true;
        area.runs
            .lock()
            .expect("estado")
            .insert("run-1".to_owned(), obsoleta);

        let vista = en_runtime(area.consultar("run-1")).expect("sincronizada");

        assert_eq!(vista.fase, Some(crate::athena::FaseRun::Failed));
        assert_eq!(vista.errores.len(), 1);
        assert_eq!(vista.errores[0].codigo, "model_permanent_error");
    }

    #[test]
    fn olvidar_un_run_lo_saca_del_area() {
        let simulado = AthenaSimulado::arrancar();
        let area = area(&simulado);
        area.runs
            .lock()
            .expect("estado")
            .insert("run-1".to_owned(), ProyeccionRun::nueva("run-1", "x", "y"));

        area.olvidar("run-1");

        assert!(area.proyeccion("run-1").is_none());
        assert!(area.proyecciones().is_empty());
    }
}
