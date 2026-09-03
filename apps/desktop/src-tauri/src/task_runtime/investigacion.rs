//! Investigacion profunda: que herramientas se validan y como se congela el plan.
//!
//! El plan se decide contra las capacidades reales del Broker y se guarda:
//! la segunda etapa y una recuperacion aplican el mismo, sin renegociar.

use super::*;

/// Decisión de Investigación profunda tomada al enviar el turno.
///
/// Se congela deliberadamente: cuando la investigación viaja dentro de un flujo
/// semántico, entre la validación y el envío real media una tarea de embeddings
/// y, posiblemente, un reinicio de la aplicación. Reconsultar las capacidades en
/// ese punto permitiría que un Broker con otras herramientas cambiara una
/// investigación ya autorizada por la persona.
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct ResearchPlan {
    /// Habilidades que ejecuta el Broker, validadas contra lo que anunciaba.
    pub skills: Vec<String>,
    /// Herramientas que ejecuta ChatyGPT y que el Broker pausa para pedirle.
    #[serde(default)]
    pub client_tools: Vec<String>,
    /// Habilidades que sacan datos del equipo según las capacidades 2.8.
    #[serde(default)]
    pub egress_skills: Vec<String>,
    /// Vueltas máximas del bucle del agente, acotadas al tope del contrato.
    #[serde(default = "default_research_iterations")]
    pub max_iterations: u32,
}

pub(super) fn default_research_iterations() -> u32 {
    RESEARCH_ITERATIONS
}

/// Vueltas que se piden por investigación.
pub(super) const RESEARCH_ITERATIONS: u32 = 12;

/// Tope del contrato del Broker. El bucle **entero** cuenta contra él: las
/// pausas para pedir una herramienta no lo reinician, así que este número es la
/// profundidad total de una investigación, no la de un tramo.
pub(super) const MAX_RESEARCH_ITERATIONS: u32 = 20;

/// Herramientas que ChatyGPT ejecuta por su cuenta durante una investigación.
///
/// La lista es cerrada a propósito: cada nombre aquí es código que corre en el
/// equipo de la persona a petición de un modelo, así que ampliarla es una
/// decisión, no una configuración.
pub(super) const RESEARCH_CLIENT_TOOLS: [&str; 1] = ["fetch_url"];

/// Habilidades que se delegan al Broker si las anuncia.
///
/// `web_search` se queda en el Broker porque ChatyGPT no tiene motor de
/// búsqueda: implementarlo exigiría un proveedor externo, una credencial y
/// sacar tráfico del equipo hacia un tercero. `fetch_url`, en cambio, se
/// ejecuta aquí para que cada fuente abierta sea una subtarea visible con su
/// URL, que es donde está la cita.
///
/// **Coste asumido a sabiendas:** las búsquedas que ejecuta el Broker no pausan
/// la tarea ni aparecen en `pending_tool_calls`, así que ChatyGPT no llega a
/// saber qué se buscó. Los pasos registrados dirán «abrí esta URL» sin el
/// «busqué esto» que la produjo. Mover `web_search` a herramienta de cliente no
/// costaría iteraciones —cada llamada consume una vuelta la ejecute quien la
/// ejecute, solo añade un viaje HTTP—, pero exige antes decidir el proveedor de
/// búsqueda. Mientras esa decisión no se tome, la mitad del recorrido es una
/// caja negra y conviene que el registro no aparente lo contrario.
pub(super) const RESEARCH_BROKER_SKILLS: [&str; 3] =
    ["web_search", "calculator", "current_datetime"];

/// Definición de `fetch_url` tal y como la ve el modelo.
pub(super) fn fetch_url_tool_definition() -> serde_json::Value {
    json!({
        "name": "fetch_url",
        "description": "Descarga una página web y devuelve su texto para poder citarla. \
                        Úsala con enlaces concretos obtenidos de una búsqueda previa.",
        "parameters": {
            "type": "object",
            "properties": {
                "url": {
                    "type": "string",
                    "description": "URL http o https completa de la página que se quiere leer."
                }
            },
            "required": ["url"],
            "additionalProperties": false
        }
    })
}

/// Valida las capacidades y decide el plan. Falla antes de persistir nada.
pub(super) fn deep_research_plan(
    capabilities: &BrokerCapabilities,
) -> Result<ResearchPlan, AppError> {
    if !capabilities
        .strategies
        .iter()
        .any(|strategy| strategy == "agent")
    {
        return Err(AppError::Conflict(
            "Broker AI no anuncia la estrategia agent necesaria para Investigación profunda"
                .to_owned(),
        ));
    }
    if capabilities.client_tool_passthrough == Some(false) {
        return Err(AppError::Conflict(
            "Broker AI no admite herramientas de cliente, necesarias para ver cada fuente abierta"
                .to_owned(),
        ));
    }
    // Buscar sigue siendo del Broker: sin esa habilidad la investigación se
    // quedaría en abrir enlaces que el modelo recuerde, que es justo lo que el
    // prompt prohíbe.
    if !capabilities
        .agent_skills
        .iter()
        .any(|skill| skill == "web_search")
    {
        return Err(AppError::Conflict(
            "Broker AI no anuncia la habilidad web_search necesaria para Investigación profunda"
                .to_owned(),
        ));
    }
    Ok(ResearchPlan {
        skills: RESEARCH_BROKER_SKILLS
            .into_iter()
            .filter(|candidate| {
                capabilities
                    .agent_skills
                    .iter()
                    .any(|skill| skill == candidate)
            })
            .map(str::to_owned)
            .collect(),
        client_tools: RESEARCH_CLIENT_TOOLS.map(str::to_owned).to_vec(),
        egress_skills: capabilities.agent_skills_egress.clone(),
        max_iterations: RESEARCH_ITERATIONS.min(MAX_RESEARCH_ITERATIONS),
    })
}

