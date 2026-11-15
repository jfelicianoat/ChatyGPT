/**
 * Estado y acciones de la confirmación de herramientas.
 *
 * Cuando el Broker pausa un turno pidiendo permiso («ChatyGPT quiere realizar
 * una acción»), las decisiones de la persona y el reanudado del turno vivían
 * sueltos dentro de `App.tsx`. Aquí viajan juntos, y la tarjeta recibe
 * `ReturnType<typeof useConfirmaciones>` en lugar de props sueltas.
 */
import { useEffect, useState } from "react";

import { type ConversationView, type Loadable, type LocalTaskSnapshot } from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";

type Opciones = {
  /** Turno en curso: es el que lleva las llamadas pendientes de decidir. */
  currentTurn: Loadable<LocalTaskSnapshot> | null;
  conversation: Loadable<ConversationView> | null;
  /** Actualiza el turno con la respuesta del reanudado. */
  setActiveTurn: (value: Loadable<LocalTaskSnapshot>) => void;
  /** La navegación (proyectos, conversaciones) deja rastro tras reanudar. */
  reloadNavigation: () => Promise<void>;
  loadConversation: (
    conversationId: string,
    selectConversationAttachments?: boolean
  ) => Promise<boolean>;
};

export function useConfirmaciones({
  currentTurn,
  conversation,
  setActiveTurn,
  reloadNavigation,
  loadConversation
}: Opciones) {
  const [toolDecisions, setToolDecisions] = useState<Record<string, boolean>>({});
  const [toolDecisionBusy, setToolDecisionBusy] = useState(false);

  // Un turno nuevo (o nuevas llamadas en el mismo turno) se presenta sin
  // decisiones heredadas: nada marcado de una pausa anterior.
  useEffect(() => {
    setToolDecisions({});
  }, [
    currentTurn?.state === "ready" ? currentTurn.value.id : null,
    currentTurn?.state === "ready"
      ? currentTurn.value.pendingToolCalls.map((call) => call.toolCallId).join("|")
      : ""
  ]);

  const submitToolDecisions = async () => {
    if (currentTurn?.state !== "ready") return;
    const calls = currentTurn.value.pendingToolCalls;
    if (calls.length === 0 || calls.some((call) => toolDecisions[call.toolCallId] === undefined)) {
      return;
    }
    setToolDecisionBusy(true);
    try {
      const task = await platform.resolveToolCalls(
        currentTurn.value.id,
        calls.map((call) => ({
          toolCallId: call.toolCallId,
          approved: toolDecisions[call.toolCallId]
        }))
      );
      setActiveTurn({ state: "ready", value: task });
      await reloadNavigation();
      if (conversation?.state === "ready") {
        await loadConversation(conversation.value.id);
      }
    } catch (error) {
      setActiveTurn({ state: "error", message: describeError(error) });
    } finally {
      setToolDecisionBusy(false);
    }
  };

  return {
    toolDecisions,
    setToolDecisions,
    toolDecisionBusy,
    submitToolDecisions
  };
}
