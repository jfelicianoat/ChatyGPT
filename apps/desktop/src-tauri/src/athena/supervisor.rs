//! Proyección de un run, construida a partir de lo que Athena publica.
//!
//! Regla que gobierna este módulo: **aquí no se deduce el estado del agente**.
//! Cada campo procede de un evento o de una instantánea del runtime. Si Athena
//! no lo ha dicho, la interfaz no lo muestra; y cuando el runtime y la
//! proyección discrepan, gana el runtime — por eso una instantánea nueva
//! sustituye lo acumulado en lugar de mezclarse con ello.
//!
//! Tampoco se expone el razonamiento del modelo. No hace falta filtrarlo: los
//! eventos de Athena no llevan el contenido de los mensajes del asistente, solo
//! hechos operativos (qué herramienta, qué fichero, qué veredicto). Lo que la
//! interfaz enseña son esos hechos, no el pensamiento que llevó a ellos.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::Value;

use super::contracts::{
    EstadoRun, EstrategiaEjecucion, EventoRuntime, InstantaneaRun, MarcoEstado, MensajeFlujo,
    ResultadoMostrable,
};

/// Cuántas entradas se conservan de cada historial. Lo viejo se descarta: la
/// interfaz muestra lo que está pasando, no un registro completo.
const LIMITE_HISTORIAL: usize = 200;

/// Fase del run tal y como la publica Athena.
///
/// No hay ninguna fase inventada por ChatyGPT: son los estados del runtime más
/// `Starting`, que es el hueco entre pedir el run y recibir su primer estado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum FaseRun {
    Starting,
    Running,
    WaitingPermission,
    Verifying,
    Completed,
    Failed,
    /// El trabajo termino y no se pudo comprobar. No es lo mismo que haber
    /// fallado, y contarlo igual le echa la culpa al cambio de una maquina rota
    /// o de un proyecto que nunca definio checks. Athena las distingue desde
    /// ADR-027; esto es lo que hace que la distincion llegue a una persona.
    Unverified,
    Cancelled,
    RecoveryPending,
}

impl FaseRun {
    fn desde_estado(estado: EstadoRun) -> Option<Self> {
        match estado {
            EstadoRun::Idle | EstadoRun::Running => Some(Self::Running),
            EstadoRun::WaitingPermission => Some(Self::WaitingPermission),
            EstadoRun::Verifying => Some(Self::Verifying),
            EstadoRun::Completed => Some(Self::Completed),
            EstadoRun::Failed => Some(Self::Failed),
            EstadoRun::Cancelled => Some(Self::Cancelled),
            EstadoRun::RecoveryPending => Some(Self::RecoveryPending),
            // Un estado que este cliente no conoce no cambia la fase: es mejor
            // seguir mostrando la última conocida que inventar una.
            EstadoRun::Desconocido => None,
        }
    }

    /// Nombre estable de la fase, el mismo que viaja a la interfaz.
    pub fn palabra(self) -> &'static str {
        match self {
            Self::Starting => "starting",
            Self::Running => "running",
            Self::WaitingPermission => "waiting_permission",
            Self::Verifying => "verifying",
            Self::Completed => "completed",
            Self::Failed => "failed",
            Self::Unverified => "unverified",
            Self::Cancelled => "cancelled",
            Self::RecoveryPending => "recovery_pending",
        }
    }

    pub fn es_terminal(self) -> bool {
        // `Unverified` es terminal: el run no va a cambiar solo. Dejarlo fuera
        // haria que la interfaz siguiera sondeando un run acabado para siempre.
        matches!(
            self,
            Self::Completed | Self::Failed | Self::Unverified | Self::Cancelled
        )
    }
}

/// Estado de una tarea del TaskManager, cuando el run usa tareas.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoTarea {
    Pending,
    Running,
    Completed,
    Failed,
    Cancelled,
    Killed,
    RecoveryPending,
}

impl EstadoTarea {
    #[allow(dead_code)]
    fn desde_texto(valor: &str) -> Option<Self> {
        Some(match valor {
            "pending" => Self::Pending,
            "running" => Self::Running,
            "completed" => Self::Completed,
            "failed" => Self::Failed,
            "cancelled" => Self::Cancelled,
            "killed" => Self::Killed,
            "recovery_pending" => Self::RecoveryPending,
            _ => return None,
        })
    }
}

/// Una tarea o subagente en curso, con su presupuesto si Athena lo publica.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TareaVista {
    pub id: String,
    pub nombre: String,
    pub estado: EstadoTarea,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub iteraciones: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llamadas_herramienta: Option<u64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detalle: Option<String>,
    /// Especialista al que Athena asignó la tarea. Vacío cuando el run no es
    /// jerárquico, que es la mayoría de las veces.
    #[serde(default)]
    pub rol: String,
    /// Tareas de las que ésta depende. Es lo que permite dibujar el grafo en vez
    /// de una lista, y viene de Athena: la interfaz no infiere ninguna.
    #[serde(default)]
    pub dependencias: Vec<String>,
    /// Ficheros que la tarea cambió, según su propia evidencia.
    #[serde(default)]
    pub ficheros: Vec<String>,
}

/// Un delegado del run: un especialista al que se le encargó una parte.
///
/// Lista aparte de `tareas` a propósito. Athena lo dice en su propio catálogo de
/// eventos: «una tarea *usa* un subagente; no *es* uno, y una vista que los confundiera
/// no podría dibujar el plan». Mezclarlos hacía que un run jerárquico enseñara el doble
/// de trabajo del que había.
///
/// Lo que se muestra son hechos operativos. El transcript del delegado no llega hasta
/// aquí —Athena entrega un resumen, nunca la conversación del hijo— y su razonamiento
/// tampoco existe en el bus.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct DelegadoVista {
    /// Sesión del delegado. Es su nombre para todo lo demás.
    pub sesion: String,
    /// Sesión de quien lo encargó: el run, o la tarea que lo usa.
    pub padre: String,
    /// Tarea del plan a la que pertenece, cuando el padre es una tarea.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub tarea: Option<String>,
    pub rol: String,
    /// Quién lo ejecuta. Vacío si el despliegue no lo publica.
    pub proveedor: String,
    pub estado: EstadoTarea,
    pub encargo: String,
    /// Cierto cuando se le puede volver a preguntar sin gastar un delegado nuevo.
    pub continuable: bool,
    /// Veces que ya se le ha vuelto a preguntar.
    pub seguimientos: u32,
    /// Cuántas preguntas más admite, cuando Athena lo dice al devolver el resultado.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub seguimientos_restantes: Option<u32>,
    /// Qué está haciendo, derivado de sus propios eventos. Nunca su razonamiento.
    pub actividad: Vec<String>,
    /// Lo que informó al terminar: un resumen, no un transcript.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resumen: Option<String>,
    pub ficheros: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub llamadas_herramienta: Option<u64>,
    /// Por qué su tarea no puede avanzar, dicho por el ejecutor del grafo.
    pub bloqueos: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<ErrorVista>,
}

