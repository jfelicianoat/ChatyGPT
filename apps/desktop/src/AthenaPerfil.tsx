/**
 * Elegir para qué clase de trabajo es un run.
 *
 * Un perfil cambia dos cosas a la vez: **qué herramientas existen** —es un filtro
 * estructural, no un permiso— y **qué cuenta como prueba**. Por eso se elige al crear el
 * run y no después: cambiarlo a mitad haría que la evidencia ya reunida dejase de
 * significar lo que decía (ADR-028).
 *
 * La lista viene de Athena. Una copia local caducaría en silencio en cuanto el
 * despliegue añadiera un perfil, y quien eligiera de esa copia estaría eligiendo entre
 * los perfiles de otro Athena.
 */

import { useEffect, useState } from "react";

import type { AthenaListadoPerfiles, AthenaPerfil } from "./domain";

type Props = {
  /** El nombre elegido. Vacío = el de por defecto del despliegue. */
  valor: string;
  onCambiar: (nombre: string) => void;
  onListar: () => Promise<AthenaListadoPerfiles>;
  deshabilitado?: boolean;
};

/** Qué demuestra cada clase de evidencia, dicho para que se entienda. */
export function nombreEvidencia(evidencia: string): string {
  switch (evidencia) {
    case "executed_checks":
      return "Ejecuta las comprobaciones del proyecto y las hace pasar";
    case "produced_artifacts":
      return "Comprueba que los entregables existen y no están vacíos";
    case "none":
      return "No comprueba nada por su cuenta";
    default:
      return evidencia;
  }
}

export function AthenaPerfilSelector({
  valor,
  onCambiar,
  onListar,
  deshabilitado
}: Props) {
  const [listado, setListado] = useState<AthenaListadoPerfiles | null>(null);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    let cancelado = false;
    void (async () => {
      try {
        const respuesta = await onListar();
        if (!cancelado) {
          setListado(respuesta);
        }
      } catch (fallo) {
        if (!cancelado) {
          // Sin lista no se ofrece elegir. Enseñar un desplegable vacío invitaría a
          // creer que este Athena sólo tiene un perfil, que es una afirmación distinta
          // de no haber podido preguntarlo.
          setError(fallo instanceof Error ? fallo.message : String(fallo));
        }
      }
    })();
    return () => {
      cancelado = true;
    };
  }, [onListar]);

  if (error) {
    return (
      <div className="athena-form-field">
        <small role="alert">No se pudo consultar los perfiles: {error}</small>
      </div>
    );
  }
  if (!listado || listado.profiles.length === 0) {
    return null;
  }

  const elegido: AthenaPerfil | undefined =
    listado.profiles.find((perfil) => perfil.name === (valor || listado.default)) ??
    listado.profiles[0];

  return (
    <div className="athena-form-field">
      <label htmlFor="athena-perfil">Para qué es este trabajo</label>
      <select
        id="athena-perfil"
        value={valor}
        disabled={deshabilitado}
        onChange={(evento) => onCambiar(evento.target.value)}
      >
        <option value="">
          El de por defecto de este Athena ({listado.default})
        </option>
        {listado.profiles.map((perfil) => (
          <option key={perfil.name} value={perfil.name}>
            {perfil.name}
          </option>
        ))}
      </select>
      {elegido ? (
        <div className="athena-perfil-detalle">
          {elegido.description ? <p>{elegido.description}</p> : null}
          <dl>
            <div>
              <dt>Trabaja sobre</dt>
              <dd>{elegido.subject}</dd>
            </div>
            <div>
              <dt>Cómo comprueba</dt>
              <dd>{nombreEvidencia(elegido.evidence)}</dd>
            </div>
            {/* Qué demuestra —y qué no—. Un perfil puede dar evidencia más débil, pero
                no puede callárselo: es la mitad del contrato que hace que elegirlo sea
                una decisión y no una apuesta. */}
            {elegido.proves ? (
              <div>
                <dt>Qué demuestra</dt>
                <dd>{elegido.proves}</dd>
              </div>
            ) : null}
            {elegido.tools.length > 0 ? (
              <div>
                <dt>Qué puede usar</dt>
                <dd>{elegido.tools.join(", ")}</dd>
              </div>
            ) : null}
          </dl>
        </div>
      ) : null}
      <small>
        El perfil queda fijado al crear el trabajo: no se puede cambiar a mitad.
      </small>
    </div>
  );
}
