-- Añade la medición del tiempo hasta el primer estado visible al enviar.
--
-- SQLite no permite ampliar un CHECK existente. Se reconstruye únicamente la
-- tabla acotada de duraciones, conservando sus muestras y sin tocar contenido
-- de conversaciones ni tareas.

ALTER TABLE performance_samples RENAME TO performance_samples_before_remote_start;

CREATE TABLE performance_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric TEXT NOT NULL CHECK (
        metric IN (
            'app_start',
            'conversation_open',
            'conversation_search',
            'remote_operation_start',
            'ui_response'
        )
    ),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0 AND duration_ms <= 600000),
    recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

INSERT INTO performance_samples(id, metric, duration_ms, recorded_at)
SELECT id, metric, duration_ms, recorded_at
FROM performance_samples_before_remote_start;

DROP TABLE performance_samples_before_remote_start;

CREATE INDEX idx_performance_samples_metric
    ON performance_samples(metric, id DESC);
