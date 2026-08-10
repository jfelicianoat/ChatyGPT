//! Objetivos de rendimiento y estadística sobre las muestras locales.
//!
//! La Fase 4 exige medir de forma visible el arranque, la apertura de una
//! conversación, la búsqueda y la respuesta inmediata de la interfaz. Este
//! módulo concentra dos decisiones para que no se dispersen por el código:
//!
//! 1. **Qué se mide y con qué objetivo.** El vocabulario de métricas es cerrado
//!    y cada una lleva su presupuesto asociado. La misma lista está replicada
//!    como CHECK en la migración `0017`, de modo que la base de datos rechaza
//!    una métrica desconocida aunque el código se equivoque.
//! 2. **Cómo se resume.** El percentil usa rango más cercano sobre la muestra
//!    ordenada: es determinista, no interpola valores que nunca ocurrieron y
//!    con pocas muestras devuelve una medición real y no una estimación.
//!
//! Una métrica sin muestras **no** obtiene veredicto. `meets_budget` es
//! `Option<bool>` justamente para que la interfaz no pueda presentar como
//! cumplido un objetivo que nadie ha ejecutado todavía.

use serde::Serialize;

/// Muestras conservadas por métrica. Al superarse, se podan las más antiguas.
pub const MAX_SAMPLES_PER_METRIC: i64 = 200;

/// Duración máxima admitida en una muestra.
///
/// Diez minutos no es un objetivo: es el límite por encima del cual la medida
/// deja de describir la aplicación y describe otra cosa —el equipo suspendido,
/// la ventana minimizada durante horas— por lo que se rechaza en lugar de
/// contaminar los percentiles.
pub const MAX_SAMPLE_MS: i64 = 600_000;

/// Muestras admitidas en una sola llamada de registro.
pub const MAX_SAMPLES_PER_CALL: usize = 100;

/// Métrica de rendimiento observable localmente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum PerformanceMetric {
    /// Desde que la vista web empieza a cargar hasta que la aplicación es usable.
    AppStart,
    /// Desde el clic en una conversación hasta que sus mensajes están en pantalla.
    ConversationOpen,
    /// Consulta de conversaciones resuelta contra SQLite.
    ConversationSearch,
    /// Interacción de la persona hasta el fotograma que la refleja.
    UiResponse,
}

impl PerformanceMetric {
    /// Todas las métricas, en el orden en que se presentan.
    pub const ALL: [PerformanceMetric; 4] = [
        PerformanceMetric::AppStart,
        PerformanceMetric::ConversationOpen,
        PerformanceMetric::ConversationSearch,
        PerformanceMetric::UiResponse,
    ];

    /// Clave persistida. Coincide con el CHECK de la migración `0017`.
    pub fn as_str(self) -> &'static str {
        match self {
            PerformanceMetric::AppStart => "app_start",
            PerformanceMetric::ConversationOpen => "conversation_open",
            PerformanceMetric::ConversationSearch => "conversation_search",
            PerformanceMetric::UiResponse => "ui_response",
        }
    }

    /// Reconoce una métrica recibida desde la interfaz.
    pub fn parse(value: &str) -> Option<Self> {
        Self::ALL
            .into_iter()
            .find(|metric| metric.as_str() == value.trim())
    }

    /// Objetivo de rendimiento, comparado siempre contra el percentil 95.
    ///
    /// Los valores son los presupuestos iniciales adoptados por el proyecto y
    /// viven aquí, en un único sitio, para poder ajustarlos sin tocar la
    /// instrumentación ni la interfaz.
    pub fn budget_ms(self) -> i64 {
        match self {
            PerformanceMetric::AppStart => 2_000,
            PerformanceMetric::ConversationOpen => 400,
            PerformanceMetric::ConversationSearch => 300,
            PerformanceMetric::UiResponse => 100,
        }
    }

    /// Nombre legible para la interfaz.
    pub fn label(self) -> &'static str {
        match self {
            PerformanceMetric::AppStart => "Arranque de la aplicación",
            PerformanceMetric::ConversationOpen => "Apertura de una conversación",
            PerformanceMetric::ConversationSearch => "Búsqueda de conversaciones",
            PerformanceMetric::UiResponse => "Respuesta inmediata de la interfaz",
        }
    }

    /// Qué abarca exactamente la medida, incluidas sus limitaciones.
    pub fn description(self) -> &'static str {
        match self {
            PerformanceMetric::AppStart => {
                "Desde que la vista web empieza a cargar hasta que hay lista de \
                 conversaciones y la primera conversación en pantalla. No incluye \
                 el arranque del proceso ni la creación de WebView2, que la \
                 interfaz no puede observar."
            }
            PerformanceMetric::ConversationOpen => {
                "Desde el clic en una conversación hasta que sus mensajes, adjuntos \
                 y archivos de proyecto están cargados."
            }
            PerformanceMetric::ConversationSearch => {
                "Consulta contra SQLite ya escrita la búsqueda. No incluye la espera \
                 deliberada de 250 ms que evita consultar en cada tecla."
            }
            PerformanceMetric::UiResponse => {
                "Desde la interacción hasta el fotograma que la refleja. Solo son \
                 observables las interacciones de 16 ms o más, el mínimo que expone \
                 la API del navegador: las más rápidas no se registran, así que los \
                 percentiles son un límite superior y nunca una cifra optimista."
            }
        }
    }
}