/// Uso de una herramienta.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UsoHerramienta {
    pub nombre: String,
    pub estado: &'static str,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub correlacion: Option<String>,
    /// Cierto cuando el resultado se externalizó por tamaño.
    pub externalizado: bool,
    /// Cómo enseñar el resultado, tal y como lo derivó Athena.
    ///
    /// Ausente mientras la tool está en curso —todavía no hay resultado— y también
    /// cuando el runtime no publicó ninguna: no se inventa una presentación de
    /// repuesto, porque una presentación inventada no se distingue de una real.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub presentacion: Option<PresentacionVista>,
}

/// La presentación de un resultado, copiada de lo que publicó Athena.
///
/// Aquí no se decide nada: `clase` viene del runtime y esta capa solo elige con qué
/// palabras dibujarla. Deducirla del contenido —«esto parece una lista»— era justo lo
/// que ADR-026 vino a quitar de en medio.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PresentacionVista {
    /// `text`, `items`, `change`, `record` o `reference`.
    pub clase: String,
    pub titulo: String,
    pub resumen: String,
    pub elementos: Vec<String>,
    /// Los campos sueltos de un resultado estructurado, en el orden en que llegaron.
    pub hechos: Vec<HechoVista>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub referencia: Option<String>,
}

/// Un campo de un resultado estructurado, ya legible.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct HechoVista {
    pub nombre: String,
    pub valor: String,
}

/// Petición de permiso mostrada al usuario.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PermisoVista {
    pub request_id: String,
    pub herramienta: String,
    /// Operación concreta, distinta del nombre de la herramienta.
    pub operacion: String,
    pub accion: String,
    pub riesgo: String,
    pub nivel: String,
    pub motivo: String,
    pub efectos: Vec<String>,
    /// Ficheros o recursos que la acción toca, según Athena.
    pub recursos: Vec<String>,
    /// Carpeta sobre la que se ejecuta.
    pub workspace: String,
    /// Argumentos saneados en origen: aquí no se recorta nada más.
    pub argumentos: Vec<ArgumentoVista>,
    pub solo_lectura: bool,
    pub destructivo: bool,
    pub confirmado: bool,
    pub segundos_restantes: f64,
    /// Cierto cuando el plazo se agotó: ya no se puede responder.
    pub caducado: bool,
}

/// Un argumento tal y como se enseña: valor corto, resumen de uno largo, o la
/// marca de que Athena lo redactó.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArgumentoVista {
    pub nombre: String,
    pub valor: String,
    /// Tamaño original cuando el valor venía resumido.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub caracteres: Option<u64>,
    pub redactado: bool,
    pub resumido: bool,
}

/// Comprobación de verificación.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ComprobacionVista {
    pub nombre: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paso: Option<bool>,
}

/// Error mostrado al usuario: la clase y el mensaje que Athena publicó, con la
/// acción de recuperación que el runtime decidió. No se reinterpreta ninguna.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ErrorVista {
    pub codigo: String,
    pub mensaje: String,
    /// Cual de los huecos fue, cuando el codigo dice que no se pudo comprobar.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub razon: Option<String>,
    /// El dato tipado que Athena adjunta al fallo: `ADMIN_AUTH_REQUIRED` cuando el
    /// broker rechaza la credencial, y cosas por el estilo. Es lo único accionable de
    /// muchos fallos y se estaba tirando: en pantalla quedaba «HTTP 403» sin decir que
    /// lo que hay que hacer es renovar el token.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detalle: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub recuperacion: Option<String>,
}

/// Artefacto disponible para abrir.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArtefactoVista {
    pub clave: String,
    pub uri: String,
    pub tipo: String,
    pub tamano: u64,
}

/// Todo lo que el área de Athena muestra de un run.
#[derive(Debug, Clone, Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct ProyeccionRun {
    pub run_id: String,
    pub objetivo: String,
    /// Revisión del encargo sobre la que se está trabajando.
    ///
    /// Cero mientras no se sabe: la instantánea no la trae, así que hasta que alguien
    /// la lee o llega un `goal.revised` no hay número que enseñar. Poner `1` por defecto
    /// sería afirmar que el encargo no se ha tocado, que es una afirmación distinta de
    /// no saberlo — y la que haría fallar la siguiente revisión sin explicar por qué.
    pub objetivo_revision: u32,
    /// Por qué se cambió el encargo la última vez, dicho por quien lo cambió.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub motivo_revision: Option<String>,
    /// El perfil con el que se pidió el run, si se pidió alguno.
    ///
    /// Es lo que se **pidió**, no lo que Athena confirma: la instantánea no lo publica
    /// todavía. Queda fijado al crear el run y no hay camino para cambiarlo después —
    /// cambiar de perfil a mitad cambiaría qué herramientas existen y qué cuenta como
    /// prueba, y la evidencia ya reunida dejaría de significar lo que decía.
    pub perfil_solicitado: String,
    pub fase: Option<FaseRun>,
    pub carpeta: String,
    /// El identificador de espacio de trabajo que usa Athena.
    ///
    /// Distinto de la ruta, y es el que hace de proyecto para la memoria: pedirla por
    /// ruta funcionaría hasta el día que dos máquinas montasen la misma carpeta en
    /// sitios distintos.
    pub workspace_id: String,
    /// Cierto cuando Athena tuvo que reconstruir estado dañado tras un fallo.
    pub degradado: bool,
    pub reanudable: bool,
    /// Cierto mientras el flujo de eventos esté conectado.
    pub conectado: bool,
    /// Identidad que permite responder a los permisos de este run.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub suscriptor: Option<String>,
    pub controla: bool,
    /// Último evento aplicado. Sobrevive a la tarea que escucha el flujo, que es
    /// justo lo que hace falta para reanudar en vez de resincronizar.
    #[serde(skip)]
    pub ultimo_evento: Option<String>,

    pub tareas: Vec<TareaVista>,
    /// Los especialistas a los que se les encargó parte del trabajo.
    pub delegados: Vec<DelegadoVista>,
    pub herramientas: Vec<UsoHerramienta>,
    pub permisos: Vec<PermisoVista>,
    pub comprobaciones: Vec<ComprobacionVista>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub verificacion: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resumen_verificacion: Option<String>,
    pub ficheros_modificados: Vec<String>,
    pub artefactos: Vec<ArtefactoVista>,
    pub errores: Vec<ErrorVista>,
    /// Explicaciones operativas, en orden. Nunca razonamiento del modelo.
    pub actividad: Vec<String>,
    /// Evidencia final: lo que permitió dar el trabajo por terminado.
    pub evidencia: Vec<String>,
    pub ciclos_reparacion: u64,
    /// Con qué estrategia se está ejecutando este run, cuando Athena ya lo ha dicho.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub estrategia: Option<EstrategiaVista>,
}

