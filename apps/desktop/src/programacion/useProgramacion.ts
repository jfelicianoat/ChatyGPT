/**
 * Estado y acciones del planificador de tareas.
 *
 * Vivia dentro de `App.tsx`, que habia crecido hasta ser inmanejable. Sacarlo a
 * un hook deja el estado junto a los manejadores que lo tocan y permite que la
 * tarjeta reciba `ReturnType<typeof useProgramacion>` en lugar de setenta props
 * sueltas.
 */
import { useEffect, useMemo, useRef, useState } from "react";

import {
  filterScheduledTasks,
  scheduledCalendarOccurrences,
  scheduledNotifications,
  scheduledTaskDuplicateDraft,
  type Loadable,
  type ScheduledNotificationView,
  type ScheduledHistoryPeriodFilter,
  type ScheduledHistorySort,
  type ScheduledHistoryStatusFilter,
  type ScheduledRunPageView,
  type ScheduledTaskTemplateView,
  type ScheduledTaskView,
  type WindowsStartupStatus
} from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";
import {
  canSaveScheduleTemplate,
  defaultScheduledLocalTime,
  loadSchedulerReadNotifications,
  pendingScheduledRunNotifications,
  persistSchedulerReadNotifications,
  resolvedSchedulerTimezone,
  scheduledLocalTimeValue,
  schedulerCalendarConflictCount,
  schedulerCalendarDays,
  schedulerReadNotificationsExist,
  validateScheduleDraft
} from "../schedulerView";

/**
 * @param bootstrapListo si el arranque de la aplicacion ya termino; hasta
 *   entonces no se sondea al backend, igual que hacia `App.tsx`.
 */
