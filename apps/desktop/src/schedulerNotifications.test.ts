/**
 * Máquina de estado de los avisos de automatizaciones (fase 4).
 *
 * Equivocarse aquí se paga en las dos direcciones: avisar de más molesta cada
 * diez segundos, y avisar de menos deja pasar desapercibida una automatización
 * que falló. Estas pruebas fijan las cuatro reglas que lo evitan.
 */

import { describe, expect, it } from "vitest";
import { pendingScheduledRunNotifications } from "./schedulerView";
import type { ScheduledTaskView } from "./domain";

type Run = ScheduledTaskView["runs"][number];

const run = (id: string, status: Run["status"]): Run => ({
  id,
  dueAt: "2026-08-05T18:00:00Z",
  status,
  attempt: 1,
  createdAt: "2026-08-05T17:59:00Z",
  updatedAt: "2026-08-05T18:00:05Z"
});

const task = (runs: Run[]): ScheduledTaskView =>
  ({
    id: "task-1",
    name: "Resumen diario",
    conversationId: "conversation-1",
    conversationTitle: "Novedades",
    prompt: "Resume el día",
    scheduleExpression: "daily",
    timezone: "Europe/Madrid",
    enabled: true,
    createdAt: "2026-08-01T10:00:00Z",
    updatedAt: "2026-08-05T18:00:05Z",
    runs
  }) as unknown as ScheduledTaskView;

const decide = (
  runs: Run[],
  known: Record<string, string> = {},
  overrides: { historyInitialized?: boolean; permissionGranted?: boolean } = {}
) =>
  pendingScheduledRunNotifications({
    tasks: [task(runs)],
    knownStates: new Map(Object.entries(known)),
    historyInitialized: overrides.historyInitialized ?? true,
    permissionGranted: overrides.permissionGranted ?? true
  });

describe("avisos de ejecuciones programadas", () => {
  it("avisa cuando una ejecución pasa a terminal", () => {
    const { notifications } = decide([run("run-1", "completed")], { "run-1": "running" });
    expect(notifications).toHaveLength(1);
    expect(notifications[0].runId).toBe("run-1");
    expect(notifications[0].title).toBe("Tarea programada completada");
    expect(notifications[0].body).toContain("Resumen diario");
    expect(notifications[0].body).toContain("Completada");
  });

  it("no repite el aviso mientras el estado siga siendo el mismo", () => {
    // El sondeo vuelve cada diez segundos y sigue leyendo «completed».
    const { notifications } = decide([run("run-1", "completed")], {
      "run-1": "completed"
    });
    expect(notifications).toEqual([]);
  });

  it("no avisa en el primer sondeo de la sesión", () => {
    // Sin historial previo no hay transición que observar: todo lo terminal ya
    // lo estaba antes de abrir la aplicación.
    const { notifications } = decide([run("run-1", "failed")], {}, {
      historyInitialized: false
    });
    expect(notifications).toEqual([]);
  });

  it("ignora las transiciones que no terminan nada", () => {
    const { notifications } = decide([run("run-1", "running")], { "run-1": "claimed" });
    expect(notifications).toEqual([]);
  });

  it("distingue el título de una finalización con fallo", () => {
    const failed = decide([run("run-1", "failed")], { "run-1": "running" });
    expect(failed.notifications[0].title).toBe("Tarea programada finalizada");
    const cancelled = decide([run("run-2", "cancelled")], { "run-2": "running" });
    expect(cancelled.notifications[0].title).toBe("Tarea programada finalizada");
  });

  it("usa una etiqueta estable por ejecución", () => {
    // Windows sustituye el aviso anterior del mismo run en vez de apilarlos.
    const { notifications } = decide([run("run-1", "completed")], { "run-1": "running" });
    expect(notifications[0].tag).toBe("chatygpt-run-1");
  });
});

describe("estado recordado", () => {
  it("recuerda el estado aunque no haya permiso para avisar", () => {
    // Lo importante: conceder el permiso más tarde no debe provocar una ráfaga
    // de avisos atrasados sobre ejecuciones que ya se vieron terminadas.
    const sinPermiso = decide([run("run-1", "completed")], { "run-1": "running" }, {
      permissionGranted: false
    });
    expect(sinPermiso.notifications).toEqual([]);
    expect(sinPermiso.nextStates.get("run-1")).toBe("completed");

    const despues = pendingScheduledRunNotifications({
      tasks: [task([run("run-1", "completed")])],
      knownStates: sinPermiso.nextStates,
      historyInitialized: true,
      permissionGranted: true
    });
    expect(despues.notifications).toEqual([]);
  });

  it("recuerda también en el primer sondeo, para no avisar después", () => {
    const primero = decide([run("run-1", "failed")], {}, { historyInitialized: false });
    expect(primero.nextStates.get("run-1")).toBe("failed");
  });

  it("no muta el mapa recibido", () => {
    const known = new Map([["run-1", "running"]]);
    const { nextStates } = pendingScheduledRunNotifications({
      tasks: [task([run("run-1", "completed")])],
      knownStates: known,
      historyInitialized: true,
      permissionGranted: true
    });
    expect(known.get("run-1")).toBe("running");
    expect(nextStates.get("run-1")).toBe("completed");
  });

  it("conserva ejecuciones que ya no aparecen en la respuesta", () => {
    // Las tarjetas solo traen los diez runs recientes; olvidar los anteriores
    // los haría parecer nuevos si vuelven a aparecer al filtrar el historial.
    const { nextStates } = decide([run("run-2", "completed")], {
      "run-1": "completed",
      "run-2": "running"
    });
    expect(nextStates.get("run-1")).toBe("completed");
    expect(nextStates.size).toBe(2);
  });
});
