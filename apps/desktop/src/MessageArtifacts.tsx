import { useState } from "react";

import type { TaskArtifact } from "./domain";
import { describeError } from "./errors";
import { platform } from "./platform";

type ArtifactApi = {
  getLocalTask(localTaskId: string): Promise<{ remoteTaskId?: string | null }>;
  listTaskArtifacts(remoteTaskId: string): Promise<TaskArtifact[]>;
  saveTaskArtifact(remoteTaskId: string, artifactId: string): Promise<string>;
  saveTaskArtifactAs(remoteTaskId: string, artifactId: string): Promise<string | null>;
  revealTaskArtifact(remoteTaskId: string, artifactId: string): Promise<string>;
};

type Props = {
  localTaskId: string;
  api?: ArtifactApi;
};

type State =
  | { kind: "idle" }
  | { kind: "loading" }
  | { kind: "ready"; remoteTaskId: string; artifacts: TaskArtifact[] }
  | { kind: "error"; message: string };

type Busy = { artifactId: string; action: "keep" | "save-as" | "reveal" } | null;

const formatSize = (bytes?: number | null) =>
  bytes === undefined || bytes === null
    ? null
    : bytes < 1024
      ? `${bytes} B`
      : bytes < 1024 * 1024
        ? `${(bytes / 1024).toFixed(1)} KB`
        : `${(bytes / (1024 * 1024)).toFixed(1)} MB`;

/**
 * Ficheros que produjo una respuesta (auditoría 28-sep-2026, H23).
 *
 * «Conservar en el equipo» guarda una copia verificada y la asocia al mensaje:
 * sigue accesible aunque el Broker pode el artefacto remoto. «Guardar como…»
 * la copia donde la persona elija y «Mostrar en el Explorador» la localiza.
 */
export function MessageArtifacts({ localTaskId, api = platform }: Props) {
  const [state, setState] = useState<State>({ kind: "idle" });
  const [busy, setBusy] = useState<Busy>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = async () => {
    setState({ kind: "loading" });
    setNotice(null);
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

  const refresh = async (remoteTaskId: string) => {
    const artifacts = await api.listTaskArtifacts(remoteTaskId);
    setState({ kind: "ready", remoteTaskId, artifacts });
  };

  const run = async (
    artifact: TaskArtifact,
    action: NonNullable<Busy>["action"]
  ) => {
    if (state.kind !== "ready") return;
    const remoteTaskId = state.remoteTaskId;
    setBusy({ artifactId: artifact.artifactId, action });
    setNotice(null);
    try {
      if (action === "keep") {
        await api.saveTaskArtifact(remoteTaskId, artifact.artifactId);
        setNotice(`«${artifact.filename || artifact.artifactId}» se conserva en este equipo.`);
        await refresh(remoteTaskId);
      } else if (action === "save-as") {
        const destination = await api.saveTaskArtifactAs(remoteTaskId, artifact.artifactId);
        if (destination) setNotice(`Guardado en ${destination}`);
        await refresh(remoteTaskId);
      } else {
        await api.revealTaskArtifact(remoteTaskId, artifact.artifactId);
      }
    } catch (error) {
      setNotice(describeError(error));
    } finally {
      setBusy(null);
    }
  };

  const busyLabel = (artifact: TaskArtifact, action: NonNullable<Busy>["action"], label: string) =>
    busy?.artifactId === artifact.artifactId && busy.action === action ? "Un momento…" : label;

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
            {state.artifacts.map((artifact) => {
              const kept = Boolean(artifact.localPath);
              const size = formatSize(artifact.sizeBytes);
              return (
                <span key={artifact.artifactId} className="message-artifact">
                  <span>
                    <strong>{artifact.filename || artifact.artifactId}</strong>
                    <small>
                      {[size, kept ? "conservado en este equipo" : artifact.available ? "en Broker AI" : null]
                        .filter(Boolean)
                        .join(" · ")}
                    </small>
                  </span>
                  <span className="message-artifact-actions">
                    {!kept && artifact.available && (
                      <button
                        className="secondary"
                        disabled={busy !== null}
                        onClick={() => void run(artifact, "keep")}
                      >
                        {busyLabel(artifact, "keep", "Conservar en el equipo")}
                      </button>
                    )}
                    {(kept || artifact.available) && (
                      <button
                        className="secondary"
                        disabled={busy !== null}
                        onClick={() => void run(artifact, "save-as")}
                      >
                        {busyLabel(artifact, "save-as", "Guardar como…")}
                      </button>
                    )}
                    {kept && (
                      <button
                        className="secondary"
                        disabled={busy !== null}
                        onClick={() => void run(artifact, "reveal")}
                      >
                        {busyLabel(artifact, "reveal", "Mostrar en el Explorador")}
                      </button>
                    )}
                    {!kept && !artifact.available && (
                      <small>Ya no está disponible en el Broker y no se conservó una copia.</small>
                    )}
                  </span>
                </span>
              );
            })}
          </div>
        ))}
      {notice && (
        <small className="message-artifact-notice" role="status">
          {notice}
        </small>
      )}
    </section>
  );
}
