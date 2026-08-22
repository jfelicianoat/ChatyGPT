/**
 * Los delegados de un run: quién hace qué parte, con quién los ejecuta y qué informaron.
 *
 * Panel propio y no una rama del plan. Athena lo dice en su catálogo de eventos: una
 * tarea *usa* un subagente, no *es* uno. Enseñarlos juntos hacía que un run jerárquico
 * mostrara el doble de trabajo del que había.
 *
 * Lo que se enseña son hechos operativos —rol, proveedor, estado, ficheros, bloqueos,
 * lo que informó— y nunca el razonamiento del delegado: Athena entrega un resumen y el
 * transcript del hijo no sale de su sesión, así que aquí no hay nada que filtrar.
 */

import type { AthenaDelegado } from "./domain";
import { nombreEstadoTarea, nombreRol, simboloTarea } from "./athenaView";

type Props = {
  delegados: AthenaDelegado[];
};

/** Cuántas líneas de actividad se enseñan de cada delegado. */
const LIMITE_ACTIVIDAD = 6;

export function AthenaDelegados({ delegados }: Props) {
  if (delegados.length === 0) {
    return null;
  }
  return (
    <section className="athena-delegados" aria-label="Delegados">
      <h4>Delegados</h4>
      <ul>
        {delegados.map((delegado) => (
          <li key={delegado.sesion} data-estado={delegado.estado}>
            <p className="athena-delegado-cabecera">
              <span className="athena-marca" aria-hidden="true">
                {simboloTarea(delegado.estado)}
              </span>
              <span className="athena-rol">{nombreRol(delegado.rol)}</span>
              <span className="athena-nota">{nombreEstadoTarea(delegado.estado)}</span>
              {/* De quién es el delegado. Callarlo presentaría como propios de Athena
                  delegados que puede ejecutar otro proveedor desde la fase 2. */}
              {delegado.proveedor ? (
                <span className="athena-nota"> · lo ejecuta {delegado.proveedor}</span>
              ) : null}
              {delegado.continuable ? (
                <span className="athena-nota">
                  {" "}
                  · se le puede volver a preguntar
                  {delegado.seguimientosRestantes !== undefined
                    ? ` (${delegado.seguimientosRestantes})`
                    : ""}
                </span>
              ) : (
                <span className="athena-nota"> · un solo encargo</span>
              )}
            </p>

            {delegado.encargo ? (
              <p className="athena-motivo">{delegado.encargo}</p>
            ) : null}

            <p className="athena-nota">
              {delegado.tarea ? `Tarea ${delegado.tarea} · ` : ""}
              encargado por {delegado.padre}
              {delegado.seguimientos > 0
                ? ` · ${delegado.seguimientos} seguimiento(s)`
                : ""}
              {delegado.llamadasHerramienta !== undefined
                ? ` · ${delegado.llamadasHerramienta} llamadas`
                : ""}
            </p>

            {/* El informe, que es lo que el delegado contestó. No es su transcript:
                Athena entrega un resumen por construcción. */}
            {delegado.resumen ? (
              <p className="athena-delegado-informe">{delegado.resumen}</p>
            ) : null}

            {delegado.bloqueos.length > 0 ? (
              <p className="athena-aviso">{delegado.bloqueos.join(" · ")}</p>
            ) : null}

            {delegado.error ? (
              <p className="athena-aviso">
                <strong>{delegado.error.codigo}</strong> {delegado.error.mensaje}
              </p>
            ) : null}

            {delegado.ficheros.length > 0 ? (
              <p className="athena-nota">Tocó: {delegado.ficheros.join(", ")}</p>
            ) : null}

            {delegado.actividad.length > 0 ? (
              <ol className="athena-delegado-actividad">
                {delegado.actividad.slice(-LIMITE_ACTIVIDAD).map((linea, indice) => (
                  <li key={`${linea}-${indice}`}>{linea}</li>
                ))}
              </ol>
            ) : null}
          </li>
        ))}
      </ul>
    </section>
  );
}
