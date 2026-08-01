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
