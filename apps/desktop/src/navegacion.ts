/** Areas de la ventana principal.
 *
 * Vive fuera de `App.tsx` para que los paneles extraidos puedan tipar el
 * cambio de area sin importar el componente que los pinta.
 */
export type WorkspaceDestination =
  | "chats"
  | "projects"
  | "gpts"
  | "workflows"
  | "athena"
  | "automations"
  | "settings";
