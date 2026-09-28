// @vitest-environment jsdom
/**
 * H13 (auditoría 28-sep-2026): cambiar de flujo, de área o cerrar la ventana
 * ya no descarta en silencio lo que no se guardó.
 *
 * La reproducción original modificaba la descripción, abría otro flujo y al
 * volver encontraba la versión guardada sin haber preguntado nada.
 */

import { cleanup, fireEvent, render, screen } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, beforeEach, expect, it, vi } from "vitest";

import type { WorkflowView } from "./domain";

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
import { WORKFLOW_DRAFT_STORAGE_KEY } from "./workflowDrafts";

const first: WorkflowView = {
  id: "w1",
  name: "Flujo uno",
  description: "Descripción guardada",
  projectId: null,
  publishedVersionNo: 1,
  nodeCount: 2,
  updatedAt: "2026-09-28",
  definition: {
    nodes: [
      { id: "input", kind: "input", label: "Entrada", x: 20, y: 50, attachmentIds: [] },
      { id: "result", kind: "result", label: "Resultado", x: 500, y: 50, attachmentIds: [] }
    ],
    edges: [{ id: "edge", source: "input", target: "result" }]
  }
};
const second: WorkflowView = { ...first, id: "w2", name: "Flujo dos" };

const studio = () => (
  <WorkflowStudio
    projects={[]}
    customGpts={[]}
    onOpenBrokerCredential={() => {}}
    onOpenAutomations={() => {}}
  />
);

beforeEach(() => {
  calls.clear();
  window.localStorage.clear();
  method("listWorkflows").mockResolvedValue([first, second]);
  method("getWorkflow").mockImplementation(async (id: string) => (id === "w1" ? first : second));
  method("listWorkflowRuns").mockResolvedValue([]);
});

afterEach(() => {
  vi.restoreAllMocks();
  cleanup();
});

it("conserva los cambios sin guardar al abrir otro flujo y volver", async () => {
  render(studio());
  await screen.findByDisplayValue("Flujo uno");
  fireEvent.change(screen.getByLabelText("Descripción"), {
    target: { value: "Trabajo sin guardar" }
  });
  await userEvent.click(screen.getByRole("button", { name: /Flujo dos/ }));
  await screen.findByDisplayValue("Flujo dos");
  expect(screen.getByText(/Los cambios sin guardar de «Flujo uno» se conservan/)).toBeDefined();

  await userEvent.click(screen.getByRole("button", { name: /Flujo uno/ }));
  await screen.findByDisplayValue("Flujo uno");
  expect((screen.getByLabelText("Descripción") as HTMLInputElement).value).toBe(
    "Trabajo sin guardar"
  );
  expect(screen.getByText(/Cambios sin guardar recuperados/)).toBeDefined();
  // Nada se escribió en la versión guardada sin que la persona lo pidiera.
  expect(method("saveWorkflow")).not.toHaveBeenCalled();
});

it("sobrevive a cambiar de área o cerrar la ventana, y se puede descartar", async () => {
  const { unmount } = render(studio());
  await screen.findByDisplayValue("Flujo uno");
  fireEvent.change(screen.getByLabelText("Descripción"), {
    target: { value: "Idea a medias" }
  });
  unmount();
  expect(window.localStorage.getItem(WORKFLOW_DRAFT_STORAGE_KEY)).toContain("Idea a medias");

  render(studio());
  await screen.findByDisplayValue("Idea a medias");
  vi.spyOn(window, "confirm").mockReturnValue(true);
  await userEvent.click(screen.getByRole("button", { name: "Descartar cambios" }));
  await screen.findByDisplayValue("Descripción guardada");
  expect(window.localStorage.getItem(WORKFLOW_DRAFT_STORAGE_KEY)).toBeNull();
});

it("guardar retira el borrador local; un fallo al guardar lo conserva", async () => {
  method("saveWorkflow").mockRejectedValueOnce(new Error("disco lleno"));
  render(studio());
  await screen.findByDisplayValue("Flujo uno");
  fireEvent.change(screen.getByLabelText("Descripción"), { target: { value: "Versión nueva" } });
  await userEvent.click(screen.getByRole("button", { name: /Guardar borrador/ }));
  await screen.findByText("disco lleno");
  expect(window.localStorage.getItem(WORKFLOW_DRAFT_STORAGE_KEY)).toContain("Versión nueva");

  method("saveWorkflow").mockResolvedValueOnce({ ...first, description: "Versión nueva" });
  method("getWorkflow").mockResolvedValue({ ...first, description: "Versión nueva" });
  await userEvent.click(screen.getByRole("button", { name: /Guardar borrador/ }));
  await screen.findByText("Borrador guardado.");
  expect(window.localStorage.getItem(WORKFLOW_DRAFT_STORAGE_KEY)).toBeNull();
});
