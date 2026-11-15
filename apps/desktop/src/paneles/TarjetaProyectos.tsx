import type { ProjectSummary } from "../domain";
import type { DialogState } from "../dialogs";
import type { WorkspaceDestination } from "../navegacion";

/** Lista de proyectos de la portada. Extraída de App.tsx (CH-03): recibe el estado y los manejadores tal cual. */
export type TarjetaProyectosProps = {
  projects: ProjectSummary[];
  openDialog: (dialog: DialogState) => void;
  setSelectedProjectId: (projectId: string | null) => void;
  setWorkspaceDestination: (destination: WorkspaceDestination) => void;
  openProjectKnowledge: (project: ProjectSummary) => Promise<void> | void;
};

export function TarjetaProyectos({ projects, openDialog, setSelectedProjectId, setWorkspaceDestination, openProjectKnowledge }: TarjetaProyectosProps) {
  return (
    <section className="projects-card" aria-labelledby="projects-heading">
      <div className="panel-heading">
        <div>
          <span className="kicker">Organización</span>
          <h3 id="projects-heading">Proyectos</h3>
        </div>
        <button className="primary" onClick={() => openDialog({ kind: "project-create" })}>
          Crear proyecto
        </button>
      </div>
      {projects.length === 0 ? (
        <p className="muted">Crea un proyecto para reunir chats, instrucciones y archivos relacionados.</p>
      ) : (
        <div className="project-home-list">
          {projects.map((project) => (
            <article key={project.id}>
              <div>
                <strong>{project.name}</strong>
                <small>{project.conversationCount} conversación(es)</small>
              </div>
              <div className="task-actions">
                <button
                  className="secondary"
                  onClick={() => {
                    setSelectedProjectId(project.id);
                    setWorkspaceDestination("chats");
                  }}
                >
                  Ver chats
                </button>
                <button className="secondary" onClick={() => void openProjectKnowledge(project)}>
                  Conocimiento
                </button>
              </div>
            </article>
          ))}
        </div>
      )}
    </section>
  );
}
