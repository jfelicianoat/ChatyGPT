-- Recuerda qué run de Athena estaba abierto para poder re-engancharse tras reiniciar.
--
-- ChatyGPT no guarda el estado del agente: la verdad sigue estando en Athena.
-- Lo único que se persiste aquí es la referencia mínima para volver a preguntar
-- por él, más un espejo de la última fase vista para poder ordenar la lista sin
-- consultar al runtime.

CREATE TABLE athena_runs (
    run_id TEXT PRIMARY KEY,
    objective TEXT NOT NULL,
    workspace TEXT NOT NULL,
    subscriber_id TEXT,
    last_phase TEXT,
    started_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    updated_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    closed_at TEXT
);

CREATE INDEX idx_athena_runs_abiertos ON athena_runs(closed_at, updated_at DESC);
