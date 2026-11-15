/**
 * Estado y acciones de los diálogos modales de la aplicación.
 *
 * El modelo de cada diálogo ya vivía en `dialogs.ts`; el estado del diálogo
 * abierto, su valor, la ocupación y el interruptor grande de `submitDialog`
 * (crear o renombrar proyectos, renombrar o archivar conversaciones, probar
 * un GPT en un chat real) vivían sueltos dentro de `App.tsx`. Aquí viajan
 * juntos.
 */
import { useEffect, useRef, useState, type Dispatch, type RefObject, type SetStateAction } from "react";

import { type Loadable, type LocalTaskSnapshot } from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";
import { dialogCopy, type DialogState } from "../dialogs";

type Opciones = {
  /** Proyecto seleccionado en la navegación: los alta lo seleccionan. */
  selectedProjectId: string | null;
  setSelectedProjectId: Dispatch<SetStateAction<string | null>>;
  /** Conversación abierta: archivar o borrar la cierra. */
  setConversation: Dispatch<SetStateAction<Loadable<import("../domain").ConversationView> | null>>;
  setActiveTurn: Dispatch<SetStateAction<Loadable<LocalTaskSnapshot> | null>>;
  setActiveTurnConversationId: Dispatch<SetStateAction<string | null>>;
  /** La prueba de un GPT abre chat nuevo: el scroll sigue al último mensaje. */
  followConversationScrollRef: RefObject<boolean>;
  reloadNavigation: () => Promise<void>;
  loadConversation: (conversationId: string) => Promise<boolean>;
  openConversation: (conversationId: string) => Promise<void>;
  setNavigationError: (message: string | null) => void;
};

export function useDialogos({
  selectedProjectId,
  setSelectedProjectId,
  setConversation,
  setActiveTurn,
  setActiveTurnConversationId,
  followConversationScrollRef,
  reloadNavigation,
  loadConversation,
  openConversation,
  setNavigationError
}: Opciones) {
  const [dialog, setDialog] = useState<DialogState | null>(null);
  const [dialogValue, setDialogValue] = useState("");
  const [dialogBusy, setDialogBusy] = useState(false);
  const dialogBusyRef = useRef(false);

  useEffect(() => {
    dialogBusyRef.current = dialogBusy;
  }, [dialogBusy]);

  const openDialog = (nextDialog: DialogState) => {
    const copy = dialogCopy(nextDialog);
    setDialog(nextDialog);
    setDialogValue(copy.initialValue ?? "");
    setNavigationError(null);
  };

  const submitDialog = async () => {
    if (!dialog) return;
    const copy = dialogCopy(dialog);
    if (copy.fieldLabel && !copy.allowEmpty && !dialogValue.trim()) return;
    setDialogBusy(true);
    try {
      switch (dialog.kind) {
        case "project-create": {
          const project = await platform.createProject(dialogValue.trim());
          setSelectedProjectId(project.id);
          break;
        }
        case "project-rename":
          await platform.renameProject(dialog.project.id, dialogValue.trim());
          break;
        case "project-instructions":
          await platform.updateProjectInstructions(dialog.project.id, dialogValue);
          break;
        case "project-archive":
          await platform.archiveProject(dialog.project.id);
          if (selectedProjectId === dialog.project.id) {
            setSelectedProjectId(null);
          }
          break;
        case "conversation-rename":
          await platform.renameConversation(
            dialog.conversation.id,
            dialogValue.trim()
          );
          await loadConversation(dialog.conversation.id);
          break;
        case "conversation-archive":
          await platform.archiveConversation(dialog.conversation.id);
          setConversation(null);
          break;
        case "conversation-delete":
          await platform.deleteConversation(dialog.conversation.id);
          setConversation(null);
          break;
        case "custom-gpt-test": {
          // La prueba usa exactamente el mismo recorrido durable que un chat
          // normal: queda en SQLite, aparece en Recientes y puede reanudarse.
          const prompt = dialogValue.trim();
          const created = await platform.createConversation(
            `Prueba · ${dialog.customGpt.name}`,
            dialog.customGpt.defaultProjectId ?? undefined
          );
          await platform.setConversationCustomGpt(created.id, dialog.customGpt.id);
          setDialog(null);
          await reloadNavigation();
          await openConversation(created.id);
          followConversationScrollRef.current = true;
          setActiveTurn({ state: "loading" });
          setActiveTurnConversationId(created.id);
          try {
            const task = await platform.sendChatTurn(
              created.id,
              prompt,
              [],
              false,
              false,
              false,
              false
            );
            setActiveTurn({ state: "ready", value: task });
            await loadConversation(created.id);
          } catch (error) {
            // El chat ya existe: mostramos el fallo dentro de él para que no se
            // pierda el contexto ni parezca que el botón no hizo nada.
            setActiveTurn({ state: "error", message: describeError(error) });
          }
          break;
        }
      }
      await reloadNavigation();
      setDialog(null);
    } catch (error) {
      setNavigationError(describeError(error));
    } finally {
      setDialogBusy(false);
    }
  };

  return {
    dialog,
    setDialog,
    dialogValue,
    setDialogValue,
    dialogBusy,
    dialogBusyRef,
    openDialog,
    submitDialog
  };
}
