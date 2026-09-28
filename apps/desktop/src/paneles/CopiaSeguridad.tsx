import { useState } from "react";

import type { BackupReport } from "../domain";
import { describeError } from "../errors";
import { platform } from "../platform";

type Stage =
  | { kind: "idle" }
  | { kind: "working"; label: string }
  | { kind: "created"; report: BackupReport }
  | { kind: "inspected"; report: BackupReport }
  | { kind: "scheduled"; report: BackupReport }
  | { kind: "error"; message: string };

const size = (bytes: number) =>
  bytes < 1024 * 1024
    ? `${(bytes / 1024).toFixed(0)} KB`
    : `${(bytes / (1024 * 1024)).toFixed(1)} MB`;

function ReportFacts({ report }: { report: BackupReport }) {
  return (
    <dl className="facts">
      <div><dt>Carpeta</dt><dd>{report.folder}</dd></div>
      <div><dt>Creada</dt><dd>{new Date(report.createdAt).toLocaleString("es-ES")}</dd></div>
      <div><dt>Conversaciones</dt><dd>{report.conversationCount}</dd></div>
      <div><dt>Adjuntos</dt><dd>{report.attachmentCount}</dd></div>
      <div><dt>Tamaño</dt><dd>{size(report.totalBytes)}</dd></div>
      <div><dt>Versión</dt><dd>{report.appVersion} · esquema {report.schemaVersion}</dd></div>
    </dl>
  );
}

/**
 * Copias de seguridad (auditoría 28-sep-2026, H16).
 *
 * La base local es la fuente de verdad; exportar chats no conserva GPTs,
 * permisos ni programaciones. Una copia aquí es completa y se verifica antes
 * de darla por buena, y restaurar se aplica al reiniciar sin borrar nada.
 */
export function CopiaSeguridad() {
  const [stage, setStage] = useState<Stage>({ kind: "idle" });

  const create = async () => {
    try {
      const folder = await platform.pickBackupFolder();
      if (!folder) return;
      setStage({ kind: "working", label: "Creando y verificando la copia…" });
      setStage({ kind: "created", report: await platform.createBackup(folder) });
    } catch (error) {
      setStage({ kind: "error", message: describeError(error) });
    }
  };

  const inspect = async () => {
    try {
      const folder = await platform.pickBackupFolder();
      if (!folder) return;
      setStage({ kind: "working", label: "Comprobando la copia…" });
      setStage({ kind: "inspected", report: await platform.inspectBackup(folder) });
    } catch (error) {
      setStage({ kind: "error", message: describeError(error) });
    }
  };

  const restore = async (report: BackupReport) => {
    if (
      !window.confirm(
        `¿Restaurar la copia del ${new Date(report.createdAt).toLocaleString("es-ES")}?\n\n` +
          "Se aplicará la próxima vez que abras ChatyGPT. Tus datos actuales no se borran: se " +
          "apartan en una carpeta dentro del directorio de datos."
      )
    ) {
      return;
    }
    try {
      setStage({ kind: "working", label: "Programando la restauración…" });
      setStage({ kind: "scheduled", report: await platform.scheduleBackupRestore(report.folder) });
    } catch (error) {
      setStage({ kind: "error", message: describeError(error) });
    }
  };

  return (
    <section className="backup-card panel" aria-labelledby="backup-heading">
      <div className="panel-heading">
        <div>
          <span className="kicker">Protección de datos</span>
          <h3 id="backup-heading">Copias de seguridad</h3>
        </div>
      </div>
      <p className="muted">
        Guarda en la carpeta que elijas una copia completa: conversaciones, proyectos, GPTs y sus
        versiones, flujos, programaciones, memoria y adjuntos. Se comprueba la huella de cada fichero
        y que la base abre correctamente antes de darla por buena.
      </p>
      <div className="task-actions">
        <button className="primary" onClick={() => void create()} disabled={stage.kind === "working"}>
          Crear copia…
        </button>
        <button className="secondary" onClick={() => void inspect()} disabled={stage.kind === "working"}>
          Restaurar una copia…
        </button>
      </div>
      {stage.kind === "working" && <p role="status">{stage.label}</p>}
      {stage.kind === "error" && <p className="error" role="alert">{stage.message}</p>}
      {stage.kind === "created" && (
        <div className="diagnostic" role="status">
          <strong>Copia creada y verificada.</strong>
          <ReportFacts report={stage.report} />
          <small>{stage.report.notIncluded.join(" ")}</small>
        </div>
      )}
      {stage.kind === "inspected" && (
        <div className="diagnostic">
          <strong>La copia está completa y abre correctamente.</strong>
          <ReportFacts report={stage.report} />
          <ul>
            {stage.report.notIncluded.map((item) => (
              <li key={item}>{item}</li>
            ))}
          </ul>
          <button className="primary" onClick={() => void restore(stage.report)}>
            Restaurar esta copia al reiniciar
          </button>
        </div>
      )}
      {stage.kind === "scheduled" && (
        <div className="diagnostic" role="status">
          <strong>Restauración programada.</strong>
          <span>
            Cierra ChatyGPT y vuelve a abrirlo para completarla. Hasta entonces sigues trabajando
            con tus datos actuales.
          </span>
        </div>
      )}
    </section>
  );
}
