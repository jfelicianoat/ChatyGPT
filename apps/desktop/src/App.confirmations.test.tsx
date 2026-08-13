// @vitest-environment jsdom
/**
 * Pruebas de interfaz sobre las acciones sensibles.
 *
 * El 5 de agosto de 2026 se descubrió que cinco acciones enviaban
 * `confirmed: true` a Rust sin haber preguntado a nadie: la comprobación del
 * backend existía, pero el frontend la satisfacía por su cuenta. La prueba de
 * contrato en Python impide que vuelva a ocurrir leyendo el código fuente; esta
 * lo comprueba desde el otro lado, **ejecutando la interfaz**: monta la
 * aplicación, pulsa el botón real y verifica que cancelar no ejecuta nada.
 *
 * Es la diferencia entre «la confirmación está escrita» y «la confirmación
 * funciona». Ambas comprobaciones se complementan: un análisis estático no
 * detecta que la pregunta se ignore, y esta no detecta una ruta que nadie use.
 */

import { cleanup, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";

/** Respuestas por defecto que permiten montar la aplicación sin Broker real. */
const DEFAULTS: Record<string, unknown> = {
  bootstrap: {
    appVersion: "0.1.0",
    databasePath: "C:/pruebas/chatygpt.db",
    logPath: null,
    schemaVersion: 18,
    recoveredTasks: 0,
    recoveredAttachments: 0,
    recoveryItems: []
  },
  diagnoseBroker: {
    reachable: true,
    ready: true,
    baseUrl: "http://127.0.0.1:8765",
    contractVersion: "2.7",
    strategies: ["single"],
    presets: {},
    workLanes: ["inference"],
    agentSkills: [],
    latencyMs: 4,
    message: "Broker AI está listo"
  },
  getWindowsStartupStatus: {
    supported: true,
    enabled: false,
    credentialProtected: true,
    message: "Disponible"
  },
  getBrokerCredential: {
    source: "protected",
    protected: true,
    environmentPresent: false,
    message: "Credencial cifrada para tu cuenta de Windows."
  },
  listAuthorizedFolders: [
    {
      id: "folder-1",
      path: "D:/Exportaciones",
      displayName: "D:/Exportaciones",
      permissions: { write: true, purpose: "export" },
      grantedAt: "2026-08-01T10:00:00Z",
      revokedAt: null
    }
  ],
  getMemoryOverview: { enabled: false, items: [] },
  getLatestMemorySearch: null,
  getPerformanceReport: {
    sampleLimit: 200,
    totalSamples: 12,
    metrics: [
      {
        metric: "app_start",
        label: "Arranque de la aplicación",
        description: "Desde que la vista web empieza a cargar.",
        budgetMs: 2000,
        samples: 12,
        p50Ms: 900,
        p95Ms: 1400,
        maxMs: 1800,
        meetsBudget: true,
        lastRecordedAt: "2026-08-05T09:00:00Z"
      }
    ]
  }
};

/**
 * Doble de `platform` que registra cada llamada.
 *
 * Se usa un proxy para no tener que declarar las más de cien órdenes: las que
 * la prueba no necesita devuelven una lista vacía, que es lo que la interfaz
 * espera de casi todas ellas.
 */
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
  platform: new Proxy(
    {},
    {
      get: (_target, property: string) => platformMethod(property)
    }
  )
}));

vi.mock("@tauri-apps/api/webviewWindow", () => ({
  getCurrentWebviewWindow: () => ({
    onDragDropEvent: async () => () => undefined
  })
}));

import { App } from "./App";

/** Espera a que el arranque termine y la pantalla de Inicio esté montada. */
async function mountHome() {
  render(<App />);
  await waitFor(() => expect(platformMethod("bootstrap")).toHaveBeenCalled());
  await screen.findByRole("heading", { name: "Credencial de Broker AI" });
}

