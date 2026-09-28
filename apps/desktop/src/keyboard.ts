export type KeyboardShortcutAction =
  | "new-conversation"
  | "focus-search"
  | "focus-composer"
  | "go-home"
  | "open-help";

export type KeyboardShortcutInput = {
  key: string;
  ctrlKey?: boolean;
  shiftKey?: boolean;
  altKey?: boolean;
  metaKey?: boolean;
  isComposing?: boolean;
  editableTarget?: boolean;
  modalOpen?: boolean;
};

export function keyboardShortcutAction(
  input: KeyboardShortcutInput
): KeyboardShortcutAction | null {
  if (input.isComposing || input.modalOpen || input.metaKey) return null;
  const key = input.key.toLocaleLowerCase("es-ES");

  if (input.ctrlKey && !input.altKey) {
    if (!input.shiftKey && key === "n") return "new-conversation";
    if (!input.shiftKey && key === "f") return "focus-search";
    if (input.shiftKey && key === "m") return "focus-composer";
    return null;
  }

  if (input.altKey && !input.ctrlKey && !input.shiftKey && key === "1") {
    return "go-home";
  }

  if (input.editableTarget || input.ctrlKey || input.altKey) return null;
  if (key === "/") return "focus-search";
  if (input.key === "?") return "open-help";
  return null;
}

export function isEditableKeyboardTarget(target: EventTarget | null): boolean {
  if (!(target instanceof HTMLElement)) return false;
  return target.isContentEditable || ["INPUT", "TEXTAREA", "SELECT"].includes(target.tagName);
}

/**
 * La tecla pertenece a una composición de texto en curso (IME).
 *
 * Con un método de entrada —japonés, chino, coreano, o los acentos muertos de
 * algunos teclados— Enter confirma la palabra que se está componiendo. Enviar
 * en ese momento manda un texto a medias (auditoría 28-sep-2026, H24).
 * WebView2 marca `isComposing`; algunos IME solo informan `keyCode` 229.
 */
export function isImeComposition(event: { isComposing?: boolean; keyCode?: number }): boolean {
  return Boolean(event.isComposing) || event.keyCode === 229;
}
