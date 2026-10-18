/**
 * Traducción de un fallo a texto para la persona.
 *
 * Extraído de `App.tsx` (fase 1 de la reducción del componente). Es la única
 * puerta por la que un error llega a la pantalla, así que conviene que su
 * comportamiento ante valores raros esté fijado por una prueba y no dependa de
 * lo que haga `String()` con cada cosa.
 */

/** Mensaje legible de cualquier valor lanzado. */
export function describeError(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}

/**
 * Quita el prefijo de rutas largas de Windows (`\?\`) de un texto visible.
 *
 * Las rutas canónicas lo llevan y la base las guarda así a propósito, pero a la
 * persona solo le dice «tecnicismo» (auditoría 14-oct, INC-13).
 */
export function withoutExtendedPathPrefix(text: string): string {
  return text.replace(/\\\\\?\\UNC\\/g, "\\\\").replace(/\\\\\?\\/g, "");
}
