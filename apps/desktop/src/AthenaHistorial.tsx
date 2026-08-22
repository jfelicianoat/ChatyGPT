/**
 * Runs de antes, leídos del registro duradero de Athena.
 *
 * Dos decisiones gobiernan esto:
 *
 * 1. **La lista y los hechos son de Athena.** ChatyGPT guarda cómo volver a preguntar,
 *    no qué pasó. Por eso aquí aparece también un run lanzado desde Telegram: el run es
 *    del runtime, no de quien lo pidió.
 * 2. **La reconstrucción usa el mismo lector que la vista en vivo.** El núcleo pasa los
 *    hechos por la misma proyección, así que un run releído se lee igual que se leyó
 *    cuando pasaba. Un segundo lector aquí garantizaría que antes o después los dos
 *    contaran cosas distintas del mismo run.
 */

import { useCallback, useEffect, useState } from "react";

import { AthenaDelegados } from "./AthenaDelegados";
import type { AthenaHistoria, AthenaResumenRun } from "./domain";
import {
  nombreEstadoTarea,
  nombreEstrategia,
  nombreFase,
  nombreVerificacion,
  pistaDeDetalle
} from "./athenaView";

type Props = {
  onListar: () => Promise<AthenaResumenRun[]>;
  onAbrir: (runId: string) => Promise<AthenaHistoria>;
};

/** Cuántos hechos en bruto se enseñan del registro. */
const LIMITE_HECHOS = 200;