/// Resumen de una métrica sobre las muestras conservadas.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct MetricSummary {
    pub metric: &'static str,
    pub label: &'static str,
    pub description: &'static str,
    pub budget_ms: i64,
    pub samples: i64,
    pub p50_ms: Option<i64>,
    pub p95_ms: Option<i64>,
    pub max_ms: Option<i64>,
    /// `None` mientras no haya ninguna muestra: sin ejecución no hay veredicto.
    pub meets_budget: Option<bool>,
    pub last_recorded_at: Option<String>,
}

/// Informe completo presentado en la interfaz.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PerformanceReportView {
    pub metrics: Vec<MetricSummary>,
    pub sample_limit: i64,
    pub total_samples: i64,
}

/// Percentil por rango más cercano sobre una muestra ordenada de menor a mayor.
///
/// Devuelve siempre un valor observado. Con una sola muestra, cualquier
/// percentil es esa muestra; es correcto y evita fingir precisión estadística
/// que un puñado de mediciones no tiene.
pub fn percentile(sorted_durations: &[i64], percentile: u32) -> Option<i64> {
    if sorted_durations.is_empty() {
        return None;
    }
    let percentile = percentile.clamp(1, 100) as usize;
    let count = sorted_durations.len();
    // Rango más cercano: ceil(p/100 · n), calculado con enteros.
    let rank = (percentile * count).div_ceil(100);
    let index = rank.saturating_sub(1).min(count - 1);
    Some(sorted_durations[index])
}

/// Resume una métrica. `durations` puede llegar en cualquier orden.
pub fn summarize(
    metric: PerformanceMetric,
    durations: &[i64],
    last_recorded_at: Option<String>,
) -> MetricSummary {
    let mut sorted = durations.to_vec();
    sorted.sort_unstable();
    let p95 = percentile(&sorted, 95);
    MetricSummary {
        metric: metric.as_str(),
        label: metric.label(),
        description: metric.description(),
        budget_ms: metric.budget_ms(),
        samples: sorted.len() as i64,
        p50_ms: percentile(&sorted, 50),
        p95_ms: p95,
        max_ms: sorted.last().copied(),
        meets_budget: p95.map(|value| value <= metric.budget_ms()),
        last_recorded_at,
    }
}

/// Valida una muestra recibida desde la interfaz.
pub fn is_reportable_sample(duration_ms: i64) -> bool {
    (0..=MAX_SAMPLE_MS).contains(&duration_ms)
}

#[cfg(test)]
mod tests {
    use super::{is_reportable_sample, percentile, summarize, PerformanceMetric, MAX_SAMPLE_MS};

    #[test]
    fn metric_vocabulary_is_closed_and_round_trips() {
        for metric in PerformanceMetric::ALL {
            assert_eq!(PerformanceMetric::parse(metric.as_str()), Some(metric));
            assert!(metric.budget_ms() > 0);
            assert!(!metric.label().is_empty());
            assert!(!metric.description().is_empty());
        }
        assert_eq!(
            PerformanceMetric::parse("  app_start  "),
            Some(PerformanceMetric::AppStart)
        );
        assert_eq!(PerformanceMetric::parse("prompt del usuario"), None);
        assert_eq!(PerformanceMetric::parse(""), None);
    }

