//! Prioriza únicamente contexto opcional recuperado. La ventana del turno,
//! resúmenes aprobados e instrucciones permanecen en el ensamblador existente.

use std::collections::HashSet;
use std::time::{Duration, Instant};

use serde::Serialize;
use serde_json::{json, Value};

use crate::broker::BrokerClient;
use crate::db::{ContextMessage, MemoryItemView, SelectedAttachmentChunk};
use crate::logging;

#[derive(Debug, Clone)]
pub(crate) struct ContextFilterConfig {
    pub enabled: bool,
    pub shadow_mode: bool,
    pub timeout_ms: u64,
    pub include_threshold: f64,
    pub uncertain_threshold: f64,
    pub min_confidence: f64,
    pub max_context_tokens: usize,
    pub max_candidates: usize,
}

impl Default for ContextFilterConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            shadow_mode: true,
            timeout_ms: 75_000,
            include_threshold: 0.80,
            uncertain_threshold: 0.55,
            min_confidence: 0.85,
            max_context_tokens: 6_000,
            max_candidates: 12,
        }
    }
}

impl ContextFilterConfig {
    pub(crate) fn from_env() -> Self {
        Self::from_lookup(|name| std::env::var(name).ok())
    }

    fn from_lookup(get: impl Fn(&str) -> Option<String>) -> Self {
        let mut config = Self::default();
        let boolean = |name, fallback| match get(name).as_deref() {
            Some("1" | "true") => true,
            Some("0" | "false") => false,
            _ => fallback,
        };
        config.enabled = boolean("CHATYGPT_SYSTEM1_CONTEXT_ENABLED", config.enabled);
        config.shadow_mode = boolean("CHATYGPT_SYSTEM1_CONTEXT_SHADOW", config.shadow_mode);
        macro_rules! numeric {
            ($field:ident, $name:literal) => {
                if let Some(value) = get($name).and_then(|value| value.parse().ok()) {
                    config.$field = value;
                }
            };
        }
        numeric!(timeout_ms, "CHATYGPT_SYSTEM1_CONTEXT_TIMEOUT_MS");
        numeric!(include_threshold, "CHATYGPT_SYSTEM1_CONTEXT_INCLUDE_THRESHOLD");
        numeric!(uncertain_threshold, "CHATYGPT_SYSTEM1_CONTEXT_UNCERTAIN_THRESHOLD");
        numeric!(min_confidence, "CHATYGPT_SYSTEM1_CONTEXT_MIN_CONFIDENCE");
        numeric!(max_context_tokens, "CHATYGPT_SYSTEM1_CONTEXT_MAX_TOKENS");
        numeric!(max_candidates, "CHATYGPT_SYSTEM1_CONTEXT_MAX_CANDIDATES");
        if !config.valid() {
            // Una configuración incoherente conserva el comportamiento anterior.
            config.enabled = false;
            logging::warn("system1.config_invalid", None, &[]);
        }
        config
    }

