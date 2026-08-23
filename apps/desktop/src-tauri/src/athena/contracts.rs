//! Tipos del contrato del servicio de Athena.
//!
//! Son *proyecciones*: lo que Athena publica por HTTP, no sus objetos internos.
//! Se declaran aquí, en un solo sitio, para que un cambio de contrato del
//! runtime rompa la compilación en lugar de manifestarse como un campo vacío en
//! la interfaz. La versión del formato viaja en `wire_version`; si Athena la
//! sube, esta capa es la que debe adaptarse.

//!
//! Muchos campos de este módulo no los lee ningún código de ChatyGPT: los lee
//! serde. Están porque son el contrato de Athena, y tenerlos completos hace que
//! un cambio en el otro lado aparezca como un error de deserialización en vez
//! de como un silencio. Por eso el `allow` está aquí y acotado a este fichero,
//! y no cubriendo el módulo entero.
#![allow(dead_code)]
use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// Versión del formato que este cliente sabe interpretar.
pub const WIRE_VERSION_SOPORTADA: u32 = 1;

/// Estado de un run según Athena. Es su vocabulario, no el nuestro.
///
/// `RecoveryPending` existe porque el runtime murió mientras el run estaba
/// vivo: no terminó y no falló, y presentarlo como cualquiera de las dos cosas
/// sería mentir sobre trabajo que quizá quedó a medias.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum EstadoRun {
    Idle,
    Running,
    WaitingPermission,
    Verifying,
    Completed,
    Failed,
    Cancelled,
    RecoveryPending,
    /// Un estado que este cliente no conoce todavía.
    #[serde(other)]
    Desconocido,
}

impl EstadoRun {
    /// Cierto solo cuando el run terminó de verdad.
    pub fn es_terminal(self) -> bool {
        matches!(self, Self::Completed | Self::Failed | Self::Cancelled)
    }

    /// Cierto mientras el run sigue vivo y merece la pena escuchar sus eventos.
    pub fn esta_vivo(self) -> bool {
        matches!(
            self,
            Self::Idle | Self::Running | Self::WaitingPermission | Self::Verifying
        )
    }
}

/// Qué capacidades se le conceden a un run.
///
/// El valor por defecto es `Ask` en ambas: en una aplicación de escritorio una
/// aprobación es barata de contestar y un error caro de deshacer.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ModoCapacidad {
    Off,
    #[default]
    Ask,
    Allow,
}

impl ModoCapacidad {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Off => "off",
            Self::Ask => "ask",
            Self::Allow => "allow",
        }
    }
}

/// Petición para abrir un run.
#[derive(Debug, Clone, Serialize)]
pub struct SolicitudRun {
    pub objective: String,
    pub workspace: String,
    pub writes: &'static str,
    #[serde(rename = "exec")]
    pub ejecucion: &'static str,
    pub max_iterations: u32,
    pub max_repair_cycles: u32,
    pub session_timeout_seconds: f64,
    /// Para qué se usa Athena en este run. Se omite cuando no se eligió ninguno, para
    /// que decida el despliegue: mandar una cadena vacía sería pedir un perfil sin
    /// nombre, y Athena lo rechazaría con razón.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub profile: String,
    /// Con qué modelo corre el run. Se omite cuando no se eligió, igual que el perfil:
    /// una cadena vacía sería pedir un modelo sin nombre y Athena la rechazaría.
    #[serde(skip_serializing_if = "str::is_empty")]
    pub model: String,
}

/// Respuesta a la apertura de un run.
#[derive(Debug, Clone, Deserialize)]
pub struct RunCreado {
    pub run_id: String,
    pub workspace_id: String,
    pub writes: String,
    #[serde(rename = "exec")]
    pub ejecucion: String,
    /// Con qué perfil quedó fijado el run. Athena no lo devuelve todavía; se conserva el
    /// campo porque el día que lo haga, quien lea esto sabrá que es de Athena y no una
    /// copia de lo que ChatyGPT creía haber pedido.
    #[serde(default)]
    pub profile: String,
}

