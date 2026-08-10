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
