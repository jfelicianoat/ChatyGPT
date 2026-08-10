import { describe, expect, it } from "vitest";
import {
  budgetVerdictLabel,
  budgetVerdictTone,
  formatDuration,
  isInteractionEntry,
  isReportableSample,
  MAX_SAMPLE_MS,
  MAX_SAMPLES_PER_CALL,
  PerformanceSampleBuffer,
  roundedSample
} from "./performance";

describe("muestras de rendimiento", () => {
  it("rechaza duraciones que no describen a la aplicación", () => {
    expect(isReportableSample(0)).toBe(true);
    expect(isReportableSample(1_500)).toBe(true);
    expect(isReportableSample(MAX_SAMPLE_MS)).toBe(true);
    expect(isReportableSample(-1)).toBe(false);
    expect(isReportableSample(MAX_SAMPLE_MS + 1)).toBe(false);
    expect(isReportableSample(Number.NaN)).toBe(false);
    expect(isReportableSample(Number.POSITIVE_INFINITY)).toBe(false);
  });

  it("persiste milisegundos enteros y nunca negativos", () => {
    expect(roundedSample(12.4)).toBe(12);
    expect(roundedSample(12.6)).toBe(13);
    expect(roundedSample(-0.2)).toBe(0);
  });

  it("solo considera interacciones deliberadas", () => {
    expect(isInteractionEntry({ duration: 40, interactionId: 7 })).toBe(true);
    // Desplazamiento y eventos continuos: no describen respuesta a una acción.
    expect(isInteractionEntry({ duration: 40, interactionId: 0 })).toBe(false);
    expect(isInteractionEntry({ duration: 40 })).toBe(false);
    // Una duración imposible se descarta aunque la interacción sea real.
    expect(isInteractionEntry({ duration: MAX_SAMPLE_MS + 1, interactionId: 7 })).toBe(
      false
    );
  });
});

describe("búfer de muestras", () => {
  it("acumula por métrica y vacía en una sola operación", () => {
    const buffer = new PerformanceSampleBuffer();
    buffer.push("conversation_open", 120);
    buffer.push("conversation_open", 180);
    buffer.push("conversation_search", 40);
    expect(buffer.size).toBe(3);

    const batches = buffer.drain();
    expect(batches).toEqual([
      { metric: "conversation_open", durationsMs: [120, 180] },
      { metric: "conversation_search", durationsMs: [40] }
    ]);
    // Vaciar es definitivo: un segundo envío no puede duplicar muestras.
    expect(buffer.size).toBe(0);
    expect(buffer.drain()).toEqual([]);
  });

  it("descarta lo antiguo en lugar de crecer sin límite", () => {
    const buffer = new PerformanceSampleBuffer(3);
    for (const duration of [10, 20, 30, 40, 50]) {
      buffer.push("ui_response", duration);
    }
    expect(buffer.peek("ui_response")).toEqual([30, 40, 50]);
    expect(buffer.size).toBe(3);
  });

  it("no acumula muestras inadmisibles", () => {
    const buffer = new PerformanceSampleBuffer();
    expect(buffer.push("app_start", -5)).toBe(false);
    expect(buffer.push("app_start", Number.NaN)).toBe(false);
    expect(buffer.push("app_start", 900)).toBe(true);
    expect(buffer.peek("app_start")).toEqual([900]);
  });

  it("divide en lotes que el backend admite", () => {
    const buffer = new PerformanceSampleBuffer(250);
    for (let index = 0; index < 250; index += 1) {
      buffer.push("ui_response", 20);
    }
    const batches = buffer.drain();
    expect(batches).toHaveLength(3);
    expect(batches[0].durationsMs).toHaveLength(MAX_SAMPLES_PER_CALL);
    expect(batches[1].durationsMs).toHaveLength(MAX_SAMPLES_PER_CALL);
    expect(batches[2].durationsMs).toHaveLength(50);
  });
});

describe("presentación del informe", () => {
  it("no declara cumplido un objetivo que nadie ha ejecutado", () => {
    expect(budgetVerdictLabel(null)).toBe("Sin medir");
    expect(budgetVerdictTone(null)).toBe("");
    expect(budgetVerdictLabel(true)).toBe("Dentro del objetivo");
    expect(budgetVerdictTone(true)).toBe("success");
    expect(budgetVerdictLabel(false)).toBe("Fuera del objetivo");
    expect(budgetVerdictTone(false)).toBe("warning");
  });

  it("muestra milisegundos hasta el segundo y después segundos", () => {
    expect(formatDuration(null)).toBe("—");
    expect(formatDuration(0)).toBe("0 ms");
    expect(formatDuration(999)).toBe("999 ms");
    expect(formatDuration(1_240)).toBe("1.24 s");
    expect(formatDuration(12_400)).toBe("12.4 s");
  });
});
