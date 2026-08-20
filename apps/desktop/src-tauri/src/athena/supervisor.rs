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

use super::contracts::{EstadoRun, EventoRuntime, InstantaneaRun, MarcoEstado, MensajeFlujo};

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
            Self::Cancelled => "cancelled",
            Self::RecoveryPending => "recovery_pending",
        }
    }

    pub fn es_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
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
    pub fase: Option<FaseRun>,
    pub carpeta: String,
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
        self.tareas = tareas_de(&instantanea.working_memory);
    }

    fn aplicar_evento(&mut self, evento: &EventoRuntime) {
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
                self.fase = Some(FaseRun::Failed);
                self.permisos.clear();
                self.registrar_error(carga, None);
                self.anotar("El agente ha fallado");
            }
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
            "subagent.started" => self.registrar_subagente(evento, EstadoTarea::Running),
            "subagent.completed" => self.registrar_subagente(evento, EstadoTarea::Completed),
            "subagent.failed" => self.registrar_subagente(evento, EstadoTarea::Failed),
            "subagent.cancelled" => self.registrar_subagente(evento, EstadoTarea::Cancelled),
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
        if let Some(entrada) = self.herramientas.iter_mut().rev().find(|item| {
            item.estado == "en curso"
                && (evento.correlation_id.is_none() || item.correlacion == evento.correlation_id)
        }) {
            entrada.estado = estado;
            entrada.externalizado = externalizado;
        } else if let Some(nombre) = nombre {
            self.herramientas.push(UsoHerramienta {
                nombre,
                estado,
                correlacion: evento.correlation_id.clone(),
                externalizado,
            });
            recortar(&mut self.herramientas);
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

    fn registrar_subagente(&mut self, evento: &EventoRuntime, estado: EstadoTarea) {
        let carga = &evento.payload;
        let rol = texto(carga, "role").unwrap_or_else(|| "subagente".to_owned());
        let id = evento.correlation_id.clone().unwrap_or_else(|| rol.clone());
        let detalle = texto(carga, "message").or_else(|| texto(carga, "objective"));
        if let Some(existente) = self.tareas.iter_mut().find(|item| item.id == id) {
            existente.estado = estado;
            if detalle.is_some() {
                existente.detalle = detalle;
            }
        } else {
            self.tareas.push(TareaVista {
                id,
                nombre: rol.clone(),
                estado,
                iteraciones: numero(carga, "max_iterations"),
                llamadas_herramienta: numero(carga, "tool_calls"),
                detalle,
            });
            recortar(&mut self.tareas);
        }
        self.anotar(&format!("Subagente {rol}: {}", palabra_estado(estado)));
    }

    fn registrar_error(&mut self, carga: &BTreeMap<String, Value>, recuperacion: Option<String>) {
        let Some(codigo) = texto(carga, "error_code").or_else(|| texto(carga, "reason")) else {
            return;
        };
        self.errores.push(ErrorVista {
            codigo,
            mensaje: texto(carga, "message").unwrap_or_default(),
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
                id: format!("paso-{indice}"),
                nombre: descripcion.to_owned(),
                estado: if paso_actual == Some(indice as u64) && estado == EstadoTarea::Pending {
                    EstadoTarea::Running
                } else {
                    estado
                },
                iteraciones: None,
                llamadas_herramienta: None,
                detalle: None,
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

fn recortar<T>(items: &mut Vec<T>) {
    if items.len() > LIMITE_HISTORIAL {
        items.drain(..items.len() - LIMITE_HISTORIAL);
    }
}