export function AthenaHistorial({ onListar, onAbrir }: Props) {
  const [runs, setRuns] = useState<AthenaResumenRun[]>([]);
  const [abierto, setAbierto] = useState<AthenaHistoria | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [cargando, setCargando] = useState(false);

  const refrescar = useCallback(async () => {
    try {
      setRuns(await onListar());
      setError(null);
    } catch (fallo) {
      setError(fallo instanceof Error ? fallo.message : String(fallo));
    }
  }, [onListar]);

  useEffect(() => {
    void refrescar();
  }, [refrescar]);

  const abrir = async (runId: string) => {
    setCargando(true);
    setError(null);
    try {
      setAbierto(await onAbrir(runId));
    } catch (fallo) {
      // Athena distingue «no consta historia» de «run vacío», y el 404 llega con su
      // mensaje. Enseñar una historia vacía haría pasar la ausencia por el hecho.
      setAbierto(null);
      setError(fallo instanceof Error ? fallo.message : String(fallo));
    } finally {
      setCargando(false);
    }
  };

  return (
    <section className="athena-historial" aria-label="Historial">
      <h3>Trabajos anteriores</h3>
      {error ? <p className="athena-aviso">{error}</p> : null}
      {runs.length === 0 && !error ? (
        <p className="athena-nota">Athena no recuerda ningún run todavía.</p>
      ) : null}
      <ul className="athena-historial-lista">
        {runs.map((resumen) => (
          <li key={resumen.runId}>
            <span>{resumen.objective || resumen.runId}</span>
            <span className="athena-nota">
              {resumen.status} · {resumen.updatedAt}
            </span>
            <button type="button" disabled={cargando} onClick={() => void abrir(resumen.runId)}>
              Ver qué pasó
            </button>
          </li>
        ))}
      </ul>

      {abierto ? (
        <article className="athena-historia" aria-label="Run anterior">
          <header>
            <h4>{abierto.proyeccion.objetivo}</h4>
            <p className="athena-fase">{nombreFase(abierto.proyeccion.fase)}</p>
            <p className="athena-nota">
              {abierto.proyeccion.perfilSolicitado
                ? `Perfil: ${abierto.proyeccion.perfilSolicitado} · `
                : ""}
              Encargo en su revisión {abierto.proyeccion.objetivoRevision || "?"}
            </p>
          </header>

          {/* Lo que Athena concluye de sus propios hechos. Se enseña como suyo. */}
          <dl className="athena-historia-resumen">
            <div>
              <dt>Cómo se ejecutó</dt>
              <dd>{nombreEstrategia(abierto.resumen.executedAs || "—")}</dd>
            </div>
            <div>
              <dt>Cómo acabó</dt>
              <dd>{abierto.resumen.status}</dd>
            </div>
            <div>
              <dt>Verificación</dt>
              <dd>
                {abierto.resumen.verification
                  ? nombreVerificacion(abierto.resumen.verification)
                  : "No consta que se pronunciara"}
              </dd>
            </div>
            <div>
              <dt>Permisos pedidos</dt>
              <dd>{abierto.resumen.permissionRequests}</dd>
            </div>
          </dl>

          {Object.keys(abierto.resumen.tasks).length > 0 ? (
            <section aria-label="Tareas del plan">
              <h5>Tareas</h5>
              <ul>
                {Object.entries(abierto.resumen.tasks).map(([id, estado]) => (
                  <li key={id}>
                    {id} — {estado}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          <AthenaDelegados delegados={abierto.proyeccion.delegados} />

          {abierto.proyeccion.comprobaciones.length > 0 ? (
            <section aria-label="Comprobaciones">
              <h5>Comprobaciones</h5>
              <ul>
                {abierto.proyeccion.comprobaciones.map((comprobacion, indice) => (
                  <li key={`${comprobacion.nombre}-${indice}`}>
                    {comprobacion.nombre} —{" "}
                    {comprobacion.paso === undefined
                      ? "sin veredicto"
                      : comprobacion.paso
                        ? "pasó"
                        : "falló"}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {abierto.proyeccion.ficherosModificados.length > 0 ? (
            <section aria-label="Ficheros del run">
              <h5>Ficheros que cambiaron</h5>
              <ul>
                {abierto.proyeccion.ficherosModificados.map((ruta) => (
                  <li key={ruta}>{ruta}</li>
                ))}
              </ul>
            </section>
          ) : null}

          {abierto.proyeccion.artefactos.length > 0 ? (
            <section aria-label="Artefactos del run">
              <h5>Resultados guardados</h5>
              <ul>
                {abierto.proyeccion.artefactos.map((item) => (
                  <li key={item.clave}>
                    {item.tipo} · {item.tamano} caracteres
                  </li>
                ))}
              </ul>
              {/* Se nombran, no se abren: un resultado externalizado caduca, y ofrecer
                  abrirlo aquí prometería algo que puede ya no estar. */}
              <p className="athena-nota">
                Se conservan un tiempo limitado; puede que alguno ya no esté.
              </p>
            </section>
          ) : null}

          {abierto.proyeccion.errores.length > 0 ? (
            <section aria-label="Errores del run">
              <h5>Errores</h5>
              <ul>
                {abierto.proyeccion.errores.map((fallo, indice) => (
                  <li key={`${fallo.codigo}-${indice}`}>
                    <strong>{fallo.codigo}</strong> {fallo.mensaje}
                    {fallo.detalle ? <em> · {pistaDeDetalle(fallo.detalle)}</em> : null}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          <details className="athena-historia-hechos">
            <summary>Los hechos, en orden ({abierto.hechos.length})</summary>
            <ol>
              {abierto.hechos.slice(0, LIMITE_HECHOS).map((hecho) => (
                <li key={hecho.secuencia}>
                  <code>{hecho.nombre}</code> · {hecho.cuando}
                  {/* Quién lo hizo. Sin esto, un run con delegados se leería como si
                      todo lo hubiera hecho el padre. */}
                  {hecho.delegado ? (
                    <em> · lo hizo {hecho.actor}</em>
                  ) : null}
                  {hecho.tarea ? <em> · tarea {hecho.tarea}</em> : null}
                </li>
              ))}
            </ol>
            {abierto.hechos.length > LIMITE_HECHOS ? (
              <p className="athena-nota">
                {abierto.hechos.length} en total; se enseñan {LIMITE_HECHOS}.
              </p>
            ) : null}
          </details>

          <p className="athena-nota">
            {/* Athena agrega las métricas por estrategia, no por run: enseñar aquí un
                número «de este run» obligaría a inventarlo. */}
            Athena mide el coste agregado por estrategia, no por run: aquí no hay métricas
            de este trabajo en concreto.
          </p>
          {abierto.proyeccion.tareas.length > 0 ? (
            <p className="athena-nota">
              Plan reconstruido: {abierto.proyeccion.tareas.length} tareas ·{" "}
              {abierto.proyeccion.tareas
                .map((tarea) => nombreEstadoTarea(tarea.estado))
                .join(", ")}
            </p>
          ) : null}
        </article>
      ) : null}
    </section>
  );
}
