// @vitest-environment jsdom
import { beforeEach, describe, expect, it } from "vitest";
import {
  IMAGE_DESCRIPTION_STORAGE_KEY,
  loadImageDescriptionPreference,
  normalizeImageDescriptionPreference,
  persistImageDescriptionPreference,
  shouldDescribeImages
} from "./ingestionPreferences";

describe("preferencia de descripción de imágenes", () => {
  beforeEach(() => window.localStorage.clear());

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
});
