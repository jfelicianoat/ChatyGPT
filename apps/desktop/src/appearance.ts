export type AppearancePreference = "system" | "light" | "dark";
export type ResolvedAppearance = "light" | "dark";

export const APPEARANCE_STORAGE_KEY = "chatygpt.appearance.v1";

export function normalizeAppearancePreference(value: unknown): AppearancePreference {
  return value === "light" || value === "dark" || value === "system" ? value : "system";
}

export function resolveAppearance(
  preference: AppearancePreference,
  systemPrefersDark: boolean
): ResolvedAppearance {
  return preference === "system" ? (systemPrefersDark ? "dark" : "light") : preference;
}

export function loadAppearancePreference(): AppearancePreference {
  try {
    return normalizeAppearancePreference(window.localStorage.getItem(APPEARANCE_STORAGE_KEY));
  } catch {
    return "system";
  }
}

export function persistAppearancePreference(preference: AppearancePreference): void {
  try {
    window.localStorage.setItem(APPEARANCE_STORAGE_KEY, preference);
  } catch {
    // La preferencia sigue activa durante esta sesión si WebView2 bloquea el almacenamiento.
  }
}

export function systemPrefersDark(): boolean {
  return typeof window.matchMedia === "function" &&
    window.matchMedia("(prefers-color-scheme: dark)").matches;
}

export function applyAppearancePreference(
  preference: AppearancePreference
): ResolvedAppearance {
  const resolved = resolveAppearance(preference, systemPrefersDark());
  document.documentElement.dataset.appearance = preference;
  document.documentElement.dataset.theme = resolved;
  document.documentElement.style.colorScheme = resolved;
  document
    .querySelector<HTMLMetaElement>('meta[name="theme-color"]')
    ?.setAttribute("content", resolved === "dark" ? "#171512" : "#f3f0e9");
  return resolved;
}

export function subscribeToSystemAppearance(onChange: () => void): () => void {
  if (typeof window.matchMedia !== "function") return () => undefined;
  const media = window.matchMedia("(prefers-color-scheme: dark)");
  media.addEventListener("change", onChange);
  return () => media.removeEventListener("change", onChange);
}