    fn valid(&self) -> bool {
        self.timeout_ms > 0 && self.timeout_ms <= 600_000
            && self.max_context_tokens > 0 && self.max_context_tokens <= 250_000
            && self.max_candidates > 0 && self.max_candidates <= 64
            && self.include_threshold.is_finite()
            && self.uncertain_threshold.is_finite()
            && self.min_confidence.is_finite()
            && (0.0..=1.0).contains(&self.include_threshold)
            && (0.0..=self.include_threshold).contains(&self.uncertain_threshold)
            && (0.0..=1.0).contains(&self.min_confidence)
    }
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct CandidateScore {
    pub id: String,
    pub kind: &'static str,
    pub relevance: f64,
    pub confidence: f64,
    pub confidence_is_calibrated: bool,
    pub provider: Option<String>,
    pub provider_fallback: bool,
    pub included: bool,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub(crate) struct ContextFilterTrace {
    pub mode: &'static str,
    pub reason: &'static str,
    pub candidate_tokens: usize,
    pub tokens_before: usize,
    pub tokens_after: usize,
    pub proposed_tokens_after: usize,
    pub included: usize,
    pub excluded: usize,
    pub proposed_excluded: usize,
    pub latency_ms: u128,
    pub fallback: bool,
    pub implicit_reference_signal: bool,
    pub scores: Vec<CandidateScore>,
}

#[derive(Clone)]
struct Candidate {
    id: String,
    kind: &'static str,
    text: String,
    scope: Option<String>,
    tokens: usize,
}

pub(crate) fn prompt_tokens(request: &Value) -> usize {
    request.pointer("/content/prompt").and_then(Value::as_str)
        .map_or(0, |text| text.chars().count().div_ceil(4))
}

fn candidates(memories: &[MemoryItemView], chunks: &[SelectedAttachmentChunk]) -> Vec<Candidate> {
    let mut result: Vec<Candidate> = memories.iter()
        // Preferencias e instrucciones representan restricciones activas.
        .filter(|memory| memory.category == "fact")
        .map(|memory| Candidate {
            id: memory.id.clone(), kind: "memory", text: memory.content.clone(),
            scope: memory.custom_gpt_name.clone().or_else(|| memory.project_name.clone()),
            tokens: memory.content.chars().count().div_ceil(4),
        }).collect();
    let mut seen_attachments = HashSet::new();
    for chunk in chunks {
        let anchor = seen_attachments.insert(chunk.attachment_id.clone());
        // Sin un fragmento del archivo, el ensamblador enviaría el archivo
        // completo al Broker. Tampoco se recorta una vista global deliberada.
        if anchor || chunk.reason.starts_with("Vista global del documento") {
            continue;
        }
        result.push(Candidate {
            id: chunk.id.clone(), kind: "attachment_chunk", text: chunk.text.clone(),
            scope: Some(chunk.attachment_name.clone()),
            tokens: chunk.text.chars().count().div_ceil(4),
        });
    }
    result
}

fn reference_signal(query: &str) -> bool {
    // Señal conservadora para retener niveles intermedios, nunca resolución.
    let query = query.to_lowercase();
    ["lo mismo", "segunda opción", "segunda opcion", "eso", "otro modelo", "the second", "same", "that"]
        .iter().any(|phrase| query.contains(phrase))
}

/// Todos los juicios deben ser válidos antes de cambiar una sola fuente.
/// Un error tardío revierte también los juicios anteriores de esta selección.
pub(crate) async fn filter_context(
    broker: &BrokerClient,
    query: &str,
    history: &[ContextMessage],
    baseline: &Value,
    memories: &mut Vec<MemoryItemView>,
    chunks: &mut Vec<SelectedAttachmentChunk>,
) -> Option<ContextFilterTrace> {
    let config = &broker.context_filter;
    if !config.enabled { return None; }
    let all = candidates(memories, chunks);
    let before = prompt_tokens(baseline);
    let mut trace = ContextFilterTrace {
        mode: "skipped", reason: "fits_budget", candidate_tokens: all.iter().map(|item| item.tokens).sum(),
        tokens_before: before, tokens_after: before, proposed_tokens_after: before,
        included: all.len(), excluded: 0, proposed_excluded: 0, latency_ms: 0,
        fallback: false, implicit_reference_signal: reference_signal(query), scores: Vec::new(),
    };
    if !config.valid() {
        trace.mode = "fallback"; trace.reason = "invalid_config"; trace.fallback = true;
        return Some(trace);
    }
    if all.is_empty() { trace.reason = "no_candidates"; return Some(trace); }
    if before <= config.max_context_tokens { return Some(trace); }
    let started = Instant::now();
    let timeout = Duration::from_millis(config.timeout_ms);
    let judgments = tokio::time::timeout(timeout, async {
        let capabilities = match broker.capabilities_snapshot() {
            Some(value) => value,
            None => broker.capabilities().await.map_err(|_| "capabilities_unavailable")?,
        };
        if !capabilities.system1_judgments { return Err("not_supported"); }
        let mut scores = Vec::new();
        for candidate in all.iter().take(config.max_candidates) {
            let judgment = broker.judge_context(json!({
                "current_request": query,
                "recent_history": history,
                "candidate": {"kind": candidate.kind, "scope": candidate.scope, "content": candidate.text}
            }), timeout).await.map_err(|_| "invalid_or_failed_judgment")?;
            if !judgment.accepted {
                let reason = match judgment.reason_code.as_deref() {
                    Some("LOW_CONFIDENCE" | "INSUFFICIENT_MARGIN") => "low_confidence",
                    _ => "rejected_judgment",
                };
                return Err(reason);
            }
            let confidence = judgment.confidence.ok_or("invalid_judgment")?;
            if confidence < config.min_confidence { return Err("low_confidence"); }
            scores.push(CandidateScore {
                id: candidate.id.clone(), kind: candidate.kind,
                relevance: judgment.decision.ok_or("invalid_judgment")? / 10.0,
                confidence, confidence_is_calibrated: judgment.confidence_is_calibrated,
                provider: judgment.provider, provider_fallback: judgment.fallback_used, included: true,
            });
        }
        Ok(scores)
    }).await;
    trace.latency_ms = started.elapsed().as_millis();
    trace.scores = match judgments {
        Ok(Ok(scores)) => scores,
        failure => {
            trace.mode = "fallback"; trace.fallback = true;
            trace.reason = match failure { Ok(Err(reason)) => reason, _ => "timeout" };
            return Some(trace);
        }
    };
    let mut remaining_tokens = before;
    let mut removed: HashSet<(&str, &str)> = HashSet::new();
    // Decide primero descartes claros, después encaja los niveles intermedios.
    let mut order: Vec<usize> = (0..trace.scores.len()).collect();
    order.sort_by(|a, b| trace.scores[*a].relevance.total_cmp(&trace.scores[*b].relevance));
    for index in order {
        let score = &mut trace.scores[index];
        let discard = score.relevance < config.uncertain_threshold
            || (score.relevance < config.include_threshold
                && remaining_tokens > config.max_context_tokens
                && !trace.implicit_reference_signal);
        if discard {
            score.included = false;
            remaining_tokens = remaining_tokens.saturating_sub(all[index].tokens);
            removed.insert((all[index].kind, all[index].id.as_str()));
        }
    }
    trace.proposed_excluded = removed.len();
    trace.proposed_tokens_after = remaining_tokens;
    trace.mode = if config.shadow_mode { "shadow" } else { "applied" };
    trace.reason = "scored";
    if !config.shadow_mode {
        memories.retain(|memory| !removed.contains(&("memory", memory.id.as_str())));
        chunks.retain(|chunk| !removed.contains(&("attachment_chunk", chunk.id.as_str())));
        trace.excluded = removed.len();
        trace.included = all.len() - removed.len();
    }
    Some(trace)
}

pub(crate) fn attach_trace(request: &mut Value, mut trace: ContextFilterTrace) {
    trace.tokens_after = prompt_tokens(request);
    if trace.mode == "applied" { trace.proposed_tokens_after = trace.tokens_after; }
    logging::info("system1.context_selection", None, &[
        ("mode", logging::code(trace.mode)), ("reason", logging::code(trace.reason)),
        ("candidate_tokens", logging::count(trace.candidate_tokens as i64)),
        ("tokens_before", logging::count(trace.tokens_before as i64)),
        ("tokens_after", logging::count(trace.tokens_after as i64)),
        ("included", logging::count(trace.included as i64)),
        ("excluded", logging::count(trace.excluded as i64)),
        ("latency_ms", logging::millis(trace.latency_ms)),
        ("fallback", logging::flag(trace.fallback)),
        ("implicit_reference_signal", logging::flag(trace.implicit_reference_signal)),
    ]);
    if let Some(metadata) = request.pointer_mut("/content/metadata").and_then(Value::as_object_mut) {
        metadata.insert("system1_context".to_owned(), json!(trace));
    }
}

#[cfg(test)]
mod tests;
