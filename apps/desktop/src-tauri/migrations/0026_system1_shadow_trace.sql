-- Traza de la selección System 1 calculada en modo sombra.
--
-- La sombra se evalúa en segundo plano después de crear la tarea, para no
-- retrasar el turno. No puede escribirse en `request_json`: ese cuerpo es la
-- petición idempotente que ya se envió (o se reenviará) al broker.
-- Aditiva y opcional: las tareas anteriores conservan su significado.
ALTER TABLE broker_tasks
    ADD COLUMN system1_trace_json TEXT
        CHECK (system1_trace_json IS NULL OR json_valid(system1_trace_json));
