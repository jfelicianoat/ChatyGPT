/**
 * Área de Athena.
 *
 * Pinta la proyección que el núcleo mantiene a partir de los eventos del
 * runtime. No deduce nada: si Athena no lo ha publicado, aquí no aparece. Y no
 * muestra el razonamiento del modelo —que ni siquiera llega hasta esta capa—,
 * sino los hechos operativos: qué herramienta, qué fichero, qué veredicto.
 */

import { useCallback, useEffect, useRef, useState } from "react";

import type {
  AthenaEstadoArea,
  AthenaResumenRun,
  AthenaRun,
  AuthorizedFolderView
} from "./domain";
import { platform } from "./platform";
import {
  INTERVALO_SONDEO_MS,
  debeSeguirSondeando,
  esFaseTerminal,
  etiquetasPermiso,
  mensajeServicio,
  motivoBloqueoPermiso,
  nombreEstadoTarea,
  nombreRol,
  ordenarPlan,
  progresoPlan,
  simboloTarea,
  nombreFase,
  nombreVerificacion,
  permisoActivo,
  puedeCancelarse,
  puedeLanzarse,
  puedeReanudarse,
  resumenActividad,
  textoArgumento,
  tiempoRestante
} from "./athenaView";

type Props = {
  carpetas: AuthorizedFolderView[];
};

