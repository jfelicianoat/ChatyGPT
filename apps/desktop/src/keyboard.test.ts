import { describe, expect, it } from "vitest";
import { keyboardShortcutAction } from "./keyboard";

describe("keyboard shortcuts", () => {
  it("maps announced application shortcuts", () => {
    expect(keyboardShortcutAction({ key: "n", ctrlKey: true })).toBe("new-conversation");
    expect(keyboardShortcutAction({ key: "f", ctrlKey: true })).toBe("focus-search");
    expect(keyboardShortcutAction({ key: "M", ctrlKey: true, shiftKey: true })).toBe("focus-composer");
    expect(keyboardShortcutAction({ key: "1", altKey: true })).toBe("go-home");
    expect(keyboardShortcutAction({ key: "?", shiftKey: true })).toBe("open-help");
  });

  it("does not steal unmodified keys while the user is writing", () => {
    expect(keyboardShortcutAction({ key: "/", editableTarget: true })).toBeNull();
    expect(keyboardShortcutAction({ key: "?", editableTarget: true, shiftKey: true })).toBeNull();
  });

  it("does not run application shortcuts during composition or over a dialog", () => {
    expect(keyboardShortcutAction({ key: "n", ctrlKey: true, isComposing: true })).toBeNull();
    expect(keyboardShortcutAction({ key: "f", ctrlKey: true, modalOpen: true })).toBeNull();
  });

  it("keeps unknown browser and operating-system combinations untouched", () => {
    expect(keyboardShortcutAction({ key: "r", ctrlKey: true })).toBeNull();
    expect(keyboardShortcutAction({ key: "k", metaKey: true })).toBeNull();
  });
});