/// Plan conservador cuando el endpoint de capacidades no puede leerse.
///
/// El contrato 2.7 exige no convertir ese fallo en «capacidad ausente». La
/// petición se envía con las herramientas estándar y será el 409/422 del
/// Broker quien decida si su configuración concreta no puede ejecutarla.
pub(super) fn unverified_deep_research_plan() -> ResearchPlan {
    ResearchPlan {
        skills: RESEARCH_BROKER_SKILLS.map(str::to_owned).to_vec(),
        client_tools: RESEARCH_CLIENT_TOOLS.map(str::to_owned).to_vec(),
        egress_skills: ["web_search", "fetch_url"].map(str::to_owned).to_vec(),
        max_iterations: RESEARCH_ITERATIONS.min(MAX_RESEARCH_ITERATIONS),
    }
}

/// Convierte una petición de chat en una de investigación aplicando el plan.
///
/// Es una función pura: no consulta al Broker, por lo que puede ejecutarse en la
/// segunda etapa de un flujo semántico o durante una recuperación sin red.
pub(super) fn apply_deep_research_plan(
    mut request: serde_json::Value,
    plan: &ResearchPlan,
) -> Result<serde_json::Value, AppError> {
    let research_skills = &plan.skills;
    let data_classification = request
        .pointer("/risk/data_classification")
        .and_then(serde_json::Value::as_str)
        .unwrap_or("internal");
    if matches!(data_classification, "confidential" | "local_only") {
        let mut blocked = research_skills
            .iter()
            .filter(|skill| plan.egress_skills.contains(skill))
            .cloned()
            .collect::<Vec<_>>();
        blocked.extend(
            plan.client_tools
                .iter()
                .filter(|tool| plan.egress_skills.contains(tool))
                .cloned(),
        );
        blocked.sort();
        blocked.dedup();
        if !blocked.is_empty() {
            return Err(AppError::Conflict(format!(
                "Investigación profunda necesita herramientas que envían datos a Internet ({}) y no puede usarlas con la clasificación {}. Cambia el chat a Uso personal o desactiva Investigación profunda",
                blocked.join(", "),
                if data_classification == "local_only" {
                    "Solo en este equipo"
                } else {
                    "Confidencial"
                }
            )));
        }
    }
    // El contrato prohíbe que una herramienta de cliente se llame igual que una
    // habilidad activa en la misma tarea: dos definiciones del mismo nombre son
    // ambiguas para el modelo. Se comprueba aquí porque el plan viene
    // persistido y podría haberse escrito con otra versión del código.
    if let Some(collision) = plan
        .client_tools
        .iter()
        .find(|tool| research_skills.contains(tool))
    {
        return Err(AppError::BrokerContract(format!(
            "la herramienta de cliente {collision} colisiona con una habilidad del Broker"
        )));
    }
    let client_tools = plan
        .client_tools
        .iter()
        .map(|tool| match tool.as_str() {
            "fetch_url" => Ok(fetch_url_tool_definition()),
            other => Err(AppError::BrokerContract(format!(
                "el plan declara una herramienta de cliente desconocida: {other}"
            ))),
        })
        .collect::<Result<Vec<_>, AppError>>()?;
    let original_prompt = request
        .pointer("/content/prompt")
        .and_then(serde_json::Value::as_str)
        .ok_or_else(|| {
            AppError::BrokerContract(
                "la petición de investigación no contiene un prompt válido".to_owned(),
            )
        })?
        .to_owned();
    request["content"]["prompt"] = json!(format!(
        "Ejecuta una investigación profunda y trazable. No la trates como una sola búsqueda. \
         Primero define un plan breve; después realiza varias búsquedas, abre y contrasta \
         fuentes independientes; por último redacta un informe en Markdown que diferencie \
         hechos, discrepancias e incertidumbres. Cada afirmación relevante debe quedar \
         respaldada por una cita o enlace recuperado durante el workflow. No inventes fuentes.\n\n\
         Objetivo de investigación:\n{original_prompt}"
    ));
    request["content"]["metadata"]["workflow_kind"] = json!("deep_research");
    request["content"]["metadata"]["workflow_version"] = json!("research-agent-v1");
    request["execution"] = json!({
        "strategy": "agent",
        "preset": "fast",
        "timeout_seconds": 1800,
        "long_context": "fail",
        "agent": {
            "skills": research_skills,
            "max_iterations": plan.max_iterations.min(MAX_RESEARCH_ITERATIONS),
            "client_tools": client_tools
        }
    });
    request["generation"]["max_output_tokens"] = json!(8000);
    // La estrategia `agent` rechaza el formato JSON con 422. El campo del
    // contrato es `output.format` —en `generation` solo van `temperature` y
    // `max_output_tokens`—, así que se fija donde de verdad está: saneando
    // `generation` el 422 llegaría igual y el saneado no haría nada.
    request["output"]["format"] = json!("markdown");
    Ok(request)
}
