// @vitest-environment jsdom
/**
 * Regresiones de interfaz de la auditoría QA del 14 de octubre de 2026.
 *
 * - INC-02: sin credencial el aviso dice que falta, no que el Broker la
 *   «rechaza», y no atribuye el fallo a una rotación del token que no existe.
 * - INC-10: la fecha propuesta para una programación se renueva al entrar en
 *   Automatizaciones si ya quedó en el pasado (el panel no se desmonta).
 */

import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

const DEFAULTS: Record<string, unknown> = {
  bootstrap: {
    appVersion: "0.3.0",
    databasePath: "C:/pruebas/chatygpt.db",
    logPath: null,
    schemaVersion: 25,
    recoveredTasks: 0,
    recoveredAttachments: 0,
    recoveredWorkflows: 0,
    recoveryItems: []
  },
  diagnoseBroker: {
    reachable: true,
    ready: true,
    baseUrl: "http://127.0.0.1:8765",
    strategies: ["single"],
    presets: {},
    workLanes: ["inference"],
    agentSkills: [],
    latencyMs: 4,
    message: "Broker AI está listo"
  },
  getWindowsStartupStatus: { supported: true, enabled: false, credentialProtected: true, message: "" },
  getBrokerCredential: { source: "protected", protected: true, environmentPresent: false, message: "" },
  getAthenaStatus: { estado: "conectado", urlBase: "", credencialConfigurada: true, versionContrato: 1, runsActivos: 0 },
  listAthenaProfiles: { default: "software_engineering", profiles: [] },
  listAthenaModels: { default: "", models: [] },
  getMemoryOverview: { enabled: false, items: [] },
  getLatestMemorySearch: null,
  getPerformanceReport: { sampleLimit: 200, totalSamples: 0, metrics: [] },
  getEffectiveExecutionPolicy: {
    dataClassification: "public",
    maxCostUsd: 0.1,
    strategy: "single",
    preset: "fast",
    priority: 50,
    longContext: "fail",
    sources: { dataClassification: "chat", maxCostUsd: "chat", routing: "chat" },
    customGptName: null,
    destination: { host: "127.0.0.1:8765", localMachine: true, encrypted: false, warning: null },
    notes: []
  }
};

const callLog = new Map<string, ReturnType<typeof vi.fn>>();

function platformMethod(name: string) {
  let mock = callLog.get(name);
  if (!mock) {
    mock = vi.fn(async () =>
      Object.prototype.hasOwnProperty.call(DEFAULTS, name) ? DEFAULTS[name] : []
    );
    callLog.set(name, mock);
  }
  return mock;
}

vi.mock("./platform", () => ({
  platform: new Proxy({}, { get: (_target, property: string) => platformMethod(property) })
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({ onDragDropEvent: async () => () => undefined })
}));

import { App } from "./App";

async function mountHome(diagnostic: Record<string, unknown> = {}) {
  platformMethod("listConversations").mockResolvedValue([]);
  platformMethod("diagnoseBroker").mockResolvedValue({
    ...(DEFAULTS.diagnoseBroker as object),
    ...diagnostic
  });
  render(<App />);
  await screen.findByRole("button", { name: "Automatizaciones" });
}

describe("Regresiones de la auditoría 2026-10-14", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
    window.localStorage.clear();
  });
  afterEach(() => cleanup());

  it("INC-02: sin credencial el aviso dice que falta", async () => {
    await mountHome({ ready: false, credential: "missing" });
    const text = (await screen.findByText(/Falta la credencial de Broker AI/)).textContent ?? "";
    expect(text).not.toMatch(/rechaza/);
    expect(document.body.textContent).not.toMatch(/cada arranque/);
  });

  it("INC-02: una credencial rechazada ya no se achaca a que el token rota en cada arranque", async () => {
    await mountHome({ ready: false, credential: "rejected" });
    await screen.findByText(/rechaza la credencial guardada/);
    expect(document.body.textContent).not.toMatch(/cada arranque/);
  });

  it("INC-10: al volver a Automatizaciones una fecha vencida se renueva y una futura se respeta", async () => {
    await mountHome();
    const automations = screen.getByRole("button", { name: "Automatizaciones" });
    const chats = screen.getByRole("button", { name: "Chats" });
    await userEvent.click(automations);
    const when = await waitFor(() => {
      const input = document.querySelector<HTMLInputElement>(".scheduler-form input[type=datetime-local]");
      expect(input).not.toBeNull();
      return input!;
    });

    fireEvent.change(when, { target: { value: "2020-01-01T10:00" } });
    expect(when.value).toBe("2020-01-01T10:00");
    await userEvent.click(chats);
    await userEvent.click(automations);
    await waitFor(() => expect(when.value > "2020-01-01T10:00").toBe(true));
    expect(new Date(when.value).getTime()).toBeGreaterThan(Date.now());

    fireEvent.change(when, { target: { value: "2099-05-01T10:00" } });
    await userEvent.click(chats);
    await userEvent.click(automations);
    expect(when.value).toBe("2099-05-01T10:00");
  });
});
