import { describe, expect, it } from "vitest";
import { normalizeAppearancePreference, resolveAppearance } from "./appearance";

describe("appearance preferences", () => {
  it("falls back to the Windows setting for absent or corrupted values", () => {
    expect(normalizeAppearancePreference(null)).toBe("system");
    expect(normalizeAppearancePreference("sepia")).toBe("system");
  });

  it("keeps every supported explicit preference", () => {
    expect(normalizeAppearancePreference("system")).toBe("system");
    expect(normalizeAppearancePreference("light")).toBe("light");
    expect(normalizeAppearancePreference("dark")).toBe("dark");
  });

  it("resolves system dynamically while explicit themes remain stable", () => {
    expect(resolveAppearance("system", true)).toBe("dark");
    expect(resolveAppearance("system", false)).toBe("light");
    expect(resolveAppearance("light", true)).toBe("light");
    expect(resolveAppearance("dark", false)).toBe("dark");
  });
});
