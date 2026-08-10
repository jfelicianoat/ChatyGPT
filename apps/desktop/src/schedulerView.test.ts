// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  canSaveScheduleTemplate,
  defaultScheduledLocalTime,
  validateScheduleDraft,
  loadSchedulerReadNotifications,
  MAX_READ_NOTIFICATIONS,
  persistSchedulerReadNotifications,
  scheduledLocalTimeValue,
  scheduledRunLabel,
  SCHEDULER_READ_NOTIFICATIONS_KEY,
  schedulerReadNotificationsExist
} from "./schedulerView";

describe("hora local de una automatización", () => {
  it("usa la hora de pared del equipo, no UTC", () => {
    // Un valor de `datetime-local` nunca lleva zona: si se formateara en UTC,
    // la persona vería una hora distinta a la que eligió.
    const local = new Date(2026, 7, 5, 9, 7);
    expect(scheduledLocalTimeValue(local)).toBe("2026-08-05T09:07");
  });

  it("rellena con ceros mes, día, hora y minuto", () => {
    expect(scheduledLocalTimeValue(new Date(2026, 0, 2, 3, 4))).toBe("2026-01-02T03:04");
  });

  it("propone dentro de una hora, para que la fecha ya sea futura", () => {
    const now = new Date(2026, 7, 5, 23, 30);
    // Cruza la medianoche correctamente en vez de quedarse en el mismo día.
    expect(defaultScheduledLocalTime(now)).toBe("2026-08-06T00:30");
  });
});

describe("revisión del borrador de una automatización", () => {
  const now = new Date(2026, 7, 5, 12, 0);
  const complete = {
    name: "Resumen diario",
    conversationId: "conversation-1",
    prompt: "Resume las novedades del día",
    at: "2026-08-05T18:00",
    confirmed: true
  };

  it("acepta un borrador completo con fecha futura", () => {
    const result = validateScheduleDraft(complete, now);
    expect(result.status).toBe("valid");
    if (result.status !== "valid") throw new Error("se esperaba válido");
    expect(new Date(result.dueAtIso).getTime()).toBeGreaterThan(now.getTime());
  });

  it("trata un formulario a medias como incompleto, no como error", () => {
    // Mientras se rellena no se le dice nada a la persona: solo se desactiva
    // el botón. Confundir esto con un error llenaría la pantalla de avisos.
    for (const missing of [
      { ...complete, name: "   " },
      { ...complete, conversationId: "" },
      { ...complete, prompt: "" },
      { ...complete, at: "" }
    ]) {
      expect(validateScheduleDraft(missing, now).status).toBe("incomplete");
    }
  });

  it("exige la confirmación como un dato más del formulario", () => {
    // Es lo que impide activar una automatización sin haberlo decidido.
    expect(
      validateScheduleDraft({ ...complete, confirmed: false }, now).status
    ).toBe("incomplete");
  });

  it("rechaza una fecha pasada con una explicación", () => {
    const result = validateScheduleDraft(
      { ...complete, at: "2026-08-05T09:00" },
      now
    );
    expect(result.status).toBe("invalid-date");
    if (result.status !== "invalid-date") throw new Error("se esperaba fecha inválida");
    expect(result.message).toBe("Elige una fecha y hora futuras.");
  });

  it("rechaza también el instante exacto de ahora y una fecha ilegible", () => {
    // Programar «para ahora mismo» no deja margen a la propia comprobación.
    const sameInstant = validateScheduleDraft(
      { ...complete, at: "2026-08-05T12:00" },
      now
    );
    expect(sameInstant.status).toBe("invalid-date");
    expect(validateScheduleDraft({ ...complete, at: "no es fecha" }, now).status).toBe(
      "invalid-date"
    );
  });
});

describe("guardar como plantilla", () => {
  it("solo necesita nombre e instrucción", () => {
    // Una plantilla no programa nada: no pide conversación, fecha ni
    // confirmación, porque usarla obliga a revisarla y confirmarla después.
    expect(canSaveScheduleTemplate({ name: "Resumen", prompt: "Resume" })).toBe(true);
    expect(canSaveScheduleTemplate({ name: "  ", prompt: "Resume" })).toBe(false);
    expect(canSaveScheduleTemplate({ name: "Resumen", prompt: "   " })).toBe(false);
  });
});

describe("etiquetas de ejecución", () => {
  it("nombra cada estado sin dejar ninguno sin texto", () => {
    const states = [
      "claimed",
      "running",
      "completed",
      "failed",
      "cancelled",
      "skipped"
    ] as const;
    const labels = states.map((state) => scheduledRunLabel(state));
    expect(labels).toEqual([
      "Preparando",
      "En ejecución",
      "Completada",
      "Fallida",
      "Cancelada",
      "Omitida"
    ]);
    // Ninguna etiqueta repetida: dos estados distintos no pueden verse igual.
    expect(new Set(labels).size).toBe(states.length);
  });
});

describe("avisos leídos", () => {
  beforeEach(() => {
    window.localStorage.clear();
  });

  afterEach(() => {
    vi.restoreAllMocks();
  });

  it("distingue no haber abierto nunca de haberlo leído todo", () => {
    expect(schedulerReadNotificationsExist()).toBe(false);
    persistSchedulerReadNotifications(new Set());
    expect(schedulerReadNotificationsExist()).toBe(true);
    expect(loadSchedulerReadNotifications().size).toBe(0);
  });

  it("conserva los identificadores entre sesiones", () => {
    persistSchedulerReadNotifications(new Set(["run-1", "run-2"]));
    expect([...loadSchedulerReadNotifications()].sort()).toEqual(["run-1", "run-2"]);
  });

  it("acota lo guardado y conserva lo más reciente", () => {
    const ids = new Set(
      Array.from({ length: MAX_READ_NOTIFICATIONS + 50 }, (_, index) => `run-${index}`)
    );
    persistSchedulerReadNotifications(ids);
    const stored = loadSchedulerReadNotifications();
    expect(stored.size).toBe(MAX_READ_NOTIFICATIONS);
    expect(stored.has(`run-${MAX_READ_NOTIFICATIONS + 49}`)).toBe(true);
    expect(stored.has("run-0")).toBe(false);
  });

  it("degrada a «ninguno leído» ante un valor dañado", () => {
    window.localStorage.setItem(SCHEDULER_READ_NOTIFICATIONS_KEY, "{no es json");
    expect(loadSchedulerReadNotifications().size).toBe(0);

    // Un JSON válido pero de otra forma tampoco puede romper la interfaz.
    window.localStorage.setItem(SCHEDULER_READ_NOTIFICATIONS_KEY, '{"leidos":true}');
    expect(loadSchedulerReadNotifications().size).toBe(0);

    // Se descartan los elementos que no son identificadores.
    window.localStorage.setItem(SCHEDULER_READ_NOTIFICATIONS_KEY, '["run-1",7,null]');
    expect([...loadSchedulerReadNotifications()]).toEqual(["run-1"]);
  });

  it("no rompe si WebView2 deniega el almacenamiento local", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new Error("almacenamiento denegado");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new Error("almacenamiento denegado");
    });

    // El historial durable sigue en SQLite: perder la marca no es un error.
    expect(loadSchedulerReadNotifications().size).toBe(0);
    expect(schedulerReadNotificationsExist()).toBe(false);
    expect(() => persistSchedulerReadNotifications(new Set(["run-1"]))).not.toThrow();
  });
});
