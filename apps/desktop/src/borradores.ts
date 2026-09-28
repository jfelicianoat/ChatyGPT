/**
 * Borradores del compositor, uno por conversación (auditoría 28-sep-2026, H10).
 *
 * Antes había un único texto en `App`: lo escrito en A aparecía en B listo
 * para enviarse allí. Ahora cada conversación conserva el suyo, también tras
 * cerrar la aplicación.
 *
 * Cada borrador lleva un identificador de operación ligado a su texto exacto
 * (H12). Si un envío se repite con el mismo texto porque no se supo si el
 * primero llegó, Rust devuelve la tarea ya creada en lugar de duplicarla; si
 * la persona cambia el texto, es otra operación.
 *
 * Retención: los borradores viven en el almacenamiento local de la ventana
 * (el mismo perfil de WebView2 que la aplicación, en este equipo). Se borran
 * al enviarse y caducan a los 30 días sin tocarse; como mucho se guardan 200.
 */

export type ConversationDraft = {
  text: string;
  /** Identificador de la operación de envío para este texto. */
  operationId: string;
  /** Texto para el que se generó `operationId`. */
  operationText: string;
  updatedAt: number;
};

export type DraftStore = Record<string, ConversationDraft>;

export const DRAFT_STORAGE_KEY = "chatygpt.borradores.v1";
export const DRAFT_RETENTION_MS = 30 * 24 * 60 * 60 * 1000;
export const MAX_STORED_DRAFTS = 200;
const MAX_DRAFT_CHARACTERS = 200_000;

type StorageLike = Pick<Storage, "getItem" | "setItem" | "removeItem">;

const browserStorage = (): StorageLike | null => {
  try {
    return typeof window !== "undefined" ? window.localStorage : null;
  } catch {
    return null;
  }
};

export const newOperationId = (): string => {
  const random =
    typeof crypto !== "undefined" && typeof crypto.randomUUID === "function"
      ? crypto.randomUUID()
      : `${Date.now().toString(36)}-${Math.random().toString(36).slice(2)}`;
  return random.replace(/[^A-Za-z0-9_-]/g, "").slice(0, 64);
};

/** Quita lo caducado y lo que excede el máximo, conservando lo más reciente. */
export const pruneDrafts = (store: DraftStore, now = Date.now()): DraftStore => {
  const entries = Object.entries(store)
    .filter(
      ([, draft]) =>
        draft.text.trim().length > 0 && now - draft.updatedAt <= DRAFT_RETENTION_MS
    )
    .sort(([, left], [, right]) => right.updatedAt - left.updatedAt)
    .slice(0, MAX_STORED_DRAFTS);
  return Object.fromEntries(entries);
};

export const loadDrafts = (storage: StorageLike | null = browserStorage()): DraftStore => {
  if (!storage) return {};
  try {
    const raw = storage.getItem(DRAFT_STORAGE_KEY);
    if (!raw) return {};
    const parsed = JSON.parse(raw) as unknown;
    if (!parsed || typeof parsed !== "object" || Array.isArray(parsed)) return {};
    const store: DraftStore = {};
    for (const [conversationId, value] of Object.entries(parsed as Record<string, unknown>)) {
      const draft = value as Partial<ConversationDraft>;
      if (typeof draft?.text === "string" && typeof draft.operationId === "string") {
        store[conversationId] = {
          text: draft.text,
          operationId: draft.operationId,
          operationText: typeof draft.operationText === "string" ? draft.operationText : draft.text,
          updatedAt: typeof draft.updatedAt === "number" ? draft.updatedAt : Date.now()
        };
      }
    }
    return pruneDrafts(store);
  } catch {
    return {};
  }
};

export const saveDrafts = (
  store: DraftStore,
  storage: StorageLike | null = browserStorage()
): void => {
  if (!storage) return;
  try {
    const pruned = pruneDrafts(store);
    if (Object.keys(pruned).length === 0) {
      storage.removeItem(DRAFT_STORAGE_KEY);
    } else {
      storage.setItem(DRAFT_STORAGE_KEY, JSON.stringify(pruned));
    }
  } catch {
    // Sin almacenamiento local el borrador vive solo en memoria: se pierde al
    // cerrar, pero nunca se mezcla entre conversaciones.
  }
};

/** Texto del borrador de una conversación (vacío si no hay). */
export const draftText = (store: DraftStore, conversationId: string): string =>
  store[conversationId]?.text ?? "";

/** Actualiza el texto; el identificador de operación se conserva. */
export const withDraftText = (
  store: DraftStore,
  conversationId: string,
  text: string,
  now = Date.now()
): DraftStore => {
  const bounded = text.slice(0, MAX_DRAFT_CHARACTERS);
  const previous = store[conversationId];
  if (!bounded.trim()) {
    if (!previous) return store;
    const next = { ...store };
    delete next[conversationId];
    return next;
  }
  return {
    ...store,
    [conversationId]: {
      text: bounded,
      operationId: previous?.operationId ?? newOperationId(),
      operationText: previous?.operationText ?? bounded,
      updatedAt: now
    }
  };
};

/**
 * Identificador de operación para enviar `text` desde esta conversación.
 *
 * Se reutiliza solo si el texto es exactamente el mismo del intento anterior:
 * repetir lo mismo es reintentar; cambiarlo es una operación nueva.
 */
export const operationFor = (
  store: DraftStore,
  conversationId: string,
  text: string
): { store: DraftStore; operationId: string } => {
  const previous = store[conversationId];
  if (previous && previous.operationText === text) {
    return { store, operationId: previous.operationId };
  }
  const operationId = newOperationId();
  return {
    store: {
      ...store,
      [conversationId]: {
        text,
        operationId,
        operationText: text,
        updatedAt: Date.now()
      }
    },
    operationId
  };
};

/** El envío fue aceptado: el borrador desaparece. */
export const withoutDraft = (store: DraftStore, conversationId: string): DraftStore => {
  if (!store[conversationId]) return store;
  const next = { ...store };
  delete next[conversationId];
  return next;
};

/** El envío fue rechazado: el texto vuelve a su conversación con su operación. */
export const withRestoredDraft = (
  store: DraftStore,
  conversationId: string,
  text: string,
  operationId: string
): DraftStore => ({
  ...store,
  [conversationId]: {
    text,
    operationId,
    operationText: text,
    updatedAt: Date.now()
  }
});
