-- Fase 4: medición local de los objetivos de rendimiento.
--
-- La tabla guarda exclusivamente duraciones. No admite texto libre: `metric`
-- está restringido por CHECK a un vocabulario cerrado y el resto de columnas son
-- un entero y una marca de tiempo. Por construcción no puede contener un
-- prompt, un título de conversación, una ruta ni un identificador de dominio,
-- de modo que medir el rendimiento nunca crea un segundo registro de contenido
-- personal al margen de la conversación.
--
-- Las muestras se conservan acotadas: el runtime poda cada métrica a sus
-- últimas ejecuciones dentro de la misma transacción que inserta, así el
-- expediente no crece sin límite aunque la aplicación se use durante meses.

CREATE TABLE IF NOT EXISTS performance_samples (
    id INTEGER PRIMARY KEY AUTOINCREMENT,
    metric TEXT NOT NULL CHECK (
        metric IN (
            'app_start',
            'conversation_open',
            'conversation_search',
            'ui_response'
        )
    ),
    duration_ms INTEGER NOT NULL CHECK (duration_ms >= 0 AND duration_ms <= 600000),
    recorded_at TEXT NOT NULL DEFAULT (strftime('%Y-%m-%dT%H:%M:%fZ', 'now'))
) STRICT;

-- La poda y los percentiles leen siempre por métrica y por orden de inserción.
CREATE INDEX IF NOT EXISTS idx_performance_samples_metric
    ON performance_samples(metric, id DESC);
