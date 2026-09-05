/**
 * Estado y acciones de la memoria personal.
 *
 * El interruptor global, las fichas, la edicion en linea y la busqueda
 * semantica vivian sueltos dentro de `App.tsx`. Aqui viajan juntos, y la
 * tarjeta recibe `ReturnType<typeof useMemoria>` en lugar de treinta props.
 */
import { useState } from "react";

import {
  memoryUpdateNotice,
  type Loadable,
  type MemoryItemView,
  type MemoryOverview,
  type MemorySearchView
} from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";

/** Borrador de la ficha que se esta editando en linea. */
export type MemoryEditDraft = {
  content: string;
  category: MemoryItemView["category"];
  projectId: string;
  sensitive: boolean;
};

type Opciones = {
  /** Relee la auditoria: cada cambio de memoria deja rastro. */
  refreshAuditEvents: () => Promise<void>;
};

export function useMemoria({ refreshAuditEvents }: Opciones) {
  const [memory, setMemory] = useState<Loadable<MemoryOverview>>({ state: "loading" });
  const [memoryDraft, setMemoryDraft] = useState("");
  const [memoryCategory, setMemoryCategory] = useState<"preference" | "instruction" | "fact">("preference");
  const [memoryProjectId, setMemoryProjectId] = useState("global");
  const [memorySensitive, setMemorySensitive] = useState(false);
  const [memoryBusy, setMemoryBusy] = useState(false);
  const [memoryEditingId, setMemoryEditingId] = useState<string | null>(null);
  const [memoryEditDraft, setMemoryEditDraft] = useState<MemoryEditDraft | null>(null);
  const [memoryEditError, setMemoryEditError] = useState<string | null>(null);
  const [memoryNotice, setMemoryNotice] = useState<string | null>(null);
  const [memorySearchQuery, setMemorySearchQuery] = useState("");
  const [memorySearchProjectId, setMemorySearchProjectId] = useState("global");
  const [memorySearch, setMemorySearch] = useState<Loadable<MemorySearchView> | null>(null);

  const toggleMemory = async () => {
    if (memory.state !== "ready") return;
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.setMemoryEnabled(!memory.value.enabled) });
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const createMemory = async () => {
    if (!memoryDraft.trim()) return;
    setMemoryBusy(true);
    try {
      const overview = await platform.createMemoryItem(
        memoryDraft,
        memoryCategory,
        memorySensitive ? "sensitive" : "normal",
        memoryProjectId === "global" ? undefined : memoryProjectId
      );
      setMemory({ state: "ready", value: overview });
      setMemoryDraft("");
      setMemorySensitive(false);
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const toggleMemoryItem = async (memoryId: string, enabled: boolean) => {
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.setMemoryItemEnabled(memoryId, enabled) });
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const beginMemoryEdit = (item: MemoryItemView) => {
    setMemoryEditingId(item.id);
    setMemoryEditDraft({
      content: item.content,
      category: item.category,
      projectId: item.projectId ?? "global",
      sensitive: item.sensitivity === "sensitive"
    });
    setMemoryEditError(null);
    setMemoryNotice(null);
  };

  const cancelMemoryEdit = () => {
    setMemoryEditingId(null);
    setMemoryEditDraft(null);
    setMemoryEditError(null);
  };

  const saveMemoryEdit = async () => {
    if (!memoryEditingId || !memoryEditDraft?.content.trim()) return;
    const editedId = memoryEditingId;
    const previous = memory.state === "ready"
      ? memory.value.items.find((item) => item.id === editedId)
      : undefined;
    const contentChanged = previous?.content !== memoryEditDraft.content.trim();
    setMemoryBusy(true);
    setMemoryEditError(null);
    try {
      const overview = await platform.updateMemoryItem(
        editedId,
        memoryEditDraft.content,
        memoryEditDraft.category,
        memoryEditDraft.sensitive ? "sensitive" : "normal",
        memoryEditDraft.projectId === "global" ? undefined : memoryEditDraft.projectId
      );
      setMemory({ state: "ready", value: overview });
      cancelMemoryEdit();
      setMemoryNotice(memoryUpdateNotice(contentChanged));
      requestAnimationFrame(() => {
        document
          .querySelector<HTMLButtonElement>(
            `[data-memory-id="${editedId}"] .memory-edit-button`
          )
          ?.focus();
      });
      await refreshAuditEvents();
    } catch (error) {
      setMemoryEditError(describeError(error));
    } finally {
      setMemoryBusy(false);
    }
  };

  const removeMemoryItem = async (memoryId: string) => {
    if (!window.confirm("¿Eliminar este recuerdo de forma permanente?")) return;
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.deleteMemoryItem(memoryId) });
      await refreshAuditEvents();
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };

  const reindexMemoryItem = async (memoryId: string) => {
    setMemoryBusy(true);
    try {
      setMemory({ state: "ready", value: await platform.reindexMemoryItem(memoryId) });
    } catch (error) {
      setMemory({ state: "error", message: describeError(error) });
    } finally {
      setMemoryBusy(false);
    }
  };


  const runMemorySearch = async () => {
    if (!memorySearchQuery.trim()) return;
    setMemorySearch({ state: "loading" });
    try {
      const result = await platform.startMemorySearch(
        memorySearchQuery,
        memorySearchProjectId === "global" ? undefined : memorySearchProjectId
      );
      setMemorySearch({ state: "ready", value: result });
    } catch (error) {
      setMemorySearch({ state: "error", message: describeError(error) });
    }
  };

  return {
    memory,
    setMemory,
    memoryDraft,
    setMemoryDraft,
    memoryCategory,
    setMemoryCategory,
    memoryProjectId,
    setMemoryProjectId,
    memorySensitive,
    setMemorySensitive,
    memoryBusy,
    setMemoryBusy,
    memoryEditingId,
    setMemoryEditingId,
    memoryEditDraft,
    setMemoryEditDraft,
    memoryEditError,
    setMemoryEditError,
    memoryNotice,
    setMemoryNotice,
    memorySearchQuery,
    setMemorySearchQuery,
    memorySearchProjectId,
    setMemorySearchProjectId,
    memorySearch,
    setMemorySearch,
    toggleMemory,
    createMemory,
    toggleMemoryItem,
    beginMemoryEdit,
    cancelMemoryEdit,
    saveMemoryEdit,
    removeMemoryItem,
    reindexMemoryItem,
    runMemorySearch
  };
}
