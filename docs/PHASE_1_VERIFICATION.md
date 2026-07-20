# Evidencias de Fase 1

Fecha: 2026-07-20.

Estado: **Fase 1 en curso**. Este documento cubre organización local y el corte
vertical de adjuntos; no declara completada la fase completa.

## Matriz del corte

| Requisito | Estado | Evidencia | Prueba ejecutada | Resultado | Limitación | Manual pendiente |
|---|---|---|---|---|---|---|
| Crear y listar proyectos | Verificado automáticamente | repositorio Rust y comandos Tauri | test Rust `projects_search_and_lifecycle_are_audited` | correcto | sin descripción editable en UI | crear desde la ventana |
| Asociar conversaciones | Verificado automáticamente | `project_id` y `move_conversation` | test Rust + claves foráneas | relación persistente y reversible | sin drag-and-drop | mover desde selector |
| Buscar historial | Verificado automáticamente | búsqueda parametrizada sobre título y partes de mensaje | test por contenido y comodín literal | correcto | `LIKE`, todavía sin FTS | validar experiencia con muchos chats |
| Renombrar conversación/proyecto | Verificado automáticamente por compilación y repositorio | comandos tipados + auditoría | Cargo test/check/clippy | correcto | interacción no automatizada | probar diálogos |
| Archivar y eliminar | Verificado automáticamente | borrado lógico, confirmación backend y auditoría | test de ciclo de vida | correcto | restauración aún no implementada | probar confirmación visible |
| Proteger tareas activas | Verificado automáticamente | guardia sobre `broker_tasks.local_state` | test `conversation_with_active_task_cannot_be_hidden` | archivar/eliminar se bloquean | cancelación real pendiente | probar con tarea larga |
| Recuperar progreso en UI | Revisado estáticamente + compilado | mensajes exponen vínculo y estado de tarea | TypeScript + Cargo + Tauri build | compila | no se cerró la app durante una tarea real | cierre/reapertura |
| Auditoría | Verificado automáticamente | `audit_events` | recuento de eventos en test Rust | cuatro operaciones trazadas | no hay inspector UI | inspeccionar futura pantalla |
| Ejecutable Windows | Verificado automáticamente | Tauri CLI | `tauri build --no-bundle` | `target/release/chatygpt.exe` generado | sin MSI/NSIS | abrir con perfil Windows |
| Importar y deduplicar adjuntos | Verificado automáticamente | copia administrada + SHA-256 + esquema 2 | tests Rust de archivo y SQLite | una copia reutilizable entre conversaciones | límite local 512 MB | seleccionar y arrastrar en ventana real |
| Ingesta durable | Revisado estáticamente + compilado | subida multipart en streaming, polling y recuperación | Cargo check/test/clippy | estados reales `uploading/received/converting/ready/failed` | Broker inaccesible desde sandbox Codex | probar PDF contra A9_Mega |
| Usar `file_id` en chat | Verificado automáticamente por tipos, transacción y compilación | solo adjuntos `ready` asociados a la conversación | Rust + TypeScript + Vite | el compositor se bloquea durante ingesta | falta prueba visual | enviar pregunta sobre un PDF |
| Mantener adjuntos activos | Verificado por compilación | la selección permanece tras enviar y solo cambia por acción explícita o al cambiar de conversación | TypeScript + Vite | el PDF se incluye en turnos sucesivos | sin prueba E2E visual | realizar dos preguntas seguidas sobre el mismo PDF |

## Comandos ejecutados

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Resultado: 8 pruebas Rust correctas y Clippy sin errores, incluida la migración
de una base existente desde esquema 1 a esquema 2 sin perder conversaciones.

```powershell
tsc -b --pretty false
vite build
python -m unittest discover -s tests -v
```

Resultado: TypeScript y Vite correctos; 13 pruebas Python correctas.

```powershell
node node_modules\@tauri-apps\cli\tauri.js build --no-bundle --ci `
  --config scripts\tauri-build-validation.json
```

Resultado: aplicación de producción generada correctamente. La validación usó
un archivo temporal que anuló únicamente `beforeBuildCommand`, porque el
frontend ya estaba compilado. El archivo temporal se eliminó después.

## Decisiones

- Los proyectos archivados se ocultan y sus conversaciones vuelven a “Sin
  proyecto”; no se pierde el chat.
- La eliminación de conversación es lógica mediante `deleted_at`.
- Archivar o eliminar exige confirmación tanto en interfaz como en el comando
  backend.
- Una tarea no terminal impide ocultar su conversación.
- La búsqueda escapa `%`, `_` y el propio carácter de escape para que la entrada
  se trate literalmente.

## Siguiente corte

Visualización de citas y fuentes devueltas por Broker AI, seguida de herramientas
con confirmación explícita y estados recuperables.
