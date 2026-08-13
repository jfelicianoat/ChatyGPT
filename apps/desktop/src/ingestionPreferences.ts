export type ImageDescriptionPreference = "describe" | "ignore";

export const IMAGE_DESCRIPTION_STORAGE_KEY = "chatygpt.ingestion.describe-images.v1";

export function normalizeImageDescriptionPreference(
  value: unknown
): ImageDescriptionPreference {
  return value === "ignore" ? "ignore" : "describe";
}

export function loadImageDescriptionPreference(): ImageDescriptionPreference {
  try {
    return normalizeImageDescriptionPreference(
      window.localStorage.getItem(IMAGE_DESCRIPTION_STORAGE_KEY)
    );
  } catch {
    return "describe";
  }
}

export function persistImageDescriptionPreference(
  preference: ImageDescriptionPreference
): void {
  try {
    window.localStorage.setItem(IMAGE_DESCRIPTION_STORAGE_KEY, preference);
  } catch {
    // La preferencia sigue activa durante esta sesión si WebView2 bloquea el almacenamiento.
  }
}

export function shouldDescribeImages(preference: ImageDescriptionPreference): boolean {
  return preference === "describe";
}
