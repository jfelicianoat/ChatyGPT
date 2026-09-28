// @vitest-environment jsdom
/**
 * Regresiones de interfaz de la auditoría del 28 de septiembre de 2026.
 *
 * Nacen de `docs/AUDIT_2026-09-28/evidence/ui.test.tsx`, que reproducía los
 * fallos con la aplicación real y un puente `platform` simulado. Aquí las
 * mismas situaciones afirman ya el comportamiento correcto:
 *
 * - H10: el borrador de una conversación no aparece en otra, y vuelve al
 *   regresar; sobrevive a un reinicio.
 * - H11: una carga lenta o la aceptación tardía de un envío no cambian la
 *   pantalla después de la última elección de la persona.
 * - H12: si el envío se aceptó y solo falla la recarga, el texto no se
 *   restaura como no enviado y se ofrece actualizar sin reenviar.
 * - H24: Enter durante una composición IME no envía.
 * - H22: una búsqueda antigua que termina tarde no pisa la vigente.
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
import { DRAFT_STORAGE_KEY } from "./borradores";

const summaries = ["A", "B", "C"].map((id) => ({
  id,
  title: `Chat ${id}`,
  projectId: null,
  updatedAt: "2026-09-28T10:00:00Z"
}));

const view = (id: string) => ({
  ...summaries.find((item) => item.id === id),
  customGptId: null,
  executionPreferences: {
    dataClassification: "public",
    strategy: "single",
    preset: "fast",
    maxCostUsd: 0.1,
    longContext: "fail",
    priority: 50
  },
  messages: [],
  researchRuns: [],
  totalMessageCount: 0,
  hasEarlierMessages: false
});

const acceptedTask = {
  id: "task-A",
  remoteTaskId: "remote-A",
  remoteStatus: "completed",
  localState: "terminal",
  consecutivePollErrors: 0,
  result: {},
  progress: { phase: "completed" },
  pendingToolCalls: [],
  updatedAt: "2026-09-28T10:00:00Z"
};

const composer = () => screen.getByPlaceholderText("Escribe un mensaje…") as HTMLTextAreaElement;

async function setup() {
  platformMethod("listConversations").mockResolvedValue(summaries);
  platformMethod("getConversation").mockImplementation(async (id: string) => view(id));
  render(<App />);
  await screen.findByRole("heading", { name: "Chat A" });
}

describe("Regresiones de la auditoría 2026-09-28", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
    window.localStorage.clear();
  });
  afterEach(() => cleanup());

  it("H10: cada conversación conserva su borrador y no lo presta a otra", async () => {
    await setup();
    await userEvent.type(composer(), "Contenido privado destinado a A");
    await userEvent.click(screen.getByRole("button", { name: "Chat B" }));
    await screen.findByRole("heading", { name: "Chat B" });
    expect(composer().value).toBe("");

    await userEvent.type(composer(), "Borrador de B");
    await userEvent.click(screen.getByRole("button", { name: "Chat A" }));
    await screen.findByRole("heading", { name: "Chat A" });
    expect(composer().value).toBe("Contenido privado destinado a A");

    // Y sobrevive a cerrar la aplicación.
    const stored = JSON.parse(window.localStorage.getItem(DRAFT_STORAGE_KEY) ?? "{}");
    expect(stored.A.text).toBe("Contenido privado destinado a A");
    expect(stored.B.text).toBe("Borrador de B");
  });

  it("H11: una carga antigua de B no desplaza la selección posterior de C", async () => {
    await setup();
    let finishB!: (value: unknown) => void;
    platformMethod("getConversation").mockImplementation((id: string) =>
      id === "B" ? new Promise((resolve) => (finishB = resolve)) : Promise.resolve(view(id))
    );
    await userEvent.click(screen.getByRole("button", { name: "Chat B" }));
    await userEvent.click(screen.getByRole("button", { name: "Chat C" }));
    await screen.findByRole("heading", { name: "Chat C" });
    await act(async () => finishB(view("B")));
    expect(screen.getByRole("heading", { name: "Chat C" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Chat B" })).toBeNull();
  });

  it("H11: la aceptación tardía de un envío en A no saca a la persona de B", async () => {
    await setup();
    let finish!: (value: unknown) => void;
    platformMethod("sendChatTurn").mockReturnValue(new Promise((resolve) => (finish = resolve)));
    await userEvent.type(composer(), "Pregunta de A");
    await userEvent.click(screen.getByRole("button", { name: "Enviar" }));
    await userEvent.click(screen.getByRole("button", { name: "Chat B" }));
    await screen.findByRole("heading", { name: "Chat B" });
    await act(async () => finish(acceptedTask));
    expect(screen.getByRole("heading", { name: "Chat B" })).toBeDefined();
    expect(screen.queryByRole("heading", { name: "Chat A" })).toBeNull();
  });

  it("H12: un fallo de recarga tras aceptar no restaura el texto ni invita a reenviarlo", async () => {
    await setup();
    platformMethod("sendChatTurn").mockResolvedValue(acceptedTask);
    platformMethod("getConversation").mockRejectedValueOnce(
      new Error("Fallo simulado de lectura posterior al commit")
    );
    await userEvent.type(composer(), "Pregunta aceptada");
    await userEvent.click(screen.getByRole("button", { name: "Enviar" }));
    await screen.findByText(/El mensaje se envió y está guardado/);
    expect(composer().value).toBe("");
    expect(platformMethod("sendChatTurn")).toHaveBeenCalledTimes(1);

    // Actualizar vuelve a leer; nunca vuelve a enviar.
    await userEvent.click(screen.getByRole("button", { name: "Actualizar la vista" }));
    await waitFor(() =>
      expect(screen.queryByText(/El mensaje se envió y está guardado/)).toBeNull()
    );
    expect(platformMethod("sendChatTurn")).toHaveBeenCalledTimes(1);
  });

  it("H12: reintentar un envío rechazado con el mismo texto reutiliza su operación", async () => {
    await setup();
    platformMethod("sendChatTurn")
      .mockRejectedValueOnce(new Error("Broker AI no está accesible"))
      .mockResolvedValueOnce(acceptedTask);
    await userEvent.type(composer(), "Pregunta repetida");
    await userEvent.click(screen.getByRole("button", { name: "Enviar" }));
    await waitFor(() => expect(composer().value).toBe("Pregunta repetida"));
    await userEvent.click(screen.getByRole("button", { name: "Enviar" }));
    await waitFor(() => expect(platformMethod("sendChatTurn")).toHaveBeenCalledTimes(2));
    const [first, second] = platformMethod("sendChatTurn").mock.calls;
    expect(first[7]).toBeTruthy();
    expect(second[7]).toBe(first[7]);
  });

  it("H24: Enter durante una composición IME no envía; el Enter posterior sí", async () => {
    await setup();
    platformMethod("sendChatTurn").mockReturnValue(new Promise(() => {}));
    fireEvent.change(composer(), { target: { value: "文字 en composición" } });
    fireEvent.keyDown(composer(), { key: "Enter", code: "Enter", isComposing: true, keyCode: 229 });
    await new Promise((resolve) => setTimeout(resolve, 50));
    expect(platformMethod("sendChatTurn")).not.toHaveBeenCalled();
    fireEvent.keyDown(composer(), { key: "Enter", code: "Enter", keyCode: 13 });
    await waitFor(() => expect(platformMethod("sendChatTurn")).toHaveBeenCalledTimes(1));
  });

  it("H22: una búsqueda que termina tarde no sustituye los resultados de la vigente", async () => {
    await setup();
    let finishFirst!: (value: unknown) => void;
    platformMethod("searchConversations").mockImplementation((query: string) =>
      query === "arb"
        ? new Promise((resolve) => (finishFirst = resolve))
        : Promise.resolve([
            { ...summaries[2], matchedMessageId: "m-1", snippet: "…el árbol de la plaza…" }
          ])
    );
    const search = screen.getByLabelText("Buscar conversaciones");
    await userEvent.type(search, "arb");
    await waitFor(() => expect(platformMethod("searchConversations")).toHaveBeenCalledTimes(1));
    await userEvent.type(search, "ol");
    await screen.findByText("…el árbol de la plaza…");
    await act(async () => finishFirst([{ ...summaries[0], snippet: "resultado antiguo" }]));
    expect(screen.queryByText("resultado antiguo")).toBeNull();
    expect(screen.getByText("…el árbol de la plaza…")).toBeDefined();
  });

  it("H01: lo que se aplicará al próximo mensaje se ve sin desplegar opciones", async () => {
    platformMethod("getEffectiveExecutionPolicy").mockResolvedValue({
      dataClassification: "confidential",
      maxCostUsd: 0.1,
      strategy: "single",
      preset: "fast",
      priority: 50,
      longContext: "fail",
      sources: { dataClassification: "gpt", maxCostUsd: "chat", routing: "gpt" },
      customGptName: "Asesor",
      destination: {
        host: "192.168.1.52:8765",
        localMachine: false,
        encrypted: false,
        warning: "La conexión con Broker AI (192.168.1.52:8765) no está cifrada."
      },
      notes: [
        "El GPT «Asesor» exige «Confidencial», más restrictivo que la opción del chat.",
        "El GPT admite hasta 1.00 USD, pero esta conversación limita el gasto a 0.10 USD; sube el límite del chat si quieres permitirlo."
      ]
    });
    await setup();
    const note = await screen.findByLabelText("Qué se aplicará al próximo mensaje");
    expect(note.textContent).toContain("exige «Confidencial»");
    expect(note.textContent).toContain("limita el gasto a 0.10 USD");
    expect(screen.getByText(/Respuesta directa · Confidencial · hasta 0.10 USD · Prioridad alta/)).toBeDefined();
    // Con 1024 px el panel de contexto empieza cerrado: la conversación
    // conserva el ancho. Se abre como lo haría la persona.
    expect(screen.queryByLabelText("Privacidad del próximo mensaje")).toBeNull();
    await userEvent.click(screen.getByRole("button", { name: "Contexto" }));
    const privacy = screen.getByLabelText("Privacidad del próximo mensaje");
    expect(privacy.textContent).toContain("La exige el GPT «Asesor»");
    expect(privacy.textContent).toContain("conexión sin cifrar");
    const routing = screen.getAllByRole("combobox").find((element) =>
      element.closest("label")?.textContent?.startsWith("Forma de responder")
    ) as HTMLSelectElement;
    expect(routing.disabled).toBe(true);
  });
});
