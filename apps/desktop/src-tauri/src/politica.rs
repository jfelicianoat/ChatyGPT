//! Política de ejecución efectiva: qué privacidad, qué coste y a qué destino.
//!
//! Antes de la auditoría del 28-sep-2026 el chat enseñaba sus propias opciones
//! mientras la petición real las sustituía por el perfil del GPT: un chat en
//! «confidencial» con un GPT público salía como público. Aquí vive la única
//! regla de precedencia; la usan la petición real, la vista previa del chat y
//! los nodos de los flujos, de modo que lo visible y lo enviado no puedan
//! divergir.
//!
//! La regla es de restricción: una elección explícita de privacidad o de
//! límite de gasto nunca se amplía por heredar otra configuración.

use serde::Serialize;
use url::{Host, Url};

use crate::db::{ConversationExecutionPreferences, CustomGptContext};

/// Orden de restricción de las clasificaciones del contrato del Broker.
pub fn classification_rank(value: &str) -> u8 {
    match value {
        "public" => 0,
        "internal" => 1,
        "confidential" => 2,
        "local_only" => 3,
        // Un valor desconocido no puede relajar nada: se trata como el más
        // estricto para que un error de datos nunca abra la frontera.
        _ => 3,
    }
}

/// La más restrictiva de dos clasificaciones.
pub fn stricter_classification<'a>(left: &'a str, right: &'a str) -> &'a str {
    if classification_rank(right) > classification_rank(left) {
        right
    } else {
        left
    }
}

/// De dónde sale cada valor de la política efectiva.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PolicySources {
    /// `chat`, `gpt`, `context` (historial ya protegido) o `memory`.
    pub data_classification: String,
    /// `chat` o `gpt`.
    pub max_cost_usd: String,
    /// `chat` o `gpt`: forma de responder, preset, prioridad y contexto largo.
    pub routing: String,
}

/// Combina las preferencias del chat con el perfil congelado del GPT.
///
/// - Privacidad: la más estricta de las dos.
/// - Coste máximo: el menor de los dos.
/// - Enrutado (estrategia, preset, prioridad, contexto largo): el del GPT si
///   lo fija, porque es lo que hace reproducible su comportamiento.
pub fn merge_execution_preferences(
    chat: &ConversationExecutionPreferences,
    gpt_profile: Option<&ConversationExecutionPreferences>,
) -> (ConversationExecutionPreferences, PolicySources) {
    let Some(profile) = gpt_profile else {
        return (
            chat.clone(),
            PolicySources {
                data_classification: "chat".to_owned(),
                max_cost_usd: "chat".to_owned(),
                routing: "chat".to_owned(),
            },
        );
    };
    let data_classification =
        stricter_classification(&chat.data_classification, &profile.data_classification).to_owned();
    let classification_source = if classification_rank(&profile.data_classification)
        > classification_rank(&chat.data_classification)
    {
        "gpt"
    } else {
        "chat"
    };
    let (max_cost_usd, cost_source) = if profile.max_cost_usd < chat.max_cost_usd {
        (profile.max_cost_usd, "gpt")
    } else {
        (chat.max_cost_usd, "chat")
    };
    (
        ConversationExecutionPreferences {
            data_classification,
            strategy: profile.strategy.clone(),
            preset: profile.preset.clone(),
            max_cost_usd,
            long_context: profile.long_context.clone(),
            priority: profile.priority,
        },
        PolicySources {
            data_classification: classification_source.to_owned(),
            max_cost_usd: cost_source.to_owned(),
            routing: "gpt".to_owned(),
        },
    )
}

/// Adónde viajan de verdad los prompts y resultados.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrokerDestination {
    /// Servidor y puerto, sin credenciales ni rutas.
    pub host: String,
    /// El Broker corre en este mismo equipo.
    pub local_machine: bool,
    /// La conexión va cifrada con TLS y certificado validado.
    pub encrypted: bool,
    /// Advertencia legible cuando el transporte no protege el contenido.
    pub warning: Option<String>,
}