/// La estrategia de ejecución, lista para enseñar.
///
/// Copia de lo que publicó Athena sin interpretarlo. Traducir aquí `reason_code` a una
/// frase distinta de la que vino sería que la interfaz opinara sobre una decisión que no
/// tomó; lo único que se hace más abajo es darle a cada código un nombre legible.
#[derive(Debug, Clone, Default, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EstrategiaVista {
    pub solicitada: String,
    pub seleccionada: String,
    pub codigo: String,
    pub motivo: String,
    pub veredicto_politica: String,
    pub explicacion_politica: String,
    pub criterios: Vec<String>,
    pub senales_supuestas: Vec<String>,
}

impl From<&EstrategiaEjecucion> for EstrategiaVista {
    fn from(origen: &EstrategiaEjecucion) -> Self {
        Self {
            solicitada: origen.execution_mode.clone(),
            seleccionada: origen.executed_as.clone(),
            codigo: origen.reason_code.clone(),
            motivo: origen.reason.clone(),
            veredicto_politica: origen.policy_verdict.clone(),
            explicacion_politica: origen.policy_explanation.clone(),
            criterios: origen.criteria_met.clone(),
            senales_supuestas: origen.assumed_signals.clone(),
        }
    }
}

impl ProyeccionRun {
    pub fn nueva(run_id: &str, objetivo: &str, carpeta: &str) -> Self {
        Self {
            run_id: run_id.to_owned(),
            objetivo: objetivo.to_owned(),
            carpeta: carpeta.to_owned(),
            fase: Some(FaseRun::Starting),
            ..Self::default()
        }
    }

    /// Aplica un mensaje del flujo. Es el único camino por el que cambia.
    /// Registra una tarea que empieza, o la actualiza si ya estaba.
    ///
    /// Idempotente porque una reconexión puede reentregar el evento, y una tarea
    /// duplicada en la vista se lee como trabajo duplicado.
    fn tarea_empieza(&mut self, carga: &BTreeMap<String, Value>, correlacion: Option<&str>) {
        let id = match texto(carga, "task_id").or_else(|| correlacion.map(str::to_owned)) {
            Some(valor) => valor,
            None => return,
        };
        let rol = texto(carga, "role").unwrap_or_default();
        let objetivo = texto(carga, "goal").unwrap_or_else(|| id.clone());
        if let Some(existente) = self.tareas.iter_mut().find(|tarea| tarea.id == id) {
            existente.estado = EstadoTarea::Running;
            // El plan de la instantánea no trae rol ni dependencias; el evento sí. Se
            // completa lo que faltaba en vez de sustituir la tarea, que ya es la misma.
            if existente.rol.is_empty() {
                existente.rol = rol;
            }
            if existente.dependencias.is_empty() {
                existente.dependencias = lista(carga, "dependencies");
            }
            return;
        }
        self.tareas.push(TareaVista {
            id,
            nombre: objetivo,
            estado: EstadoTarea::Running,
            iteraciones: None,
            llamadas_herramienta: None,
            detalle: None,
            rol,
            dependencias: lista(carga, "dependencies"),
            ficheros: Vec::new(),
        });
    }

    /// Cierra una tarea con lo que su evidencia dice, no con lo que dijo el modelo.
    fn tarea_termina(
        &mut self,
        carga: &BTreeMap<String, Value>,
        correlacion: Option<&str>,
        completada: bool,
    ) {
        let id = match texto(carga, "task_id").or_else(|| correlacion.map(str::to_owned)) {
            Some(valor) => valor,
            None => return,
        };
        let ficheros = lista(carga, "files_changed");
        let resumen = texto(carga, "summary");
        if let Some(tarea) = self.tareas.iter_mut().find(|tarea| tarea.id == id) {
            tarea.estado = if completada {
                EstadoTarea::Completed
            } else {
                EstadoTarea::Failed
            };
            tarea.detalle = resumen;
            tarea.ficheros = ficheros.clone();
        }
        // Los ficheros de una tarea son ficheros del run: si no subieran, la
        // vista diría que no se tocó nada mientras el grafo cambiaba medio repo.
        for fichero in ficheros {
            if !self.ficheros_modificados.contains(&fichero) {
                self.ficheros_modificados.push(fichero);
            }
        }
    }

    /// Devuelve una petición de permiso concreta, si sigue en pie.
    pub fn permiso(&self, request_id: &str) -> Option<&PermisoVista> {
        self.permisos
            .iter()
            .find(|permiso| permiso.request_id == request_id)
    }

    /// Retira una petición ya contestada.
    ///
    /// Se hace en cuanto se envía la respuesta, sin esperar al evento de
    /// Athena: si no, la pregunta sigue en pantalla el tiempo que tarde el
    /// viaje de vuelta y un segundo clic manda una respuesta duplicada.
    /// Devuelve si la petición estaba realmente ahí.
    pub fn retirar_permiso(&mut self, request_id: &str) -> bool {
        let antes = self.permisos.len();
        self.permisos
            .retain(|permiso| permiso.request_id != request_id);
        self.permisos.len() != antes
    }

    pub fn aplicar(&mut self, mensaje: &MensajeFlujo) {
        match mensaje {
            MensajeFlujo::Estado(estado) => self.aplicar_estado(estado),
            MensajeFlujo::Evento(evento) => {
                // El punto de reanudación avanza con cada evento aplicado, no
                // con cada evento recibido: si aplicarlo falla, reanudar desde
                // él saltaría un cambio que la vista nunca llegó a reflejar.
                self.ultimo_evento = Some(evento.event_id.clone());
                self.aplicar_evento(evento);
            }
        }
    }

    /// Una instantánea **sustituye** lo derivado, no se mezcla con ello: es la
    /// versión de Athena de la verdad y llega precisamente cuando la nuestra
    /// puede haberse quedado atrás.
    fn aplicar_estado(&mut self, estado: &MarcoEstado) {
        self.suscriptor = Some(estado.subscriber_id.clone());
        self.controla = estado.controls;
        self.conectado = true;
        if let Some(forma) = &estado.shape {
            self.estrategia = Some(EstrategiaVista::from(forma));
        }
        if let Some(instantanea) = &estado.snapshot {
            self.adoptar_instantanea(instantanea);
        }
        self.permisos = estado
            .pending_approvals
            .iter()
            .map(PermisoVista::desde_pendiente)
            .collect();
    }