/// Salud del servicio.
#[derive(Debug, Clone, Deserialize)]
pub struct SaludServicio {
    pub status: String,
    pub wire_version: u32,
    #[serde(default)]
    pub runs: u32,
}

/// Forma corta de un run, la que se usa en listados.
///
/// También se serializa: es lo que el área enseña en la lista de recuperación.
///
/// Las dos direcciones no usan el mismo estilo y eso es deliberado: se lee en
/// el `snake_case` que publica Athena y se escribe en el `camelCase` que espera
/// la interfaz. Un único `rename_all` rompería la lectura.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ResumenRun {
    pub run_id: String,
    pub workspace_id: String,
    pub status: EstadoRun,
    #[serde(default)]
    pub resumable: bool,
    #[serde(default)]
    pub degraded: bool,
    #[serde(default)]
    pub objective: String,
    #[serde(default)]
    pub files_modified: Vec<String>,
    #[serde(default)]
    pub updated_at: String,
}

/// Referencia a un resultado externalizado.
///
/// Athena entrega la referencia, no la carga: el cuerpo se pide aparte y puede
/// haber expirado, en cuyo caso el servicio responde 410 en lugar de nada.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenciaArtefacto {
    pub uri: String,
    pub store_key: String,
    #[serde(default)]
    pub media_type: String,
    #[serde(default)]
    pub size_chars: u64,
}

/// Hito registrado durante el run.
#[derive(Debug, Clone, Deserialize)]
pub struct Checkpoint {
    pub name: String,
    #[serde(default)]
    pub occurred_at: String,
    #[serde(default)]
    pub payload: BTreeMap<String, Value>,
}

/// Instantánea completa de un run: lo que el cliente recibe al conectar y al
/// reconectar. Sustituye a cualquier evento perdido mientras no había conexión.
#[derive(Debug, Clone, Deserialize)]
pub struct InstantaneaRun {
    pub run_id: String,
    pub workspace_id: String,
    pub status: EstadoRun,
    #[serde(default)]
    pub resumable: bool,
    /// Cierto cuando Athena tuvo que reconstruir estado dañado.
    #[serde(default)]
    pub degraded: bool,
    #[serde(default)]
    pub objective: String,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    #[serde(default)]
    pub working_memory: Value,
    #[serde(default)]
    pub verification: BTreeMap<String, Value>,
    #[serde(default)]
    pub tool_references: Vec<ReferenciaArtefacto>,
    #[serde(default)]
    pub checkpoints: Vec<Checkpoint>,
}

impl InstantaneaRun {
    /// Ficheros que el run dice haber modificado, según su memoria de trabajo.
    pub fn ficheros_modificados(&self) -> Vec<String> {
        self.working_memory
            .get("files_modified")
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

    /// Veredicto de verificación, si ya lo hay.
    pub fn estado_verificacion(&self) -> Option<&str> {
        self.verification.get("status").and_then(Value::as_str)
    }
}

/// De quién es un hecho del registro duradero.
///
/// `run_id` es la raíz, no la sesión que lo publicó: es lo que hace que un run con
/// delegados tenga una sola historia en vez de una por agente.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct Procedencia {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub session_id: String,
    #[serde(default)]
    pub actor: String,
    #[serde(default)]
    pub task_id: Option<String>,
    /// Si lo hizo un delegado y no el propio run.
    #[serde(default)]
    pub delegated: bool,
}

