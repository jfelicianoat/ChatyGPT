// @vitest-environment jsdom

import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { CustomGptView, ProjectSummary, WorkflowRunView, WorkflowView } from "./domain";

const calls = new Map<string, ReturnType<typeof vi.fn>>();
const method = (name: string) => {
  let mock = calls.get(name);
  if (!mock) {
    mock = vi.fn(async () => []);
    calls.set(name, mock);
  }
  return mock;
};

vi.mock("./platform", () => ({
  platform: new Proxy({}, { get: (_target, property: string) => method(property) })
}));

import { WorkflowStudio } from "./WorkflowStudio";

const workflow: WorkflowView = {
  id: "workflow-1",
  name: "Informe diario",
  description: "Resume la actividad",
  projectId: null,
  publishedVersionNo: 3,
  nodeCount: 2,
  updatedAt: "2026-08-12T12:00:00Z",
  definition: {
    nodes: [
      { id: "input", kind: "input", label: "Entrada", x: 20, y: 50, attachmentIds: [] },
      { id: "result", kind: "result", label: "Resultado", x: 500, y: 50, attachmentIds: [] }
    ],
    edges: [{ id: "edge", source: "input", target: "result" }]
  }
};

beforeEach(() => {
  calls.clear();
  vi.spyOn(window, "confirm").mockReturnValue(true);
  method("listWorkflows").mockResolvedValue([workflow]);
  method("getWorkflow").mockResolvedValue(workflow);
  method("listWorkflowRuns").mockResolvedValue([]);
});

afterEach(() => {
  vi.restoreAllMocks();
  cleanup();
});

it("programa la versión publicada con la entrada visible y ofrece abrir Automatizaciones", async () => {
  const user = userEvent.setup();
  const openAutomations = vi.fn();
  render(
    <WorkflowStudio
      projects={[]}
      customGpts={[]}
      onOpenBrokerCredential={() => undefined}
      onOpenAutomations={openAutomations}
    />
  );

  await screen.findByDisplayValue("Informe diario");
  await user.type(screen.getByPlaceholderText("Escribe la entrada inicial del flujo"), "Resume los nuevos datos");
  await user.click(screen.getByRole("button", { name: "Programar" }));
  expect(screen.getByText(/Usará la versión 3/)).toBeTruthy();
  await user.clear(screen.getByLabelText("Primera ejecución"));
  await user.type(screen.getByLabelText("Primera ejecución"), "2099-01-02T10:30");
  await user.selectOptions(screen.getByLabelText("Repetición"), "daily");
  await user.click(screen.getByRole("button", { name: "Confirmar programación" }));

  await waitFor(() => expect(method("createScheduledWorkflow")).toHaveBeenCalledTimes(1));
  expect(method("createScheduledWorkflow")).toHaveBeenCalledWith(
    "Informe diario",
    "workflow-1",
    "Resume los nuevos datos",
    expect.stringMatching(/^2099-01-02T/),
    expect.any(String),
    "daily"
  );
  await user.click(screen.getByRole("button", { name: /Ver en Automatizaciones/ }));
  expect(openAutomations).toHaveBeenCalledTimes(1);
});

