import { useEffect, useState } from "react";

import type { ArchiveOverview, ArchivedConversation } from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";

type Props = {
  /** Se llama tras restaurar o borrar para que la navegación se refresque. */
  onChanged: () => Promise<void> | void;
  onOpenConversation: (conversationId: string) => void;
};

type State =
  | { kind: "loading" }
  | { kind: "ready"; value: ArchiveOverview }
  | { kind: "error"; message: string };

const when = (value?: string | null) =>
  value ? new Date(`${value.replace(" ", "T")}${value.endsWith("Z") ? "" : "Z"}`).toLocaleString("es-ES") : "";

/**
 * Archivo y papelera (auditoría 28-sep-2026, H14).
 *
 * Archivar y eliminar se presentaban como reversibles, pero no había ningún
 * recorrido para volver atrás sin tocar SQLite. Aquí se ve lo archivado y lo
 * eliminado, se restaura con su agrupación, y borrar para siempre es una
 * decisión aparte que solo existe dentro de la papelera.
 */
export function ArchivoPanel({ onChanged, onOpenConversation }: Props) {
  const [state, setState] = useState<State>({ kind: "loading" });
  const [busyId, setBusyId] = useState<string | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = async () => {
    try {
      setState({ kind: "ready", value: await platform.getArchiveOverview() });
    } catch (error) {
      setState({ kind: "error", message: describeError(error) });
    }
  };

  useEffect(() => {
    void load();
  }, []);

  const act = async (id: string, action: () => Promise<unknown>, done: string) => {
    setBusyId(id);
    setNotice(null);
    try {
      await action();
      setNotice(done);
      await load();
      await onChanged();
    } catch (error) {
      setNotice(describeError(error));
    } finally {
      setBusyId(null);
    }
  };

  const restoreConversation = (item: ArchivedConversation) =>
    act(item.id, () => platform.restoreConversation(item.id), `«${item.title}» vuelve a tus chats.`);

  const purgeConversation = (item: ArchivedConversation) => {
    if (
      !window.confirm(
        `¿Borrar para siempre «${item.title}»? Se eliminan sus ${item.messageCount} mensajes y las copias de adjuntos que ninguna otra conversación usa. No se puede deshacer.`
      )
    ) {
      return;
    }
    void act(item.id, () => platform.purgeConversation(item.id), `«${item.title}» se borró definitivamente.`);
  };

  const conversationRow = (item: ArchivedConversation, inTrash: boolean) => (
    <article key={item.id} className="archive-row">
      <div>
        <strong>{item.title}</strong>
        <small>
          {item.messageCount} mensaje(s)
          {item.projectName ? ` · ${item.projectName}` : ""}
          {inTrash ? ` · eliminada ${when(item.deletedAt)}` : ` · archivada ${when(item.archivedAt)}`}
        </small>
      </div>
      <div className="task-actions">
        {!inTrash && (
          <button className="secondary" onClick={() => onOpenConversation(item.id)}>
            Abrir
          </button>
        )}
        <button
          className="secondary"
          disabled={busyId !== null}
          onClick={() => void restoreConversation(item)}
        >
          {busyId === item.id ? "Restaurando…" : "Restaurar"}
        </button>
        {inTrash && (
          <button
            className="secondary danger"
            disabled={busyId !== null}
            onClick={() => purgeConversation(item)}
          >
            Borrar para siempre
          </button>
        )}
      </div>
    </article>
  );

  return (
    <section className="archive-card panel" aria-labelledby="archive-heading">
      <div className="panel-heading">
        <div>
          <span className="kicker">Recuperación</span>
          <h3 id="archive-heading">Archivo y papelera</h3>
        </div>
        <button className="secondary" onClick={() => void load()}>
          Actualizar
        </button>
      </div>
      <p className="muted">
        Archivar y eliminar no borran nada: lo archivado sale de la lista de chats y lo eliminado
        va a la papelera. Desde aquí puedes devolverlo a su sitio. Solo «Borrar para siempre» lo
        elimina de este equipo; las copias de seguridad hechas antes lo conservan.
      </p>
      {notice && <p className="archive-notice" role="status">{notice}</p>}
      {state.kind === "loading" && <p className="muted">Cargando el archivo…</p>}
      {state.kind === "error" && <p className="error">{state.message}</p>}
      {state.kind === "ready" && (
        <>
          <h4>Conversaciones archivadas</h4>
          {state.value.conversations.length === 0 ? (
            <p className="muted">No hay conversaciones archivadas.</p>
          ) : (
            state.value.conversations.map((item) => conversationRow(item, false))
          )}

          <h4>Proyectos archivados</h4>
          {state.value.projects.length === 0 ? (
            <p className="muted">No hay proyectos archivados.</p>
          ) : (
            state.value.projects.map((project) => (
              <article key={project.id} className="archive-row">
                <div>
                  <strong>{project.name}</strong>
                  <small>
                    archivado {when(project.archivedAt)} · al restaurarlo vuelven{" "}
                    {project.conversationCount} conversación(es)
                  </small>
                </div>
                <button
                  className="secondary"
                  disabled={busyId !== null}
                  onClick={() =>
                    void act(
                      project.id,
                      () => platform.restoreProject(project.id),
                      `El proyecto «${project.name}» y sus conversaciones vuelven a su sitio.`
                    )
                  }
                >
                  {busyId === project.id ? "Restaurando…" : "Restaurar proyecto"}
                </button>
              </article>
            ))
          )}

          <h4>Papelera</h4>
          {state.value.trash.length === 0 ? (
            <p className="muted">La papelera está vacía.</p>
          ) : (
            state.value.trash.map((item) => conversationRow(item, true))
          )}
        </>
      )}
    </section>
  );
}