/// Un hecho del registro duradero de un run.
///
/// Distinto de `EventoRuntime`, que es lo que viaja en vivo: éste sobrevive al proceso,
/// lleva su sitio en el orden (`seq`) y dice de quién es. Sin procedencia, un run con
/// delegados se leería como si todo lo hubiera hecho el padre.
#[derive(Debug, Clone, Deserialize)]
pub struct EventoHistorico {
    pub seq: u64,
    #[serde(default)]
    pub event_id: String,
    pub name: String,
    #[serde(default)]
    pub version: u32,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub occurred_at: String,
    #[serde(default)]
    pub provenance: Procedencia,
    #[serde(default)]
    pub payload: BTreeMap<String, Value>,
}

impl EventoHistorico {
    /// El mismo hecho, en la forma que entiende la proyección.
    ///
    /// `run_id` toma la **sesión que lo publicó** y no la raíz a propósito: es lo que
    /// permite que la misma atribución que funciona en vivo —lo que hace un delegado es
    /// del delegado— funcione al releer. Poner la raíz haría que todo pareciera del
    /// padre, que es justo lo que la procedencia existe para evitar.
    pub fn como_evento(&self) -> EventoRuntime {
        EventoRuntime {
            event_id: self.event_id.clone(),
            name: self.name.clone(),
            run_id: if self.provenance.session_id.is_empty() {
                self.provenance.run_id.clone()
            } else {
                self.provenance.session_id.clone()
            },
            correlation_id: self.correlation_id.clone(),
            occurred_at: self.occurred_at.clone(),
            payload: self.payload.clone(),
        }
    }
}

/// Lo esencial de un run reconstruido por Athena a partir de sus hechos.
///
/// Viaja con los hechos porque se deriva de ellos: calcularlo en el cliente obligaría a
/// cada cliente a repetir la misma lectura y a ponerse de acuerdo en cómo se lee.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ResumenHistoria {
    #[serde(default)]
    pub status: String,
    /// `direct` o `hierarchical`.
    #[serde(default)]
    pub executed_as: String,
    /// Estado final de cada tarea del plan, por id.
    #[serde(default)]
    pub tasks: BTreeMap<String, String>,
    /// Rol de cada delegado, por sesión.
    #[serde(default)]
    pub delegates: BTreeMap<String, String>,
    #[serde(default)]
    pub verification: String,
    #[serde(default)]
    pub permission_requests: u32,
}

/// La historia de un run: los hechos y lo que Athena concluye de ellos.
#[derive(Debug, Clone, Deserialize)]
pub struct HistoriaRun {
    #[serde(default)]
    pub run_id: String,
    #[serde(default)]
    pub events: Vec<EventoHistorico>,
    #[serde(default)]
    pub summary: ResumenHistoria,
}

/// Algo que Athena cree saber de un proyecto.
///
/// Tres estados y no dos, y el orden importa (ADR-031): `proposed` es lo que dijo un
/// modelo, `verified` lo que algo comprobó, `user_confirmed` lo que una persona
/// respaldó. El último **sólo** se alcanza por HTTP: ningún módulo del runtime puede
/// llegar a él, y por eso existe este panel.
///
/// `status` es cosa aparte: dice si el recuerdo sigue vigente, si otro lo reemplazó o
/// si alguien lo retiró. Un recuerdo superado no se borra, para que «antes creíamos X»
/// sobreviva.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct RecuerdoProyecto {
    pub id: String,
    #[serde(default)]
    pub project_id: String,
    /// Qué clase de cosa se recuerda: comando verificado, convención, decisión…
    pub kind: String,
    pub content: String,
    /// De dónde salió. Un recuerdo sin origen no se puede juzgar.
    #[serde(default)]
    pub source: String,
    #[serde(default)]
    pub source_reference: Option<String>,
    #[serde(default)]
    pub confidence: f64,
    /// `proposed`, `verified` o `user_confirmed`.
    pub verification_state: String,
    #[serde(default)]
    pub scope: String,
    /// `active`, `superseded` o `forgotten`.
    pub status: String,
    /// El recuerdo al que éste sustituye, si sustituye a alguno.
    #[serde(default)]
    pub supersedes: Option<String>,
    #[serde(default)]
    pub created_at: String,
    #[serde(default)]
    pub updated_at: String,
    /// Si ya pasó el plazo de su tipo. Lo calcula Athena: una fecha ISO obligaría a cada
    /// cliente a decidir por su cuenta cuándo algo es viejo, y a discrepar entre sí.
    #[serde(default)]
    pub stale: bool,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListadoMemoria {
    #[serde(default)]
    pub project_id: String,
    #[serde(default)]
    pub items: Vec<RecuerdoProyecto>,
}

