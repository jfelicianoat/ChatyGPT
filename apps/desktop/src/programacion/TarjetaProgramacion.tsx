/**
 * Tarjeta del planificador de tareas.
 *
 * Recibe el hook entero en lugar de sesenta props sueltas: `useProgramacion`
 * ya agrupa el estado con las acciones que lo modifican, asi que pasarlo tal
 * cual evita inventarse una interfaz paralela que habria que mantener a mano.
 */
import {
  filterScheduledRuns,
  scheduledRunDetail,
  type ConversationSummary,
  type ScheduledHistoryPeriodFilter,
  type ScheduledHistorySort,
  type ScheduledHistoryStatusFilter,
  type ScheduledRunPageView,
  type ScheduledTaskView
} from "../domain";
import type { WorkspaceDestination } from "../navegacion";
import { defaultScheduledLocalTime, scheduledRunLabel } from "../schedulerView";
import type { useProgramacion } from "./useProgramacion";

type Props = {
  programacion: ReturnType<typeof useProgramacion>;
  conversations: ConversationSummary[];
  openConversation: (conversationId: string) => Promise<void>;
  setWorkspaceDestination: (destino: WorkspaceDestination) => void;
};

export function TarjetaProgramacion({
  programacion,
  conversations,
  openConversation,
  setWorkspaceDestination
}: Props) {
  const {
    scheduledHistoryTaskId,
    scheduledHistoryStatus,
    scheduledHistoryPeriod,
    scheduledHistorySort,
    scheduledHistoryPageSize,
    schedulerCalendarItems,
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
    scheduleTimezone,
    scheduledHistoryPage,
    scheduledTaskTemplates,
    scheduledTasks,
    schedulerCalendarConflicts,
    schedulerCalendarExportMessage,
    schedulerCalendarGroupedDays,
    schedulerCalendarOpen,
    schedulerCalendarRange,
    schedulerCenterItems,
    schedulerCenterOpen,
    schedulerNotifications,
    schedulerReadIds,
    schedulerUnreadCount,
    setScheduleAt,
    setScheduleConfirmed,
    setScheduleConversationId,
    setScheduleExpression,
    setScheduleName,
    setSchedulePrompt,
    setScheduleSearchQuery,
    setScheduledHistoryPageNumber,
    setScheduledHistoryPageSize,
    setScheduledHistoryPeriod,
    setScheduledHistorySort,
    setScheduledHistoryStatus,
    setSchedulerCalendarExportMessage,
    setSchedulerCalendarOpen,
    setSchedulerCalendarRange,
    setSchedulerCenterOpen,
    toggleSchedule,
    toggleScheduledHistory,
    toggleWindowsStartup,
    visibleScheduledTasks,
    windowsStartup
  } = programacion;

  return (
  <section className="scheduler-card">
    <div className="panel-heading">
      <div>
        <span className="kicker">Fase 4 · Automatización local</span>
        <h3>Tareas programadas</h3>
      </div>
      <div className="scheduler-heading-actions">
        {scheduledTasks.state === "ready" && (
          <span className="badge">
            {scheduledTasks.value.filter((task) => task.enabled).length} activa(s)
          </span>
        )}
        <button
          className={schedulerCalendarOpen ? "primary" : "secondary"}
          onClick={() => setSchedulerCalendarOpen((current) => !current)}
          aria-expanded={schedulerCalendarOpen}
        >
          Calendario
        </button>
        <button
          className={schedulerCenterOpen ? "primary" : "secondary"}
          onClick={() => setSchedulerCenterOpen((current) => !current)}
          aria-expanded={schedulerCenterOpen}
        >
          Avisos{schedulerUnreadCount > 0 ? ` (${schedulerUnreadCount})` : ""}
        </button>
        <button
          className={schedulerNotifications === "granted" ? "secondary" : "primary"}
          onClick={() => void enableSchedulerNotifications()}
          disabled={
            schedulerNotifications === "granted" ||
            schedulerNotifications === "unsupported"
          }
        >
          {schedulerNotifications === "granted"
            ? "Avisos de Windows activos"
            : schedulerNotifications === "unsupported"
              ? "Avisos no disponibles"
              : "Activar avisos de Windows"}
        </button>
      </div>
    </div>
    <p className="muted">
      Programa una instrucción para una conversación existente. La hora se guarda
      con tu zona horaria y cada ejecución queda registrada. Si ChatyGPT está
      cerrado a esa hora, la tarea se iniciará al volver a abrirlo.
    </p>
    <div className="scheduler-safety">
      La programación no modifica archivos ni concede herramientas. Crear, editar,
      reactivar o reintentar requiere tu confirmación.
    </div>
    <div className="scheduler-startup-panel">
      <div>
        <strong>Inicio con Windows</strong>
        {windowsStartup.state === "loading" && (
          <span>Comprobando la configuración…</span>
        )}
        {windowsStartup.state === "ready" && (
          <>
            <span>{windowsStartup.value.message}</span>
            <small>
              {windowsStartup.value.enabled
                ? "La credencial está protegida con DPAPI para tu cuenta de Windows. Si cambia el token del Broker, abre una vez ChatyGPT con el BAT para actualizarla."
                : "No instala servicios ni requiere permisos de administrador."}
            </small>
          </>
        )}
        {windowsStartup.state === "error" && (
          <span className="error" role="alert">{windowsStartup.message}</span>
        )}
      </div>
      <div>
        {windowsStartup.state === "ready" && windowsStartup.value.supported && (
          <button
            className={windowsStartup.value.enabled ? "secondary" : "primary"}
            onClick={() => void toggleWindowsStartup()}
            disabled={scheduleBusyId !== null}
          >
            {scheduleBusyId === "windows-startup"
              ? "Aplicando…"
              : windowsStartup.value.enabled
                ? "Desactivar"
                : "Activar"}
          </button>
        )}
        {windowsStartup.state === "error" && (
          <button
            className="secondary"
            onClick={() => void reloadWindowsStartupStatus()}
          >
            Volver a comprobar
          </button>
        )}
      </div>
    </div>
    {schedulerCenterOpen && (
      <div className="scheduler-notification-center">
        <div className="scheduler-notification-heading">
          <div>
            <strong>Centro de avisos</strong>
            <span>
              Finalizaciones recientes de las tareas programadas.
            </span>
          </div>
          {schedulerUnreadCount > 0 && (
            <button
              className="secondary"
              onClick={markAllSchedulerNotificationsRead}
            >
              Marcar todo como leído
            </button>
          )}
        </div>
        {schedulerCenterItems.length === 0 ? (
          <p className="activity-empty">
            Los avisos aparecerán cuando finalice una tarea.
          </p>
        ) : (
          <div className="scheduler-notification-list">
            {schedulerCenterItems.map((item) => {
              const unread = !schedulerReadIds.has(item.id);
              return (
                <article
                  key={item.id}
                  className={`scheduler-notification ${unread ? "unread" : ""}`}
                >
                  <div>
                    <strong>{item.taskName}</strong>
                    <span>
                      {scheduledRunLabel(item.status)}
                      {item.attempt > 1 ? ` · intento ${item.attempt}` : ""}
                      {" · "}
                      {new Date(item.updatedAt).toLocaleString("es-ES", {
                        dateStyle: "short",
                        timeStyle: "short"
                      })}
                    </span>
                    <small>{item.conversationTitle}</small>
                  </div>
                  <button
                    className="secondary"
                    onClick={() => {
                      markSchedulerNotificationRead(item);
                      void openConversation(item.conversationId);
                    }}
                  >
                    Abrir
                  </button>
                </article>
              );
            })}
          </div>
        )}
      </div>
    )}
    {schedulerCalendarOpen && (
      <div className="scheduler-calendar-panel">
        <div className="scheduler-calendar-heading">
          <div>
            <strong>Próximas automatizaciones</strong>
            <span>
              La primera fecha es la guardada. Las repeticiones posteriores son
              una proyección informativa y no ejecutan ni modifican tareas.
            </span>
          </div>
          <div className="scheduler-calendar-actions">
            <label>
              <span>Periodo</span>
              <select
                value={schedulerCalendarRange}
                onChange={(event) => {
                  setSchedulerCalendarRange(
                    Number(event.target.value) as 7 | 14 | 30
                  );
                  setSchedulerCalendarExportMessage(null);
                }}
              >
                <option value={7}>7 días</option>
                <option value={14}>14 días</option>
                <option value={30}>30 días</option>
              </select>
            </label>
            <button
              className="secondary"
              onClick={() => void exportScheduledCalendar()}
              disabled={
                scheduleBusyId !== null || schedulerCalendarItems.length === 0
              }
            >
              {scheduleBusyId === "calendar-export"
                ? "Exportando…"
                : "Exportar .ics"}
            </button>
          </div>
        </div>
        {schedulerCalendarExportMessage && (
          <div
            className={`scheduler-calendar-export-message ${
              schedulerCalendarExportMessage.kind
            }`}
            role={
              schedulerCalendarExportMessage.kind === "error" ? "alert" : "status"
            }
          >
            {schedulerCalendarExportMessage.text}
          </div>
        )}
        {schedulerCalendarConflicts > 0 && (
          <div className="scheduler-calendar-warning" role="status">
            {schedulerCalendarConflicts} coincidencia(s): hay tareas distintas
            separadas por 15 minutos o menos. Revisa si quieres evitar que compitan
            por los mismos recursos.
          </div>
        )}
        {schedulerCalendarGroupedDays.length === 0 ? (
          <p className="activity-empty">
            No hay tareas activas dentro de este periodo.
          </p>
        ) : (
          <div className="scheduler-calendar-days">
            {schedulerCalendarGroupedDays.map((day) => (
              <section key={day.key} className="scheduler-calendar-day">
                <h4>{day.label}</h4>
                <div>
                  {day.items.map((item) => (
                    <article
                      key={item.id}
                      className={`scheduler-calendar-occurrence ${
                        item.conflictingTaskIds.length > 0 ? "conflict" : ""
                      } ${item.overdue ? "overdue" : ""}`}
                    >
                      <time>
                        {new Date(item.startsAt).toLocaleTimeString("es-ES", {
                          hour: "2-digit",
                          minute: "2-digit",
                          timeZone: item.timezone
                        })}
                      </time>
                      <div>
                        <strong>{item.taskName}</strong>
                        <span>{item.conversationTitle}</span>
                        <small>
                          {item.overdue
                            ? "Atrasada"
                            : item.projected
                              ? "Proyección"
                              : "Próxima guardada"}
                          {item.conflictingTaskIds.length > 0
                            ? ` · Coincide con ${item.conflictingTaskIds.length}`
                            : ""}
                          {` · ${item.timezone}`}
                        </small>
                      </div>
                      <button
                        className="secondary compact"
                        onClick={() => void openConversation(item.conversationId)}
                      >
                        Abrir chat
                      </button>
                    </article>
                  ))}
                </div>
              </section>
            ))}
          </div>
        )}
      </div>
    )}
    <div className="scheduler-template-panel">
      <div className="scheduler-template-heading">
        <div>
          <strong>Plantillas reutilizables</strong>
          <span>
            Guardan nombre, instrucción y repetición; nunca la conversación,
            la fecha ni la autorización.
          </span>
        </div>
        {scheduledTaskTemplates.state === "ready" && (
          <span className="badge">
            {scheduledTaskTemplates.value.length} guardada(s)
          </span>
        )}
      </div>
      {scheduledTaskTemplates.state === "loading" && (
        <p className="muted">Cargando plantillas…</p>
      )}
      {scheduledTaskTemplates.state === "error" && (
        <p className="error">{scheduledTaskTemplates.message}</p>
      )}
      {scheduledTaskTemplates.state === "ready" &&
        scheduledTaskTemplates.value.length === 0 && (
          <p className="activity-empty">
            Completa nombre e instrucción para guardar tu primera plantilla.
          </p>
        )}
      {scheduledTaskTemplates.state === "ready" &&
        scheduledTaskTemplates.value.length > 0 && (
          <div className="scheduler-template-list">
            {scheduledTaskTemplates.value.map((template) => (
              <article key={template.id}>
                <div>
                  <strong>{template.name}</strong>
                  <span>
                    {template.scheduleExpression === "daily"
                      ? "Cada día"
                      : template.scheduleExpression === "weekly"
                        ? "Cada semana"
                        : "Una vez"}
                  </span>
                  <p>{template.prompt}</p>
                </div>
                <div>
                  <button
                    className="secondary"
                    onClick={() => applyScheduledTaskTemplate(template)}
                    disabled={scheduleBusyId !== null}
                  >
                    Usar
                  </button>
                  <button
                    className="danger-link"
                    onClick={() => void removeScheduledTaskTemplate(template)}
                    disabled={scheduleBusyId !== null}
                  >
                    {scheduleBusyId === template.id ? "Eliminando…" : "Eliminar"}
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
    </div>
    <div className="scheduler-form">
      <div className="scheduler-form-heading">
        <strong>
          {scheduleEditingId ? "Editar programación" : "Nueva programación"}
        </strong>
        {scheduleEditingId && (
          <button
            className="secondary"
            onClick={cancelScheduleEdit}
            disabled={scheduleBusyId !== null}
          >
            Cancelar edición
          </button>
        )}
      </div>
      <label>
        <span>Nombre de la tarea</span>
        <input
          value={scheduleName}
          onChange={(event) => setScheduleName(event.target.value)}
          placeholder="Ejemplo: Resumen del viernes"
          maxLength={120}
          disabled={scheduleBusyId !== null}
        />
      </label>
      <label>
        <span>Conversación donde aparecerá la respuesta</span>
        <select
          value={scheduleConversationId}
          onChange={(event) => setScheduleConversationId(event.target.value)}
          disabled={scheduleBusyId !== null}
        >
          <option value="">Elige una conversación</option>
          {conversations.map((item) => (
            <option key={item.id} value={item.id}>{item.title}</option>
          ))}
        </select>
      </label>
      <label>
        <span>Fecha y hora</span>
        <input
          type="datetime-local"
          value={scheduleAt}
          onChange={(event) => setScheduleAt(event.target.value)}
          min={defaultScheduledLocalTime(new Date(Date.now() - 55 * 60 * 1000))}
          disabled={scheduleBusyId !== null}
        />
        <small>
          Zona horaria: {scheduleTimezone}
        </small>
      </label>
      <label>
        <span>Repetición</span>
        <select
          value={scheduleExpression}
          onChange={(event) =>
            setScheduleExpression(
              event.target.value as ScheduledTaskView["scheduleExpression"]
            )}
          disabled={scheduleBusyId !== null}
        >
          <option value="once">Una sola vez</option>
          <option value="daily">Cada día</option>
          <option value="weekly">Cada semana</option>
        </select>
        <small>
          {scheduleExpression === "once"
            ? "No se repetirá."
            : "Mantendrá esta hora según la zona local de Windows."}
        </small>
      </label>
      <label className="scheduler-prompt">
        <span>Instrucción</span>
        <textarea
          value={schedulePrompt}
          onChange={(event) => setSchedulePrompt(event.target.value)}
          placeholder="Escribe exactamente lo que ChatyGPT deberá pedir al Broker."
          rows={4}
          maxLength={20_000}
          disabled={scheduleBusyId !== null}
        />
      </label>
      <label className="scheduler-confirmation">
        <input
          type="checkbox"
          checked={scheduleConfirmed}
          onChange={(event) => setScheduleConfirmed(event.target.checked)}
          disabled={scheduleBusyId !== null}
        />
        <span>
          Confirmo que quiero activar esta ejecución automática en la conversación
          seleccionada.
        </span>
      </label>
      <div className="scheduler-form-actions">
        <button
          className="secondary"
          onClick={() => void saveScheduledTaskTemplate()}
          disabled={
            scheduleBusyId !== null ||
            !scheduleName.trim() ||
            !schedulePrompt.trim()
          }
        >
          {scheduleBusyId === "template-create"
            ? "Guardando plantilla…"
            : "Guardar como plantilla"}
        </button>
        <button
          className="primary"
          onClick={() => void createSchedule()}
          disabled={
            scheduleBusyId !== null ||
            !scheduleName.trim() ||
            !scheduleConversationId ||
            !schedulePrompt.trim() ||
            !scheduleAt ||
            !scheduleConfirmed
          }
        >
          {scheduleBusyId === "create"
            ? "Guardando…"
            : scheduleEditingId
              ? "Guardar cambios y activar"
              : "Guardar y activar"}
        </button>
      </div>
    </div>
    {scheduleNotice && (
      <p className="scheduler-notice" role="status">{scheduleNotice}</p>
    )}
    {scheduleError && (
      <p className="error" role="alert">{scheduleError}</p>
    )}
    {scheduledTasks.state === "ready" &&
      scheduledTasks.value.some((task) => task.runs.length > 0) && (
        <div className="scheduler-history-filters">
          <strong>Filtrar historiales</strong>
          <label>
            <span>Estado</span>
            <select
              value={scheduledHistoryStatus}
              onChange={(event) => {
                setScheduledHistoryStatus(
                  event.target.value as ScheduledHistoryStatusFilter
                );
                setScheduledHistoryPageNumber(1);
              }}
            >
              <option value="all">Todos</option>
              <option value="active">En curso</option>
              <option value="completed">Completadas</option>
              <option value="failed">Fallidas</option>
              <option value="cancelled">Canceladas</option>
            </select>
          </label>
          <label>
            <span>Fecha</span>
            <select
              value={scheduledHistoryPeriod}
              onChange={(event) => {
                setScheduledHistoryPeriod(
                  event.target.value as ScheduledHistoryPeriodFilter
                );
                setScheduledHistoryPageNumber(1);
              }}
            >
              <option value="all">Cualquier fecha</option>
              <option value="today">Hoy</option>
              <option value="7d">Últimos 7 días</option>
              <option value="30d">Últimos 30 días</option>
            </select>
          </label>
          <button
            className="secondary"
            onClick={() => void exportScheduledHistory()}
            disabled={scheduleBusyId !== null}
          >
            {scheduleBusyId === "export"
              ? "Exportando…"
              : "Exportar historial visible"}
          </button>
        </div>
      )}
    {scheduledTasks.state === "loading" && (
      <p className="muted">Cargando programaciones…</p>
    )}
    {scheduledTasks.state === "error" && (
      <p className="error">{scheduledTasks.message}</p>
    )}
    {scheduledTasks.state === "ready" &&
      scheduledTasks.value.length > 0 && (
        <label className="scheduler-search">
          <span>Buscar tareas</span>
          <input
            type="search"
            value={scheduleSearchQuery}
            onChange={(event) => setScheduleSearchQuery(event.target.value)}
            placeholder="Nombre, conversación o texto de la instrucción"
          />
          <small>
            {visibleScheduledTasks.length} de {scheduledTasks.value.length} visible(s)
          </small>
        </label>
      )}
    {scheduledTasks.state === "ready" && (
      scheduledTasks.value.length === 0 ? (
        <p className="activity-empty">
          Todavía no has creado ninguna tarea programada.
        </p>
      ) : visibleScheduledTasks.length === 0 ? (
        <p className="activity-empty">
          No hay tareas que coincidan con la búsqueda.
        </p>
      ) : (
        <div className="scheduler-list">
          {visibleScheduledTasks.map((task) => (
            <article
              key={task.id}
              className={`scheduler-item ${task.enabled ? "enabled" : ""}`}
            >
              <div className="scheduler-item-heading">
                <div>
                  <strong>{task.name}</strong>
                  <span>{task.targetKind === "workflow" ? `Flujo · ${task.workflowName} · versión ${task.workflowVersionNo}` : task.conversationTitle}</span>
                </div>
                <span className={`badge ${task.enabled ? "success" : ""}`}>
                  {task.enabled
                    ? "Activa"
                    : task.runs.length > 0
                    ? scheduledRunLabel(task.runs[0].status)
                    : "Pausada"}
                </span>
              </div>
              <p>{task.prompt}</p>
              <small>
                {task.nextRunAt
                  ? new Date(task.nextRunAt).toLocaleString("es-ES", {
                      dateStyle: "medium",
                      timeStyle: "short",
                      timeZone: task.timezone
                    })
                  : "Sin próxima ejecución"}{" "}
                · {task.timezone} ·{" "}
                {task.scheduleExpression === "daily"
                  ? "Diaria"
                  : task.scheduleExpression === "weekly"
                    ? "Semanal"
                    : "Una vez"}
              </small>
              {task.runs.length > 0 && (
                <div className="scheduler-history">
                  <div className="scheduler-history-heading">
                    <strong>Actividad reciente</strong>
                    <button
                      className="secondary compact"
                      onClick={() => toggleScheduledHistory(task)}
                      aria-expanded={scheduledHistoryTaskId === task.id}
                    >
                      {scheduledHistoryTaskId === task.id
                        ? "Ocultar historial completo"
                        : "Ver historial completo"}
                    </button>
                  </div>
                  {filterScheduledRuns(
                    task.runs,
                    scheduledHistoryStatus,
                    scheduledHistoryPeriod
                  ).length === 0 && (
                    <span className="scheduler-history-empty">
                      No hay ejecuciones que coincidan con los filtros.
                    </span>
                  )}
                  {filterScheduledRuns(
                    task.runs,
                    scheduledHistoryStatus,
                    scheduledHistoryPeriod
                  ).map((run) => {
                    const detail = scheduledRunDetail(run);
                    return (
                      <div key={run.id} className="scheduler-history-entry">
                        <div className="scheduler-history-row">
                          <span>
                            {scheduledRunLabel(run.status)}
                            {run.attempt > 1 ? ` · intento ${run.attempt}` : ""}
                          </span>
                          <div>
                            <time>
                              {new Date(run.updatedAt).toLocaleString("es-ES", {
                                dateStyle: "short",
                                timeStyle: "medium"
                              })}
                            </time>
                            {run.status === "failed" && (
                              <button
                                className="secondary compact"
                                onClick={() => void retryScheduledRun(task, run)}
                                disabled={
                                  scheduleBusyId !== null ||
                                  task.runs.some((item) =>
                                    ["claimed", "running"].includes(item.status)
                                  )
                                }
                              >
                                {scheduleBusyId === run.id
                                  ? "Reintentando…"
                                  : "Reintentar"}
                              </button>
                            )}
                            {run.status === "running" && (run.brokerTaskId || run.workflowRunId) && (
                              <button
                                className="danger-link compact"
                                onClick={() => void cancelScheduledRun(task, run)}
                                disabled={scheduleBusyId !== null}
                              >
                                {scheduleBusyId === run.id
                                  ? "Cancelando…"
                                  : "Cancelar ejecución"}
                              </button>
                            )}
                          </div>
                        </div>
                        {detail && (
                          <details className="scheduler-run-detail">
                            <summary>Ver detalle</summary>
                            <strong>{detail.label}</strong>
                            <p>{detail.text}</p>
                          </details>
                        )}
                      </div>
                    );
                  })}
                </div>
              )}
              {scheduledHistoryTaskId === task.id && (
                <div className="scheduler-full-history">
                  <div className="scheduler-full-history-heading">
                    <div>
                      <strong>Historial completo</strong>
                      <span>Los filtros superiores también se aplican aquí.</span>
                    </div>
                    <div>
                      <label>
                        <span>Orden</span>
                        <select
                          value={scheduledHistorySort}
                          onChange={(event) => {
                            setScheduledHistorySort(
                              event.target.value as ScheduledHistorySort
                            );
                            setScheduledHistoryPageNumber(1);
                          }}
                        >
                          <option value="newest">Más recientes primero</option>
                          <option value="oldest">Más antiguas primero</option>
                        </select>
                      </label>
                      <label>
                        <span>Por página</span>
                        <select
                          value={scheduledHistoryPageSize}
                          onChange={(event) => {
                            setScheduledHistoryPageSize(
                              Number(event.target.value) as ScheduledRunPageView["pageSize"]
                            );
                            setScheduledHistoryPageNumber(1);
                          }}
                        >
                          <option value={10}>10</option>
                          <option value={25}>25</option>
                          <option value={50}>50</option>
                        </select>
                      </label>
                    </div>
                  </div>
                  {scheduledHistoryPage?.state === "loading" && (
                    <p className="muted">Cargando historial completo…</p>
                  )}
                  {scheduledHistoryPage?.state === "error" && (
                    <p className="error">{scheduledHistoryPage.message}</p>
                  )}
                  {scheduledHistoryPage?.state === "ready" && (
                    <>
                      <div className="scheduler-full-history-summary">
                        {scheduledHistoryPage.value.total === 0
                          ? "No hay ejecuciones que coincidan con los filtros."
                          : `${scheduledHistoryPage.value.total} ejecución(es) · página ${scheduledHistoryPage.value.page} de ${Math.max(
                              1,
                              Math.ceil(
                                scheduledHistoryPage.value.total /
                                  scheduledHistoryPage.value.pageSize
                              )
                            )}`}
                      </div>
                      <div className="scheduler-full-history-list">
                        {scheduledHistoryPage.value.items.map((run) => {
                          const detail = scheduledRunDetail(run);
                          return (
                            <div key={run.id} className="scheduler-history-entry">
                              <div className="scheduler-history-row">
                                <span>
                                  {scheduledRunLabel(run.status)}
                                  {run.attempt > 1
                                    ? ` · intento ${run.attempt}`
                                    : ""}
                                </span>
                                <div>
                                  <time>
                                    {new Date(run.updatedAt).toLocaleString("es-ES", {
                                      dateStyle: "short",
                                      timeStyle: "medium"
                                    })}
                                  </time>
                                  {run.status === "failed" && (
                                    <button
                                      className="secondary compact"
                                      onClick={() => void retryScheduledRun(task, run)}
                                      disabled={scheduleBusyId !== null}
                                    >
                                      {scheduleBusyId === run.id
                                        ? "Reintentando…"
                                        : "Reintentar"}
                                    </button>
                                  )}
                                  {run.status === "running" && (run.brokerTaskId || run.workflowRunId) && (
                                    <button
                                      className="danger-link compact"
                                      onClick={() => void cancelScheduledRun(task, run)}
                                      disabled={scheduleBusyId !== null}
                                    >
                                      {scheduleBusyId === run.id
                                        ? "Cancelando…"
                                        : "Cancelar ejecución"}
                                    </button>
                                  )}
                                </div>
                              </div>
                              {detail && (
                                <details className="scheduler-run-detail">
                                  <summary>Ver detalle</summary>
                                  <strong>{detail.label}</strong>
                                  <p>{detail.text}</p>
                                </details>
                              )}
                            </div>
                          );
                        })}
                      </div>
                      {scheduledHistoryPage.value.total >
                        scheduledHistoryPage.value.pageSize && (
                          <div className="scheduler-pagination">
                            <button
                              className="secondary"
                              onClick={() =>
                                setScheduledHistoryPageNumber((current) =>
                                  Math.max(1, current - 1)
                                )
                              }
                              disabled={scheduledHistoryPage.value.page <= 1}
                            >
                              Anterior
                            </button>
                            <span>Página {scheduledHistoryPage.value.page}</span>
                            <button
                              className="secondary"
                              onClick={() =>
                                setScheduledHistoryPageNumber((current) => current + 1)
                              }
                              disabled={
                                scheduledHistoryPage.value.page >=
                                Math.ceil(
                                  scheduledHistoryPage.value.total /
                                    scheduledHistoryPage.value.pageSize
                                )
                              }
                            >
                              Siguiente
                            </button>
                          </div>
                        )}
                    </>
                  )}
                </div>
              )}
              <div className="scheduler-item-actions">
                {task.targetKind === "workflow" ? (
                  <button className="secondary" onClick={() => setWorkspaceDestination("workflows")}>Abrir Flujos</button>
                ) : (
                  <button className="secondary" onClick={() => task.conversationId && openConversation(task.conversationId)}>Abrir conversación</button>
                )}
                {(task.runs.length === 0 ||
                  task.scheduleExpression !== "once") && (
                  <button
                    className="secondary"
                    onClick={() => void toggleSchedule(task)}
                    disabled={scheduleBusyId !== null}
                  >
                    {task.enabled ? "Pausar" : "Reactivar"}
                  </button>
                )}
                <button
                  className="primary"
                  onClick={() => void runScheduledTaskNow(task)}
                  disabled={
                    scheduleBusyId !== null ||
                    task.runs.some((run) =>
                      ["claimed", "running"].includes(run.status)
                    )
                  }
                >
                  {scheduleBusyId === `run-now:${task.id}`
                    ? "Iniciando…"
                    : "Ejecutar ahora"}
                </button>
                {task.targetKind !== "workflow" && <button className="secondary" onClick={() => duplicateSchedule(task)} disabled={scheduleBusyId !== null}>Duplicar</button>}
                {task.targetKind !== "workflow" && <button
                  className="secondary"
                  onClick={() => beginScheduleEdit(task)}
                  disabled={scheduleBusyId !== null || task.runs.some((run) => ["claimed", "running"].includes(run.status))}
                >Editar</button>}
                <button
                  className="danger-link"
                  onClick={() => void removeSchedule(task)}
                  disabled={
                    scheduleBusyId !== null ||
                    task.runs.some((run) =>
                      ["claimed", "running"].includes(run.status)
                    )
                  }
                >
                  Eliminar
                </button>
              </div>
            </article>
          ))}
        </div>
      )
    )}
  </section>
  );
}
