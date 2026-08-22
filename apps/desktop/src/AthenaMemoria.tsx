/**
 * Lo que Athena cree saber de un proyecto, para que alguien pueda mirarlo.
 *
 * Existe porque el escalón más alto de la memoria —«una persona lo respalda»— no se
 * puede alcanzar sin una persona, y una persona no puede respaldar lo que no ve. Sin
 * este panel, `user_confirmed` era un estado inalcanzable con nombre (ADR-031).
 *
 * La regla del lado del cliente: **la interfaz no convierte propuestas en hechos**.
 * Nada aquí sube de estado solo; hay un botón, y lo pulsa alguien.
 */

import { useCallback, useEffect, useState } from "react";

import type { AthenaRecuerdo } from "./domain";

type Props = {
  /** El espacio de trabajo cuyo proyecto se consulta. */
  workspaceId: string;
  onListar: (workspaceId: string) => Promise<AthenaRecuerdo[]>;
  onConfirmar: (memoryId: string) => Promise<AthenaRecuerdo>;
  /**
   * Retira un recuerdo.
   *
   * Preguntar antes es cosa de quien pasa esta función, no de este panel: la
   * pregunta tiene que vivir donde se afirma la confirmación, o la comprobación
   * estructural que la vigila deja de verla.
   */
  onOlvidar: (memoryId: string) => Promise<void>;
};

/** Qué clase de cosa se recuerda, en castellano. */
export function nombreClaseRecuerdo(clase: string): string {
  switch (clase) {
    case "verified_command":
      return "Comando que funcionó";
    case "project_convention":
      return "Convención del proyecto";
    case "architecture_decision":
      return "Decisión de arquitectura";
    case "known_constraint":
      return "Restricción conocida";
    case "domain_fact":
      return "Hecho del dominio";
    case "user_confirmed_fact":
      return "Hecho confirmado por una persona";
    case "environment_fact":
      return "Hecho del entorno";
    default:
      return clase;
  }
}

/**
 * Cuánto peso ha ganado un recuerdo.
 *
 * El orden es el punto del tipo, y por eso las tres frases dicen *quién* lo respalda y
 * no *cuánto de fiable es*: «propuesto» y «verificado» no son grados de confianza, son
 * autores distintos.
 */
export function nombreVerificacion(estado: string): string {
  switch (estado) {
    case "proposed":
      return "Lo dijo el modelo; nadie lo ha comprobado";
    case "verified":
      return "Algo lo comprobó";
    case "user_confirmed":
      return "Una persona respondió por ello";
    default:
      return estado;
  }
}

/** Si el recuerdo sigue en pie, lo reemplazaron o lo retiraron. */
export function nombreEstadoRecuerdo(estado: string): string {
  switch (estado) {
    case "active":
      return "Vigente";
    case "superseded":
      return "Sustituido por uno más nuevo";
    case "forgotten":
      return "Retirado";
    default:
      return estado;
  }
}

export function AthenaMemoria({
  workspaceId,
  onListar,
  onConfirmar,
  onOlvidar
}: Props) {
  const [recuerdos, setRecuerdos] = useState<AthenaRecuerdo[]>([]);
  const [error, setError] = useState<string | null>(null);
  const [ocupado, setOcupado] = useState(false);

  const refrescar = useCallback(async () => {
    if (!workspaceId) {
      return;
    }
    try {
      setRecuerdos(await onListar(workspaceId));
      setError(null);
    } catch (fallo) {
      setError(fallo instanceof Error ? fallo.message : String(fallo));
    }
  }, [workspaceId, onListar]);

  useEffect(() => {
    void refrescar();
  }, [refrescar]);

  const confirmar = async (id: string) => {
    setOcupado(true);
    try {
      await onConfirmar(id);
      await refrescar();
    } catch (fallo) {
      setError(fallo instanceof Error ? fallo.message : String(fallo));
    } finally {
      setOcupado(false);
    }
  };

  const olvidar = async (recuerdo: AthenaRecuerdo) => {
    setOcupado(true);
    try {
      await onOlvidar(recuerdo.id);
      await refrescar();
    } catch (fallo) {
      setError(fallo instanceof Error ? fallo.message : String(fallo));
    } finally {
      setOcupado(false);
    }
  };

  if (!workspaceId) {
    return null;
  }

  return (
    <section className="athena-memoria" aria-label="Memoria del proyecto">
      <h4>Lo que Athena recuerda de este proyecto</h4>
      {error ? <p className="athena-aviso">{error}</p> : null}
      {recuerdos.length === 0 && !error ? (
        <p className="athena-nota">
          Todavía no ha aprendido nada de este proyecto.
        </p>
      ) : null}
      <ul>
        {recuerdos.map((recuerdo) => (
          <li key={recuerdo.id} data-estado={recuerdo.verificationState}>
            <p className="athena-recuerdo-contenido">{recuerdo.content}</p>
            <dl>
              <div>
                <dt>Tipo</dt>
                <dd>{nombreClaseRecuerdo(recuerdo.kind)}</dd>
              </div>
              <div>
                {/* Quién lo respalda, que no es lo mismo que si sigue vigente. */}
                <dt>Quién responde</dt>
                <dd>{nombreVerificacion(recuerdo.verificationState)}</dd>
              </div>
              <div>
                <dt>Estado</dt>
                <dd>
                  {nombreEstadoRecuerdo(recuerdo.status)}
                  {/* Lo viejo se etiqueta, no se tira: un recuerdo caduco sigue
                      diciendo qué se creía, y borrarlo escondería que se creyó. */}
                  {recuerdo.stale ? " · ha pasado su plazo" : ""}
                </dd>
              </div>
              <div>
                <dt>De dónde salió</dt>
                <dd>
                  {recuerdo.source}
                  {recuerdo.sourceReference ? ` · ${recuerdo.sourceReference}` : ""}
                </dd>
              </div>
              <div>
                <dt>Cuándo</dt>
                <dd>{recuerdo.createdAt}</dd>
              </div>
              {/* La confianza sólo se enseña si Athena la dio: un 0 por defecto se lee
                  como «no se fía», que es una afirmación que nadie hizo. */}
              {recuerdo.confidence > 0 ? (
                <div>
                  <dt>Confianza</dt>
                  <dd>{Math.round(recuerdo.confidence * 100)}%</dd>
                </div>
              ) : null}
              {recuerdo.supersedes ? (
                <div>
                  <dt>Sustituye a</dt>
                  <dd>{recuerdo.supersedes}</dd>
                </div>
              ) : null}
            </dl>
            <div className="athena-decision">
              {/* Confirmar es lo único que sube un recuerdo de estado, y sólo lo hace
                  una persona: la interfaz nunca lo hace por su cuenta. */}
              {recuerdo.verificationState !== "user_confirmed" ? (
                <button
                  type="button"
                  disabled={ocupado}
                  onClick={() => void confirmar(recuerdo.id)}
                >
                  Respondo por esto
                </button>
              ) : null}
              <button
                type="button"
                disabled={ocupado}
                onClick={() => void olvidar(recuerdo)}
              >
                Retirar
              </button>
            </div>
          </li>
        ))}
      </ul>
      <p className="athena-nota">
        Athena no ofrece corregir un recuerdo: lo que hace es sustituirlo cuando aprende
        algo que lo contradice, y el viejo queda marcado. Aquí sólo se puede respaldar o
        retirar.
      </p>
    </section>
  );
}
