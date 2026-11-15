import type { Loadable, PerformanceReportView } from "../domain";
import { budgetVerdictLabel, budgetVerdictTone, formatDuration } from "../performance";

/** Mediciones de rendimiento locales (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaRendimientoProps = {
  performanceReport: Loadable<PerformanceReportView>;
  performanceBusy: boolean;
  clearPerformanceSamples: () => Promise<void> | void;
};

export function TarjetaRendimiento({ performanceReport, performanceBusy, clearPerformanceSamples }: TarjetaRendimientoProps) {
  return (
    <section className="performance-card" aria-labelledby="performance-heading">
      <div>
        <span className="kicker">Medición local</span>
        <h3 id="performance-heading">Rendimiento</h3>
        <p>
          Mediciones tomadas en este equipo mientras usas la aplicación. Se
          guardan únicamente duraciones: ni textos, ni títulos, ni rutas. Cada
          objetivo se compara con el percentil 95 de las muestras conservadas.
        </p>
      </div>
      {performanceReport.state === "loading" && <small>Cargando mediciones…</small>}
      {performanceReport.state === "error" && (
        <small role="alert">{performanceReport.message}</small>
      )}
      {performanceReport.state === "ready" && (
        <>
          <div className="performance-grid">
            {performanceReport.value.metrics.map((summary) => (
              <article key={summary.metric} className="performance-metric">
                <header>
                  <strong>{summary.label}</strong>
                  <span
                    className={`badge ${budgetVerdictTone(summary.meetsBudget)}`}
                  >
                    {budgetVerdictLabel(summary.meetsBudget)}
                  </span>
                </header>
                <dl>
                  <div>
                    <dt>Objetivo</dt>
                    <dd>≤ {formatDuration(summary.budgetMs)} (p95)</dd>
                  </div>
                  <div>
                    <dt>p95</dt>
                    <dd>{formatDuration(summary.p95Ms)}</dd>
                  </div>
                  <div>
                    <dt>Mediana</dt>
                    <dd>{formatDuration(summary.p50Ms)}</dd>
                  </div>
                  <div>
                    <dt>Peor caso</dt>
                    <dd>{formatDuration(summary.maxMs)}</dd>
                  </div>
                  <div>
                    <dt>Muestras</dt>
                    <dd>{summary.samples}</dd>
                  </div>
                </dl>
                <small>{summary.description}</small>
              </article>
            ))}
          </div>
          <div className="performance-actions">
            <small aria-live="polite">
              {performanceReport.value.totalSamples} muestras conservadas ·
              se guardan como máximo {performanceReport.value.sampleLimit} por
              métrica y las antiguas se descartan.
            </small>
            <button
              className="secondary"
              disabled={
                performanceBusy || performanceReport.value.totalSamples === 0
              }
              onClick={() => void clearPerformanceSamples()}
            >
              {performanceBusy ? "Vaciando…" : "Vaciar mediciones"}
            </button>
          </div>
        </>
      )}
    </section>
  );
}