/// Describe el destino a partir de la URL configurada del Broker.
///
/// «Solo modelos locales» es una garantía del Broker sobre qué modelos
/// atienden la tarea, no sobre en qué equipo corren: si el Broker está en otra
/// máquina de la red, el contenido viaja hasta ella, y sin HTTPS viaja en claro.
pub fn broker_destination(base_url: &str) -> BrokerDestination {
    let Ok(url) = Url::parse(base_url) else {
        return BrokerDestination {
            host: base_url.to_owned(),
            local_machine: false,
            encrypted: false,
            warning: Some("La dirección configurada de Broker AI no es válida.".to_owned()),
        };
    };
    let local_machine = match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(address)) => address.is_loopback(),
        Some(Host::Ipv6(address)) => address.is_loopback(),
        None => false,
    };
    let encrypted = url.scheme() == "https";
    let host = match (url.host_str(), url.port_or_known_default()) {
        (Some(host), Some(port)) => format!("{host}:{port}"),
        (Some(host), None) => host.to_owned(),
        _ => base_url.to_owned(),
    };
    let warning = (!local_machine && !encrypted).then(|| {
        format!(
            "La conexión con Broker AI ({host}) no está cifrada: los mensajes y la \
             credencial viajan en claro por la red local. Configura HTTPS en el Broker \
             para protegerlos."
        )
    });
    BrokerDestination {
        host,
        local_machine,
        encrypted,
        warning,
    }
}

/// Política que se aplicará al próximo mensaje de una conversación.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct EffectiveExecutionPolicy {
    pub data_classification: String,
    pub max_cost_usd: f64,
    pub strategy: String,
    pub preset: String,
    pub priority: u16,
    pub long_context: String,
    pub sources: PolicySources,
    /// Nombre del GPT cuyo perfil interviene, si lo hay.
    pub custom_gpt_name: Option<String>,
    pub destination: BrokerDestination,
    /// Explicaciones de cada ajuste que no coincide con lo elegido en el chat.
    pub notes: Vec<String>,
}

fn classification_label(value: &str) -> &'static str {
    match value {
        "public" => "Contenido público",
        "internal" => "Uso personal",
        "confidential" => "Confidencial",
        _ => "Solo modelos locales",
    }
}