    pub fn adoptar_instantanea(&mut self, instantanea: &InstantaneaRun) {
        self.run_id = instantanea.run_id.clone();
        self.workspace_id = instantanea.workspace_id.clone();
        if !instantanea.objective.is_empty() {
            self.objetivo = instantanea.objective.clone();
        }
        if let Some(fase) = FaseRun::desde_estado(instantanea.status) {
            self.fase = Some(fase);
        }
        self.degradado = instantanea.degraded;
        self.reanudable = instantanea.resumable;
        self.ficheros_modificados = instantanea.ficheros_modificados();
        self.verificacion = instantanea.estado_verificacion().map(str::to_owned);
        self.resumen_verificacion = instantanea
            .verification
            .get("summary")
            .and_then(Value::as_str)
            .map(str::to_owned);
        self.artefactos = instantanea
            .tool_references
            .iter()
            .map(|referencia| ArtefactoVista {
                clave: referencia.store_key.clone(),
                uri: referencia.uri.clone(),
                tipo: referencia.media_type.clone(),
                tamano: referencia.size_chars,
            })
            .collect();
        self.errores = instantanea
            .working_memory
            .get("errors")
            .and_then(Value::as_array)
            .into_iter()
            .flatten()
            .filter_map(|error| {
                let codigo = error.get("code")?.as_str()?.to_owned();
                Some(ErrorVista {
                    codigo,
                    mensaje: error
                        .get("message")
                        .and_then(Value::as_str)
                        .unwrap_or_default()
                        .to_owned(),
                    razon: error
                        .get("reason")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    detalle: error
                        .get("detail")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                    recuperacion: error
                        .get("recovery_action")
                        .and_then(Value::as_str)
                        .map(str::to_owned),
                })
            })
            .collect();
        self.tareas = tareas_de(&instantanea.working_memory);
    }

    fn aplicar_evento(&mut self, evento: &EventoRuntime) {
        // Lo que hace un delegado se le atribuye al delegado, no al run. Antes esto no
        // se planteaba porque los eventos del hijo ni siquiera llegaban; ahora llegan, y
        // contarlos como del padre enseñaría como propias las escrituras de otro agente.
        if self.atribuir_a_delegado(evento) {
            return;
        }
        let carga = &evento.payload;
        match evento.name.as_str() {
            "agent.started" => {
                self.fase = Some(FaseRun::Running);
                self.anotar("El agente ha empezado");
            }
            "agent.completed" => {
                self.fase = Some(FaseRun::Completed);
                self.permisos.clear();
                self.ciclos_reparacion = numero(carga, "repair_cycles").unwrap_or(0);
                if let Some(resumen) = texto(carga, "verification") {
                    self.evidencia.push(resumen.clone());
                    self.resumen_verificacion = Some(resumen);
                }
                self.anotar("Trabajo terminado y verificado");
            }
            "agent.failed" => {
                // Dos finales distintos llegan por el mismo evento, y el codigo
                // tipado es lo unico que los separa. Leerlo del mensaje seria
                // acertar hasta la primera vez que alguien lo reescriba.
                let sin_comprobar = texto(carga, "error_code")
                    .is_some_and(|codigo| codigo == "verification_inconclusive");
                self.fase = Some(if sin_comprobar {
                    FaseRun::Unverified
                } else {
                    FaseRun::Failed
                });
                self.permisos.clear();
                self.registrar_error(carga, None);
                self.anotar(if sin_comprobar {
                    "El trabajo termino, pero no se pudo comprobar"
                } else {
                    "El agente ha fallado"
                });
            }
            // El nivel de grafo. Un run jerárquico anuncia su plan antes de
            // ejecutarlo, y cada tarea se sigue por separado: la vista puede
            // dibujar el plan entero desde el principio en vez de descubrirlo.
            "graph.started" => {
                self.fase = Some(FaseRun::Running);
                if let Some(total) = carga.get("tasks").and_then(Value::as_u64) {
                    self.anotar(&format!("Plan preparado: {total} tareas"));
                }
            }
            "graph.completed" => {
                self.fase = Some(FaseRun::Completed);
                self.permisos.clear();
                self.anotar("El plan terminó");
            }
            "graph.failed" => {
                self.fase = Some(FaseRun::Failed);
                self.permisos.clear();
                self.registrar_error(carga, None);
                self.anotar("El plan falló");
            }
            "graph.cancelled" => {
                self.fase = Some(FaseRun::Cancelled);
                self.permisos.clear();
                self.anotar("El plan fue cancelado");
            }
            "task.started" => self.tarea_empieza(carga, evento.correlation_id.as_deref()),
            "task.completed" | "task.failed" => self.tarea_termina(
                carga,
                evento.correlation_id.as_deref(),
                evento.name == "task.completed",
            ),
            "agent.cancelled" => {
                self.fase = Some(FaseRun::Cancelled);
                // Las preguntas de un run cancelado se retiran: contestarlas ya
                // no haría nada y dejarlas en pantalla invita a intentarlo.
                self.permisos.clear();
                self.anotar("Cancelado");
            }
            "tool.started" => {
                if let Some(nombre) = texto(carga, "tool_name") {
                    self.herramientas.push(UsoHerramienta {
                        nombre: nombre.clone(),
                        estado: "en curso",
                        correlacion: evento.correlation_id.clone(),
                        externalizado: false,
                        presentacion: None,
                    });
                    recortar(&mut self.herramientas);
                    self.anotar(&format!("Usando {nombre}"));
                }
            }
            "tool.completed" => self.cerrar_herramienta(evento, "terminada"),
            "tool.failed" => {
                self.cerrar_herramienta(evento, "fallida");
                self.registrar_error(carga, None);
            }
            "permission.requested" => self.registrar_permiso(evento),
            "permission.resolved" => self.resolver_permiso(evento),
            "verification.started" => {
                self.fase = Some(FaseRun::Verifying);
                self.anotar("Verificando el cambio");
            }
            "verification.check.started" => {
                if let Some(nombre) = texto(carga, "check") {
                    self.comprobaciones
                        .push(ComprobacionVista { nombre, paso: None });
                    recortar(&mut self.comprobaciones);
                }
            }
            "verification.check.completed" => {
                if let Some(nombre) = texto(carga, "check") {
                    let paso = booleano(carga, "passed");
                    if let Some(entrada) = self
                        .comprobaciones
                        .iter_mut()
                        .rev()
                        .find(|item| item.nombre == nombre && item.paso.is_none())
                    {
                        entrada.paso = paso;
                    } else {
                        self.comprobaciones.push(ComprobacionVista { nombre, paso });
                    }
                }
            }
            "verification.completed" => {
                self.verificacion = texto(carga, "status");
            }
            "verification.failed" => {
                self.verificacion = Some("failed".to_owned());
                if let Some(motivo) = texto(carga, "reason") {
                    self.anotar(&format!("La verificación falló: {motivo}"));
                }
            }
            "file.changed" => {
                if let Some(ruta) = texto(carga, "path") {
                    if !self.ficheros_modificados.contains(&ruta) {
                        self.ficheros_modificados.push(ruta.clone());
                    }
                    self.anotar(&format!("Modificado {ruta}"));
                }
            }
            "process.started" => {
                self.anotar("Ejecutando un comando");
            }
            "process.failed" => {
                self.registrar_error(carga, None);
            }
            "recovery.action" => {
                if let Some(accion) = texto(carga, "action") {
                    if accion == "return_evidence" {
                        self.ciclos_reparacion += 1;
                        self.anotar("Reparando tras la verificación fallida");
                    }
                    if let Some(ultimo) = self.errores.last_mut() {
                        if ultimo.recuperacion.is_none() {
                            ultimo.recuperacion = Some(accion);
                        }
                    }
                }
            }
            "recovery.exhausted" => {
                self.anotar("Sin más intentos de recuperación");
            }
            "subagent.started" => self.delegado_empieza(evento),
            "subagent.continued" => self.delegado_continua(evento),
            "subagent.completed" => self.delegado_termina(evento, EstadoTarea::Completed),
            "subagent.failed" => self.delegado_termina(evento, EstadoTarea::Failed),
            "subagent.cancelled" => self.delegado_termina(evento, EstadoTarea::Cancelled),
            "task.blocked" => self.registrar_bloqueo(carga),
            // El encargo cambió y el bucle ya lo recogió. Esto no es lo mismo que
            // haberlo escrito: `POST /goal` responde `applied: false` a propósito, y
            // hasta este evento el agente seguía trabajando contra el anterior.
            "goal.revised" => self.revisar_objetivo(carga),
            // El proveedor no ofrece algo que este run necesitaba. Se enseña tal cual y
            // sin buscarle un sustituto: un respaldo que no cumple no es un respaldo, y
            // seguir como si nada convertiría una carencia declarada en un fallo raro
            // más adelante.
            "capability.missing" => self.registrar_capacidad_ausente(carga),
            "context.compacted" => {
                self.anotar("Contexto compactado para seguir cabiendo");
            }
            "session.resumed" => {
                self.anotar("Sesión reanudada desde su memoria de trabajo");
            }
            _ => {}
        }
    }