describe("navegación principal simplificada", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("cambia de área sin mezclar los paneles de cada destino", async () => {
    await mountHome();

    expect(screen.getByRole("button", { name: "Chats" }).getAttribute("aria-current"))
      .toBe("page");
    expect(document.querySelector(".home-chats")).not.toBeNull();

    await userEvent.click(screen.getByRole("button", { name: "Proyectos" }));

    expect(screen.getByRole("button", { name: "Proyectos" }).getAttribute("aria-current"))
      .toBe("page");
    expect(document.querySelector(".home-projects")).not.toBeNull();
    expect(screen.getByRole("heading", { name: "Proyectos" })).toBeDefined();

    await userEvent.click(screen.getByRole("button", { name: "Flujos" }));
    expect(screen.getByRole("button", { name: "Flujos" }).getAttribute("aria-current"))
      .toBe("page");
    expect(screen.getByRole("heading", { name: "Flujos" })).toBeDefined();
    expect(screen.getByRole("button", { name: "Crear flujo" })).toBeDefined();
  });

  it("permite elegir si los documentos nuevos describen sus imágenes", async () => {
    localStorage.removeItem("chatygpt.ingestion.describe-images.v1");
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Ajustes" }));
    const withImages = screen.getByRole("radio", { name: /Con imágenes/ });
    const withoutImages = screen.getByRole("radio", { name: /Sin imágenes/ });

    expect(withImages.getAttribute("aria-checked")).toBe("true");
    await userEvent.click(withoutImages);
    expect(withoutImages.getAttribute("aria-checked")).toBe("true");
    expect(localStorage.getItem("chatygpt.ingestion.describe-images.v1")).toBe("ignore");
    localStorage.removeItem("chatygpt.ingestion.describe-images.v1");
  });

  it("guarda un perfil de ejecución propio para un GPT personal", async () => {
    const savedGpt = {
      id: "gpt-profile",
      name: "Analista profundo",
      description: "Contrasta varias perspectivas",
      iconRef: "research" as const,
      instructions: "Analiza la entrada y justifica la conclusión.",
      conversationStarters: [],
      toolPermissions: { runCode: "deny", renameConversation: "deny" },
      preferredModel: null,
      executionProfile: {
        dataClassification: "confidential",
        strategy: "mixture_of_agents",
        preset: "slow",
        maxCostUsd: 0.75,
        longContext: "fail",
        priority: 50
      },
      defaultProjectId: null,
      versionNo: 1,
      createdAt: "2026-08-12T18:00:00Z",
      updatedAt: "2026-08-12T18:00:00Z"
    };
    platformMethod("createCustomGpt").mockResolvedValue(savedGpt);
    platformMethod("listCustomGpts")
      .mockResolvedValueOnce([])
      .mockResolvedValueOnce([savedGpt]);

    await mountHome();
    await userEvent.click(screen.getByRole("button", { name: "GPTs" }));
    const panel = screen.getByRole("heading", { name: "Mis GPTs" }).closest("section");
    expect(panel).not.toBeNull();
    const form = within(panel as HTMLElement);
    await userEvent.type(form.getByLabelText("Nombre"), savedGpt.name);
    await userEvent.click(form.getByRole("button", { name: "Icono Investigación" }));
    await userEvent.type(form.getByLabelText("Instrucciones"), savedGpt.instructions);
    await userEvent.click(form.getByRole("checkbox", { name: /Usar un perfil propio/ }));
    await userEvent.selectOptions(form.getByLabelText("Privacidad de los datos"), "confidential");
    await userEvent.selectOptions(form.getByLabelText("Forma de responder"), "mixture_of_agents");
    await userEvent.selectOptions(form.getByLabelText("Profundidad"), "slow");
    const cost = form.getByLabelText("Límite por respuesta (USD)");
    await userEvent.clear(cost);
    await userEvent.type(cost, "0.75");
    await userEvent.selectOptions(form.getByLabelText("Prioridad en la cola"), "50");
    await userEvent.click(form.getByRole("button", { name: "Crear GPT" }));

    await waitFor(() => expect(platformMethod("createCustomGpt")).toHaveBeenCalledWith(
      savedGpt.name,
      "",
      "research",
      savedGpt.instructions,
      [],
      { runCode: "deny", renameConversation: "deny" },
      null,
      null,
      savedGpt.executionProfile
    ));
    expect(await form.findByText(/GPT creado con su versión 1/)).toBeDefined();
  }, 10_000);

  it("prueba un GPT en un chat real que queda guardado", async () => {
    const gpt = {
      id: "gpt-test",
      name: "Analista documental",
      description: "Responde usando sus fuentes",
      iconRef: "research" as const,
      instructions: "Distingue hechos de inferencias.",
      conversationStarters: ["Resume la documentación disponible"],
      toolPermissions: { runCode: "deny", renameConversation: "deny" },
      preferredModel: null,
      executionProfile: null,
      defaultProjectId: "project-docs",
      versionNo: 2,
      createdAt: "2026-08-12T18:00:00Z",
      updatedAt: "2026-08-12T18:00:00Z"
    };
    const created = {
      id: "conversation-test",
      title: "Prueba · Analista documental",
      projectId: "project-docs",
      updatedAt: "2026-08-12T19:00:00Z"
    };
    const conversation = {
      ...created,
      customGptId: gpt.id,
      executionPreferences: {
        dataClassification: "public",
        strategy: "single",
        preset: "fast",
        maxCostUsd: 0.1,
        longContext: "fail",
        priority: 50
      },
      messages: [],
      researchRuns: []
    };
    const task = {
      id: "task-test",
      remoteStatus: "completed",
      localState: "completed",
      consecutivePollErrors: 0,
      result: { assistant_content: "Resultado de prueba" },
      progress: { phase: "completed" },
      pendingToolCalls: [],
      updatedAt: "2026-08-12T19:00:01Z"
    };
    platformMethod("listCustomGpts").mockResolvedValue([gpt]);
    platformMethod("createConversation").mockResolvedValue(created);
    platformMethod("setConversationCustomGpt").mockResolvedValue(conversation);
    platformMethod("getConversation").mockResolvedValue(conversation);
    platformMethod("listAttachments").mockResolvedValue([]);
    platformMethod("listProjectFiles").mockResolvedValue([]);
    platformMethod("sendChatTurn").mockResolvedValue(task);

    await mountHome();
    await userEvent.click(screen.getByRole("button", { name: "GPTs" }));
    await userEvent.click(await screen.findByRole("button", { name: "Probar" }));

    const question = screen.getByLabelText("Pregunta de prueba");
    expect((question as HTMLTextAreaElement).value).toBe(gpt.conversationStarters[0]);
    await userEvent.clear(question);
    await userEvent.type(question, "¿Qué conclusiones principales encuentras?");
    await userEvent.click(screen.getByRole("button", { name: "Crear chat y probar" }));

    await waitFor(() => expect(platformMethod("createConversation")).toHaveBeenCalledWith(
      "Prueba · Analista documental",
      "project-docs"
    ));
    expect(platformMethod("setConversationCustomGpt"))
      .toHaveBeenCalledWith("conversation-test", "gpt-test");
    await waitFor(() => expect(platformMethod("sendChatTurn")).toHaveBeenCalledWith(
      "conversation-test",
      "¿Qué conclusiones principales encuentras?",
      [],
      false,
      false,
      false,
      false
    ));
    expect(await screen.findByRole("heading", { name: created.title })).toBeDefined();
  }, 10_000);

  it("inserta el primer nodo útil entre Entrada y Resultado", async () => {
    const flow = {
      id: "flow-1",
      name: "Revisar informe",
      description: null,
      projectId: null,
      publishedVersionNo: null,
      nodeCount: 2,
      updatedAt: "2026-08-12T10:00:00Z",
      definition: {
        nodes: [
          { id: "input-1", kind: "input", label: "Entrada", x: 35, y: 55, attachmentIds: [] },
          { id: "result-1", kind: "result", label: "Resultado", x: 720, y: 55, attachmentIds: [] }
        ],
        edges: [{ id: "edge-direct", source: "input-1", target: "result-1" }]
      }
    };
    platformMethod("createWorkflow").mockResolvedValue(flow);
    platformMethod("listWorkflows").mockResolvedValue([flow]);
    platformMethod("getWorkflow").mockResolvedValue(flow);
    platformMethod("listWorkflowRuns").mockResolvedValue([]);
    platformMethod("saveWorkflow").mockImplementation(async (...args: unknown[]) => ({
      ...flow,
      nodeCount: (args[4] as typeof flow.definition).nodes.length,
      definition: args[4]
    }));

    await mountHome();
    await userEvent.click(screen.getByRole("button", { name: "Flujos" }));
    await userEvent.type(screen.getByLabelText("Nombre del flujo"), flow.name);
    await userEvent.click(screen.getByRole("button", { name: "Crear flujo" }));
    await screen.findByRole("button", { name: "Instrucción rápida" });
    await userEvent.click(screen.getByRole("button", { name: "Instrucción rápida" }));
    await userEvent.click(screen.getByRole("button", { name: "Guardar borrador" }));

    await waitFor(() => expect(platformMethod("saveWorkflow")).toHaveBeenCalled());
    const definition = platformMethod("saveWorkflow").mock.calls.at(-1)?.[4] as typeof flow.definition;
    const prompt = definition.nodes.find((node) => node.kind === "prompt");
    expect(prompt).toBeDefined();
    expect(definition.edges).toEqual(expect.arrayContaining([
      expect.objectContaining({ source: "input-1", target: prompt?.id }),
      expect.objectContaining({ source: prompt?.id, target: "result-1" })
    ]));
    expect(definition.edges).not.toContainEqual(expect.objectContaining({ id: "edge-direct" }));
  });

  it("guía hasta la credencial cuando un flujo recibe ADMIN_AUTH_REQUIRED", async () => {
    const flow = {
      id: "flow-auth",
      name: "Flujo protegido",
      description: null,
      projectId: null,
      publishedVersionNo: 1,
      nodeCount: 3,
      updatedAt: "2026-08-12T16:26:19Z",
      definition: {
        nodes: [
          { id: "input-auth", kind: "input", label: "Entrada", x: 35, y: 55, attachmentIds: [] },
          { id: "gpt-auth", kind: "custom_gpt", label: "Analizar", x: 360, y: 55, customGptId: "gpt-1", attachmentIds: [] },
          { id: "result-auth", kind: "result", label: "Resultado", x: 720, y: 55, attachmentIds: [] }
        ],
        edges: [
          { id: "edge-auth-1", source: "input-auth", target: "gpt-auth" },
          { id: "edge-auth-2", source: "gpt-auth", target: "result-auth" }
        ]
      }
    };
    const failedRun = {
      id: "run-auth",
      workflowId: flow.id,
      workflowVersionId: "version-auth",
      versionNo: 1,
      status: "failed",
      inputText: "Prueba",
      outputs: {},
      error: null,
      nodeRuns: [
        { id: "nr-input", nodeId: "input-auth", nodeKind: "input", nodeLabel: "Entrada", status: "completed", outputText: "Prueba", updatedAt: "2026-08-12T16:26:19Z" },
        { id: "nr-gpt", nodeId: "gpt-auth", nodeKind: "custom_gpt", nodeLabel: "Analizar", status: "failed", error: { message: "Broker AI devolvió HTTP 403: ADMIN_AUTH_REQUIRED" }, updatedAt: "2026-08-12T16:26:19Z" },
        { id: "nr-result", nodeId: "result-auth", nodeKind: "result", nodeLabel: "Resultado", status: "skipped", updatedAt: "2026-08-12T16:26:19Z" }
      ],
      completedAt: "2026-08-12T16:26:19Z",
      updatedAt: "2026-08-12T16:26:19Z"
    };
    platformMethod("listWorkflows").mockResolvedValue([flow]);
    platformMethod("getWorkflow").mockResolvedValue(flow);
    platformMethod("listWorkflowRuns").mockResolvedValue([failedRun]);

    await mountHome();
    await userEvent.click(screen.getByRole("button", { name: "Flujos" }));
    await userEvent.click(await screen.findByText("Historial reciente (1)"));
    await userEvent.click(screen.getByRole("button", { name: /Fallido/ }));

    expect(await screen.findByText("El Broker necesita una credencial nueva")).toBeDefined();
    expect(screen.getByRole("button", { name: "Ya la renové: reintentar" })).toBeDefined();
    await userEvent.click(screen.getByRole("button", { name: "Renovar credencial" }));

    await waitFor(() => {
      expect(screen.getByRole("button", { name: "Ajustes" }).getAttribute("aria-current")).toBe("page");
      expect(document.activeElement).toBe(screen.getByLabelText("Token administrativo"));
    });
  });

  it("permite revisar y aprobar una rama pausada", async () => {
    const flow = {
      id: "flow-approval",
      name: "Publicar informe",
      description: null,
      projectId: null,
      publishedVersionNo: 1,
      nodeCount: 3,
      updatedAt: "2026-08-12T17:00:00Z",
      definition: {
        nodes: [
          { id: "input-approval", kind: "input", label: "Entrada", x: 35, y: 55, attachmentIds: [] },
          { id: "approval", kind: "approval", label: "Revisión final", x: 360, y: 55, attachmentIds: [] },
          { id: "result-approval", kind: "result", label: "Resultado", x: 720, y: 55, attachmentIds: [] }
        ],
        edges: [
          { id: "ea-1", source: "input-approval", target: "approval" },
          { id: "ea-2", source: "approval", target: "result-approval" }
        ]
      }
    };
    const waitingRun = {
      id: "run-approval",
      workflowId: flow.id,
      workflowVersionId: "version-approval",
      versionNo: 1,
      status: "waiting_approval",
      inputText: "Informe confidencial",
      outputs: {},
      error: null,
      nodeRuns: [
        { id: "na-input", nodeId: "input-approval", nodeKind: "input", nodeLabel: "Entrada", status: "completed", outputText: "Informe confidencial", updatedAt: "2026-08-12T17:00:00Z" },
        { id: "na-approval", nodeId: "approval", nodeKind: "approval", nodeLabel: "Revisión final", status: "waiting_approval", inputText: "### Salida de Entrada\nInforme confidencial", updatedAt: "2026-08-12T17:00:00Z" },
        { id: "na-result", nodeId: "result-approval", nodeKind: "result", nodeLabel: "Resultado", status: "pending", updatedAt: "2026-08-12T17:00:00Z" }
      ],
      updatedAt: "2026-08-12T17:00:00Z"
    };
    platformMethod("listWorkflows").mockResolvedValue([flow]);
    platformMethod("getWorkflow").mockResolvedValue(flow);
    platformMethod("listWorkflowRuns").mockResolvedValue([waitingRun]);
    platformMethod("decideWorkflowApproval").mockResolvedValue({
      ...waitingRun,
      status: "completed",
      outputs: { Resultado: "Informe confidencial" },
      nodeRuns: waitingRun.nodeRuns.map((node) => ({ ...node, status: "completed" }))
    });

    await mountHome();
    await userEvent.click(screen.getByRole("button", { name: "Flujos" }));
    await userEvent.click(await screen.findByText("Historial reciente (1)"));
    await userEvent.click(screen.getByRole("button", { name: /Esperando aprobación/ }));

    expect((await screen.findAllByText("Revisión final")).length).toBeGreaterThanOrEqual(1);
    await userEvent.click(screen.getByText("Ver contenido pendiente"));
    expect(screen.getByText("Informe confidencial")).toBeDefined();
    await userEvent.click(screen.getByRole("button", { name: "Aprobar y continuar" }));

    await waitFor(() => expect(platformMethod("decideWorkflowApproval"))
      .toHaveBeenCalledWith("run-approval", "approval", true));
    expect(await screen.findByText("Rama aprobada. El flujo continúa desde este punto.")).toBeDefined();
  });
});

