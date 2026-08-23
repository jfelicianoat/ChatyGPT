/**
 * Elegir con qué modelo trabaja un run.
 *
 * Se elige al crear el run y no después, por la misma razón que el perfil: el modelo es
 * quien toma todas las decisiones del bucle, y cambiarlo a mitad dejaría un run cuya
 * primera mitad la pensó uno y la segunda otro, sin que nada en el resultado lo dijera.
 *
 * La lista viene de Athena. Escribirla aquí la dejaría caducada en cuanto el despliegue
 * cambiara la suya, y quien eligiera de esa copia estaría eligiendo entre los modelos de
 * otro Athena.
 *
 * No hay adjetivos. Athena publica nombres, y poner aquí «el mejor para código» sería
 * inventar una recomendación que nadie mantiene: lo que sí es un hecho es cuál corre
 * cuando no se elige, y eso se enseña.
 */

import { useEffect, useState } from "react";

import type { AthenaListadoModelos } from "./domain";

type Props = {
  /** El nombre elegido. Vacío = el de por defecto del despliegue. */
  valor: string;
  onCambiar: (nombre: string) => void;
  onListar: () => Promise<AthenaListadoModelos>;
  deshabilitado?: boolean;
};

export function AthenaModeloSelector({ valor, onCambiar, onListar, deshabilitado }: Props) {
  const [listado, setListado] = useState<AthenaListadoModelos | null>(null);
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
          // No haber podido preguntar es distinto de que no haya elección, y se dice.
          // Un desplegable vacío afirmaría lo segundo sin haberlo comprobado.
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
        <small role="alert">No se pudo consultar los modelos: {error}</small>
      </div>
    );
  }
  // Sin modelos que ofrecer no hay nada que elegir: este despliegue corre con uno fijo.
  // Un selector de un solo elemento pediría una decisión que no existe.
  if (!listado || listado.models.length <= 1) {
    return null;
  }

  return (
    <div className="athena-form-field">
      <label htmlFor="athena-modelo">Con qué modelo</label>
      <select
        id="athena-modelo"
        value={valor}
        disabled={deshabilitado}
        onChange={(evento) => onCambiar(evento.target.value)}
      >
        <option value="">El de por defecto de este Athena ({listado.default})</option>
        {listado.models.map((modelo) => (
          <option key={modelo.name} value={modelo.name}>
            {modelo.name}
            {modelo.default ? " (por defecto)" : ""}
          </option>
        ))}
      </select>
      <small>
        Al elegir uno, Athena le pide al broker que no lo sustituya: si ese modelo no está
        disponible, el trabajo falla diciéndolo en vez de correr con otro.
      </small>
    </div>
  );
}
