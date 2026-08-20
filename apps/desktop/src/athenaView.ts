/**
 * Lógica del área de Athena, separada del JSX para poder probarla.
 *
 * Aquí no se decide en qué estado está el agente: eso lo dice Athena y llega ya
 * resuelto en la proyección. Lo que hay son decisiones de presentación —cómo se
 * llama una fase, si conviene seguir sondeando, qué se puede pulsar— y ninguna
 * de ellas inventa información que el runtime no haya publicado.
 */

import type {
  AthenaArgumento,
  AthenaEstadoArea,
  AthenaEstadoTarea,
  AthenaFase,
  AthenaPermiso,
  AthenaRun
} from "./domain";

/** Fases en las que el run ya no va a cambiar por su cuenta. */
const FASES_TERMINALES: AthenaFase[] = ["completed", "failed", "cancelled"];

/** Cada cuánto se pide la proyección mientras el run sigue vivo. */
export const INTERVALO_SONDEO_MS = 1000;

export function esFaseTerminal(fase: AthenaFase | undefined): boolean {
  return fase !== undefined && FASES_TERMINALES.includes(fase);
}

/**
 * Nombre legible de la fase.
 *
 * `recovery_pending` no se traduce como «pendiente» a secas a propósito: no es
 * una cola de espera, es un run que quedó a medias y necesita una decisión.
 */
export function nombreFase(fase: AthenaFase | undefined): string {
  switch (fase) {
    case "starting":
      return "Arrancando";
    case "running":
      return "Trabajando";
    case "waiting_permission":
      return "Esperando tu autorización";
    case "verifying":
      return "Verificando";
    case "completed":
      return "Terminado";
    case "failed":
      return "Fallido";
    case "cancelled":
      return "Cancelado";
    case "recovery_pending":
      return "Interrumpido: necesita decisión";
    default:
      return "Sin estado";
  }
}

export function nombreEstadoTarea(estado: AthenaEstadoTarea): string {
  switch (estado) {
    case "pending":
      return "Pendiente";
    case "running":
      return "En marcha";
    case "completed":
      return "Terminada";
    case "failed":
      return "Fallida";
    case "cancelled":
      return "Cancelada";
    case "killed":
      return "Detenida";
    case "recovery_pending":
      return "Por recuperar";
    default:
      return estado;
  }
}

/** Sigue sondeando mientras el run pueda cambiar solo. */
export function debeSeguirSondeando(run: AthenaRun | null): boolean {
  if (!run) {
    return false;
  }
  return !esFaseTerminal(run.fase);
}

/**
 * Un run puede cancelarse mientras no haya terminado.
 *
 * Se permite también en `waiting_permission`: quien está mirando la petición
 * puede preferir parar del todo en vez de contestarla.
 */
export function puedeCancelarse(run: AthenaRun | null): boolean {
  return run !== null && !esFaseTerminal(run.fase);
}

/** Solo se reanuda lo que Athena marcó como reanudable. */
export function puedeReanudarse(run: AthenaRun | null): boolean {
  return run !== null && run.reanudable && run.fase === "recovery_pending";
}

/**
 * Se puede responder a un permiso solo si este cliente controla el run.
 *
 * Los intents viajan por otra conexión que el flujo de eventos, así que sin la
 * identidad que entrega ese flujo el servicio rechazaría la respuesta.
 */
export function puedeResponderPermiso(run: AthenaRun | null): boolean {
  return run !== null && run.controla && run.suscriptor !== undefined;
}

/** Permiso al que hay que atender ahora, si hay alguno. */
export function permisoActivo(run: AthenaRun | null): AthenaPermiso | null {
  if (!run || run.permisos.length === 0) {
    return null;
  }
  return run.permisos[0];
}

/**
 * Por qué no se puede responder ahora mismo, o `null` si sí se puede.
 *
 * Devuelve el motivo en vez de un booleano porque cada uno se arregla de una
 * forma distinta, y un botón desactivado sin explicación es peor que un botón
 * que falla.
 */
export function motivoBloqueoPermiso(
  run: AthenaRun | null,
  permiso: AthenaPermiso
): string | null {
  if (permiso.caducado) {
    return "El plazo se agotó. Athena ya la ha dado por denegada.";
  }
  if (run && esFaseTerminal(run.fase)) {
    return "El run terminó; esta petición ya no se aplica.";
  }
  if (!puedeResponderPermiso(run)) {
    return "Otra ventana controla este run: solo desde ella se puede responder.";
  }
  return null;
}

