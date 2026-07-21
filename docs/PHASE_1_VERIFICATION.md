# Evidencias de Fase 1

Fecha: 2026-07-21.

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
| Herramientas opt-in | Verificado automáticamente | el modo normal conserva `single`; el conmutador crea estrategia `agent` con passthrough | test Rust `tools_mode_uses_agent_passthrough_only_when_enabled` | ninguna tool se ofrece por defecto | depende de un modelo con function calling | pedir explícitamente renombrar el chat |
| Confirmación de herramientas | Verificado automáticamente | `waiting_for_tools` persiste llamada y argumentos; aprobar/rechazar prepara resultados antes de HTTP | test Rust `waiting_tool_call_is_persisted_and_decisions_are_durable` | decisiones recuperables | prueba E2E real pendiente | aprobar y rechazar desde la ventana |
| Acción local confirmada | Revisado y compilado | `rename_conversation` valida título, audita el cambio y solo se ejecuta tras autorización | Cargo + TypeScript + Vite | una herramienta disponible | todavía sin deshacer | comprobar cambio de título |
| Exportar Markdown | Verificado automáticamente | formato estable con mensajes y fuentes realmente utilizadas | test Rust `export_detects_external_changes_and_requires_overwrite_confirmation` | archivo legible y sin rutas internas ni identificadores del Broker | no incluye citas por afirmación inexistentes en el contrato | exportar una conversación real |
| Sobrescritura segura | Verificado automáticamente | selector nativo, detección de modificación externa y reemplazo atómico | tests Rust de conflicto y escritura atómica | no se altera un destino modificado sin confirmación | la confirmación visual depende de Windows | probar aceptar y cancelar el diálogo |
| Trazabilidad del exportado | Verificado automáticamente | huellas SHA-256 antes/después y registro durable en `export_records` + auditoría | tests Rust + migración existente | exportación completada o conflictiva queda registrada | sin inspector de auditoría en UI | inspeccionar futura pantalla |
| Sandbox de código opt-in | Verificado automáticamente | `run_code` solo entra en `agent.skills` cuando el usuario habilita el permiso para ese turno | test Rust `sandbox_is_explicit_and_requires_broker_capability` | el modo normal nunca ofrece ejecución de código | requiere Docker y sandbox activo en A9_Mega | ejecutar un cálculo real desde la ventana |
| Guardia de capacidad | Verificado automáticamente | UI condicionada por diagnóstico y validación backend inmediata de `sandbox_run_code` + `agent_skills` | Rust + TypeScript | una petición manipulada tampoco activa un sandbox ausente | disponibilidad real depende del Broker | comprobar indicador de capacidad |
| Frontera de aislamiento | Revisado contra contrato local del Broker | contenedor desechable, sin red, volúmenes del host ni privilegios; filesystem raíz de solo lectura | revisión `AI_Broker/docs/Phase_8_Sandbox.md` | ChatyGPT no ejecuta código generado en su propio proceso | comparte kernel con WSL2 según modelo de amenaza del Broker | prueba controlada en A9_Mega |
| Inspector de actividad | Verificado automáticamente | últimos 50 eventos con resumen, categoría, severidad, actor y conversación legibles | Rust + TypeScript + Vite | cronología visible y actualizable desde Inicio | sin filtros ni paginación todavía | revisar presentación con historial real |
| Privacidad del inspector | Verificado automáticamente | DTO derivado sin `payload_json`, identificadores técnicos, rutas ni hashes | test Rust `audit_inspector_exposes_only_safe_presentation_fields` | una exportación interna no filtra su destino ni su SHA-256 | los títulos de conversación sí son visibles por diseño | revisar eventos reales sensibles |
| Recuperación visible al reiniciar | Verificado automáticamente | candidatos capturados antes de reanudar y expuestos en el informe de arranque | test Rust `pending_conversation_is_identified_for_visible_startup_recovery` | aviso global con tareas y adjuntos recuperados | el cierre real depende de una tarea suficientemente larga | realizar prueba manual cerrando la app |
| Acceso a conversación recuperada | Verificado por tipos y compilación | el aviso conserva la relación durable tarea-conversación y ofrece `Abrir conversación` | TypeScript + Tauri build | navegación directa sin mostrar identificadores | las pruebas durables sin conversación no tienen acceso directo | validar botón tras reinicio real |

## Comandos ejecutados

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
```

Resultado: 16 pruebas Rust correctas y Clippy sin errores, incluida la migración
de una base existente hasta esquema 3 sin perder conversaciones y la
materialización de fuentes en la respuesta asistente. También se verificaron
el reemplazo atómico, las huellas finales y el bloqueo de una sobrescritura
cuando el archivo cambió fuera de ChatyGPT.

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

Validación E2E manual cerrando y reabriendo la aplicación durante una tarea
real suficientemente larga. Las citas por afirmación quedan condicionadas a que Broker AI amplíe
su contrato con citas estructuradas; mientras tanto la app muestra únicamente
fuentes documentales realmente enviadas en cada turno.