    #[test]
    fn percentile_returns_an_observed_value_by_nearest_rank() {
        let sorted = [10, 20, 30, 40, 50, 60, 70, 80, 90, 100];
        assert_eq!(percentile(&sorted, 50), Some(50));
        assert_eq!(percentile(&sorted, 95), Some(100));
        assert_eq!(percentile(&sorted, 100), Some(100));
        // El resultado siempre es una medición real, nunca un valor interpolado.
        assert!(percentile(&sorted, 95).is_some_and(|value| sorted.contains(&value)));
        // Una única muestra es su propio percentil.
        assert_eq!(percentile(&[42], 50), Some(42));
        assert_eq!(percentile(&[42], 95), Some(42));
        assert_eq!(percentile(&[], 95), None);
    }

    #[test]
    fn summary_without_samples_has_no_verdict() {
        let summary = summarize(PerformanceMetric::AppStart, &[], None);
        assert_eq!(summary.samples, 0);
        assert_eq!(summary.p50_ms, None);
        assert_eq!(summary.p95_ms, None);
        assert_eq!(summary.max_ms, None);
        // Lo esencial: sin ejecución no se declara cumplido ni incumplido.
        assert_eq!(summary.meets_budget, None);
    }

    #[test]
    fn summary_compares_the_ninety_fifth_percentile_against_the_budget() {
        let budget = PerformanceMetric::ConversationOpen.budget_ms();
        // Veinte muestras rápidas y una lenta: la lenta no puede tumbar el veredicto.
        let mut durations = vec![120; 20];
        durations.push(budget + 500);
        let summary = summarize(PerformanceMetric::ConversationOpen, &durations, None);
        assert_eq!(summary.samples, 21);
        assert_eq!(summary.p50_ms, Some(120));
        assert_eq!(summary.p95_ms, Some(120));
        assert_eq!(summary.max_ms, Some(budget + 500));
        assert_eq!(summary.meets_budget, Some(true));

        // Con la mayoría por encima del objetivo, el veredicto es incumplido.
        let slow = vec![budget + 1; 10];
        let summary = summarize(PerformanceMetric::ConversationOpen, &slow, None);
        assert_eq!(summary.meets_budget, Some(false));

        // El límite exacto cumple: el objetivo es «como máximo», no «por debajo».
        let exact = vec![budget; 10];
        assert_eq!(
            summarize(PerformanceMetric::ConversationOpen, &exact, None).meets_budget,
            Some(true)
        );
    }

    #[test]
    fn summary_orders_unsorted_input() {
        let summary = summarize(PerformanceMetric::UiResponse, &[90, 20, 50, 10, 70], None);
        assert_eq!(summary.p50_ms, Some(50));
        assert_eq!(summary.max_ms, Some(90));
    }

    #[test]
    fn samples_outside_the_admissible_range_are_rejected() {
        assert!(is_reportable_sample(0));
        assert!(is_reportable_sample(1_500));
        assert!(is_reportable_sample(MAX_SAMPLE_MS));
        assert!(!is_reportable_sample(-1));
        assert!(!is_reportable_sample(MAX_SAMPLE_MS + 1));
    }

    /// El vocabulario vive en dos sitios —este módulo y el CHECK de la
    /// migración— porque la base de datos debe rechazar por sí misma una
    /// métrica desconocida. Esta prueba impide que ambas listas se separen.
    #[test]
    fn metric_keys_match_the_check_of_the_migration() {
        const MIGRATION: &str = include_str!("../migrations/0017_performance_samples.sql");
        for metric in PerformanceMetric::ALL {
            assert!(
                MIGRATION.contains(&format!("'{}'", metric.as_str())),
                "la migración no admite la métrica {}",
                metric.as_str()
            );
        }
        assert!(MIGRATION.contains(&format!("duration_ms <= {MAX_SAMPLE_MS}")));
    }
}