/** Nombre del riesgo tal y como lo clasificó Athena. */
export function nombreRiesgo(riesgo: string): string {
  switch (riesgo) {
    case "none":
      return "Sin riesgo";
    case "low":
      return "Riesgo bajo";
    case "medium":
      return "Riesgo medio";
    case "high":
      return "Riesgo alto";
    case "critical":
      return "Riesgo crítico";
    default:
      return riesgo;
  }
}

/**
 * Etiquetas cortas que resumen la naturaleza de la acción.
 *
 * Salen de banderas que puso el runtime; esta capa no deduce ninguna.
 */
export function etiquetasPermiso(permiso: AthenaPermiso): string[] {
  const etiquetas: string[] = [nombreRiesgo(permiso.riesgo)];
  if (permiso.soloLectura) {
    etiquetas.push("Solo lectura");
  }
  if (permiso.destructivo) {
    etiquetas.push("Destructiva");
  }
  if (permiso.caducado) {
    etiquetas.push("Caducada");
  }
  return etiquetas;
}

/**
 * Cómo se enseña el valor de un argumento.
 *
 * Un valor redactado se nombra, no se pinta; uno resumido dice cuánto se quedó
 * fuera. Reconstruir el original aquí sería deshacer el saneado del runtime.
 */
export function textoArgumento(argumento: AthenaArgumento): string {
  if (argumento.redactado) {
    return "(oculto por seguridad)";
  }
  if (argumento.resumido && argumento.caracteres !== undefined) {
    return `${argumento.valor}… (${argumento.caracteres} caracteres en total)`;
  }
  return argumento.valor;
}

/** Cuenta atrás legible para una petición de permiso. */
export function tiempoRestante(permiso: AthenaPermiso): string {
  const segundos = Math.max(0, Math.round(permiso.segundosRestantes));
  if (segundos <= 0) {
    return "sin tiempo";
  }
  if (segundos < 60) {
    return `${segundos} s`;
  }
  const minutos = Math.floor(segundos / 60);
  const resto = segundos % 60;
  return resto === 0 ? `${minutos} min` : `${minutos} min ${resto} s`;
}

/** Explicación del estado del servicio, sin tecnicismos ni URLs. */
export function mensajeServicio(estado: AthenaEstadoArea | null): string {
  if (!estado) {
    return "Comprobando el servicio de Athena…";
  }
  switch (estado.estado) {
    case "conectado":
      return "Athena está disponible.";
    case "incompatible":
      return "Athena responde con una versión que esta aplicación no sabe leer. Actualiza una de las dos.";
    case "no_disponible":
      return "Athena no está disponible. El chat normal sigue funcionando.";
    default:
      return "Comprobando el servicio de Athena…";
  }
}

/** Solo se puede lanzar un run si el servicio responde y hay carpeta y objetivo. */
export function puedeLanzarse(
  estado: AthenaEstadoArea | null,
  objetivo: string,
  carpeta: string
): boolean {
  return (
    estado?.estado === "conectado" && objetivo.trim().length > 0 && carpeta.trim().length > 0
  );
}

/**
 * Veredicto de verificación en palabras.
 *
 * `inconclusive` se dice tal cual: un run que no pudo demostrar nada no es un
 * run correcto, y llamarlo de otra forma sería precisamente lo que el runtime
 * se niega a hacer.
 */
export function nombreVerificacion(estado: string | undefined): string | null {
  switch (estado) {
    case "passed":
      return "La verificación pasó";
    case "failed":
      return "La verificación falló";
    case "inconclusive":
      return "No se pudo verificar";
    default:
      return null;
  }
}

/** Resumen corto de lo que el run lleva hecho, para la cabecera del área. */
export function resumenActividad(run: AthenaRun | null): string {
  if (!run) {
    return "";
  }
  const partes: string[] = [];
  if (run.ficherosModificados.length > 0) {
    partes.push(
      run.ficherosModificados.length === 1
        ? "1 fichero modificado"
        : `${run.ficherosModificados.length} ficheros modificados`
    );
  }
  if (run.ciclosReparacion > 0) {
    partes.push(
      run.ciclosReparacion === 1
        ? "1 ciclo de reparación"
        : `${run.ciclosReparacion} ciclos de reparación`
    );
  }
  if (run.errores.length > 0) {
    partes.push(run.errores.length === 1 ? "1 error" : `${run.errores.length} errores`);
  }
  return partes.join(" · ");
}
