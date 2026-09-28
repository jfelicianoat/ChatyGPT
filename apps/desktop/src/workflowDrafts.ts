/**
 * Cambios sin guardar del editor de flujos (auditoría 28-sep-2026, H13).
 *
 * Abrir otro flujo, cambiar de área o cerrar la ventana desmontaba el editor y
 * descartaba el trabajo sin preguntar. Ahora cada edición se copia aquí en
 * cuanto ocurre; al volver a ese flujo se recupera y se avisa. La versión
 * guardada y la publicada no se tocan hasta que la persona guarda.
 */

import type { WorkflowDefinition, WorkflowView } from "./domain";

export type WorkflowLocalDraft = {
  name: string;
  description: string;
  projectId: string | null;
  definition: WorkflowDefinition;
  savedAt: number;
};

export const WORKFLOW_DRAFT_STORAGE_KEY = "chatygpt.flujos.borradores.v1";

type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;

const storage = (): StorageLike | null => {
  try {
    return typeof window !== "undefined" ? window.localStorage : null;
  } catch {
    return null;
  }
};

const readAll = (target: StorageLike | null): Record<string, WorkflowLocalDraft> => {
  if (!target) return {};
  try {
    const parsed = JSON.parse(target.getItem(WORKFLOW_DRAFT_STORAGE_KEY) ?? "{}") as unknown;
    return parsed && typeof parsed === "object" && !Array.isArray(parsed)
      ? (parsed as Record<string, WorkflowLocalDraft>)
      : {};
  } catch {
    return {};
  }
};

const writeAll = (target: StorageLike | null, drafts: Record<string, WorkflowLocalDraft>) => {
  if (!target) return;
  try {
    if (Object.keys(drafts).length === 0) {
      target.removeItem(WORKFLOW_DRAFT_STORAGE_KEY);
    } else {
      target.setItem(WORKFLOW_DRAFT_STORAGE_KEY, JSON.stringify(drafts));
    }
  } catch {
    // Sin almacenamiento, los cambios viven mientras el editor siga abierto.
  }
};

export const loadWorkflowDraft = (
  workflowId: string,
  target: StorageLike | null = storage()
): WorkflowLocalDraft | null => {
  const draft = readAll(target)[workflowId];
  return draft && draft.definition && Array.isArray(draft.definition.nodes) ? draft : null;
};

export const saveWorkflowDraft = (
  workflow: WorkflowView,
  target: StorageLike | null = storage()
): void => {
  const drafts = readAll(target);
  drafts[workflow.id] = {
    name: workflow.name,
    description: workflow.description ?? "",
    projectId: workflow.projectId ?? null,
    definition: workflow.definition,
    savedAt: Date.now()
  };
  writeAll(target, drafts);
};

export const discardWorkflowDraft = (
  workflowId: string,
  target: StorageLike | null = storage()
): void => {
  const drafts = readAll(target);
  if (!(workflowId in drafts)) return;
  delete drafts[workflowId];
  writeAll(target, drafts);
};

/** Aplica un borrador local sobre la versión guardada del flujo. */
export const applyWorkflowDraft = (
  workflow: WorkflowView,
  draft: WorkflowLocalDraft
): WorkflowView => ({
  ...workflow,
  name: draft.name,
  description: draft.description,
  projectId: draft.projectId,
  definition: draft.definition
});
