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
  AthenaEstrategia,
  AthenaEstadoArea,
  AthenaEstadoTarea,
  AthenaFase,
  AthenaPermiso,
  AthenaRun,
  AthenaTarea
} from "./domain";

/** Fases en las que el run ya no va a cambiar por su cuenta. */
const FASES_TERMINALES: AthenaFase[] = [
  "completed",
  "failed",
  // Un run sin comprobar tampoco va a cambiar solo. Dejarlo fuera haria que la
  // interfaz siguiera sondeando para siempre un run que ya termino.
  "unverified",
  "cancelled"
];

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
    case "unverified":
      // No «Fallido». Athena distingue «tu cambio esta mal» de «no pude
      // comprobarlo» desde ADR-027, y esta linea es donde esa distincion llega
      // a una persona: la primera manda a revisar el trabajo, la segunda a
      // revisar el proyecto.
      return "Terminado sin comprobar";
    case "cancelled":
      return "Cancelado";
    case "recovery_pending":
      return "Interrumpido: necesita decisión";
    default:
      return "Sin estado";
  }
}

/**
 * Si conviene avisar de que la vista no está recibiendo lo que pasa en el run.
 *
 * Sólo mientras el run pueda cambiar. Cuando termina, Athena cierra el flujo
 * porque ya no queda nada que contar, así que la vista se queda sin conexión
 * **siempre**: anunciarlo entonces presentaba como avería el final normal de
 * todos los runs, y se leía junto a «Fallido» como si fueran el mismo problema.
 */
export function debeAvisarDeDesconexion(run: AthenaRun): boolean {
  return !run.conectado && !esFaseTerminal(run.fase);
}

/**
 * Lo que un detalle tipado de Athena significa para quien tiene que arreglarlo.
 *
 * Los dos codigos que se traducen son los del broker y estan en su contrato:
 * uno se arregla renovando la credencial y el otro no —pedir otro token no
 * repara un llavero roto—, y esa diferencia es justo la que decide que hace
 * quien lo lee. Lo que no se reconoce se enseña tal cual: inventar una
 * explicacion para un codigo nuevo seria peor que enseñar el codigo.
 */
