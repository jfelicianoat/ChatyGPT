import { describe, expect, it } from "vitest";
import { dialogCopy, type DialogState } from "./dialogs";
import type { ConversationView, ProjectSummary } from "./domain";

const project = {
  id: "project-1",
  name: "TFM",
  description: null,
  instructions: null,
  conversationCount: 3,
  createdAt: "2026-08-01T10:00:00Z",
  updatedAt: "2026-08-01T10:00:00Z"
} as unknown as ProjectSummary;

const conversation = {
  id: "conversation-1",
  title: "Normativa europea"
} as unknown as ConversationView;

describe("texto de las ventanas", () => {
  it("marca como destructiva solo la acción que lo es", () => {
    const destructive: DialogState[] = [
      { kind: "project-archive", project },
      { kind: "conversation-archive", conversation },
      { kind: "conversation-delete", conversation }
    ];
    for (const dialog of destructive) {
      expect(dialogCopy(dialog).destructive).toBe(true);
    }

    const safe: DialogState[] = [
      { kind: "project-create" },
      { kind: "project-rename", project },
      { kind: "project-instructions", project },
      { kind: "conversation-rename", conversation }
    ];
    for (const dialog of safe) {
      expect(dialogCopy(dialog).destructive).toBeUndefined();
    }
  });

  it("no ofrece campo de texto en las ventanas de solo confirmar", () => {
    // Archivar o eliminar no piden nada: solo confirmación.
    expect(dialogCopy({ kind: "project-archive", project }).fieldLabel).toBeUndefined();
    expect(
      dialogCopy({ kind: "conversation-delete", conversation }).fieldLabel
    ).toBeUndefined();
  });

  it("propone el valor actual al renombrar, para no obligar a reescribirlo", () => {
    expect(dialogCopy({ kind: "project-rename", project }).initialValue).toBe("TFM");
    expect(dialogCopy({ kind: "conversation-rename", conversation }).initialValue).toBe(
      "Normativa europea"
    );
    // Crear parte de vacío.
    expect(dialogCopy({ kind: "project-create" }).initialValue).toBeUndefined();
  });

  it("distingue guardar instrucciones de actualizarlas", () => {
    const empty = dialogCopy({ kind: "project-instructions", project });
    expect(empty.action).toBe("Guardar instrucciones");
    expect(empty.initialValue).toBe("");
    // Permite vaciarlas: borrar las instrucciones es una decisión válida.
    expect(empty.allowEmpty).toBe(true);
    expect(empty.multiline).toBe(true);
    expect(empty.maxLength).toBe(8_000);

    const filled = dialogCopy({
      kind: "project-instructions",
      project: { ...project, instructions: "Cita siempre la fuente." }
    });
    expect(filled.action).toBe("Actualizar instrucciones");
    expect(filled.initialValue).toBe("Cita siempre la fuente.");
  });

  it("describe cada ventana con título, explicación y acción", () => {
    const all: DialogState[] = [
      { kind: "project-create" },
      { kind: "project-rename", project },
      { kind: "project-instructions", project },
      { kind: "project-archive", project },
      { kind: "conversation-rename", conversation },
      { kind: "conversation-archive", conversation },
      { kind: "conversation-delete", conversation }
    ];
    for (const dialog of all) {
      const copy = dialogCopy(dialog);
      expect(copy.title.length).toBeGreaterThan(0);
      expect(copy.description.length).toBeGreaterThan(0);
      expect(copy.action.length).toBeGreaterThan(0);
    }
  });

  it("no promete un borrado que todavía no ocurre", () => {
    // El backend marca la conversación como eliminada; no borra registros.
    const copy = dialogCopy({ kind: "conversation-delete", conversation });
    expect(copy.description).toContain("no borra físicamente");
  });
});
