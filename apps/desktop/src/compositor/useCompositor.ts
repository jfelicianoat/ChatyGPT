/**
 * Estado y acciones del compositor de mensajes.
 *
 * El borrador por conversación (H10), la política efectiva de ejecución
 * (H01/H04: privacidad, coste y destino calculados en Rust) y el teclado del
 * compositor (H24) vivían sueltos dentro de `App.tsx`. Aquí viajan juntos, y
 * la tarjeta de opciones recibe lo que necesita como props.
 */
import { useLayoutEffect, useEffect, useRef, useState, type RefObject } from "react";

import {
  type ConversationView,
  type EffectiveExecutionPolicy,
  type Loadable
} from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";
import { draftText, loadDrafts, saveDrafts, withDraftText, type DraftStore } from "../borradores";
import { isImeComposition } from "../keyboard";

type Opciones = {
  /** Conversación abierta: dueño del borrador y origen de la política. */
  conversation: Loadable<ConversationView> | null;
  /** Recuerdos activos: entran en la señal que refresca la política. */
  memoryItemCount: number;
  /** Conversación seleccionada según App: descarta respuestas llegadas tarde. */
  selectedConversationRef: RefObject<string | null>;
};

export function useCompositor({
  conversation,
  memoryItemCount,
  selectedConversationRef
}: Opciones) {
  const [draft, setDraft] = useState("");
  const draftsRef = useRef<DraftStore>(loadDrafts());
  const draftOwnerRef = useRef<string | null>(null);
  const [executionPolicy, setExecutionPolicy] =
    useState<Loadable<EffectiveExecutionPolicy> | null>(null);
  const openConversationId = conversation?.state === "ready" ? conversation.value.id : null;

  // H10: al cambiar de conversación, el texto del compositor se guarda con la
  // que lo tenía y se recupera el de la que se abre. Se hace antes de pintar
  // para que nunca se vea, ni un instante, el borrador de otra conversación.
  useLayoutEffect(() => {
    if (!openConversationId || draftOwnerRef.current === openConversationId) return;
    draftOwnerRef.current = openConversationId;
    setDraft(draftText(draftsRef.current, openConversationId));
  }, [openConversationId]);

  useEffect(() => {
    const owner = draftOwnerRef.current;
    if (!owner) return;
    const next = withDraftText(draftsRef.current, owner, draft);
    if (next !== draftsRef.current) {
      draftsRef.current = next;
      saveDrafts(next);
    }
  }, [draft]);

  // H01/H04: la privacidad, el coste y el destino que se aplicarán de verdad
  // al próximo mensaje se calculan en Rust con la misma regla que la petición.
  const policySignal =
    conversation?.state === "ready"
      ? [
          conversation.value.id,
          conversation.value.customGptId ?? "",
          JSON.stringify(conversation.value.executionPreferences),
          conversation.value.messages.length,
          memoryItemCount
        ].join("|")
      : "";
  useEffect(() => {
    if (conversation?.state !== "ready") {
      setExecutionPolicy(null);
      return;
    }
    const conversationId = conversation.value.id;
    let current = true;
    setExecutionPolicy((previous) => previous ?? { state: "loading" });
    platform.getEffectiveExecutionPolicy(conversationId)
      .then((value) => {
        if (!current || selectedConversationRef.current !== conversationId) return;
        // Una respuesta sin la forma del contrato no se enseña como política:
        // mostrar valores inventados sería peor que no mostrar nada.
        if (!value || typeof value !== "object" || !("sources" in value) || !value.destination) {
          setExecutionPolicy({
            state: "error",
            message: "No se pudo calcular la política efectiva."
          });
          return;
        }
        setExecutionPolicy({ state: "ready", value });
      })
      .catch((error) => {
        if (current) setExecutionPolicy({ state: "error", message: describeError(error) });
      });
    return () => {
      current = false;
    };
  }, [policySignal]);

  const composerKeyDown = (
    event: React.KeyboardEvent<HTMLTextAreaElement>,
    { canSend, onSend }: { canSend: boolean; onSend: () => void }
  ) => {
    // Con un IME, Enter confirma la composición: no envía (H24).
    if (isImeComposition(event.nativeEvent)) return;
    if (event.key === "Enter" && !event.shiftKey && canSend) {
      event.preventDefault();
      void onSend();
    }
  };

  return {
    draft,
    setDraft,
    /** Almacén de borradores por conversación; lo comparte el envío del turno. */
    draftsRef,
    executionPolicy,
    composerKeyDown
  };
}