/// Un perfil de Athena: para qué clase de trabajo sirve este run.
///
/// No es el perfil de un subagente. `AthenaProfile` dice qué clase de trabajo es el run
/// entero —qué herramientas existen y qué cuenta como prueba—; `SubagentProfile` reparte
/// autoridad dentro. Se componen, no se mezclan (ADR-028).
///
/// Athena **no publica versión** de un perfil, así que aquí no hay ninguna: enseñar un
/// número que nadie mantiene invitaría a confiar en que subiría al cambiar el perfil.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct PerfilAthena {
    pub name: String,
    /// Sobre qué trabaja: un repositorio, una carpeta de documentos…
    #[serde(default)]
    pub subject: String,
    /// Qué clase de evidencia da por buena.
    #[serde(default)]
    pub evidence: String,
    /// Qué demuestra esa evidencia — incluido lo que **no** demuestra.
    #[serde(default)]
    pub proves: String,
    /// Las herramientas que existen bajo este perfil. Es un filtro estructural: lo que
    /// no está aquí no es que se deniegue, es que no existe.
    #[serde(default)]
    pub tools: Vec<String>,
    #[serde(default)]
    pub description: String,
}

/// Un modelo que este despliegue admite para un run.
///
/// Sin adjetivos ni descripciones: Athena publica el nombre y nada más, y rellenar aquí
/// un «rápido» o un «el mejor para código» sería inventar una recomendación que nadie
/// mantiene. Lo que sí es un hecho es cuál corre si no se elige, y eso viaja en `default`.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ModeloAthena {
    pub name: String,
    /// Si es el que se usa cuando no se pide ninguno.
    #[serde(default)]
    pub default: bool,
}

/// Los modelos que ofrece este despliegue, y cuál usa si no se pide ninguno.
///
/// Vacío significa que este Athena no ofrece elección —contesta 404 a `/v1/models`— y no
/// que no tenga modelos. La interfaz no enseña selector en ese caso, que es distinto de
/// enseñar uno vacío.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ListadoModelos {
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub models: Vec<ModeloAthena>,
}

/// Los perfiles que ofrece este despliegue, y cuál usa si no se pide ninguno.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ListadoPerfiles {
    #[serde(default)]
    pub default: String,
    #[serde(default)]
    pub profiles: Vec<PerfilAthena>,
}

/// El encargo de un run, en su versión número `revision`.
///
/// La revisión no es burocracia: es lo único que impide que dos personas mirando el
/// mismo run se pisen sin enterarse (ADR-029). ChatyGPT la conserva porque tiene que
/// decir sobre cuál escribe.
///
/// Se lee en el `snake_case` que publica Athena y se escribe en el `camelCase` que
/// espera la interfaz, igual que `ResumenRun`: un único `rename_all` rompería la lectura.
#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(rename_all(serialize = "camelCase", deserialize = "snake_case"))]
pub struct ObjetivoRun {
    pub text: String,
    pub revision: u32,
    /// Por qué se cambió, dicho por quien lo cambió. Vacío en la primera.
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub revised_at: String,
}

/// Petición de revisión del encargo.
///
/// `base_revision` no tiene valor por defecto ni aquí ni en Athena. Uno implícito
/// —«la última»— convertiría cada revisión en un pisotón.
#[derive(Debug, Clone, Serialize)]
pub struct SolicitudRevision {
    pub objective: String,
    pub base_revision: u32,
    pub reason: String,
}

