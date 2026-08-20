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

use super::contracts::MensajeFlujo;
use super::supervisor::{PermisoVista, ProyeccionRun};
use super::{AthenaClient, OpcionesReconexion, OpcionesRun};
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
    /// Responde y habla una versión de contrato que entendemos.
    Conectado,
    /// No responde: los runs de Athena quedan deshabilitados, el chat normal no.
    NoDisponible,
    /// Responde pero con un contrato que este cliente no sabe leer.
    Incompatible,
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
            Ok(salud) => EstadoAreaAthena {
                estado: EstadoServicio::Conectado,
                url_base: self.cliente.base_url(),
                credencial_configurada,
                version_contrato: Some(salud.wire_version),
                detalle: None,
                runs_activos: activos,
            },
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
            mapa.insert(
                run_id.clone(),
                ProyeccionRun::nueva(&run_id, objetivo, carpeta),
            );
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
