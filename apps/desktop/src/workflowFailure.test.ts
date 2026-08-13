import { expect, it } from "vitest";

import type { WorkflowRunView } from "./domain";
import { describeWorkflowFailure } from "./workflowFailure";

const failedRun = (message: string, runError = true): WorkflowRunView => ({
  id: "run-1",
  workflowId: "workflow-1",
  workflowVersionId: "version-1",
  versionNo: 1,
  status: "failed",
  inputText: "entrada",
  outputs: {},
  error: runError ? { message, node_id: "gpt", node_label: "Analista" } : null,
  nodeRuns: [{
    id: "node-run-1",
    nodeId: "gpt",
    nodeKind: "custom_gpt",
    nodeLabel: "Analista",
    status: "failed",
    error: { message },
    updatedAt: "2026-08-12T12:00:00Z"
  }],
  updatedAt: "2026-08-12T12:00:00Z"
});

it("explica una credencial caducada y conserva el nodo responsable", () => {
  const failure = describeWorkflowFailure(failedRun("Broker AI devolvió HTTP 403: ADMIN_AUTH_REQUIRED"));

  expect(failure?.kind).toBe("credential");
  expect(failure?.title).toBe("El Broker necesita una credencial nueva");
  expect(failure?.failedNodes[0]?.label).toBe("Analista");
});

it("distingue un Broker inaccesible de un fallo del nodo", () => {
  const failure = describeWorkflowFailure(failedRun("Broker AI no está accesible: error sending request"));

  expect(failure?.kind).toBe("connection");
  expect(failure?.title).toBe("Broker AI no está disponible");
});

it("recupera el error del nodo en ejecuciones antiguas sin error general", () => {
  const failure = describeWorkflowFailure(failedRun("CONTRACT_VALIDATION_FAILED: campo incorrecto", false));

  expect(failure?.kind).toBe("contract");
  expect(failure?.technicalMessage).toContain("CONTRACT_VALIDATION_FAILED");
});
