/**
 * Presentación local de las automatizaciones: hora, etiquetas y avisos leídos.
 *
 * Extraído de `App.tsx` (fase 1 de la reducción del componente). Son las tres
 * piezas del scheduler que no pertenecen al dominio durable —viven en la hora
 * local del equipo, en el idioma de la interfaz y en `localStorage`— pero que
 * sí toman decisiones: qué fecha se propone por defecto, cómo se nombra un
 * estado y cuántos avisos se conservan como leídos.
 */

import type { ScheduledCalendarOccurrence, ScheduledTaskView } from "./domain";

/** Clave versionada de la marca local de avisos leídos. */
export const SCHEDULER_READ_NOTIFICATIONS_KEY = "chatygpt.scheduler.readNotifications.v1";

/**
 * Máximo de avisos leídos que se conservan.
 *
 * Es una preferencia de interfaz reconstruible: el historial durable sigue en
 * SQLite, así que acotarla no pierde dominio y evita que `localStorage` crezca
 * sin límite.
 */
export const MAX_READ_NOTIFICATIONS = 200;

/** Valor para un `<input type="datetime-local">` en la hora local del equipo. */
export function scheduledLocalTimeValue(value: Date): string {
  const part = (value: number) => String(value).padStart(2, "0");
  return `${value.getFullYear()}-${part(value.getMonth() + 1)}-${part(value.getDate())}T${part(value.getHours())}:${part(value.getMinutes())}`;
}

/** Fecha propuesta al crear una automatización: dentro de una hora. */
export function defaultScheduledLocalTime(now = new Date()): string {
  return scheduledLocalTimeValue(new Date(now.getTime() + 60 * 60 * 1000));
}

/** Nombre legible del estado de una ejecución. */
export function scheduledRunLabel(
  status: ScheduledTaskView["runs"][number]["status"]
): string {
  switch (status) {
    case "claimed":
      return "Preparando";
    case "running":
      return "En ejecución";
    case "completed":
      return "Completada";
    case "failed":
      return "Fallida";
    case "cancelled":
      return "Cancelada";
    case "skipped":
      return "Omitida";
  }
}

/** Estados de una ejecución que ya no van a cambiar. */
const TERMINAL_RUN_STATUSES = ["completed", "failed", "cancelled"] as const;

/** Aviso de Windows listo para emitirse. */
export type ScheduledRunNotification = {
  runId: string;
  title: string;
  body: string;
  /** Etiqueta estable: Windows sustituye el aviso anterior del mismo run. */
  tag: string;
};

/**
 * Decide qué avisos emitir y cuál es el estado conocido después.
 *
 * Extraído de `App.tsx` (fase 4). Es la máquina de estado más delicada de la
 * interfaz porque el error se paga caro en las dos direcciones: avisar de más
 * molesta cada diez segundos, y avisar de menos deja pasar desapercibida una
 * automatización que falló. Las reglas que la gobiernan son cuatro, y estaban
 * repartidas entre condiciones anidadas dentro de un `setInterval`:
 *
 * 1. **Solo transiciones.** Se avisa cuando el estado *cambia* a terminal, no
 *    cada vez que se lee un estado terminal, o el sondeo repetiría el aviso.
 * 2. **Nunca en el primer sondeo.** Sin historial previo no hay transición que
 *    observar: todo lo terminal ya lo estaba antes de abrir la aplicación.
 * 3. **Solo estados terminales.** Pasar de `claimed` a `running` no es noticia.
 * 4. **Sin permiso no se emite, pero sí se recuerda.** El estado conocido se
 *    actualiza igual, de modo que conceder el permiso más tarde no provoca una
 *    ráfaga de avisos atrasados.
 *
 * Devuelve también el estado siguiente en vez de mutar: así la decisión es
 * comprobable y el componente se limita a guardarlo.
 */
export function pendingScheduledRunNotifications({
  tasks,
  knownStates,
  historyInitialized,
  permissionGranted
}: {
  tasks: ScheduledTaskView[];
  knownStates: ReadonlyMap<string, string>;
  historyInitialized: boolean;
  permissionGranted: boolean;
}): {
  notifications: ScheduledRunNotification[];
  nextStates: Map<string, string>;
} {
  const notifications: ScheduledRunNotification[] = [];
  const nextStates = new Map(knownStates);
  for (const task of tasks) {
    for (const run of task.runs) {
      const previous = knownStates.get(run.id);
      const terminal = (TERMINAL_RUN_STATUSES as readonly string[]).includes(run.status);
      if (historyInitialized && permissionGranted && terminal && previous !== run.status) {
        notifications.push({
          runId: run.id,
          title:
            run.status === "completed"
              ? "Tarea programada completada"
              : "Tarea programada finalizada",
          body: `${task.name} · ${task.conversationTitle ?? task.workflowName ?? "Sin destino"} · ${scheduledRunLabel(run.status)}`,
          tag: `chatygpt-${run.id}`
        });
      }
      nextStates.set(run.id, run.status);
    }
  }
  return { notifications, nextStates };
}

/** Resultado de revisar el formulario antes de tocar el backend. */
export type ScheduleDraftValidation =
  /** Faltan datos: el formulario no está listo y no se dice nada todavía. */
  | { status: "incomplete" }
  /** Los datos están, pero la fecha no sirve. Se explica por qué. */
  | { status: "invalid-date"; message: string }
  | { status: "valid"; dueAtIso: string };

