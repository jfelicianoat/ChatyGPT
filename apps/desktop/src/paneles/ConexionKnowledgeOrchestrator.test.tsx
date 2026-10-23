// @vitest-environment jsdom
/**
 * El plazo de una pregunta con Knowledge Orchestrator se lee y se guarda en
 * minutos, pero viaja en segundos: con 600 s fijos las preguntas grandes morían.
 */

import { cleanup, render, screen, waitFor } from "@testing-library/react";
import userEvent from "@testing-library/user-event";
import { afterEach, expect, it, vi } from "vitest";

const platformMock = vi.hoisted(() => ({
  getKnowledgeOrchestratorStatus: vi.fn(async () => ({
    baseUrl: "http://127.0.0.1:8766/api/v1",
    credentialStored: true,
    reachable: true,
    message: "Conectado."
  })),
  getKnowledgeOrchestratorTimeout: vi.fn(async () => 1_800),
  setKnowledgeOrchestratorTimeout: vi.fn(async (seconds: number) => seconds)
}));

vi.mock("../platform", () => ({ platform: platformMock }));

import { ConexionKnowledgeOrchestrator } from "./ConexionKnowledgeOrchestrator";

afterEach(cleanup);

it("muestra el plazo en minutos y lo guarda en segundos", async () => {
  render(<ConexionKnowledgeOrchestrator />);
  const input = await screen.findByLabelText(/Plazo de una pregunta/);
  await waitFor(() => expect(input).toHaveProperty("value", "30"));
  const guardar = screen.getByRole("button", { name: "Guardar plazo" });
  expect(guardar).toHaveProperty("disabled", true);

  await userEvent.clear(input);
  await userEvent.type(input, "60");
  await userEvent.click(guardar);

  expect(platformMock.setKnowledgeOrchestratorTimeout).toHaveBeenCalledWith(3_600);
  await waitFor(() => expect(guardar).toHaveProperty("disabled", true));
});