export function AthenaArea({ carpetas }: Props) {
  const [aviso, setAviso] = useState<string | null>(null);
  const [estado, setEstado] = useState<AthenaEstadoArea | null>(null);
  const [objetivo, setObjetivo] = useState("");
  const [carpetaId, setCarpetaId] = useState("");
  const [runId, setRunId] = useState<string | null>(null);
  const [run, setRun] = useState<AthenaRun | null>(null);
  const [porRecuperar, setPorRecuperar] = useState<AthenaResumenRun[]>([]);
  const [artefacto, setArtefacto] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);
  const sondeo = useRef<number | null>(null);

  const informar = useCallback((error: unknown) => {
    setAviso(error instanceof Error ? error.message : String(error));
  }, []);

  const refrescarEstado = useCallback(async () => {
    try {
      setEstado(await platform.getAthenaStatus());
    } catch (error) {
      informar(error);
    }
  }, [informar]);

  const refrescarRecuperacion = useCallback(async () => {
    try {
      setPorRecuperar(await platform.listAthenaRecoveryRuns());
    } catch {
      // Que no haya servicio no es un error que merezca interrumpir: el estado
      // ya lo dice, y la lista simplemente queda vacía.
      setPorRecuperar([]);
    }
  }, []);

  // Un run abierto sobrevive al cierre de ChatyGPT: el núcleo recuerda a cuál
  // volver a engancharse y Athena devuelve su estado. Lo que se recupera es la
  // conexión, no el estado, que nunca vivió aquí.
  const reengancharse = useCallback(async () => {
    try {
      const seguidos = await platform.listAthenaTrackedRuns();
      const vivo = seguidos.find((vista) => !esFaseTerminal(vista.fase));
      if (vivo) {
        setRunId(vivo.runId);
        setRun(vivo);
      }
    } catch {
      // Sin servicio no hay nada que re-enganchar; el estado ya lo explica.
    }
  }, []);

  useEffect(() => {
    void refrescarEstado();
    void refrescarRecuperacion();
    void reengancharse();
  }, [refrescarEstado, refrescarRecuperacion, reengancharse]);

  // Sondeo de la proyección mientras el run pueda cambiar por su cuenta. Lo que
  // se pide ya viene resuelto desde los eventos; aquí no se calcula estado.
  useEffect(() => {
    if (!runId) {
      return;
    }
    let cancelado = false;
    const pedir = async () => {
      try {
        const vista = await platform.getAthenaRun(runId);
        if (!cancelado) {
          setRun(vista);
        }
      } catch (error) {
        if (!cancelado) {
          informar(error);
        }
      }
    };
    void pedir();
    sondeo.current = window.setInterval(() => {
      void pedir();
    }, INTERVALO_SONDEO_MS);
    return () => {
      cancelado = true;
      if (sondeo.current !== null) {
        window.clearInterval(sondeo.current);
        sondeo.current = null;
      }
    };
  }, [runId, informar]);

  // Cuando el run termina se deja de sondear: seguir preguntando por algo que
  // ya no cambia solo gasta sin informar.
  useEffect(() => {
    if (!debeSeguirSondeando(run) && sondeo.current !== null) {
      window.clearInterval(sondeo.current);
      sondeo.current = null;
    }
  }, [run]);

  const lanzar = async () => {
    setOcupado(true);
    setAviso(null);
    try {
      const identificador = await platform.startAthenaRun(objetivo, carpetaId);
      setRunId(identificador);
      setRun(null);
      setArtefacto(null);
    } catch (error) {
      informar(error);
    } finally {
      setOcupado(false);
    }
  };

  const responder = async (requestId: string, permitir: boolean) => {
    if (!runId) {
      return;
    }
    // `ocupado` desactiva los dos botones mientras la respuesta viaja: el
    // núcleo ya retira la petición al enviarla, pero el segundo clic podría
    // salir antes de que la proyección lo refleje.
    setOcupado(true);
    setAviso(null);
    try {
      await platform.resolveAthenaPermission(runId, requestId, permitir);
    } catch (error) {
      informar(error);
    } finally {
      setOcupado(false);
      // La pregunta ya no está en el núcleo; se refresca en vez de esperar al
      // siguiente sondeo, para que no siga en pantalla contestada.
      try {
        setRun(await platform.getAthenaRun(runId));
      } catch {
        // El sondeo volverá a intentarlo.
      }
    }
  };

  const cancelar = async () => {
    if (!runId) {
      return;
    }
    try {
      await platform.cancelAthenaRun(runId);
    } catch (error) {
      informar(error);
    }
  };

  const reanudar = async (identificador: string) => {
    if (!carpetaId) {
      setAviso("Elige la carpeta del run antes de reanudarlo.");
      return;
    }
    try {
      await platform.resumeAthenaRun(identificador, carpetaId);
      setRunId(identificador);
      await refrescarRecuperacion();
    } catch (error) {
      informar(error);
    }
  };

  const abrirArtefacto = async (clave: string) => {
    try {
      setArtefacto(await platform.fetchAthenaArtifact(clave));
    } catch (error) {
      informar(error);
    }
  };

  const permiso = permisoActivo(run);
  const bloqueoPermiso = permiso ? motivoBloqueoPermiso(run, permiso) : null;
  const veredicto = nombreVerificacion(run?.verificacion);

  return (
    <section className="athena-area" aria-label="Athena">
      <header className="athena-cabecera">
        <h2>Athena</h2>
        <p className="athena-servicio" data-estado={estado?.estado ?? "desconocido"}>
          {mensajeServicio(estado)}
        </p>
        {estado && !estado.credencialConfigurada ? (
          <p className="athena-aviso">
            Falta la credencial de Athena. El servicio la regenera en cada arranque.
          </p>
        ) : null}
        {aviso ? (
          <p className="athena-aviso" role="alert">
            {aviso}
          </p>
        ) : null}
      </header>

      <form
        className="athena-lanzador"
        onSubmit={(evento) => {
          evento.preventDefault();
          void lanzar();
        }}
      >
        <label htmlFor="athena-objetivo">Objetivo</label>
        <textarea
          id="athena-objetivo"
          value={objetivo}
          onChange={(evento) => setObjetivo(evento.target.value)}
          placeholder="Qué quieres que haga Athena en el repositorio"
          rows={2}
        />
        <label htmlFor="athena-carpeta">Carpeta autorizada</label>
        <select
          id="athena-carpeta"
          value={carpetaId}
          onChange={(evento) => setCarpetaId(evento.target.value)}
        >
          <option value="">Elige una carpeta…</option>
          {carpetas.map((carpeta) => (
            <option key={carpeta.id} value={carpeta.id}>
              {carpeta.displayName}
            </option>
          ))}
        </select>
        <p className="athena-nota">
          Cada cambio y cada comando te pedirán permiso, uno a uno.
        </p>
        <button type="submit" disabled={ocupado || !puedeLanzarse(estado, objetivo, carpetaId)}>
          Lanzar
        </button>
      </form>

      {porRecuperar.length > 0 ? (
        <section className="athena-recuperacion" aria-label="Runs interrumpidos">
          <h3>Interrumpidos</h3>
          <p className="athena-nota">
            Estos runs quedaron a medias cuando el runtime se detuvo. No están terminados.
          </p>
          <ul>
            {porRecuperar.map((resumen) => (
              <li key={resumen.runId}>
                <span>{resumen.objective || resumen.runId}</span>
                {resumen.degraded ? <em> (estado reconstruido)</em> : null}
                <button type="button" onClick={() => void reanudar(resumen.runId)}>
                  Reanudar
                </button>
              </li>
            ))}
          </ul>
        </section>
      ) : null}

      {run ? (
        <article className="athena-run" aria-label="Run en curso">
          <header>
            <h3>{run.objetivo}</h3>
            <p className="athena-fase" data-fase={run.fase ?? "desconocida"}>
              {nombreFase(run.fase)}
              {run.conectado ? "" : " · sin conexión con el run"}
            </p>
            <p className="athena-resumen">{resumenActividad(run)}</p>
            {run.degradado ? (
              <p className="athena-aviso">
                Athena tuvo que reconstruir el estado de este run tras un fallo.
              </p>
            ) : null}
            {puedeCancelarse(run) ? (
              <button type="button" onClick={() => void cancelar()}>
                Cancelar
              </button>
            ) : null}
            {puedeReanudarse(run) ? (
              <button type="button" onClick={() => void reanudar(run.runId)}>
                Reanudar
              </button>
            ) : null}
          </header>

          {permiso ? (
            <section
              className="athena-permiso"
              aria-label="Permiso pendiente"
              data-caducado={permiso.caducado ? "si" : "no"}
            >
              <h4>Athena necesita tu autorización</h4>
              <p className="athena-accion">{permiso.accion}</p>
              <p className="athena-motivo">{permiso.motivo}</p>

              <dl className="athena-permiso-datos">
                <dt>Herramienta</dt>
                <dd>
                  {permiso.herramienta}
                  {permiso.operacion ? ` · ${permiso.operacion}` : ""}
                </dd>
                <dt>Carpeta</dt>
                <dd>{permiso.workspace || "—"}</dd>
                {permiso.recursos.length > 0 ? (
                  <>
                    <dt>Afecta a</dt>
                    <dd>
                      <ul className="athena-recursos">
                        {permiso.recursos.map((recurso) => (
                          <li key={recurso}>{recurso}</li>
                        ))}
                      </ul>
                    </dd>
                  </>
                ) : null}
              </dl>

              {permiso.argumentos.length > 0 ? (
                <details className="athena-argumentos">
                  <summary>Argumentos ({permiso.argumentos.length})</summary>
                  <dl>
                    {permiso.argumentos.map((argumento) => (
                      <div key={argumento.nombre}>
                        <dt>{argumento.nombre}</dt>
                        <dd data-redactado={argumento.redactado ? "si" : "no"}>
                          {textoArgumento(argumento)}
                        </dd>
                      </div>
                    ))}
                  </dl>
                </details>
              ) : null}

              {permiso.efectos.length > 0 ? (
                <>
                  <p className="athena-nota">Puede provocar:</p>
                  <ul>
                    {permiso.efectos.map((efecto) => (
                      <li key={efecto}>{efecto}</li>
                    ))}
                  </ul>
                </>
              ) : null}

              <p className="athena-nota">
                {etiquetasPermiso(permiso).join(" · ")} · nivel {permiso.nivel} · queda{" "}
                {tiempoRestante(permiso)}
              </p>

              <div className="athena-decision">
                <button
                  type="button"
                  disabled={ocupado || bloqueoPermiso !== null}
                  onClick={() => void responder(permiso.requestId, true)}
                >
                  Permitir una vez
                </button>
                <button
                  type="button"
                  disabled={ocupado || bloqueoPermiso !== null}
                  onClick={() => void responder(permiso.requestId, false)}
                >
                  Denegar
                </button>
              </div>
              <p className="athena-nota">
                No hay «permitir siempre»: cada acción se autoriza por separado.
              </p>
              {bloqueoPermiso ? <p className="athena-aviso">{bloqueoPermiso}</p> : null}
            </section>
          ) : null}

          {run.tareas.length > 0 ? (
            <section className="athena-plan" aria-label="Plan">
              <h4>
                Plan{" "}
                <small className="athena-nota">{progresoPlan(run.tareas)}</small>
              </h4>
              {/* Sangrado por dependencia, no por orden de llegada: un plan leído
                  como lista no dice qué esperaba a qué. */}
              <ul className="athena-tareas">
                {ordenarPlan(run.tareas).map(({ tarea, nivel }) => (
                  <li
                    key={tarea.id}
                    data-estado={tarea.estado}
                    style={{ marginLeft: `${nivel * 18}px` }}
                  >
                    <span className="athena-marca" aria-hidden="true">
                      {simboloTarea(tarea.estado)}
                    </span>
                    <span className="athena-tarea-nombre">{tarea.nombre}</span>
                    {tarea.rol ? (
                      <span className="athena-rol">{nombreRol(tarea.rol)}</span>
                    ) : null}
                    <span className="athena-nota"> {nombreEstadoTarea(tarea.estado)}</span>
                    {tarea.detalle ? (
                      <p className="athena-motivo">{tarea.detalle}</p>
                    ) : null}
                    {tarea.ficheros.length > 0 ? (
                      <p className="athena-nota">
                        Tocó: {tarea.ficheros.join(", ")}
                      </p>
                    ) : null}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {run.herramientas.length > 0 ? (
            <section aria-label="Herramientas">
              <h4>Herramientas</h4>
              <ul>
                {run.herramientas.slice(-8).map((uso, indice) => (
                  <li key={`${uso.nombre}-${uso.correlacion ?? indice}`}>
                    {uso.nombre} — {uso.estado}
                    {uso.externalizado ? " · resultado guardado aparte" : ""}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {run.comprobaciones.length > 0 || veredicto ? (
            <section aria-label="Verificación">
              <h4>Verificación</h4>
              {veredicto ? <p className="athena-veredicto">{veredicto}</p> : null}
              {run.resumenVerificacion ? <p>{run.resumenVerificacion}</p> : null}
              <ul>
                {run.comprobaciones.map((comprobacion, indice) => (
                  <li key={`${comprobacion.nombre}-${indice}`}>
                    {comprobacion.nombre} —{" "}
                    {comprobacion.paso === undefined
                      ? "en curso"
                      : comprobacion.paso
                        ? "pasó"
                        : "falló"}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {run.ficherosModificados.length > 0 ? (
            <section aria-label="Ficheros modificados">
              <h4>Ficheros modificados</h4>
              <ul>
                {run.ficherosModificados.map((ruta) => (
                  <li key={ruta}>{ruta}</li>
                ))}
              </ul>
            </section>
          ) : null}

          {run.artefactos.length > 0 ? (
            <section aria-label="Artefactos">
              <h4>Resultados guardados</h4>
              <ul>
                {run.artefactos.map((item) => (
                  <li key={item.clave}>
                    <span>
                      {item.tipo} · {item.tamano} caracteres
                    </span>
                    <button type="button" onClick={() => void abrirArtefacto(item.clave)}>
                      Abrir
                    </button>
                  </li>
                ))}
              </ul>
              {artefacto ? <pre className="athena-artefacto">{artefacto}</pre> : null}
            </section>
          ) : null}

          {run.errores.length > 0 ? (
            <section aria-label="Errores">
              <h4>Errores</h4>
              <ul>
                {run.errores.map((error, indice) => (
                  <li key={`${error.codigo}-${indice}`}>
                    <strong>{error.codigo}</strong> {error.mensaje}
                    {error.recuperacion ? <em> · {error.recuperacion}</em> : null}
                  </li>
                ))}
              </ul>
            </section>
          ) : null}

          {run.actividad.length > 0 ? (
            <section aria-label="Actividad">
              <h4>Qué está haciendo</h4>
              <ol>
                {run.actividad.slice(-12).map((linea, indice) => (
                  <li key={`${linea}-${indice}`}>{linea}</li>
                ))}
              </ol>
            </section>
          ) : null}

          {esFaseTerminal(run.fase) && run.evidencia.length > 0 ? (
            <section aria-label="Evidencia">
              <h4>Evidencia</h4>
              <ul>
                {run.evidencia.map((linea, indice) => (
                  <li key={`${linea}-${indice}`}>{linea}</li>
                ))}
              </ul>
            </section>
          ) : null}
        </article>
      ) : null}
    </section>
  );
}