export function useProgramacion(bootstrapListo: boolean) {
  const [scheduledTasks, setScheduledTasks] =
    useState<Loadable<ScheduledTaskView[]>>({ state: "loading" });
  const [scheduledTaskTemplates, setScheduledTaskTemplates] =
    useState<Loadable<ScheduledTaskTemplateView[]>>({ state: "loading" });
  const [scheduleSearchQuery, setScheduleSearchQuery] = useState("");
  const [scheduleName, setScheduleName] = useState("");
  const [scheduleConversationId, setScheduleConversationId] = useState("");
  const [schedulePrompt, setSchedulePrompt] = useState("");
  const [scheduleAt, setScheduleAt] = useState(defaultScheduledLocalTime);
  const [scheduleExpression, setScheduleExpression] =
    useState<ScheduledTaskView["scheduleExpression"]>("once");
  const [scheduleConfirmed, setScheduleConfirmed] = useState(false);
  const [scheduleEditingId, setScheduleEditingId] = useState<string | null>(null);
  const [scheduleBusyId, setScheduleBusyId] = useState<string | null>(null);
  const [scheduleError, setScheduleError] = useState<string | null>(null);
  const [scheduleNotice, setScheduleNotice] = useState<string | null>(null);
  const [scheduledHistoryStatus, setScheduledHistoryStatus] =
    useState<ScheduledHistoryStatusFilter>("all");
  const [scheduledHistoryPeriod, setScheduledHistoryPeriod] =
    useState<ScheduledHistoryPeriodFilter>("all");
  const [scheduledHistoryTaskId, setScheduledHistoryTaskId] = useState<string | null>(null);
  const [scheduledHistoryPageNumber, setScheduledHistoryPageNumber] = useState(1);
  const [scheduledHistoryPageSize, setScheduledHistoryPageSize] =
    useState<ScheduledRunPageView["pageSize"]>(10);
  const [scheduledHistorySort, setScheduledHistorySort] =
    useState<ScheduledHistorySort>("newest");
  const [scheduledHistoryPage, setScheduledHistoryPage] =
    useState<Loadable<ScheduledRunPageView> | null>(null);
  const [scheduledHistoryRefreshVersion, setScheduledHistoryRefreshVersion] = useState(0);
  const [schedulerCenterOpen, setSchedulerCenterOpen] = useState(false);
  const [schedulerCalendarOpen, setSchedulerCalendarOpen] = useState(false);
  const [schedulerCalendarRange, setSchedulerCalendarRange] =
    useState<7 | 14 | 30>(14);
  const [schedulerCalendarExportMessage, setSchedulerCalendarExportMessage] = useState<
    { kind: "success" | "error"; text: string } | null
  >(null);
  const [windowsStartup, setWindowsStartup] = useState<Loadable<WindowsStartupStatus>>({
    state: "loading"
  });
  const [schedulerReadIds, setSchedulerReadIds] =
    useState<Set<string>>(loadSchedulerReadNotifications);
  const [schedulerNotifications, setSchedulerNotifications] = useState<
    NotificationPermission | "unsupported"
  >(() => {
    if (!("Notification" in window)) return "unsupported";
    return window.Notification.permission;
  });
  const scheduledRunStatesRef = useRef<Map<string, string>>(new Map());
  const schedulerHistoryInitializedRef = useRef(false);
  const schedulerReadStateExistedRef = useRef(schedulerReadNotificationsExist());

  // El estado de arranque con Windows se pide una sola vez al montar, igual
  // que antes hacia el efecto de arranque general.
  useEffect(() => {
    platform.getWindowsStartupStatus()
      .then((value) => setWindowsStartup({ state: "ready", value }))
      .catch((error) =>
        setWindowsStartup({ state: "error", message: describeError(error) })
      );
  }, []);

  useEffect(() => {
    if (!bootstrapListo) return;
    const refresh = () => {
      platform.listScheduledTasks()
        .then((value) => {
          const { notifications, nextStates } = pendingScheduledRunNotifications({
            tasks: value,
            knownStates: scheduledRunStatesRef.current,
            historyInitialized: schedulerHistoryInitializedRef.current,
            permissionGranted: schedulerNotifications === "granted"
          });
          for (const notification of notifications) {
            try {
              new window.Notification(notification.title, {
                body: notification.body,
                tag: notification.tag
              });
            } catch {
              // El historial visible sigue siendo la fuente durable si Windows
              // rechaza el aviso.
            }
          }
          scheduledRunStatesRef.current = nextStates;
          schedulerHistoryInitializedRef.current = true;
          setScheduledTasks({ state: "ready", value });
        })
        .catch((error) =>
          setScheduledTasks({ state: "error", message: describeError(error) })
        );
    };
    const interval = window.setInterval(refresh, 10_000);
    return () => window.clearInterval(interval);
  }, [bootstrapListo, schedulerNotifications]);

  useEffect(() => {
    if (!scheduledHistoryTaskId) {
      setScheduledHistoryPage(null);
      return;
    }
    let disposed = false;
    const load = (showLoading: boolean) => {
      if (showLoading) setScheduledHistoryPage({ state: "loading" });
      platform.listScheduledRuns(
        scheduledHistoryTaskId,
        scheduledHistoryStatus,
        scheduledHistoryPeriod,
        scheduledHistorySort,
        scheduledHistoryPageNumber,
        scheduledHistoryPageSize
      )
        .then((value) => {
          if (disposed) return;
          setScheduledHistoryPage({ state: "ready", value });
          if (value.page !== scheduledHistoryPageNumber) {
            setScheduledHistoryPageNumber(value.page);
          }
        })
        .catch((error) => {
          if (!disposed) {
            setScheduledHistoryPage({ state: "error", message: describeError(error) });
          }
        });
    };
    load(true);
    const interval = window.setInterval(() => load(false), 10_000);
    return () => {
      disposed = true;
      window.clearInterval(interval);
    };
  }, [
    scheduledHistoryTaskId,
    scheduledHistoryStatus,
    scheduledHistoryPeriod,
    scheduledHistorySort,
    scheduledHistoryPageNumber,
    scheduledHistoryPageSize,
    scheduledHistoryRefreshVersion
  ]);

  const schedulerCenterItems = useMemo(
    () => scheduledTasks.state === "ready"
      ? scheduledNotifications(scheduledTasks.value)
      : [],
    [scheduledTasks]
  );
  const schedulerUnreadCount = schedulerCenterItems.filter(
    (item) => !schedulerReadIds.has(item.id)
  ).length;
  const schedulerCalendarItems = useMemo(
    () => scheduledTasks.state === "ready"
      ? scheduledCalendarOccurrences(
          scheduledTasks.value,
          new Date(),
          schedulerCalendarRange
        )
      : [],
    [scheduledTasks, schedulerCalendarRange]
  );
  const schedulerCalendarGroupedDays = useMemo(
    () => schedulerCalendarDays(schedulerCalendarItems),
    [schedulerCalendarItems]
  );
  const schedulerCalendarConflicts = schedulerCalendarConflictCount(
    schedulerCalendarItems
  );

  const createSchedule = async () => {
    const validation = validateScheduleDraft({
      name: scheduleName,
      conversationId: scheduleConversationId,
      prompt: schedulePrompt,
      at: scheduleAt,
      confirmed: scheduleConfirmed
    });
    if (validation.status === "incomplete") return;
    setScheduleBusyId("create");
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      if (validation.status === "invalid-date") {
        throw new Error(validation.message);
      }
      const dueAt = new Date(validation.dueAtIso);
      const timezone = resolvedSchedulerTimezone();
      if (scheduleEditingId) {
        await platform.updateScheduledTask(
          scheduleEditingId,
          scheduleName.trim(),
          scheduleConversationId,
          schedulePrompt.trim(),
          dueAt.toISOString(),
          timezone,
          scheduleExpression
        );
      } else {
        await platform.createScheduledTask(
          scheduleName.trim(),
          scheduleConversationId,
          schedulePrompt.trim(),
          dueAt.toISOString(),
          timezone,
          scheduleExpression
        );
      }
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleName("");
      setSchedulePrompt("");
      setScheduleAt(defaultScheduledLocalTime());
      setScheduleExpression("once");
      setScheduleConfirmed(false);
      setScheduleEditingId(null);
      setScheduleNotice(
        scheduleEditingId
          ? "Cambios guardados. La programación vuelve a estar activa con la nueva fecha."
          : scheduleExpression === "once"
          ? "Programación guardada y activa. ChatyGPT la ejecutará una sola vez a la hora indicada."
          : `Programación guardada y activa. Se repetirá ${
              scheduleExpression === "daily" ? "cada día" : "cada semana"
            } a la hora indicada.`
      );
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const beginScheduleEdit = (task: ScheduledTaskView) => {
    setScheduleEditingId(task.id);
    setScheduleName(task.name);
    setScheduleConversationId(task.conversationId ?? "");
    setSchedulePrompt(task.prompt);
    setScheduleExpression(task.scheduleExpression);
    setScheduleAt(
      task.nextRunAt
        ? scheduledLocalTimeValue(new Date(task.nextRunAt))
        : defaultScheduledLocalTime()
    );
    setScheduleConfirmed(false);
    setScheduleError(null);
    setScheduleNotice(
      "Revisa los cambios y vuelve a marcar la confirmación para guardar."
    );
    document.querySelector(".scheduler-card")?.scrollIntoView({
      behavior: "smooth",
      block: "start"
    });
  };

  const duplicateSchedule = (task: ScheduledTaskView) => {
    const duplicate = scheduledTaskDuplicateDraft(task);
    setScheduleEditingId(null);
    setScheduleName(duplicate.name);
    setScheduleConversationId(duplicate.conversationId);
    setSchedulePrompt(duplicate.prompt);
    setScheduleExpression(duplicate.scheduleExpression);
    setScheduleAt(defaultScheduledLocalTime());
    setScheduleConfirmed(duplicate.confirmed);
    setScheduleError(null);
    setScheduleNotice(
      "Copia preparada como borrador. Revisa la fecha y vuelve a confirmar antes de activarla."
    );
    document.querySelector(".scheduler-card")?.scrollIntoView({
      behavior: "smooth",
      block: "start"
    });
  };

  const cancelScheduleEdit = () => {
    setScheduleEditingId(null);
    setScheduleName("");
    setSchedulePrompt("");
    setScheduleAt(defaultScheduledLocalTime());
    setScheduleExpression("once");
    setScheduleConfirmed(false);
    setScheduleNotice(null);
  };

  const saveScheduledTaskTemplate = async () => {
    if (!canSaveScheduleTemplate({ name: scheduleName, prompt: schedulePrompt })) return;
    setScheduleBusyId("template-create");
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.createScheduledTaskTemplate(
        scheduleName.trim(),
        schedulePrompt.trim(),
        scheduleExpression
      );
      setScheduledTaskTemplates({
        state: "ready",
        value: await platform.listScheduledTaskTemplates()
      });
      setScheduleNotice(
        "Plantilla guardada. No se ha programado ni activado ninguna ejecución."
      );
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const applyScheduledTaskTemplate = (template: ScheduledTaskTemplateView) => {
    setScheduleEditingId(null);
    setScheduleName(template.name);
    setSchedulePrompt(template.prompt);
    setScheduleExpression(template.scheduleExpression);
    setScheduleConfirmed(false);
    setScheduleError(null);
    setScheduleNotice(
      "Plantilla aplicada. Elige conversación y fecha, revisa el contenido y confirma para activarla."
    );
  };

  const removeScheduledTaskTemplate = async (
    template: ScheduledTaskTemplateView
  ) => {
    if (!window.confirm(`¿Eliminar la plantilla “${template.name}”?`)) return;
    setScheduleBusyId(template.id);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.deleteScheduledTaskTemplate(template.id);
      setScheduledTaskTemplates({
        state: "ready",
        value: await platform.listScheduledTaskTemplates()
      });
      setScheduleNotice(`Se ha eliminado la plantilla “${template.name}”.`);
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const enableSchedulerNotifications = async () => {
    setScheduleError(null);
    if (!("Notification" in window)) {
      setSchedulerNotifications("unsupported");
      setScheduleError(
        "Esta versión de WebView2 no permite avisos de Windows. El historial seguirá actualizándose."
      );
      return;
    }
    try {
      const permission = await window.Notification.requestPermission();
      setSchedulerNotifications(permission);
      if (permission === "granted") {
        new window.Notification("Avisos de ChatyGPT activados", {
          body: "Te avisaré cuando termine una tarea programada.",
          tag: "chatygpt-scheduler-permission"
        });
      } else {
        setScheduleError(
          "Windows no concedió permiso para mostrar avisos. Puedes seguir usando el historial."
        );
      }
    } catch (error) {
      setScheduleError(describeError(error));
    }
  };

  const markSchedulerNotificationRead = (item: ScheduledNotificationView) => {
    setSchedulerReadIds((current) => {
      const updated = new Set(current);
      updated.add(item.id);
      persistSchedulerReadNotifications(updated);
      return updated;
    });
  };

  const markAllSchedulerNotificationsRead = () => {
    setSchedulerReadIds((current) => {
      const updated = new Set(current);
      for (const item of schedulerCenterItems) updated.add(item.id);
      persistSchedulerReadNotifications(updated);
      return updated;
    });
  };

  const toggleScheduledHistory = (task: ScheduledTaskView) => {
    if (scheduledHistoryTaskId === task.id) {
      setScheduledHistoryTaskId(null);
      return;
    }
    setScheduledHistoryTaskId(task.id);
    setScheduledHistoryPageNumber(1);
    setScheduledHistoryPage(null);
  };

  const retryScheduledRun = async (
    task: ScheduledTaskView,
    run: ScheduledTaskView["runs"][number]
  ) => {
    if (
      !window.confirm(
        `¿Reintentar “${task.name}”? Se conservará el intento fallido en el historial.`
      )
    ) return;
    setScheduleBusyId(run.id);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.retryScheduledRun(run.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleNotice(
        `Se ha iniciado un nuevo intento de “${task.name}”. El fallo anterior se conserva.`
      );
      setScheduledHistoryRefreshVersion((current) => current + 1);
    } catch (error) {
      setScheduleError(describeError(error));
      try {
        setScheduledTasks({
          state: "ready",
          value: await platform.listScheduledTasks()
        });
      } catch {
        // Se mantiene el error original si también falla la actualización visual.
      }
    } finally {
      setScheduleBusyId(null);
    }
  };

  const runScheduledTaskNow = async (task: ScheduledTaskView) => {
    const recurringNote = task.nextRunAt
      ? " La próxima fecha programada no cambiará."
      : " Esta ejecución no reactivará una programación finalizada o pausada.";
    if (!window.confirm(`¿Ejecutar ahora “${task.name}”?${recurringNote}`)) return;
    const busyId = `run-now:${task.id}`;
    setScheduleBusyId(busyId);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.runScheduledTaskNow(task.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleNotice(
        `Se ha iniciado “${task.name}” ahora. Su programación futura no ha cambiado.`
      );
      setScheduledHistoryRefreshVersion((current) => current + 1);
    } catch (error) {
      setScheduleError(describeError(error));
      try {
        setScheduledTasks({
          state: "ready",
          value: await platform.listScheduledTasks()
        });
      } catch {
        // Se mantiene el error original si también falla la actualización visual.
      }
    } finally {
      setScheduleBusyId(null);
    }
  };

  const cancelScheduledRun = async (
    task: ScheduledTaskView,
    run: ScheduledTaskView["runs"][number]
  ) => {
    const recurringNote = task.scheduleExpression === "once"
      ? ""
      : " La próxima repetición seguirá programada.";
    if (
      !window.confirm(
        `¿Cancelar la ejecución activa de “${task.name}”?${recurringNote}`
      )
    ) return;
    setScheduleBusyId(run.id);
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      await platform.cancelScheduledRun(run.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
      setScheduleNotice(
        `Se ha cancelado la ejecución activa de “${task.name}”.` + recurringNote
      );
      setScheduledHistoryRefreshVersion((current) => current + 1);
    } catch (error) {
      setScheduleError(describeError(error));
      try {
        setScheduledTasks({
          state: "ready",
          value: await platform.listScheduledTasks()
        });
      } catch {
        // Se mantiene el error de cancelación si también falla la actualización visual.
      }
    } finally {
      setScheduleBusyId(null);
    }
  };

  const exportScheduledHistory = async () => {
    setScheduleBusyId("export");
    setScheduleError(null);
    setScheduleNotice(null);
    try {
      const selection = await platform.pickScheduledHistoryExportPath();
      if (!selection) return;
      const report = await platform.exportScheduledHistory(
        selection.path,
        scheduledHistoryStatus,
        scheduledHistoryPeriod,
        selection.existed
      );
      setScheduleNotice(
        `Historial exportado: ${report.runCount} ejecución(es) en ${report.destinationPath}`
      );
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const exportScheduledCalendar = async () => {
    if (schedulerCalendarItems.length === 0) {
      setSchedulerCalendarExportMessage({
        kind: "error",
        text: "No hay fechas visibles para exportar en este periodo."
      });
      return;
    }
    setScheduleBusyId("calendar-export");
    setSchedulerCalendarExportMessage(null);
    try {
      const selection = await platform.pickScheduledCalendarExportPath();
      if (!selection) return;
      const report = await platform.exportScheduledCalendar(
        selection.path,
        schedulerCalendarItems.map((item) => ({
          occurrenceId: item.id,
          taskName: item.taskName,
          conversationTitle: item.conversationTitle,
          startsAt: item.startsAt,
          projected: item.projected,
          overdue: item.overdue
        })),
        schedulerCalendarRange,
        selection.existed
      );
      setSchedulerCalendarExportMessage({
        kind: "success",
        text: `Calendario exportado: ${report.eventCount} evento(s) en ${report.destinationPath}`
      });
    } catch (error) {
      setSchedulerCalendarExportMessage({ kind: "error", text: describeError(error) });
    } finally {
      setScheduleBusyId(null);
    }
  };

  const toggleWindowsStartup = async () => {
    if (windowsStartup.state !== "ready") return;
    const enabled = !windowsStartup.value.enabled;
    if (
      enabled &&
      !window.confirm(
        "¿Activar el inicio automático con Windows? El token actual del Broker se guardará cifrado para esta cuenta de Windows y ChatyGPT esperará a que el Broker esté disponible antes de abrirse."
      )
    ) return;
    setScheduleBusyId("windows-startup");
    try {
      const value = await platform.setWindowsStartupEnabled(enabled);
      setWindowsStartup({ state: "ready", value });
    } catch (error) {
      setWindowsStartup({ state: "error", message: describeError(error) });
    } finally {
      setScheduleBusyId(null);
    }
  };

  const reloadWindowsStartupStatus = async () => {
    setWindowsStartup({ state: "loading" });
    try {
      setWindowsStartup({
        state: "ready",
        value: await platform.getWindowsStartupStatus()
      });
    } catch (error) {
      setWindowsStartup({ state: "error", message: describeError(error) });
    }
  };

  const toggleSchedule = async (task: ScheduledTaskView) => {
    // Rust solo exige confirmación al reactivar, y con razón: pausar no ejecuta
    // nada, mientras que reactivar devuelve a la tarea la capacidad de lanzar
    // trabajos contra el Broker sin que nadie esté delante. Se pregunta en el
    // mismo caso, para que la comprobación del backend responda a una decisión.
    if (
      !task.enabled &&
      !window.confirm(
        `¿Reactivar «${task.name}»? Volverá a ejecutarse sola en la fecha prevista.`
      )
    ) {
      return;
    }
    setScheduleBusyId(task.id);
    setScheduleError(null);
    try {
      await platform.setScheduledTaskEnabled(task.id, !task.enabled);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const removeSchedule = async (task: ScheduledTaskView) => {
    if (!window.confirm(`¿Eliminar la programación “${task.name}”?`)) return;
    setScheduleBusyId(task.id);
    setScheduleError(null);
    try {
      await platform.deleteScheduledTask(task.id);
      setScheduledTasks({
        state: "ready",
        value: await platform.listScheduledTasks()
      });
    } catch (error) {
      setScheduleError(describeError(error));
    } finally {
      setScheduleBusyId(null);
    }
  };

  const visibleScheduledTasks = scheduledTasks.state === "ready"
    ? filterScheduledTasks(scheduledTasks.value, scheduleSearchQuery)
    : [];

  return {
    applyScheduledTaskTemplate,
    beginScheduleEdit,
    cancelScheduleEdit,
    cancelScheduledRun,
    createSchedule,
    duplicateSchedule,
    enableSchedulerNotifications,
    exportScheduledCalendar,
    exportScheduledHistory,
    markAllSchedulerNotificationsRead,
    markSchedulerNotificationRead,
    reloadWindowsStartupStatus,
    removeSchedule,
    removeScheduledTaskTemplate,
    retryScheduledRun,
    runScheduledTaskNow,
    saveScheduledTaskTemplate,
    scheduleAt,
    scheduleBusyId,
    scheduleConfirmed,
    scheduleConversationId,
    scheduleEditingId,
    scheduleError,
    scheduleExpression,
    scheduleName,
    scheduleNotice,
    schedulePrompt,
    scheduleSearchQuery,
    scheduledHistoryPage,
    scheduledHistoryPageNumber,
    scheduledHistoryPageSize,
    scheduledHistoryPeriod,
    scheduledHistoryRefreshVersion,
    scheduledHistorySort,
    scheduledHistoryStatus,
    scheduledHistoryTaskId,
    scheduledRunStatesRef,
    scheduledTaskTemplates,
    scheduledTasks,
    schedulerCalendarConflicts,
    schedulerCalendarExportMessage,
    schedulerCalendarGroupedDays,
    schedulerCalendarItems,
    schedulerCalendarOpen,
    schedulerCalendarRange,
    schedulerCenterItems,
    schedulerCenterOpen,
    schedulerHistoryInitializedRef,
    schedulerNotifications,
    schedulerReadIds,
    schedulerReadStateExistedRef,
    schedulerUnreadCount,
    setScheduleAt,
    setScheduleBusyId,
    setScheduleConfirmed,
    setScheduleConversationId,
    setScheduleEditingId,
    setScheduleError,
    setScheduleExpression,
    setScheduleName,
    setScheduleNotice,
    setSchedulePrompt,
    setScheduleSearchQuery,
    setScheduledHistoryPage,
    setScheduledHistoryPageNumber,
    setScheduledHistoryPageSize,
    setScheduledHistoryPeriod,
    setScheduledHistoryRefreshVersion,
    setScheduledHistorySort,
    setScheduledHistoryStatus,
    setScheduledHistoryTaskId,
    setScheduledTaskTemplates,
    setScheduledTasks,
    setSchedulerCalendarExportMessage,
    setSchedulerCalendarOpen,
    setSchedulerCalendarRange,
    setSchedulerCenterOpen,
    setSchedulerNotifications,
    setSchedulerReadIds,
    setWindowsStartup,
    toggleSchedule,
    toggleScheduledHistory,
    toggleWindowsStartup,
    visibleScheduledTasks,
    windowsStartup
  };
}
