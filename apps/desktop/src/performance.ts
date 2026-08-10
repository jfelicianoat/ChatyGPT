/**
 * Instrumentación local de los objetivos de rendimiento.
 *
 * Este módulo solo produce **duraciones**. No recibe ni deriva texto de la
 * persona: una muestra es un número de milisegundos y el nombre de una métrica
 * de un vocabulario cerrado, el mismo que valida Rust y que restringe el CHECK
 * de la migración `0017`.
 *
 * Medir tiene un coste, así que las muestras se acumulan en memoria y se envían
 * por lotes. El búfer está acotado: si la persona interactúa más rápido de lo
 * que se vacía, se descartan las muestras más antiguas en lugar de dejar que la
 * medición consuma memoria sin límite.
 */

export const PERFORMANCE_METRICS = [
  "app_start",
  "conversation_open",
  "conversation_search",
  "ui_response"
] as const;

export type PerformanceMetric = (typeof PERFORMANCE_METRICS)[number];

/** Duración máxima admitida. Coincide con el límite que impone Rust. */
export const MAX_SAMPLE_MS = 600_000;

/** Muestras que admite el backend en una sola llamada. */
export const MAX_SAMPLES_PER_CALL = 100;

/** Muestras pendientes que se conservan en memoria por métrica. */
export const MAX_PENDING_SAMPLES = 100;

/** Intervalo de vaciado del búfer. */
export const FLUSH_INTERVAL_MS = 5_000;

/**
 * Umbral mínimo de la API de Event Timing.
 *
 * No es una elección: 16 ms es el valor más bajo que el navegador acepta en
 * `durationThreshold`. Las interacciones más rápidas son invisibles, por lo que
 * los percentiles calculados son un límite superior del rendimiento real.
 */
export const INTERACTION_THRESHOLD_MS = 16;

/** Una medición admisible: finita, no negativa y dentro del rango. */
export const isReportableSample = (durationMs: number): boolean =>
  Number.isFinite(durationMs) && durationMs >= 0 && durationMs <= MAX_SAMPLE_MS;

/** Milisegundos enteros: la persistencia no guarda fracciones. */
export const roundedSample = (durationMs: number): number =>
  Math.max(0, Math.round(durationMs));

/**
 * Decide si una entrada de Event Timing describe una interacción real.
 *
 * `interactionId` mayor que cero distingue un clic o una pulsación de teclado
 * de eventos continuos como el desplazamiento, que no representan la respuesta
 * de la interfaz a una acción deliberada.
 */
export const isInteractionEntry = (entry: {
  duration: number;
  interactionId?: number;
}): boolean =>
  typeof entry.interactionId === "number" &&
  entry.interactionId > 0 &&
  isReportableSample(entry.duration);

/**
 * Búfer acotado de muestras pendientes de enviar.
 *
 * Descarta por el extremo antiguo: ante una ráfaga, las mediciones recientes
 * describen mejor el estado actual de la aplicación que las de hace un minuto.
 */
export class PerformanceSampleBuffer {
  private readonly pending = new Map<PerformanceMetric, number[]>();

  constructor(private readonly capacity: number = MAX_PENDING_SAMPLES) {}

  /** Añade una muestra. Devuelve `false` si no era admisible. */
  push(metric: PerformanceMetric, durationMs: number): boolean {
    if (!isReportableSample(durationMs)) return false;
    const samples = this.pending.get(metric) ?? [];
    samples.push(roundedSample(durationMs));
    if (samples.length > this.capacity) {
      samples.splice(0, samples.length - this.capacity);
    }
    this.pending.set(metric, samples);
    return true;
  }

  /** Muestras pendientes de una métrica, sin vaciarlas. */
  peek(metric: PerformanceMetric): number[] {
    return [...(this.pending.get(metric) ?? [])];
  }

  get size(): number {
    let total = 0;
    for (const samples of this.pending.values()) total += samples.length;
    return total;
  }

  /**
   * Vacía el búfer en lotes que el backend admite.
   *
   * Vaciar y devolver es una sola operación: quien recibe los lotes es
   * responsable de enviarlos, y un fallo de envío no puede duplicar muestras.
   */
  drain(): { metric: PerformanceMetric; durationsMs: number[] }[] {
    const batches: { metric: PerformanceMetric; durationsMs: number[] }[] = [];
    for (const metric of PERFORMANCE_METRICS) {
      const samples = this.pending.get(metric);
      if (!samples || samples.length === 0) continue;
      for (let index = 0; index < samples.length; index += MAX_SAMPLES_PER_CALL) {
        batches.push({
          metric,
          durationsMs: samples.slice(index, index + MAX_SAMPLES_PER_CALL)
        });
      }
      this.pending.delete(metric);
    }
    return batches;
  }
}

/** Etiqueta del veredicto. Sin muestras no hay veredicto que mostrar. */
export const budgetVerdictLabel = (meetsBudget: boolean | null): string => {
  if (meetsBudget === null) return "Sin medir";
  return meetsBudget ? "Dentro del objetivo" : "Fuera del objetivo";
};

/**
 * Variante de la insignia, reutilizando las clases ya existentes.
 *
 * Sin muestras se devuelve la insignia neutra: no hay color de éxito ni de
 * aviso para un objetivo que nadie ha ejecutado.
 */
export const budgetVerdictTone = (
  meetsBudget: boolean | null
): "success" | "warning" | "" => {
  if (meetsBudget === null) return "";
  return meetsBudget ? "success" : "warning";
};

/** Duración legible: milisegundos hasta el segundo, después segundos. */
export const formatDuration = (milliseconds: number | null): string => {
  if (milliseconds === null || !Number.isFinite(milliseconds)) return "—";
  if (milliseconds < 1_000) return `${Math.round(milliseconds)} ms`;
  return `${(milliseconds / 1_000).toFixed(milliseconds < 10_000 ? 2 : 1)} s`;
};