    fn cerrar_herramienta(&mut self, evento: &EventoRuntime, estado: &'static str) {
        let externalizado = booleano(&evento.payload, "externalized").unwrap_or(false);
        let nombre = texto(&evento.payload, "tool_name");
        let presentacion = presentacion_de(evento.payload.get("display"));
        if let Some(entrada) = self.herramientas.iter_mut().rev().find(|item| {
            item.estado == "en curso"
                && (evento.correlation_id.is_none() || item.correlacion == evento.correlation_id)
        }) {
            entrada.estado = estado;
            entrada.externalizado = externalizado;
            entrada.presentacion = presentacion.clone();
        } else if let Some(nombre) = nombre {
            self.herramientas.push(UsoHerramienta {
                nombre,
                estado,
                correlacion: evento.correlation_id.clone(),
                externalizado,
                presentacion: presentacion.clone(),
            });
            recortar(&mut self.herramientas);
        }
        // Una delegación terminada trae en su presentación lo que informó el delegado.
        // Es el único sitio del que sale: `subagent.completed` no lleva resumen.
        if let Some(vista) = presentacion {
            self.anotar_informe(&vista);
        }
    }

    fn registrar_permiso(&mut self, evento: &EventoRuntime) {
        let carga = &evento.payload;
        let Some(request_id) = texto(carga, "request_id") else {
            return;
        };
        let segundos = carga
            .get("seconds_remaining")
            .and_then(Value::as_f64)
            .unwrap_or(0.0);
        let vista = PermisoVista {
            request_id: request_id.clone(),
            herramienta: texto(carga, "tool_name").unwrap_or_default(),
            operacion: texto(carga, "operation").unwrap_or_default(),
            accion: texto(carga, "action").unwrap_or_default(),
            riesgo: texto(carga, "risk").unwrap_or_default(),
            nivel: texto(carga, "tier").unwrap_or_default(),
            motivo: texto(carga, "reason").unwrap_or_default(),
            efectos: lista(carga, "possible_effects"),
            recursos: lista(carga, "resources"),
            workspace: texto(carga, "workspace").unwrap_or_default(),
            argumentos: argumentos_de(carga.get("arguments")),
            solo_lectura: booleano(carga, "is_read_only").unwrap_or(false),
            destructivo: booleano(carga, "is_destructive").unwrap_or(false),
            confirmado: booleano(carga, "acknowledged").unwrap_or(false),
            segundos_restantes: segundos,
            caducado: segundos <= 0.0,
        };
        if let Some(existente) = self
            .permisos
            .iter_mut()
            .find(|item| item.request_id == request_id)
        {
            *existente = vista;
        } else {
            self.permisos.push(vista);
        }
        self.fase = Some(FaseRun::WaitingPermission);
        self.anotar("Esperando tu autorización");
    }

    fn resolver_permiso(&mut self, evento: &EventoRuntime) {
        let decision = texto(&evento.payload, "decision").unwrap_or_default();
        if let Some(identificador) = evento.correlation_id.as_deref() {
            self.permisos
                .retain(|item| item.request_id != identificador);
        } else {
            self.permisos.clear();
        }
        if self.permisos.is_empty() && self.fase == Some(FaseRun::WaitingPermission) {
            // Athena no publica un evento de "he vuelto a trabajar": la vuelta a
            // `running` la confirma la siguiente instantánea. Hasta entonces se
            // muestra como en marcha, que es lo que acaba de autorizarse.
            self.fase = Some(FaseRun::Running);
        }
        if !decision.is_empty() {
            self.anotar(&format!("Permiso resuelto: {decision}"));
        }
    }

    /// Adopta una revisión del encargo y tira la evidencia que la precedía.
    ///
    /// Athena hace lo mismo por dentro —una revisión anula `last_verification`— y la
    /// razón es la misma aquí: **la evidencia obtenida bajo una revisión no demuestra la
    /// siguiente**. Dejar en pantalla el «verificado» de ayer junto al encargo de ahora
    /// sería la forma más barata de dar por bueno un trabajo que nadie pidió.
    fn revisar_objetivo(&mut self, carga: &BTreeMap<String, Value>) {
        let Some(revision) = numero(carga, "revision") else {
            return;
        };
        if let Some(texto_nuevo) = texto(carga, "objective") {
            self.objetivo = texto_nuevo;
        }
        self.objetivo_revision = revision as u32;
        self.motivo_revision = texto(carga, "reason").filter(|valor| !valor.is_empty());
        self.verificacion = None;
        self.resumen_verificacion = None;
        self.comprobaciones.clear();
        self.evidencia.clear();
        match &self.motivo_revision {
            Some(motivo) => self.anotar(&format!(
                "El encargo cambió (revisión {revision}): {motivo}. Lo comprobado antes ya no lo respalda."
            )),
            None => self.anotar(&format!(
                "El encargo cambió (revisión {revision}). Lo comprobado antes ya no lo respalda."
            )),
        }
    }