export function pistaDeDetalle(detalle: string): string {
  switch (detalle) {
    case "ADMIN_AUTH_REQUIRED":
      return "Broker AI rechaza la credencial: renuévala en Inicio → Broker AI y reinicia Athena.";
    case "ADMIN_AUTH_BACKEND_UNAVAILABLE":
      return "El llavero de Broker AI no responde; otra credencial no lo arreglaría.";
    default:
      return detalle;
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

/** Una tarea colocada en el árbol que sus dependencias describen. */
export type AthenaNodoPlan = {
  tarea: AthenaTarea;
  /** Profundidad: 0 para las que no esperan a nadie. */
  nivel: number;
};

/**
 * Ordena las tareas por dependencia para poder dibujarlas como un plan.
 *
 * El nivel se calcula, no se pide: Athena publica las dependencias y esta capa
 * decide cómo colocarlas. Es la única deducción que hace la interfaz, y es de
 * presentación — no cambia qué tareas hay ni en qué estado están.
 *
 * Un ciclo no puede llegar hasta aquí porque el grafo se validó antes de
 * ejecutarse, pero la profundidad se acota igualmente: una vista que se cuelga
 * es peor que una vista que dibuja mal.
 */
export function ordenarPlan(tareas: AthenaTarea[]): AthenaNodoPlan[] {
  const porId = new Map(tareas.map((tarea) => [tarea.id, tarea]));
  const niveles = new Map<string, number>();

  const nivelDe = (tarea: AthenaTarea, visitados: Set<string>): number => {
    const conocido = niveles.get(tarea.id);
    if (conocido !== undefined) {
      return conocido;
    }
    if (visitados.has(tarea.id) || visitados.size > tareas.length) {
      return 0;
    }
    visitados.add(tarea.id);
    const padres = tarea.dependencias
      .map((id) => porId.get(id))
      .filter((padre): padre is AthenaTarea => padre !== undefined);
    const nivel = padres.length === 0
      ? 0
      : Math.max(...padres.map((padre) => nivelDe(padre, visitados))) + 1;
    niveles.set(tarea.id, nivel);
    return nivel;
  };

  return tareas
    .map((tarea) => ({ tarea, nivel: nivelDe(tarea, new Set()) }))
    .sort((izquierda, derecha) => izquierda.nivel - derecha.nivel);
}

/** Marca de estado para una tarea, en una vista que no tiene sitio para más. */
export function simboloTarea(estado: AthenaEstadoTarea): string {
  switch (estado) {
    case "completed":
      return "✓";
    case "running":
      return "▶";
    case "failed":
      return "✕";
    case "cancelled":
    case "killed":
      return "⊘";
    default:
      return "○";
  }
}

/** Nombre legible del especialista, o cadena vacía si el run no es jerárquico. */
export function nombreRol(rol: string): string {
  switch (rol) {
    case "explorer":
      return "Explorador";
    case "coder":
      return "Programador";
    case "verifier":
      return "Verificador";
    default:
      return rol;
  }
}

/**
 * Nombre legible de una estrategia de ejecución.
 *
 * `hierarchical` no se traduce como «jerárquico» a secas: lo que le importa a quien mira
 * es que el trabajo se repartió en tareas, no la forma del grafo.
 */
export function nombreEstrategia(estrategia: string): string {
  switch (estrategia) {
    case "auto":
      return "Que decida Athena";
    case "hierarchical":
      return "Repartido en tareas";
    case "direct":
      return "De una sola pieza";
    default:
      return estrategia;
  }
}

/**
 * Por qué se ejecutó así, en una frase corta.
 *
 * Se traduce el código y no el texto que lo acompaña: el código es estable y el texto se
 * reescribirá. Un código que no se reconozca cae al motivo que mandó Athena, que siempre
 * dice algo, en vez de dejar el hueco vacío.
 */
export function motivoEstrategia(estrategia: AthenaEstrategia): string {
  switch (estrategia.codigo) {
    case "caller_required_direct":
      return "Lo pediste así.";
    case "caller_required_hierarchical":
      return "Lo pediste así: repartir el trabajo aunque no hiciera falta.";
    case "planning_unavailable":
      return "Este Athena no tiene activado el reparto en tareas.";
    case "policy_declined":
      return "El objetivo tiene una sola cosa que comprobar al final, así que repartirlo no habría aportado nada.";
    case "policy_endorsed":
      return "El trabajo se puede repartir de verdad, así que se repartió.";
    case "plan_not_worthwhile":
      return "El reparto que salió no aportaba nada: las tareas iban una detrás de otra y todas para el mismo especialista.";
    case "plan_refused":
      return "No salió un reparto utilizable, así que se hizo de una pieza.";
    case "no_usable_plan":
      return "No salió un reparto utilizable, así que el objetivo entero quedó como una única tarea.";
    default:
      return estrategia.motivo;
  }
}

/**
 * Cierto cuando la política habría hecho otra cosa de la que se hizo.
 *
 * Es el caso que merece explicación: un objetivo que se podía repartir ejecutado de una
 * pieza, o al revés. Cuando coinciden no hay nada que contar.
 */
export function politicaDiscrepa(estrategia: AthenaEstrategia): boolean {
  if (!estrategia.veredictoPolitica) {
    return false;
  }
  const politicaRepartiria = estrategia.veredictoPolitica === "decompose";
  return politicaRepartiria !== (estrategia.seleccionada === "hierarchical");
}

/**
 * Qué opinó la política de descomposición, en palabras.
 *
 * Se enseña siempre, coincida o no con lo que se hizo. Enseñarlo sólo cuando discrepa
 * obligaba a quien mira a deducir del silencio que hubo acuerdo, y un silencio no
 * distingue «la política dijo lo mismo» de «la política no llegó a pronunciarse».
 */
export function veredictoPolitica(estrategia: AthenaEstrategia): string {
  switch (estrategia.veredictoPolitica) {
    case "decompose":
      return "Este objetivo se podía repartir en tareas.";
    case "decline":
      return "Este objetivo no necesitaba repartirse.";
    case "":
      return "La política no llegó a pronunciarse sobre este objetivo.";
    default:
      return estrategia.veredictoPolitica;
  }
}

/**
 * Nombre legible de una señal que Athena no pudo medir.
 *
 * Se enumeran una a una y no como cuenta: saber que «quedaron dos señales sin comprobar»
 * no dice cuáles, y son justo las que habría que mirar si la decisión sorprende.
 */
export function nombreSenal(senal: string): string {
  switch (senal) {
    case "has_meaningful_dependencies":
      return "Si hay partes que dependen de otras";
    case "distinct_roles_required":
      return "Si hace falta más de un especialista";
    case "independently_verifiable_outputs":
      return "Si los resultados se comprueban por separado";
    case "high_implementation_risk":
      return "Si el cambio es arriesgado";
    default:
      return senal;
  }
}

/** Nombre legible de un criterio de descomposición. */
export function nombreCriterio(criterio: string): string {
  switch (criterio) {
    case "multiple independently verifiable outputs":
      return "Varios resultados que se comprueban por separado";
    case "meaningful dependencies":
      return "Hay partes que dependen de otras";
    case "parallelisable investigation":
      return "Se puede investigar en paralelo";
    case "high implementation risk":
      return "El cambio es arriesgado";
    case "multiple files or subsystems":
      return "Toca varias partes del repositorio";
    case "more than one specialist":
      return "Hace falta más de un especialista";
    case "tasks that can run at the same time":
      return "Hay tareas que pueden ir a la vez";
    case "work for more than one specialist":
      return "Hay trabajo para más de un especialista";
    default:
      return criterio;
  }
}

/** Cuántas tareas del plan han terminado bien. */
export function progresoPlan(tareas: AthenaTarea[]): string {
  if (tareas.length === 0) {
    return "";
  }
  const hechas = tareas.filter((tarea) => tarea.estado === "completed").length;
  return `${hechas} de ${tareas.length}`;
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

/**
 * Sólo se puede cambiar el encargo de un run que aún está corriendo.
 *
 * Un run terminado no lo recogería: Athena aplica las revisiones entre iteraciones, y
 * ya no queda ninguna. Ofrecer el cambio ahí sería ofrecer una acción que no hace nada.
 */
export function puedeRevisarseElEncargo(run: AthenaRun | null): boolean {
  return run !== null && !esFaseTerminal(run.fase) && run.fase !== "recovery_pending";
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
    case "sin_credencial":
      return "Athena está disponible, pero falta su credencial. Guárdala aquí abajo para poder usarla.";
    case "credencial_invalida":
      // Que Athena esté viva no significa que sea la misma que emitió la credencial
      // guardada: al reiniciarla se genera otra. Decirlo evita mandar a nadie a revisar
      // el servicio cuando lo que hay que rehacer es la vinculación.
      return "Athena está disponible pero rechaza la credencial guardada. Es de otra sesión suya: vuelve a vincularla abajo.";
    case "incompatible":
      return "Athena responde con una versión que esta aplicación no sabe leer. Actualiza una de las dos.";
    case "no_disponible":
      return "Athena no está disponible. El chat normal sigue funcionando.";
    default:
      return "Comprobando el servicio de Athena…";
  }
}

/** Solo se puede lanzar si hay servicio autenticable, carpeta y objetivo. */
export function puedeLanzarse(
  estado: AthenaEstadoArea | null,
  objetivo: string,
  carpeta: string
): boolean {
  return (
    estado?.estado === "conectado" &&
    estado.credencialConfigurada &&
    objetivo.trim().length > 0 &&
    carpeta.trim().length > 0
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

/**
 * Por qué no se pudo comprobar un run, en palabras de quien lo va a leer.
 *
 * Los códigos vienen de `InconclusiveReason` y son estables; estas frases no.
 * Se traducen aquí y no en Athena porque son presentación, y porque un runtime
 * que devolviera texto para pantalla obligaría a cada cliente a aceptar el suyo.
 *
 * Un código que no se reconozca se enseña tal cual: inventarle una frase
 * amable sería contar algo que no se sabe.
 */
export function motivoSinComprobar(razon: string): string {
  switch (razon) {
    case "no_checks_defined":
      return "el proyecto no define comprobaciones que Athena pueda ejecutar";
    case "dependency_missing":
      return "falta algo que hay que instalar";
    case "environment_incomplete":
      return "el entorno está a medias";
    case "tool_unavailable":
      return "la comprobación no se pudo ejecutar";
    case "external_service_unavailable":
      return "un servicio del que depende no responde";
    case "partial_verification":
      return "sólo se pudo comprobar una parte";
    case "ambiguous_result":
      return "el resultado no dice ni que sí ni que no";
    default:
      return razon;
  }
}