/// Calcula la política efectiva con los mismos datos que usará la petición.
pub fn effective_policy(
    chat: &ConversationExecutionPreferences,
    custom_gpt: Option<&CustomGptContext>,
    inherited_classification: Option<&str>,
    sensitive_memory_selected: bool,
    broker_base_url: &str,
) -> EffectiveExecutionPolicy {
    let (merged, mut sources) = merge_execution_preferences(
        chat,
        custom_gpt.and_then(|gpt| gpt.execution_profile.as_ref()),
    );
    let mut data_classification = merged.data_classification.clone();
    let mut notes = Vec::new();
    let gpt_name = custom_gpt.map(|gpt| gpt.name.clone());
    if sources.data_classification == "gpt" {
        notes.push(format!(
            "El GPT «{}» exige «{}», más restrictivo que la opción del chat.",
            gpt_name.as_deref().unwrap_or("personal"),
            classification_label(&data_classification)
        ));
    }
    if let Some(inherited) = inherited_classification {
        if classification_rank(inherited) > classification_rank(&data_classification) {
            data_classification = inherited.to_owned();
            sources.data_classification = "context".to_owned();
            notes.push(format!(
                "El historial reciente ya contiene información «{}»: el siguiente mensaje la conserva.",
                classification_label(inherited)
            ));
        }
    }
    if sensitive_memory_selected && data_classification != "local_only" {
        data_classification = "local_only".to_owned();
        sources.data_classification = "memory".to_owned();
        notes.push(
            "Hay recuerdos sensibles activos: el mensaje se atenderá solo con modelos locales."
                .to_owned(),
        );
    }
    if let Some(profile) = custom_gpt.and_then(|gpt| gpt.execution_profile.as_ref()) {
        if profile.max_cost_usd > chat.max_cost_usd {
            notes.push(format!(
                "El GPT admite hasta {:.2} USD, pero esta conversación limita el gasto a {:.2} USD; sube el límite del chat si quieres permitirlo.",
                profile.max_cost_usd, chat.max_cost_usd
            ));
        }
    }
    if sources.max_cost_usd == "gpt" {
        notes.push(format!(
            "El GPT fija un límite de gasto menor: {:.2} USD.",
            merged.max_cost_usd
        ));
    }
    if sources.routing == "gpt" {
        notes.push(
            "La forma de responder y la prioridad las fija la versión publicada del GPT."
                .to_owned(),
        );
    }
    EffectiveExecutionPolicy {
        data_classification,
        max_cost_usd: merged.max_cost_usd,
        strategy: merged.strategy,
        preset: merged.preset,
        priority: merged.priority,
        long_context: merged.long_context,
        sources,
        custom_gpt_name: gpt_name,
        destination: broker_destination(broker_base_url),
        notes,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn preferences(classification: &str, cost: f64) -> ConversationExecutionPreferences {
        ConversationExecutionPreferences {
            data_classification: classification.to_owned(),
            max_cost_usd: cost,
            ..Default::default()
        }
    }

    #[test]
    fn a_public_gpt_never_relaxes_a_confidential_chat() {
        let chat = preferences("confidential", 0.1);
        let gpt = ConversationExecutionPreferences {
            strategy: "mixture_of_agents".to_owned(),
            ..preferences("public", 0.1)
        };
        let (merged, sources) = merge_execution_preferences(&chat, Some(&gpt));
        assert_eq!(merged.data_classification, "confidential");
        assert_eq!(sources.data_classification, "chat");
        // El enrutado sí es del GPT: es lo que lo hace reproducible.
        assert_eq!(merged.strategy, "mixture_of_agents");
        assert_eq!(sources.routing, "gpt");
    }

    #[test]
    fn the_lowest_spending_limit_always_wins() {
        let (merged, sources) = merge_execution_preferences(
            &preferences("internal", 0.0),
            Some(&preferences("internal", 1.0)),
        );
        assert_eq!(merged.max_cost_usd, 0.0);
        assert_eq!(sources.max_cost_usd, "chat");
        let (merged, sources) = merge_execution_preferences(
            &preferences("internal", 1.0),
            Some(&preferences("internal", 0.25)),
        );
        assert_eq!(merged.max_cost_usd, 0.25);
        assert_eq!(sources.max_cost_usd, "gpt");
    }

    #[test]
    fn a_stricter_gpt_is_applied_and_explained() {
        let policy = effective_policy(
            &preferences("public", 0.1),
            Some(&CustomGptContext {
                custom_gpt_id: "gpt".to_owned(),
                version_id: "v".to_owned(),
                name: "Asesor".to_owned(),
                icon_ref: String::new(),
                version_no: 1,
                instructions: String::new(),
                tool_permissions: Default::default(),
                preferred_model: None,
                execution_profile: Some(preferences("local_only", 0.1)),
                context_profile: "balanced".to_owned(),
                api_actions: Vec::new(),
            }),
            None,
            false,
            "http://127.0.0.1:8765",
        );
        assert_eq!(policy.data_classification, "local_only");
        assert_eq!(policy.sources.data_classification, "gpt");
        assert!(policy.notes.iter().any(|note| note.contains("Asesor")));
    }

    #[test]
    fn protected_history_and_sensitive_memory_only_tighten() {
        let policy = effective_policy(
            &preferences("internal", 0.1),
            None,
            Some("confidential"),
            false,
            "http://127.0.0.1:8765",
        );
        assert_eq!(policy.data_classification, "confidential");
        assert_eq!(policy.sources.data_classification, "context");
        let policy = effective_policy(
            &preferences("internal", 0.1),
            None,
            Some("public"),
            true,
            "http://127.0.0.1:8765",
        );
        assert_eq!(policy.data_classification, "local_only");
        assert_eq!(policy.sources.data_classification, "memory");
    }

    #[test]
    fn an_unencrypted_lan_broker_is_named_and_warned_about() {
        let lan = broker_destination("http://192.168.1.52:8765");
        assert_eq!(lan.host, "192.168.1.52:8765");
        assert!(!lan.local_machine);
        assert!(!lan.encrypted);
        assert!(lan
            .warning
            .as_deref()
            .is_some_and(|text| text.contains("no está cifrada")));

        let local = broker_destination("http://127.0.0.1:8765");
        assert!(local.local_machine);
        assert!(local.warning.is_none());

        let tls = broker_destination("https://broker.casa.lan");
        assert!(tls.encrypted);
        assert!(tls.warning.is_none());
    }

    #[test]
    fn unknown_classifications_are_treated_as_the_strictest() {
        assert_eq!(stricter_classification("internal", "raro"), "raro");
        assert_eq!(
            classification_rank("raro"),
            classification_rank("local_only")
        );
    }
}
