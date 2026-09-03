/** El estado de algo que se carga: cargando, listo o con un error explicado.
 *
 * Vivia dentro de `App.tsx`. Al sacar los paneles a componentes propios paso a
 * ser un tipo compartido, que es lo que siempre fue en la practica.
 */
export type Loadable<T> =
  | { state: "loading" }
  | { state: "ready"; value: T }
  | { state: "error"; message: string };