describe("arranque de los paneles de seguridad", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  /**
   * Defecto encontrado por esta prueba el 5 de agosto de 2026.
   *
   * La credencial y las carpetas autorizadas solo se cargaban desde
   * `reloadNavigation`, que se ejecuta después de una acción de la persona.
   * Al abrir la aplicación y no hacer nada, ambos paneles se quedaban
   * cargando indefinidamente, de modo que quien solo quisiera revisar su
   * credencial o revocar una carpeta no llegaba a verlas nunca.
   */
  it("carga credencial y carpetas autorizadas sin necesidad de actuar antes", async () => {
    await mountHome();

    await waitFor(() =>
      expect(platformMethod("getBrokerCredential")).toHaveBeenCalled()
    );
    expect(platformMethod("listAuthorizedFolders")).toHaveBeenCalled();

    // No basta con haberlas pedido: deben estar visibles.
    expect(await screen.findByRole("button", { name: "Retirar" })).toBeDefined();
    expect(await screen.findByRole("button", { name: "Revocar" })).toBeDefined();
    expect(screen.queryByText("Comprobando credencial…")).toBeNull();
    expect(screen.queryByText("Cargando permisos…")).toBeNull();
  });
});

describe("acciones sensibles en la interfaz", () => {
  beforeEach(() => {
    callLog.clear();
    vi.restoreAllMocks();
  });

  afterEach(() => {
    cleanup();
  });

  it("no retira la credencial si la persona cancela la confirmación", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Retirar" }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(platformMethod("clearBrokerCredential")).not.toHaveBeenCalled();
  });

  it("retira la credencial solo después de aceptar", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Retirar" }));

    expect(confirm).toHaveBeenCalledOnce();
    await waitFor(() =>
      expect(platformMethod("clearBrokerCredential")).toHaveBeenCalledOnce()
    );
    // La confirmación precede a la llamada, no al revés.
    expect(confirm.mock.invocationCallOrder[0]).toBeLessThan(
      platformMethod("clearBrokerCredential").mock.invocationCallOrder[0]
    );
  });

  it("no revoca una carpeta autorizada si la persona cancela", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Revocar" }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(platformMethod("revokeAuthorizedFolder")).not.toHaveBeenCalled();
  });

  it("no vacía las mediciones de rendimiento si la persona cancela", async () => {
    const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
    await mountHome();

    await userEvent.click(screen.getByRole("button", { name: "Vaciar mediciones" }));

    expect(confirm).toHaveBeenCalledOnce();
    expect(platformMethod("clearPerformanceSamples")).not.toHaveBeenCalled();
  });
});
