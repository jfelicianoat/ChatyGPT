/**
 * Ventanas de confirmación y edición: qué son y qué texto muestran.
 *
 * Extraído de `App.tsx` (fase 1 de la reducción del componente). El texto de
 * una ventana no es presentación accesoria: decide si la acción se anuncia como
 * destructiva, qué se promete que ocurrirá y con qué palabra se confirma. Aquí
 * es comprobable; dentro de un componente de 7.000 líneas, no lo era.
 */

import type { ConversationView, ProjectSummary } from "./domain";

export type DialogState =
  | { kind: "project-create" }
  | { kind: "project-rename"; project: ProjectSummary }
  | { kind: "project-instructions"; project: ProjectSummary }
  | { kind: "project-archive"; project: ProjectSummary }
  | { kind: "conversation-rename"; conversation: ConversationView }
  | { kind: "conversation-archive"; conversation: ConversationView }
  | { kind: "conversation-delete"; conversation: ConversationView };

export type DialogCopy = {
  title: string;
  description: string;
  fieldLabel?: string;
  initialValue?: string;
  multiline?: boolean;
  allowEmpty?: boolean;
  maxLength?: number;
  /** Marca la ventana como destructiva para que la interfaz lo señale. */
  destructive?: boolean;
  action: string;
};

/** Texto completo de una ventana a partir de su estado. */
export function dialogCopy(dialog: DialogState): DialogCopy {
  switch (dialog.kind) {
    case "project-create":
      return {
        title: "Nuevo proyecto",
        description: "Agrupa conversaciones relacionadas sin mover sus datos fuera de SQLite.",
        fieldLabel: "Nombre del proyecto",
        action: "Crear proyecto"
      };
    case "project-rename":
      return {
        title: "Renombrar proyecto",
        description: "Las conversaciones asociadas conservarán su relación con el proyecto.",
        fieldLabel: "Nombre del proyecto",
        initialValue: dialog.project.name,
        action: "Guardar"
      };
    case "project-instructions":
      return {
        title: "Instrucciones del proyecto",
        description:
          "Se aplicarán a todos los mensajes de los chats de este proyecto y aparecerán en el inspector de contexto.",
        fieldLabel: "Cómo debe trabajar ChatyGPT en este proyecto",
        initialValue: dialog.project.instructions ?? "",
        multiline: true,
        allowEmpty: true,
        maxLength: 8_000,
        action: dialog.project.instructions ? "Actualizar instrucciones" : "Guardar instrucciones"
      };
    case "project-archive":
      return {
        title: "Archivar proyecto",
        description:
          "El proyecto desaparecerá de la barra lateral. Sus conversaciones seguirán disponibles sin proyecto.",
        destructive: true,
        action: "Archivar"
      };
    case "conversation-rename":
      return {
        title: "Renombrar conversación",
        description: "El contenido y el historial no cambiarán.",
        fieldLabel: "Título",
        initialValue: dialog.conversation.title,
        action: "Guardar"
      };
    case "conversation-archive":
      return {
        title: "Archivar conversación",
        description:
          "La conversación saldrá de la lista activa, pero sus mensajes se conservarán localmente.",
        destructive: true,
        action: "Archivar"
      };
    case "conversation-delete":
      return {
        title: "Eliminar conversación",
        description:
          "La conversación quedará marcada como eliminada. Esta acción no borra físicamente los registros todavía.",
        destructive: true,
        action: "Eliminar"
      };
  }
}