it("muestra el nodo y la solución cuando Broker AI rechaza la credencial", async () => {
  const user = userEvent.setup();
  const openCredential = vi.fn();
  const failedRun: WorkflowRunView = {
    id: "run-failed",
    workflowId: workflow.id,
    workflowVersionId: "version-3",
    versionNo: 3,
    status: "failed",
    inputText: "Procesa esta entrada",
    outputs: {},
    error: {
      message: "Broker AI devolvió HTTP 403: ADMIN_AUTH_REQUIRED",
      node_id: "analyst",
      node_label: "Analista"
    },
    nodeRuns: [{
      id: "node-run-failed",
      nodeId: "analyst",
      nodeKind: "custom_gpt",
      nodeLabel: "Analista",
      status: "failed",
      error: { message: "Broker AI devolvió HTTP 403: ADMIN_AUTH_REQUIRED" },
      updatedAt: "2026-08-12T12:01:00Z"
    }],
    updatedAt: "2026-08-12T12:01:00Z"
  };
  method("runWorkflow").mockResolvedValue(failedRun);

  render(
    <WorkflowStudio
      projects={[]}
      customGpts={[]}
      onOpenBrokerCredential={openCredential}
      onOpenAutomations={() => undefined}
    />
  );

  await screen.findByDisplayValue("Informe diario");
  await user.type(screen.getByPlaceholderText("Escribe la entrada inicial del flujo"), "Procesa esta entrada");
  await user.click(screen.getByRole("button", { name: "Ejecutar flujo" }));

  expect(await screen.findByText("El Broker necesita una credencial nueva")).toBeTruthy();
  expect(screen.getByText(/se detuvo en «Analista»/)).toBeTruthy();
  await user.click(screen.getByRole("button", { name: "Renovar credencial" }));
  expect(openCredential).toHaveBeenCalledTimes(1);
});

it("explica el conocimiento propio que quedará autorizado al publicar un nodo GPT", async () => {
  const project: ProjectSummary = {
    id: "project-analysis",
    name: "Análisis",
    instructions: "Distingue hechos de hipótesis.",
    conversationCount: 0,
    updatedAt: "2026-08-12T12:00:00Z"
  };
  const gpt: CustomGptView = {
    id: "gpt-analyst",
    name: "Analista",
    iconRef: "research",
    instructions: "Analiza con rigor.",
    conversationStarters: [],
    toolPermissions: { runCode: "deny", renameConversation: "deny", readAuthorizedFolders: "deny", modifyAuthorizedFiles: "deny", createScheduledTasks: "deny", callExternalApis: "deny" },
    apiActions: [],
    contextProfile: "balanced",
    preferredModel: null,
    executionProfile: null,
    defaultProjectId: null,
    versionNo: 1,
    createdAt: "2026-08-12T12:00:00Z",
    updatedAt: "2026-08-12T12:00:00Z"
  };
  const workflowWithGpt: WorkflowView = {
    ...workflow,
    projectId: project.id,
    definition: {
      nodes: [
        workflow.definition.nodes[0],
        { id: "analyst", kind: "custom_gpt", label: "Analista", x: 260, y: 50, customGptId: gpt.id, attachmentIds: [] },
        workflow.definition.nodes[1]
      ],
      edges: [
        { id: "edge-1", source: "input", target: "analyst" },
        { id: "edge-2", source: "analyst", target: "result" }
      ]
    }
  };
  method("getWorkflow").mockResolvedValue(workflowWithGpt);
  method("getCustomGptKnowledge").mockResolvedValue([
    { id: "memory-1", enabled: true },
    { id: "memory-2", enabled: false }
  ]);
  method("listCustomGptFiles").mockResolvedValue([
    { id: "file-1", ingestionStatus: "ready" },
    { id: "file-2", ingestionStatus: "converting" }
  ]);
  method("getProjectKnowledge").mockResolvedValue({
    project,
    files: [],
    fileUsages: [],
    memories: [
      { id: "project-memory-1", enabled: true },
      { id: "project-memory-2", enabled: false }
    ],
    memoryEnabled: true
  });

  render(
    <WorkflowStudio
      projects={[project]}
      customGpts={[gpt]}
      onOpenBrokerCredential={() => undefined}
      onOpenAutomations={() => undefined}
    />
  );

  await screen.findByDisplayValue("Informe diario");
  const node = document.querySelector(".workflow-node-custom_gpt") as HTMLElement;
  expect(node).toBeTruthy();
  expect(within(node).getByText("⌕")).toBeTruthy();
  fireEvent.keyDown(node, { key: "Enter" });

  expect(await screen.findByText("1 dato(s) y 1 archivo(s) preparado(s)")).toBeTruthy();
  expect(screen.getAllByText(/Los cambios nuevos requieren volver a publicar/)).toHaveLength(2);
  expect(await screen.findByText("Instrucciones incluidas · 1 recuerdo(s) activo(s)")).toBeTruthy();
});
