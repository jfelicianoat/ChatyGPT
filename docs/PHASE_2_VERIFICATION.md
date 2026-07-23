# Evidencias de Fase 2

Fecha: 2026-07-22.

Estado: **Fase 2 en curso**. Este corte cubre memoria visible, manual, opt-in e
incluye un probador de recuperación semántica. La selección automática para el
chat sigue desactivada hasta completar el flujo durable de dos etapas.

## Matriz del corte

| Requisito | Estado | Evidencia | Resultado | Manual pendiente |
|---|---|---|---|---|
| Memoria desactivada por defecto | Verificado automáticamente | `feature_flags.memory = 0` y consulta defensiva en Rust | ningún recuerdo entra en un turno hasta activar la función | comprobar estado inicial en Inicio |
| Creación manual y visible | Verificado por tipos y compilación | formulario de Memoria + validación Rust de categoría, sensibilidad y 2.000 caracteres | solo el usuario crea recuerdos | crear un recuerdo desde la ventana |
| Ámbito global o proyecto | Verificado automáticamente | test Rust `memory_is_opt_in_scoped_and_user_controllable` | un recuerdo de proyecto no aparece en chats ajenos | comparar dos conversaciones |
| Control individual | Verificado automáticamente | activar, desactivar y borrar con confirmación | los elementos desactivados quedan fuera del contexto | probar los tres estados |
| Inclusión en el turno | Verificado automáticamente | test `approved_memory_is_visible_in_request_and_absent_without_items` | el prompt contiene únicamente recuerdos aprobados | preguntar por una preferencia guardada |
| Trazabilidad | Verificado automáticamente | snapshot `window-memory-v1` y fuente `memory` por recuerdo | se conserva qué memoria se utilizó en cada tarea | inspector detallado de contexto futuro |
| Auditoría | Verificado por repositorio y compilación | eventos `memory.enabled`, `created`, `item_enabled`, `item_disabled`, `deleted` | operaciones visibles en Actividad reciente sin mostrar el contenido | revisar cronología en Inicio |
| Indexación local | Verificado automáticamente | tarea durable `inference_kind=embedding`, proveedores limitados a Ollama/LM Studio, `cloud_allowed=false` y coste cero | test `memory_embedding_request_is_local_only_and_traceable` | guardar un recuerdo con Broker conectado |
| Persistencia vectorial | Verificado automáticamente | vector binario `f64`, dimensiones, modelo y SHA-256 en `embedding_records` | test `completed_memory_embedding_is_stored_with_model_and_dimensions` | comprobar estado `Índice preparado` |
| Estado y reintento | Verificado por tipos y compilación | estados `missing/indexing/ready/failed`, sondeo visual y botón `Indexar` | un fallo no elimina ni desactiva el recuerdo | detener Broker, crear y reintentar después |
| Contrato de embedding | Verificado automáticamente contra el esquema real del Broker | `selection_mode` no se envía dentro de `model_requirements`; la selección automática usa `execution.selection` por defecto | regresión `memory_embedding_request_is_local_only_and_traceable` | reintentar un recuerdo rechazado por la versión anterior |
| Error visible | Verificado automáticamente | tareas `orphaned` con `error_json` se presentan como `Error de índice` y muestran un mensaje limitado a 500 caracteres | el HTTP 422 deja de parecer un índice ausente | comprobar el recuerdo que falló originalmente |
| Recuperación tras reinicio | Revisado y compilado | la indexación usa `broker_tasks` y el mismo recuperador durable que el chat | la tarea continúa y el aviso la identifica como indexación de memoria | cerrar durante `Indexando…` |
| Búsqueda semántica manual | Verificado automáticamente | tarea durable para vectorizar la consulta y cálculo coseno local | test `semantic_memory_search_ranks_compatible_vectors_and_respects_scope` | escribir una consulta en Inicio → Memoria |
| Compatibilidad del índice | Verificado automáticamente | solo se comparan recuerdos con el mismo modelo y dimensiones que la consulta | evita mezclar espacios vectoriales incompatibles | indexar recuerdos con modelos distintos |
| Resultado explicable | Verificado por tipos y compilación | porcentaje, nivel de coincidencia, ámbito y texto original visibles | el usuario puede auditar por qué apareció cada recuerdo | ejecutar una búsqueda con dos recuerdos relacionados |
| Consulta recuperable | Revisado y compilado | tabla `memory_searches`, tarea asociada y reapertura de la última búsqueda | una búsqueda pendiente continúa tras reiniciar la app | cerrar mientras muestra `Buscando…` |

## Verificación automática

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
pnpm test
pnpm typecheck
pnpm build
```

Resultado: 21 pruebas Rust y 4 pruebas TypeScript correctas, Clippy sin
errores y frontend de producción compilado.

## Siguiente corte

Selección semántica opt-in dentro del envío de chat, con persistencia del turno
antes de vectorizar la consulta y trazabilidad de los recuerdos finalmente
incluidos. Se mantendrá desactivada por defecto hasta que el usuario la habilite.
