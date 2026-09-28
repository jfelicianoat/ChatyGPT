import { describe, expect, it } from "vitest";

import {
  DRAFT_RETENTION_MS,
  DRAFT_STORAGE_KEY,
  MAX_STORED_DRAFTS,
  draftText,
  loadDrafts,
  operationFor,
  pruneDrafts,
  saveDrafts,
  withDraftText,
  withRestoredDraft,
  withoutDraft,
  type DraftStore
} from "./borradores";

const memoryStorage = () => {
  const values = new Map<string, string>();
  return {
    getItem: (key: string) => values.get(key) ?? null,
    setItem: (key: string, value: string) => void values.set(key, value),
    removeItem: (key: string) => void values.delete(key),
    values
  };
};

describe("borradores por conversación", () => {
  it("cada conversación conserva su propio texto", () => {
    let store: DraftStore = {};
    store = withDraftText(store, "A", "Texto de A");
    store = withDraftText(store, "B", "Texto de B");
    expect(draftText(store, "A")).toBe("Texto de A");
    expect(draftText(store, "B")).toBe("Texto de B");
    expect(draftText(store, "C")).toBe("");
  });

  it("vaciar el texto elimina el borrador", () => {
    const store = withDraftText(withDraftText({}, "A", "algo"), "A", "   ");
    expect(store).toEqual({});
  });

  it("repetir el mismo texto reutiliza la operación; cambiarlo crea otra", () => {
    const first = operationFor({}, "A", "Pregunta");
    const retry = operationFor(first.store, "A", "Pregunta");
    expect(retry.operationId).toBe(first.operationId);
    const edited = operationFor(first.store, "A", "Pregunta corregida");
    expect(edited.operationId).not.toBe(first.operationId);
    expect(first.operationId).toMatch(/^[A-Za-z0-9_-]+$/);
  });

  it("un envío rechazado devuelve el texto a su conversación con la misma operación", () => {
    const restored = withRestoredDraft({}, "A", "Pregunta", "op-1");
    expect(operationFor(restored, "A", "Pregunta").operationId).toBe("op-1");
    expect(withoutDraft(restored, "A")).toEqual({});
  });

  it("sobrevive a un reinicio y respeta la política de retención", () => {
    const storage = memoryStorage();
    const now = Date.now();
    const store: DraftStore = {
      reciente: { text: "sigue", operationId: "a", operationText: "sigue", updatedAt: now },
      antiguo: {
        text: "caducado",
        operationId: "b",
        operationText: "caducado",
        updatedAt: now - DRAFT_RETENTION_MS - 1
      }
    };
    saveDrafts(store, storage);
    const loaded = loadDrafts(storage);
    expect(Object.keys(loaded)).toEqual(["reciente"]);

    saveDrafts({}, storage);
    expect(storage.values.has(DRAFT_STORAGE_KEY)).toBe(false);
  });

  it("guarda como mucho los borradores más recientes", () => {
    const store: DraftStore = {};
    for (let index = 0; index < MAX_STORED_DRAFTS + 10; index += 1) {
      store[`c${index}`] = {
        text: `t${index}`,
        operationId: `o${index}`,
        operationText: `t${index}`,
        updatedAt: 1_000 + index
      };
    }
    const pruned = pruneDrafts(store, 2_000);
    expect(Object.keys(pruned)).toHaveLength(MAX_STORED_DRAFTS);
    expect(pruned.c0).toBeUndefined();
    expect(pruned[`c${MAX_STORED_DRAFTS + 9}`]).toBeDefined();
  });

  it("un almacenamiento dañado no rompe la aplicación", () => {
    const storage = memoryStorage();
    storage.setItem(DRAFT_STORAGE_KEY, "{no es json");
    expect(loadDrafts(storage)).toEqual({});
    expect(loadDrafts(null)).toEqual({});
  });
});