    /// Fija la revisión leída de Athena, sin tocar nada más.
    ///
    /// Distinto de `revisar_objetivo`: aquí no hubo cambio, sólo se supo el número. Tirar
    /// la evidencia al enterarse de la revisión vigente borraría el trabajo de un run que
    /// nadie ha revisado.
    pub fn adoptar_objetivo(&mut self, texto_objetivo: &str, revision: u32, motivo: &str) {
        if !texto_objetivo.is_empty() {
            self.objetivo = texto_objetivo.to_owned();
        }
        self.objetivo_revision = revision;
        if !motivo.is_empty() {
            self.motivo_revision = Some(motivo.to_owned());
        }
    }

    /// El proveedor no ofrece algo que el run pedía.
    ///
    /// Athena distingue lo exigido de lo preferido y lo dice en el mismo evento. La
    /// diferencia importa: sin lo exigido el run no puede seguir, y sin lo preferido sólo
    /// trabaja peor. Juntarlos convertiría un aviso en una alarma, o al revés.
    fn registrar_capacidad_ausente(&mut self, carga: &BTreeMap<String, Value>) {
        let ausentes = lista(carga, "missing");
        if ausentes.is_empty() {
            return;
        }
        let exigido = booleano(carga, "required").unwrap_or(false);
        let nombres = ausentes.join(", ");
        if exigido {
            self.errores.push(ErrorVista {
                codigo: "unsupported_capability".to_owned(),
                mensaje: format!("El proveedor no ofrece: {nombres}"),
                razon: None,
                detalle: None,
                recuperacion: None,
            });
            recortar(&mut self.errores);
            self.anotar(&format!("Falta algo que este run necesitaba: {nombres}"));
        } else {
            self.anotar(&format!(
                "El proveedor no ofrece {nombres}; se sigue sin ello"
            ));
        }
    }

    /// Un delegado arranca. Se le abre ficha; todo lo suyo cuelga de aquí.
    fn delegado_empieza(&mut self, evento: &EventoRuntime) {
        let carga = &evento.payload;
        let rol = texto(carga, "role").unwrap_or_else(|| "subagente".to_owned());
        // El id del hijo viene en el payload desde ADR-030 y en la correlación desde
        // siempre. Se prefiere el payload: la correlación es del transporte, y un
        // despliegue que dejase de correlacionar dejaría al delegado sin nombre.
        let Some(sesion) = texto(carga, "session_id").or_else(|| evento.correlation_id.clone())
        else {
            return;
        };
        // El padre es la sesión desde la que se publicó. En un run jerárquico ésa es la
        // tarea, y por eso vale también como identificador de tarea: es el mismo.
        let padre = texto(carga, "parent_session_id").unwrap_or_else(|| evento.run_id.clone());
        let tarea = self
            .tareas
            .iter()
            .find(|item| item.id == padre)
            .map(|item| item.id.clone());
        let seguimientos_maximos = numero(carga, "max_follow_ups").unwrap_or(0);
        let encargo = texto(carga, "objective").unwrap_or_default();
        if let Some(existente) = self.delegados.iter_mut().find(|item| item.sesion == sesion) {
            // Una reconexión reentrega el evento. Un delegado duplicado en la vista se
            // lee como trabajo duplicado, que es lo contrario de lo que pasó.
            existente.estado = EstadoTarea::Running;
            return;
        }
        self.delegados.push(DelegadoVista {
            sesion,
            padre,
            tarea,
            rol: rol.clone(),
            proveedor: texto(carga, "provider").unwrap_or_default(),
            estado: EstadoTarea::Running,
            encargo,
            continuable: seguimientos_maximos > 0,
            seguimientos: 0,
            seguimientos_restantes: None,
            actividad: Vec::new(),
            resumen: None,
            ficheros: Vec::new(),
            llamadas_herramienta: None,
            bloqueos: Vec::new(),
            error: None,
        });
        recortar(&mut self.delegados);
        self.anotar(&format!("Delegado {rol}: en marcha"));
    }

    /// Se le vuelve a preguntar al mismo delegado. No es uno nuevo.
    fn delegado_continua(&mut self, evento: &EventoRuntime) {
        let carga = &evento.payload;
        let Some(sesion) = texto(carga, "session_id").or_else(|| evento.correlation_id.clone())
        else {
            return;
        };
        let pregunta = texto(carga, "question").unwrap_or_default();
        let seguimiento = numero(carga, "follow_up").unwrap_or(0) as u32;
        let gastadas = numero(carga, "tool_calls_spent");
        if let Some(delegado) = self.delegados.iter_mut().find(|item| item.sesion == sesion) {
            delegado.estado = EstadoTarea::Running;
            delegado.seguimientos = seguimiento;
            // El presupuesto es compartido: contarlo aparte haría creer que un
            // seguimiento sale gratis.
            if gastadas.is_some() {
                delegado.llamadas_herramienta = gastadas;
            }
            let linea = if pregunta.is_empty() {
                "Se le volvió a preguntar".to_owned()
            } else {
                format!("Se le volvió a preguntar: {pregunta}")
            };
            anotar_en(&mut delegado.actividad, &linea);
        }
    }

    /// Un delegado termina, bien o mal.
    fn delegado_termina(&mut self, evento: &EventoRuntime, estado: EstadoTarea) {
        let carga = &evento.payload;
        let Some(sesion) = texto(carga, "session_id").or_else(|| evento.correlation_id.clone())
        else {
            return;
        };
        let rol = texto(carga, "role").unwrap_or_else(|| "subagente".to_owned());
        let error = texto(carga, "error_code").map(|codigo| ErrorVista {
            codigo,
            mensaje: texto(carga, "message").unwrap_or_default(),
            razon: None,
            detalle: texto(carga, "detail"),
            recuperacion: None,
        });
        if let Some(delegado) = self.delegados.iter_mut().find(|item| item.sesion == sesion) {
            delegado.estado = estado;
            delegado.ficheros = lista(carga, "files_modified");
            if let Some(llamadas) = numero(carga, "tool_calls") {
                delegado.llamadas_herramienta = Some(llamadas);
            }
            delegado.error = error;
        }
        self.anotar(&format!("Delegado {rol}: {}", palabra_estado(estado)));
    }

