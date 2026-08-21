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
}

/// Respuesta a la apertura de un run.
#[derive(Debug, Clone, Deserialize)]
pub struct RunCreado {
    pub run_id: String,
    pub workspace_id: String,
    pub writes: String,
    #[serde(rename = "exec")]
    pub ejecucion: String,
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
