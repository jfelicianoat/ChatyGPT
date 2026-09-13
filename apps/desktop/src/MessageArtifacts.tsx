import { useState } from "react";

import type { TaskArtifact } from "./domain";
import { describeError } from "./errors";
import { platform } from "./platform";

type ArtifactApi = {
  getLocalTask(localTaskId: string): Promise<{ remoteTaskId?: string | null }>;
  listTaskArtifacts(remoteTaskId: string): Promise<TaskArtifact[]>;
  saveTaskArtifact(remoteTaskId: string, artifactId: string): Promise<string>;
};

type Props = {
  localTaskId: string;
  api?: ArtifactApi;
  onSaved?: (path: string) => void;
};

type State =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; remoteTaskId: string; artifacts: TaskArtifact[] }
  | { kind: "error"; message: string };

export function MessageArtifacts({
  localTaskId,
  api = platform,
  onSaved = (path) => window.alert(`Guardado en:\n${path}`)
}: Props) {
  const [state, setState] = useState<State>({ kind: "idle" });

  const load = async () => {
    setState({ kind: "loading" });
    try {
      const task = await api.getLocalTask(localTaskId);
      if (!task.remoteTaskId) {
        throw new Error("La respuesta no llegó a recibir una tarea remota.");
      }
      const artifacts = await api.listTaskArtifacts(task.remoteTaskId);
      setState({ kind: "ready", remoteTaskId: task.remoteTaskId, artifacts });
    } catch (error) {
      setState({ kind: "error", message: describeError(error) });
    }
  };

  const save = async (artifact: TaskArtifact) => {
    if (state.kind !== "ready") return;
    try {
      onSaved(await api.saveTaskArtifact(state.remoteTaskId, artifact.artifactId));
    } catch (error) {
      setState({ kind: "error", message: describeError(error) });
    }
  };

  return (
    <section className="message-artifacts" aria-label="Ficheros generados">
      <button className="context-toggle" onClick={() => void load()} disabled={state.kind === "loading"}>
        {state.kind === "loading" ? "Buscando ficheros…" : "Ver ficheros generados"}
      </button>
      {state.kind === "error" && <small className="error">{state.message}</small>}
      {state.kind === "ready" &&
        (state.artifacts.length === 0 ? (
          <small>Esta respuesta no produjo ficheros adicionales.</small>
        ) : (
          <div className="message-artifact-list">
            {state.artifacts.map((artifact) => (
              <span key={artifact.artifactId}>
                <span>{artifact.filename || artifact.artifactId}</span>
                {artifact.available ? (
                  <button className="secondary" onClick={() => void save(artifact)}>Guardar</button>
                ) : (
                  <small>Ya no está disponible en el bróker.</small>
                )}
              </span>
            ))}
          </div>
        ))}
    </section>
  );
}