    /// Atribuye a su delegado lo que hace un delegado.
    ///
    /// Los eventos del hijo viajan con **su** sesión, que es lo que llega en `run_id`.
    /// Sin esta atribución aparecerían mezclados con los del padre y quien mirase leería
    /// como propio del run el trabajo de otro agente. Devuelve cierto cuando el evento
    /// era de un delegado, para que no se cuente además como del run.
    fn atribuir_a_delegado(&mut self, evento: &EventoRuntime) -> bool {
        let Some(delegado) = self
            .delegados
            .iter_mut()
            .find(|item| item.sesion == evento.run_id)
        else {
            return false;
        };
        let carga = &evento.payload;
        match evento.name.as_str() {
            "tool.started" => {
                if let Some(nombre) = texto(carga, "tool_name") {
                    anotar_en(&mut delegado.actividad, &format!("Usando {nombre}"));
                }
            }
            "file.changed" => {
                if let Some(ruta) = texto(carga, "path") {
                    if !delegado.ficheros.contains(&ruta) {
                        delegado.ficheros.push(ruta.clone());
                    }
                    anotar_en(&mut delegado.actividad, &format!("Modificado {ruta}"));
                }
            }
            "tool.failed" | "process.failed" => {
                if let Some(codigo) = texto(carga, "error_code") {
                    anotar_en(&mut delegado.actividad, &format!("Falló: {codigo}"));
                }
            }
            _ => {}
        }
        true
    }

    /// Anota lo que informó el delegado al devolver el resultado de `delegate_task`.
    ///
    /// El resumen y las preguntas que le quedan viajan en la presentación del resultado
    /// (ADR-026), no en `subagent.completed`. Leerlos de ahí es lo que permite decir
    /// «te quedan dos preguntas» sin que la interfaz lo calcule por su cuenta.
    fn anotar_informe(&mut self, presentacion: &PresentacionVista) {
        let hecho = |nombre: &str| {
            presentacion
                .hechos
                .iter()
                .find(|item| item.nombre == nombre)
                .map(|item| item.valor.clone())
        };
        let Some(sesion) = hecho("delegate_session_id").filter(|valor| !valor.is_empty()) else {
            return;
        };
        let restantes = hecho("follow_ups_left").and_then(|valor| valor.parse::<u32>().ok());
        if let Some(delegado) = self.delegados.iter_mut().find(|item| item.sesion == sesion) {
            if !presentacion.resumen.is_empty() {
                delegado.resumen = Some(presentacion.resumen.clone());
            }
            if let Some(quedan) = restantes {
                delegado.seguimientos_restantes = Some(quedan);
                // Athena manda la cuenta, así que se cree a Athena: un delegado que ya
                // no admite preguntas deja de ofrecerse como continuable aunque su
                // perfil dijera que lo era.
                delegado.continuable = quedan > 0;
            }
            for fichero in &presentacion.elementos {
                if !delegado.ficheros.contains(fichero) {
                    delegado.ficheros.push(fichero.clone());
                }
            }
        }
    }

    /// Una tarea del plan no puede avanzar. Se anota en la tarea y en su delegado.
    fn registrar_bloqueo(&mut self, carga: &BTreeMap<String, Value>) {
        let Some(tarea) = texto(carga, "task_id") else {
            return;
        };
        let motivo = match texto(carga, "blocked_by") {
            // Bloqueada, no fallida: quien la bloquea no tiene la culpa de nada, y
            // contarlo como fallo culparía a la tarea equivocada.
            Some(culpable) => format!("Esperando a {culpable}"),
            None => "Bloqueada".to_owned(),
        };
        for delegado in self
            .delegados
            .iter_mut()
            .filter(|item| item.tarea.as_deref() == Some(tarea.as_str()))
        {
            if !delegado.bloqueos.contains(&motivo) {
                delegado.bloqueos.push(motivo.clone());
            }
        }
        self.anotar(&format!("Tarea {tarea}: {motivo}"));
    }

    fn registrar_error(&mut self, carga: &BTreeMap<String, Value>, recuperacion: Option<String>) {
        let Some(codigo) = texto(carga, "error_code").or_else(|| texto(carga, "reason")) else {
            return;
        };
        // Cuando el codigo ya es `verification_inconclusive`, `reason` dice cual
        // de los huecos fue: sin checks, dependencia que falta, entorno a medias.
        // Sin eso, quien lo lea sabe que no se comprobo y no sabe que arreglar.
        let razon = texto(carga, "reason").filter(|valor| *valor != codigo);
        self.errores.push(ErrorVista {
            codigo,
            mensaje: texto(carga, "message").unwrap_or_default(),
            razon,
            detalle: texto(carga, "detail"),
            recuperacion,
        });
        recortar(&mut self.errores);
    }

    /// Añade una explicación operativa. Frases cortas sobre lo que ocurre, que
    /// no es lo mismo que el razonamiento del modelo: eso ni llega hasta aquí.
    fn anotar(&mut self, texto: &str) {
        if self.actividad.last().map(String::as_str) == Some(texto) {
            return;
        }
        self.actividad.push(texto.to_owned());
        recortar(&mut self.actividad);
    }
}

impl PermisoVista {
    fn desde_pendiente(pendiente: &super::contracts::PermisoPendiente) -> Self {
        Self {
            request_id: pendiente.request_id.clone(),
            herramienta: pendiente.tool_name.clone(),
            operacion: pendiente.operation.clone(),
            accion: pendiente.action.clone(),
            riesgo: pendiente.risk.clone(),
            nivel: pendiente.tier.clone(),
            motivo: pendiente.reason.clone(),
            efectos: pendiente.possible_effects.clone(),
            recursos: pendiente.resources.clone(),
            workspace: pendiente.workspace.clone(),
            argumentos: argumentos_de_mapa(&pendiente.arguments),
            solo_lectura: pendiente.is_read_only,
            destructivo: pendiente.is_destructive,
            confirmado: pendiente.acknowledged,
            segundos_restantes: pendiente.seconds_remaining,
            caducado: pendiente.seconds_remaining <= 0.0,
        }
    }
}

