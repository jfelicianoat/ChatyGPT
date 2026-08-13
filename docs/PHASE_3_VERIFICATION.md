# Evidencias de Fase 3

Fecha: 2026-07-30.

Estado: **Fase 3 completada**. Permite crear y editar GPTs personales
versionados, seleccionarlos por conversación y congelar la versión exacta usada
por cada tarea, además de iniciarlos y transportarlos de forma segura, sin
conceder herramientas o acciones sensibles. Cada GPT también puede mantener
conocimiento textual y documental privado sin mezclarlo con la memoria general.

## Matriz del corte

| Requisito | Estado | Evidencia | Resultado | Manual pendiente |
|---|---|---|---|---|
| Creación guiada | Verificado por compilación y persistencia | comandos `create_custom_gpt` y formulario **Inicio → Mis GPTs** | nombre, descripción opcional e instrucciones se validan antes de guardar | crear un GPT y reiniciar la aplicación |
| Identidad visual versionada | Verificado automáticamente | test `custom_gpt_icon_is_validated_versioned_portable_and_duplicated`, selector **Icono** y prueba de `WorkflowStudio` | solo admite el catálogo seguro; cada revisión conserva su icono, exportar/importar y duplicar lo mantienen, y publicar un flujo lo congela junto a la versión del GPT | **Inicio → GPTs** → elegir un icono → guardar; comprobarlo en la ficha, selector del chat e inspector/nodo de **Flujos**; editar el icono y abrir **Historial** |
| Versiones inmutables | Verificado automáticamente | test `custom_gpt_edits_create_immutable_versions_without_tool_permissions` | una edición crea `version_no + 1`, cambia la versión activa y conserva el JSON anterior | editar el GPT dos veces y comprobar el contador visible |
| Configuración válida | Verificado automáticamente | validación Rust de 80/500/12.000 caracteres y JSON con `schemaVersion=1` | nombres o instrucciones vacíos se rechazan; la descripción se normaliza | intentar guardar sin nombre o instrucciones |
| Seguridad inicial | Verificado automáticamente | `toolsEnabled=false` cuando toda la matriz está en `deny` y test `custom_gpt_instructions_are_explicit_context_without_granting_tools` | crear, editar o usar un GPT no activa el modo agente sin permiso | seleccionar un GPT nuevo y comprobar que los permisos del compositor siguen apagados |
| Matriz versionada | Verificado automáticamente | filas `gpt_tool_permissions`, `CustomGptToolPermissions` y test `custom_gpt_permission_matrix_gates_rename_tool_without_skipping_confirmation` | cada versión conserva `deny` o `confirm` para código y renombrado | editar un GPT, habilitar una capacidad y comprobar el contador de versión |
| Denegación predeterminada | Verificado automáticamente | `COALESCE(..., 'deny')` para versiones antiguas y permisos vacíos en importación | una versión sin matriz y todo GPT importado mantienen ambas capacidades bloqueadas | importar un GPT y revisar sus dos indicadores |
| Doble comprobación | Verificado automáticamente | filtrado al construir la petición y test `frozen_custom_gpt_permission_is_rechecked_before_tool_execution` | una acción inesperada del Broker no evita la matriz congelada | intentar renombrar con el permiso denegado y comprobar que Herramientas está desactivado |
| Confirmación persistente | Verificado automáticamente | Código aislado sigue siendo de un turno y `rename_conversation` continúa por `waiting_for_tools` | `confirm` nunca equivale a ejecución automática | habilitar Renombrar, pedir un título y aprobar o rechazar la propuesta |
| Selección por conversación | Verificado automáticamente | comando `set_conversation_custom_gpt`, selector de la barra superior y test `conversation_custom_gpt_selection_and_task_version_are_durable` | elegir o quitar un GPT no afecta a otros chats | elegir un GPT, cambiar de chat y volver |
| Versión congelada por tarea | Verificado automáticamente | `broker_tasks.gpt_version_id`, copia `custom_gpt_context_json` del flujo semántico y fuente `custom_gpt` | editar el GPT después de enviar no cambia el contexto histórico | enviar, editar el GPT y abrir **Ver contexto utilizado** |
| Instrucciones explícitas al Broker | Verificado automáticamente | bloque `<custom_gpt_instructions_json>` y metadatos de versión en la petición | el Broker recibe las instrucciones exactas solo al enviar un chat que usa ese GPT | comparar una respuesta con y sin GPT |
| Iniciadores de conversación | Verificado automáticamente | configuración versionada y test `custom_gpt_starters_and_portable_json_round_trip_safely` | admite hasta seis, elimina duplicados y los muestra solo en chats vacíos que usan el GPT | añadir dos iniciadores, seleccionar el GPT en un chat nuevo y pulsar uno |
| Exportación portable básica | Verificado automáticamente | `export_custom_gpt_portable(..., false)` y `schemaVersion=1` | **Exportar** genera `.chatygpt.json` solo con configuración, sin conocimiento, IDs, permisos, herramientas ni archivos | pulsar **Exportar** en la ficha de un GPT y revisar el JSON |
| Exportación explícita con conocimiento | Verificado automáticamente | test `custom_gpt_portable_knowledge_is_explicit_filtered_and_quarantined` y `schemaVersion=2` | **Exportar con conocimiento** incluye solo texto activo y no sensible; informa cuántos datos sensibles, desactivados y archivos excluyó | crear datos de las tres clases, añadir un archivo, exportar y comprobar el resumen visible |
| Importación segura y en cuarentena | Verificado automáticamente | `deny_unknown_fields`, límite de 256 KB, compatibilidad con versiones 1/2 y test portable | crea un GPT local nuevo con permisos denegados; todo conocimiento recibido queda desactivado para revisión y nunca importa archivos | importar el paquete enriquecido, abrir **Conocimiento** y comprobar que cada dato ofrece **Usar** |
| Conocimiento privado por GPT | Verificado automáticamente | comandos `get/create/set/delete/reindex_custom_gpt_knowledge_item` y test `custom_gpt_knowledge_is_private_and_independent_from_global_memory` | cada dato pertenece a un único GPT, no aparece en Memoria y sigue disponible aunque la memoria general esté desactivada | **Inicio → Mis GPTs → Conocimiento**, añadir datos distintos a dos GPTs y compararlos |
| Recuperación semántica aislada | Verificado automáticamente | `semantic_memory_matches` limita candidatos al GPT seleccionado y conserva el filtro global/proyecto | **Buscar recuerdos** puede recuperar conocimiento del GPT sin mostrar datos de otro asistente | seleccionar un GPT, activar **Buscar recuerdos**, preguntar por uno de sus datos y revisar el contexto |
| Trazabilidad del conocimiento | Verificado por test, compilación y snapshot durable | `MemoryItemView.customGptId/customGptName` y etiqueta `Conocimiento GPT · …` | la respuesta identifica el GPT propietario en **Ver contexto utilizado** | enviar con un GPT que tenga conocimiento y abrir el inspector de contexto |
| Sensibilidad y control | Verificado por la ruta común de memoria | índice local, clasificación `local_only`, controles **Usar/No usar**, **Preparar índice** y **Eliminar** | un dato sensible no habilita proveedores cloud y cada elemento puede excluirse o borrarse | marcar un dato como sensible, enviarlo y probar los tres controles |
| Archivos privados por GPT | Verificado automáticamente | migración `0013_custom_gpt_files.sql`, comandos `list/import/remove_custom_gpt_file` y test `custom_gpt_files_follow_the_selected_gpt_without_sticky_chat_links` | admite hasta 20 archivos, reutiliza ingesta, fragmentación e índice semántico y solo resuelve los preparados | **Inicio → Mis GPTs → Conocimiento → Añadir archivos**, esperar a **Preparado** y preguntar por su contenido |
| Ámbito documental dinámico | Verificado automáticamente | `ready_custom_gpt_file_ids_for_conversation` y autorización común de adjuntos | cambiar de GPT o retirar el archivo deja de incluirlo en el siguiente turno sin borrar fuentes históricas ni crear enlaces en `conversation_attachments` | preguntar con GPT A, cambiar a GPT B y repetir; después volver a A, retirar el archivo y repetir |
| Evidencia documental del GPT | Verificado por snapshot durable | razón `Archivo de conocimiento del GPT personal seleccionado` en `context_sources` | **Ver contexto utilizado** distingue el archivo del GPT de un adjunto elegido para ese turno | enviar una pregunta respondida desde el archivo y abrir el inspector |
| GPT personal dentro de flujos | Verificado automáticamente | tests `workflow_publication_freezes_gpt_version_and_creates_durable_node_runs`, `a_published_custom_gpt_profile_reaches_the_broker_request` y prueba de `WorkflowStudio` | la versión publicada conserva instrucciones, perfil, conocimiento y archivos autorizados; la ejecución revalida retiradas y fuerza ámbito local ante datos sensibles | **Flujos** → añadir **GPT personal** → seleccionarlo → comprobar **Contexto propio al publicar** → publicar y ejecutar |
| Prueba real desde la ficha | Verificado automáticamente | prueba de interfaz `prueba un GPT en un chat real que queda guardado` | crea un chat durable, aplica el proyecto predeterminado, selecciona el GPT y envía la pregunta sin herramientas ni adjuntos implícitos; el chat conserva la respuesta o el fallo | **Inicio → GPTs → Probar** → sustituir o usar la pregunta sugerida → **Crear chat y probar**; localizar después `Prueba · nombre` en **Recientes** |
| Auditoría sin contenido | Verificado automáticamente | eventos `custom_gpt.created` y `custom_gpt.version_created` con ID y versión | las instrucciones no se copian al evento visible | revisar **Actividad reciente** |
| Activación de la fase | Verificado automáticamente | migraciones `0011_custom_gpts.sql`, `0012_conversation_custom_gpts.sql` y `0013_custom_gpt_files.sql`, esquema 13 | catálogo, selección y archivos privados quedan activos tras actualizar | comprobar **Datos locales · esquema 13** |

## Verificación automática

```powershell
cargo fmt --check
cargo test
cargo clippy --all-targets -- -D warnings
pnpm test
pnpm build
python -m unittest discover -s tests -v
```

## Siguiente fase

Iniciar la Fase 4 con Deep Research como workflow durable: estado y progreso
persistentes, fuentes trazables, recuperación tras reinicio y cancelación segura.