/**
 * Revisa el borrador de una automatización.
 *
 * Extraído de `App.tsx` (fase 3). Distingue dos situaciones que la interfaz
 * trata de forma distinta y que conviene no confundir: un formulario
 * **incompleto** no es un error —la persona todavía está rellenándolo, así que
 * no se le muestra nada—, mientras que una fecha pasada sí lo es y merece una
 * explicación.
 *
 * La confirmación cuenta como dato obligatorio: sin ella el formulario está
 * incompleto, que es lo que impide activar una automatización sin decidirlo.
 */
export function validateScheduleDraft(
  draft: {
    name: string;
    conversationId: string;
    prompt: string;
    at: string;
    confirmed: boolean;
  },
  now: Date = new Date()
): ScheduleDraftValidation {
  if (
    !draft.name.trim() ||
    !draft.conversationId ||
    !draft.prompt.trim() ||
    !draft.at ||
    !draft.confirmed
  ) {
    return { status: "incomplete" };
  }
  const dueAt = new Date(draft.at);
  if (Number.isNaN(dueAt.getTime()) || dueAt.getTime() <= now.getTime()) {
    return { status: "invalid-date", message: "Elige una fecha y hora futuras." };
  }
  return { status: "valid", dueAtIso: dueAt.toISOString() };
}

/**
 * Si el borrador puede guardarse como plantilla.
 *
 * Una plantilla solo conserva nombre, instrucción y repetición, así que no
 * necesita conversación, fecha ni confirmación: guardarla no programa nada.
 */
export function canSaveScheduleTemplate(draft: {
  name: string;
  prompt: string;
}): boolean {
  return Boolean(draft.name.trim() && draft.prompt.trim());
}

/** Zona horaria del equipo, con reserva a UTC si el sistema no la expone. */
export function resolvedSchedulerTimezone(): string {
  return Intl.DateTimeFormat().resolvedOptions().timeZone || "UTC";
}

/** Un día de la agenda, o la cesta de ejecuciones atrasadas. */
export type SchedulerCalendarDay = {
  key: string;
  label: string;
  items: ScheduledCalendarOccurrence[];
};

/**
 * Agrupa la agenda por día natural, con las atrasadas aparte.
 *
 * Extraído de `App.tsx` (fase 2). Las atrasadas no van al día que les tocaba:
 * van a una cesta propia, porque lo que la persona necesita saber es que
 * quedaron sin ejecutar, no en qué fecha debieron hacerlo. El resto se agrupa
 * por la fecha **local**, que es la que aparece en pantalla.
 *
 * Conserva el orden de llegada: la agenda ya viene ordenada desde
 * `scheduledCalendarOccurrences`, y reordenar aquí duplicaría esa decisión.
 */
export function schedulerCalendarDays(
  occurrences: ScheduledCalendarOccurrence[]
): SchedulerCalendarDay[] {
  const days = new Map<string, SchedulerCalendarDay>();
  for (const item of occurrences) {
    const date = new Date(item.startsAt);
    const key = item.overdue
      ? "overdue"
      : `${date.getFullYear()}-${String(date.getMonth() + 1).padStart(2, "0")}-${String(
          date.getDate()
        ).padStart(2, "0")}`;
    const label = item.overdue
      ? "Pendientes atrasadas"
      : date.toLocaleDateString("es-ES", {
          weekday: "long",
          day: "numeric",
          month: "long"
        });
    const day = days.get(key) ?? { key, label, items: [] };
    day.items.push(item);
    days.set(key, day);
  }
  return [...days.values()];
}

/**
 * Número de conflictos distintos en la agenda.
 *
 * Cada conflicto lo declaran **las dos** automatizaciones implicadas, así que
 * sumar las declaraciones cuenta cada pareja dos veces. Se divide entre dos
 * para informar de conflictos, no de menciones.
 */
export function schedulerCalendarConflictCount(
  occurrences: ScheduledCalendarOccurrence[]
): number {
  const mentions = occurrences.reduce(
    (total, item) => total + item.conflictingTaskIds.length,
    0
  );
  return Math.floor(mentions / 2);
}

/**
 * Avisos ya leídos, tolerando un almacenamiento ausente o dañado.
 *
 * Un valor ilegible degrada a «ninguno leído», nunca a un error: la marca de
 * lectura es una comodidad, y perderla no debe impedir usar el historial.
 */
export function loadSchedulerReadNotifications(): Set<string> {
  try {
    const stored = window.localStorage.getItem(SCHEDULER_READ_NOTIFICATIONS_KEY);
    const values = stored ? JSON.parse(stored) : [];
    return new Set(
      Array.isArray(values) ? values.filter((value) => typeof value === "string") : []
    );
  } catch {
    return new Set();
  }
}

/**
 * Indica si la clave existe, aunque esté vacía.
 *
 * Distingue «primera vez que se abre la aplicación» de «se leyeron todos los
 * avisos»: en el primer caso los avisos previos se marcan como leídos para no
 * avisar de finalizaciones que ocurrieron antes de instalar nada.
 */
export function schedulerReadNotificationsExist(): boolean {
  try {
    return window.localStorage.getItem(SCHEDULER_READ_NOTIFICATIONS_KEY) !== null;
  } catch {
    return false;
  }
}

/** Guarda los avisos leídos, acotados a los más recientes. */
export function persistSchedulerReadNotifications(ids: Set<string>): void {
  try {
    window.localStorage.setItem(
      SCHEDULER_READ_NOTIFICATIONS_KEY,
      JSON.stringify([...ids].slice(-MAX_READ_NOTIFICATIONS))
    );
  } catch {
    // El historial de ejecuciones sigue disponible aunque WebView2 no permita
    // almacenamiento local.
  }
}