/// Lee la presentación que Athena adjuntó al resultado de una tool.
///
/// Devuelve `None` cuando no vino ninguna, y también cuando vino vacía de contenido:
/// una presentación sin título, sin resumen, sin elementos y sin hechos no enseña nada,
/// y dibujar su cabecera haría creer que hay algo debajo.
fn presentacion_de(valor: Option<&Value>) -> Option<PresentacionVista> {
    let mostrable: ResultadoMostrable = serde_json::from_value(valor?.clone()).ok()?;
    let hechos: Vec<HechoVista> = mostrable
        .facts
        .iter()
        .map(|(nombre, valor)| HechoVista {
            nombre: nombre.clone(),
            valor: match valor {
                Value::String(cadena) => cadena.clone(),
                otro => otro.to_string(),
            },
        })
        .collect();
    if mostrable.title.is_empty()
        && mostrable.summary.is_empty()
        && mostrable.items.is_empty()
        && hechos.is_empty()
        && mostrable.reference_uri.is_none()
    {
        return None;
    }
    Some(PresentacionVista {
        clase: mostrable.kind,
        titulo: mostrable.title,
        resumen: mostrable.summary,
        elementos: mostrable.items,
        hechos,
        referencia: mostrable.reference_uri,
    })
}

/// Lee los argumentos ya saneados por Athena.
///
/// Aquí no se recorta ni se oculta nada: el saneado ocurre en el runtime, que
/// es quien tiene el valor original. Esta capa solo decide cómo enseñarlo.
fn argumentos_de(valor: Option<&Value>) -> Vec<ArgumentoVista> {
    match valor.and_then(Value::as_object) {
        Some(mapa) => mapa
            .iter()
            .map(|(nombre, valor)| argumento(nombre, valor))
            .collect(),
        None => Vec::new(),
    }
}

fn argumentos_de_mapa(mapa: &BTreeMap<String, Value>) -> Vec<ArgumentoVista> {
    mapa.iter()
        .map(|(nombre, valor)| argumento(nombre, valor))
        .collect()
}

fn argumento(nombre: &str, valor: &Value) -> ArgumentoVista {
    // Athena marca un valor resumido con `preview` y `chars`, y uno redactado
    // sustituyéndolo por el texto acordado.
    if let Some(objeto) = valor.as_object() {
        if let Some(preview) = objeto.get("preview").and_then(Value::as_str) {
            return ArgumentoVista {
                nombre: nombre.to_owned(),
                valor: preview.to_owned(),
                caracteres: objeto.get("chars").and_then(Value::as_u64),
                redactado: false,
                resumido: true,
            };
        }
    }
    let texto_valor = match valor {
        Value::String(cadena) => cadena.clone(),
        otro => otro.to_string(),
    };
    ArgumentoVista {
        nombre: nombre.to_owned(),
        redactado: texto_valor == "[REDACTED]",
        valor: texto_valor,
        caracteres: None,
        resumido: false,
    }
}

/// Estado de tarea publicado por el TaskManager, para cuando el run lo use.
///
/// Athena ya publica estados de tarea; ningún run de ChatyGPT los consume aún
/// porque el área todavía no dibuja el grafo. Se conserva la traducción para que
/// cuando lo haga no haya que reconstruirla, y se marca para que eso sea una
/// decisión visible y no un descuido.
#[allow(dead_code)]
pub fn estado_tarea_desde(valor: &str) -> Option<EstadoTarea> {
    EstadoTarea::desde_texto(valor)
}

fn palabra_estado(estado: EstadoTarea) -> &'static str {
    match estado {
        EstadoTarea::Pending => "pendiente",
        EstadoTarea::Running => "en marcha",
        EstadoTarea::Completed => "terminado",
        EstadoTarea::Failed => "fallido",
        EstadoTarea::Cancelled => "cancelado",
        EstadoTarea::Killed => "detenido",
        EstadoTarea::RecoveryPending => "por recuperar",
    }
}

/// Extrae las tareas del plan de la memoria de trabajo, si el run lleva uno.
fn tareas_de(memoria: &Value) -> Vec<TareaVista> {
    let Some(plan) = memoria.get("current_plan").and_then(Value::as_array) else {
        return Vec::new();
    };
    let paso_actual = memoria.get("current_step").and_then(Value::as_u64);
    plan.iter()
        .enumerate()
        .filter_map(|(indice, paso)| {
            let descripcion = paso.get("description").and_then(Value::as_str)?;
            // Un paso que viene de un grafo trae el identificador de su tarea. Sin él, la
            // instantánea llamaría «paso-0» a lo que los eventos llaman «T01», y al
            // reconectar la misma tarea aparecería dos veces: una del plan y otra del
            // primer `task.started` que llegase después.
            let identificador = paso
                .get("task_id")
                .and_then(Value::as_str)
                .map(str::to_owned)
                .unwrap_or_else(|| format!("paso-{indice}"));
            let estado = paso
                .get("status")
                .and_then(Value::as_str)
                .map(|valor| match valor {
                    "done" => EstadoTarea::Completed,
                    "in_progress" => EstadoTarea::Running,
                    "blocked" => EstadoTarea::Failed,
                    _ => EstadoTarea::Pending,
                })
                .unwrap_or(EstadoTarea::Pending);
            Some(TareaVista {
                id: identificador,
                nombre: descripcion.to_owned(),
                estado: if paso_actual == Some(indice as u64) && estado == EstadoTarea::Pending {
                    EstadoTarea::Running
                } else {
                    estado
                },
                iteraciones: None,
                llamadas_herramienta: None,
                detalle: None,
                // Ni el rol ni las dependencias viajan en la instantánea. Esto es lo
                // que se sabe hasta que lleguen los eventos de tarea, que sí los traen;
                // rellenarlos aquí sería dibujar un grafo supuesto.
                rol: String::new(),
                dependencias: Vec::new(),
                ficheros: Vec::new(),
            })
        })
        .collect()
}

fn texto(carga: &BTreeMap<String, Value>, clave: &str) -> Option<String> {
    carga.get(clave).and_then(Value::as_str).map(str::to_owned)
}

fn booleano(carga: &BTreeMap<String, Value>, clave: &str) -> Option<bool> {
    carga.get(clave).and_then(Value::as_bool)
}

fn numero(carga: &BTreeMap<String, Value>, clave: &str) -> Option<u64> {
    carga.get(clave).and_then(Value::as_u64)
}

fn lista(carga: &BTreeMap<String, Value>, clave: &str) -> Vec<String> {
    carga
        .get(clave)
        .and_then(Value::as_array)
        .map(|valores| {
            valores
                .iter()
                .filter_map(Value::as_str)
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Añade una línea de actividad a una lista, sin repetir la anterior.
///
/// Mismo criterio que `anotar` para el run: dos veces «Usando grep» seguidas no cuentan
/// dos hechos, cuentan uno visto dos veces.
fn anotar_en(lineas: &mut Vec<String>, linea: &str) {
    if lineas.last().map(String::as_str) == Some(linea) {
        return;
    }
    lineas.push(linea.to_owned());
    recortar(lineas);
}

fn recortar<T>(items: &mut Vec<T>) {
    if items.len() > LIMITE_HISTORIAL {
        items.drain(..items.len() - LIMITE_HISTORIAL);
    }
}
