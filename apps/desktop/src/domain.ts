/** El dominio de la aplicacion, repartido por area.
 *
 * Era un solo fichero de 2.100 lineas. Este modulo se queda como fachada
 * para que ningun `import ... from "./domain"` tenga que cambiar.
 */
export * from "./domain/carga";
export * from "./domain/programacion";
export * from "./domain/adjuntos";
export * from "./domain/memoria";
export * from "./domain/conversacion";
export * from "./domain/workflows";
export * from "./domain/athena";
