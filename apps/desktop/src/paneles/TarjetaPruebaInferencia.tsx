import {
  isTaskBlockingConversation,
  isTerminalTask,
  type BrokerDiagnostic,
  type Loadable,
  type LocalTaskSnapshot,
  type TaskArtifact,
} from "../domain";

/** Prueba durable de inferencia contra el Broker (ajustes). Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaPruebaInferenciaProps = {
  smokeTask: Loadable<LocalTaskSnapshot> | null;
  broker: Loadable<BrokerDiagnostic> | null;
  taskArtifacts: Loadable<TaskArtifact[]> | { state: "idle" };
  startSmokeTask: () => Promise<void> | void;
  cancelSmokeTask: () => Promise<void> | void;
  loadTaskArtifacts: () => Promise<void> | void;
  saveArtifact: (artifactId: string) => Promise<void> | void;
};

export function TarjetaPruebaInferencia({ smokeTask, broker, taskArtifacts, startSmokeTask, cancelSmokeTask, loadTaskArtifacts, saveArtifact }: TarjetaPruebaInferenciaProps) {
  return (
    <section className="task-card">
      <div className="panel-heading">
        <div>
          <span className="kicker">Recorrido durable</span>
          <h3>Prueba controlada de inferencia</h3>
        </div>
        {smokeTask?.state === "ready" && (
          <span className={`badge ${
            isTerminalTask(smokeTask.value) ? "success" : "warning"
          }`}>
            {smokeTask.value.remoteStatus}
          </span>
        )}
      </div>
      <p className="muted">
        Persiste la petición antes de enviarla y limita la ejecución a Ollama local.
      </p>
      <div className="task-actions">
        <button
          className="primary"
          onClick={startSmokeTask}
          disabled={
            broker?.state !== "ready" ||
            !broker.value.ready ||
            smokeTask?.state === "loading"
          }
        >
          {smokeTask?.state === "loading" ? "Creando…" : "Ejecutar prueba durable"}
        </button>
        {smokeTask?.state === "ready" &&
          isTaskBlockingConversation(smokeTask.value) &&
          smokeTask.value.remoteTaskId && (
            <button className="secondary danger" onClick={cancelSmokeTask}>
              Cancelar
            </button>
          )}
      </div>
      {smokeTask?.state === "ready" && smokeTask.value.result && (
        <pre className="result-preview">
          {String(
            smokeTask.value.result.assistant_content ??
            smokeTask.value.result.result_markdown ??
            JSON.stringify(smokeTask.value.result, null, 2)
          )}
        </pre>
      )}
      {smokeTask?.state === "ready" &&
        smokeTask.value.result &&
        smokeTask.value.remoteTaskId && (
          <div className="task-actions">
            <button
              className="secondary"
              onClick={loadTaskArtifacts}
              disabled={taskArtifacts.state === "loading"}
            >
              {taskArtifacts.state === "loading"
                ? "Consultando…"
                : "Ver ficheros de la tarea"}
            </button>
          </div>
        )}
      {taskArtifacts.state === "ready" && (
        <div className="diagnostic">
          {taskArtifacts.value.length === 0 ? (
            <span>La tarea no produjo ficheros aparte de su respuesta.</span>
          ) : (
            taskArtifacts.value.map((artifact) => (
              <span key={artifact.artifactId}>
                {artifact.filename || artifact.artifactId}
                {" · "}
                {artifact.final ? "entregable" : artifact.artifactType}
                {" · "}
                {artifact.available ? (
                  <button
                    className="secondary"
                    onClick={() => void saveArtifact(artifact.artifactId)}
                  >
                    Guardar
                  </button>
                ) : (
                  "ya no está en el Broker"
                )}
              </span>
            ))
          )}
        </div>
      )}
      {taskArtifacts.state === "error" && (
        <p className="error">{taskArtifacts.message}</p>
      )}
      {smokeTask?.state === "error" && (
        <p className="error">{smokeTask.message}</p>
      )}
    </section>
  );
}