/// Respuesta a una revisión aceptada.
///
/// `applied` llega en falso a propósito: escrito no es aplicado. El bucle recoge el
/// cambio entre iteraciones y lo anuncia con `goal.revised`.
#[derive(Debug, Clone, Deserialize)]
pub struct RevisionAceptada {
    pub run_id: String,
    pub goal: ObjetivoRun,
    #[serde(default)]
    pub applied: bool,
}

/// Cuerpo del 409 cuando alguien escribió sobre una revisión que ya no era la vigente.
///
/// Athena manda el objetivo actual dentro del propio rechazo para que quien llegó tarde
/// decida con él delante, en vez de tener que volver a preguntarlo.
#[derive(Debug, Clone, Deserialize)]
pub struct ConflictoObjetivo {
    pub current_revision: u32,
    #[serde(default)]
    pub current: String,
}

/// Cómo se enseña el resultado de una herramienta, ya derivado por Athena.
///
/// Viaja en `tool.completed` desde ADR-026 y existe para que ningún cliente vuelva a
/// deducir la presentación leyendo el payload interno de la tool: dos clientes deduciendo
/// por su cuenta acaban discrepando, y el que discrepa no sabe que discrepa.
///
/// `kind` es un conjunto cerrado de cinco valores —`text`, `items`, `change`, `record`,
/// `reference`— y son las formas que cambian cómo se dibuja algo, no un tipo por tool.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct ResultadoMostrable {
    #[serde(default)]
    pub kind: String,
    #[serde(default)]
    pub title: String,
    #[serde(default)]
    pub summary: String,
    #[serde(default)]
    pub items: Vec<String>,
    #[serde(default)]
    pub facts: BTreeMap<String, Value>,
    /// Dónde vive el cuerpo cuando no cupo en el evento.
    #[serde(default)]
    pub reference_uri: Option<String>,
}

/// Un evento del runtime.
#[derive(Debug, Clone, Deserialize)]
pub struct EventoRuntime {
    pub event_id: String,
    pub name: String,
    pub run_id: String,
    #[serde(default)]
    pub correlation_id: Option<String>,
    #[serde(default)]
    pub occurred_at: String,
    #[serde(default)]
    pub payload: BTreeMap<String, Value>,
}

impl EventoRuntime {
    /// Cierto para los eventos que cierran un run.
    pub fn es_final(&self) -> bool {
        matches!(
            self.name.as_str(),
            "agent.completed" | "agent.failed" | "agent.cancelled"
        )
    }

    /// Cierto cuando el evento pide una decisión humana.
    pub fn pide_permiso(&self) -> bool {
        self.name == "permission.requested"
            && self
                .payload
                .get("awaiting_decision")
                .and_then(Value::as_bool)
                .unwrap_or(false)
    }

    pub fn identificador_peticion(&self) -> Option<&str> {
        self.payload.get("request_id").and_then(Value::as_str)
    }
}

/// Petición de permiso pendiente de respuesta humana.
#[derive(Debug, Clone, Deserialize)]
pub struct PermisoPendiente {
    pub request_id: String,
    pub run_id: String,
    pub tool_name: String,
    #[serde(default)]
    pub operation: String,
    #[serde(default)]
    pub action: String,
    #[serde(default)]
    pub risk: String,
    #[serde(default)]
    pub tier: String,
    #[serde(default)]
    pub reason: String,
    #[serde(default)]
    pub possible_effects: Vec<String>,
    #[serde(default)]
    pub resources: Vec<String>,
    #[serde(default)]
    pub is_read_only: bool,
    #[serde(default)]
    pub is_destructive: bool,
    #[serde(default)]
    pub workspace: String,
    #[serde(default)]
    pub acknowledged: bool,
    /// Segundos que quedan antes de que el silencio se interprete como negativa.
    #[serde(default)]
    pub seconds_remaining: f64,
    /// Argumentos ya saneados por Athena: los valores largos llegan resumidos y
    /// los que parecen secretos, redactados. Nunca la carga completa.
    #[serde(default)]
    pub arguments: BTreeMap<String, Value>,
}

