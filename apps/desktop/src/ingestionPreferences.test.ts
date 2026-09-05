// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  IMAGE_DESCRIPTION_STORAGE_KEY,
  loadImageDescriptionPreference,
  normalizeImageDescriptionPreference,
  persistImageDescriptionPreference,
  shouldDescribeImages
} from "./ingestionPreferences";

describe("preferencia de descripción de imágenes", () => {
  beforeEach(() => window.localStorage.clear());
  afterEach(() => vi.restoreAllMocks());

  it("mantiene el comportamiento rico como valor predeterminado", () => {
    expect(normalizeImageDescriptionPreference(null)).toBe("describe");
    expect(loadImageDescriptionPreference()).toBe("describe");
    expect(shouldDescribeImages("describe")).toBe(true);
  });

  it("conserva la decisión de ignorar imágenes en este equipo", () => {
    persistImageDescriptionPreference("ignore");

    expect(window.localStorage.getItem(IMAGE_DESCRIPTION_STORAGE_KEY)).toBe("ignore");
    expect(loadImageDescriptionPreference()).toBe("ignore");
    expect(shouldDescribeImages("ignore")).toBe(false);
  });

  it("no rompe la subida de un documento si el almacenamiento está bloqueado", () => {
    vi.spyOn(Storage.prototype, "getItem").mockImplementation(() => {
      throw new DOMException("bloqueado", "SecurityError");
    });
    vi.spyOn(Storage.prototype, "setItem").mockImplementation(() => {
      throw new DOMException("bloqueado", "SecurityError");
    });

    expect(() => persistImageDescriptionPreference("ignore")).not.toThrow();
    expect(loadImageDescriptionPreference()).toBe("describe");
  });
});
