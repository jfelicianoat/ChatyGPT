# Evidencias de Fase 1

Fecha: 2026-07-20.

Estado: **Fase 1 en curso**. Este documento cubre el primer corte vertical de
organización local; no declara completada la fase completa.

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

## Comandos ejecutados

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Resultado: 4 pruebas Rust correctas y Clippy sin errores.

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

Adjuntos reutilizables: selección y arrastre local, hashing, persistencia,
subida a Broker AI, seguimiento de ingesta y uso explícito de `file_id` en un
turno.
