-- Correcciones de la auditoría del 28 de septiembre de 2026.
--
-- Todas las columnas son aditivas y opcionales: una base 0.2.0 abre sin
-- reescribir filas y los datos históricos conservan su significado.

-- H08: la intención de cancelar se persiste aunque la tarea todavía no tenga
-- identidad remota. El bucle de envío la respeta antes de cada intento.
ALTER TABLE broker_tasks
    ADD COLUMN cancel_requested_at TEXT;

-- H05: cada ejecución congela la instrucción y el destino que se reclamaron.
-- La recuperación no vuelve a leer la programación, que pudo editarse después.
ALTER TABLE scheduled_runs
    ADD COLUMN payload_json TEXT CHECK (payload_json IS NULL OR json_valid(payload_json));

-- H15: retirar una programación ya no borra su historial.
ALTER TABLE scheduled_tasks
    ADD COLUMN retired_at TEXT;

-- H14: al archivar un proyecto se recuerda qué conversaciones agrupaba para
-- poder devolverlas a su sitio al restaurarlo.
ALTER TABLE conversations
    ADD COLUMN archived_project_id TEXT;

-- H02: la clasificación con la que se produjo cada salida de un nodo viaja con
-- ella. Un nodo posterior nunca puede tratarla con una política más laxa.
ALTER TABLE workflow_node_runs
    ADD COLUMN data_classification TEXT
        CHECK (data_classification IS NULL OR data_classification IN ('public', 'internal', 'confidential', 'local_only'));

-- H05: una ejecución de flujo lanzada por una programación tiene una identidad
-- estable. Relanzar la misma operación devuelve la ejecución ya creada.
ALTER TABLE workflow_runs
    ADD COLUMN operation_key TEXT;

CREATE UNIQUE INDEX IF NOT EXISTS idx_workflow_runs_operation_key
    ON workflow_runs(operation_key)
    WHERE operation_key IS NOT NULL;

-- H23: copias locales de los ficheros que generó una tarea. Siguen accesibles
-- desde el mensaje aunque el Broker pode el artefacto remoto.
CREATE TABLE IF NOT EXISTS task_artifact_copies (
    id TEXT PRIMARY KEY,
    remote_task_id TEXT NOT NULL,
    artifact_id TEXT NOT NULL,
    filename TEXT NOT NULL,
    media_type TEXT,
    size_bytes INTEGER NOT NULL CHECK (size_bytes >= 0),
    sha256 TEXT NOT NULL,
    local_path TEXT NOT NULL,
    saved_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now')),
    UNIQUE(remote_task_id, artifact_id)
) STRICT;