/// Primer marco del flujo SSE: identidad del suscriptor y estado actual.
#[derive(Debug, Clone, Deserialize)]
pub struct MarcoEstado {
    pub subscriber_id: String,
    #[serde(default)]
    pub controls: bool,
    #[serde(default)]
    pub wire_version: u32,
    /// Con qué estrategia decidió Athena ejecutar el run, y por qué.
    ///
    /// Viaja en el estado y no sólo en `plan.decided` porque la decisión se toma antes
    /// de que nadie pueda suscribirse: un cliente que sólo escuchase el flujo no la
    /// vería nunca, y es justo lo que se quiere saber al preguntar por qué un objetivo
    /// no se planificó.
    #[serde(default)]
    pub shape: Option<EstrategiaEjecucion>,
    #[serde(default)]
    pub snapshot: Option<InstantaneaRun>,
    #[serde(default)]
    pub pending_approvals: Vec<PermisoPendiente>,
}

/// Cómo se decidió ejecutar un run.
///
/// Los códigos —`reason_code`, `execution_mode`, `executed_as`— son estables y los
/// escribe Athena; las frases están para leerse y se reescribirán. Aquí no se traduce
/// ninguna decisión: se enseña la que ya vino tomada.
#[derive(Debug, Clone, Default, Deserialize)]
pub struct EstrategiaEjecucion {
    /// Lo que pidió el cliente: `auto`, `hierarchical` o `direct`.
    #[serde(default)]
    pub execution_mode: String,
    /// Lo que se hizo: `direct` o `hierarchical`.
    #[serde(default)]
    pub executed_as: String,
    /// Código estable del motivo, para no depender de la redacción.
    #[serde(default)]
    pub reason_code: String,
    /// El motivo efectivo, en una frase.
    #[serde(default)]
    pub reason: String,
    /// Lo que opinó la política: `decompose` o `decline`.
    ///
    /// Puede no coincidir con lo que se hizo, y ése es el caso interesante: un objetivo
    /// que la política considera divisible puede acabar en el bucle porque el despliegue
    /// no tiene planificación. Enseñar sólo uno de los dos lo contaría mal.
    #[serde(default)]
    pub policy_verdict: String,
    #[serde(default)]
    pub policy_explanation: String,
    /// Criterios de descomposición que este objetivo cumple.
    #[serde(default)]
    pub criteria_met: Vec<String>,
    /// Señales que Athena no pudo medir y dejó en su valor neutro.
    #[serde(default)]
    pub assumed_signals: Vec<String>,
}

/// Lo que llega por el flujo de eventos, ya distinguido.
#[derive(Debug, Clone)]
pub enum MensajeFlujo {
    /// Instantánea inicial o de reconexión.
    Estado(Box<MarcoEstado>),
    /// Evento del runtime.
    Evento(Box<EventoRuntime>),
}

/// Decisión humana sobre un permiso. No existe un tercer valor: Athena admite
/// conceder o negar, nunca "conceder siempre".
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DecisionPermiso {
    Permitir,
    Denegar,
}

impl DecisionPermiso {
    pub fn como_texto(self) -> &'static str {
        match self {
            Self::Permitir => "allow",
            Self::Denegar => "deny",
        }
    }
}

/// Error estructurado del servicio.
#[derive(Debug, Clone, Deserialize)]
pub struct CuerpoError {
    pub error: DetalleError,
}

#[derive(Debug, Clone, Deserialize)]
pub struct DetalleError {
    #[serde(default)]
    pub code: String,
    #[serde(default)]
    pub message: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct ListadoRuns {
    #[serde(default)]
    pub runs: Vec<ResumenRun>,
}
