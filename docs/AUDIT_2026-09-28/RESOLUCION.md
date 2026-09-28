# Resolución de la auditoría del 28 de septiembre de 2026

**Versión resultante:** 0.3.0 · **Esquema SQLite:** 25 (migración `0025_audit_2026_09_28.sql`, solo aditiva).
**Base auditada:** `d0473dd` (0.2.0, esquema 24).

## 1. ¿Era correcta la auditoría?

Sí. Se comprobaron los 26 hallazgos contra el código antes de tocar nada:

- Las **11 evidencias** de `evidence/` se reprodujeron tal cual (6 de interfaz y 5 de SQL).
- Los hallazgos de análisis estático (E) se confirmaron leyendo el código señalado; en
  todos la descripción era exacta. Dos matices:
  - **H18**: la descarga de artefactos del Broker y del texto convertido ya estaba
    acotada (`read_body_limited`); lo que no lo estaba era el JSON general del Broker, la
    subida de ficheros y todo el cliente de Athena. Se corrigió lo que faltaba.
  - **H20**: además de lo descrito, el diagnóstico de la propia aplicación **nunca
    comprobaba la credencial**: con un token caducado decía «Broker AI está listo» y el
    fallo aparecía al enviar el primer mensaje.

Ninguna afirmación del informe resultó falsa. Tras las correcciones, las evidencias
originales de interfaz **fallan** —describían el defecto— y sus equivalentes con la
expectativa correcta viven en la suite normal (ver §4).

## 2. Defectos adicionales encontrados durante la revisión

| # | Defecto | Corrección | Regresión |
|---|---|---|---|
| A1 | `task_snapshot` leía el resultado de la columna del **error** (`row.get(6)` en vez de 5): la interfaz recibía el error como resultado | Índice corregido | `tests_auditoria` (H07–H09 usan la instantánea) |
| A2 | El diagnóstico del Broker no comprobaba la credencial | `diagnose()` consulta `/api/v1/auth/check`; aviso visible y banner «Ir a la credencial» | `diagnosis_reports_a_rejected_credential_instead_of_ready` |
| A3 | `mark_orphaned` podía sobrescribir una tarea ya terminal; un resumen o una investigación de una tarea huérfana quedaban «en curso» para siempre | Guarda de monotonía; cierre de resúmenes e investigaciones dependientes | `an_unmaterializable_result_closes_the_task_with_a_visible_reason` |
| A4 | Exportar a una carpeta que un GPT podía leer **le quitaba la lectura**; reautorizar una carpeta revocada **resucitaba** permisos retirados; una carpeta de solo lectura admitía escrituras | Una única regla de concesión que combina lo vigente y empieza de cero tras revocar; escribir exige permiso de escritura | `folder_permissions_combine_while_active_and_never_resurrect_after_revocation` |
| A5 | `rustls 0.23.42` con aviso RUSTSEC-2026-0285 (media) | Actualizado a 0.23.45 (parche) | `cargo audit` sin vulnerabilidades |
| A6 | El perfil real tenía una credencial caducada del Broker (el Broker rota su token) | No es código: el lanzador la renovó al validarla con el token vigente | — |

## 3. Resolución por hallazgo

| Hallazgo | Estado | Qué cambió | Dónde se prueba |
|---|---|---|---|
| **H01** política mostrada ≠ aplicada | Corregido | `politica.rs`: regla única (privacidad = la más estricta, gasto = el menor, enrutado del GPT). La usan la petición, la vista previa (`get_effective_execution_policy`) y los flujos. El chat muestra lo que se aplicará y por qué; los controles que fija el GPT se bloquean con su motivo | `politica::tests` (6), `custom_gpt_execution_profile_overrides_chat_preferences_safely` |
| **H02** clasificación perdida entre nodos | Corregido | Cada salida guarda su clasificación (`workflow_node_runs.data_classification`); cada nodo aplica la más estricta de sus entradas, también en aprobaciones y tras reiniciar | `a_sensitive_output_keeps_every_later_node_local_only_across_a_restart` |
| **H03** DNS no fijado | Corregido | Resolución única por salto, conexión clavada con `resolve_to_addrs`, sin proxies (`no_proxy`); rangos especiales (CGNAT, documentación, NAT64, 6to4, multidifusión…) rechazados | `dns_fijado` (3 pruebas con servidor real y resolvedor controlado) |
| **H04** HTTP por LAN / «Solo en este equipo» | Corregido (con límite) | La opción se llama «Solo modelos locales · máxima restricción»; el chat y Ajustes muestran el destino real y avisan de conexión sin cifrar. **No se fuerza HTTPS**: el Broker actual solo sirve HTTP y forzarlo dejaría la app inservible; el aviso lo hace visible | `an_unencrypted_lan_broker_is_named_and_warned_about` |
| **H05** duplicado tras corte del scheduler | Corregido | Identidad estable por ejecución (`chatygpt:scheduled-run:<run>`) para chats y flujos; la ejecución congela instrucción y destino (`scheduled_runs.payload_json`) | `a_scheduled_run_recovered_after_a_crash_does_not_duplicate_its_turn`, `a_recovered_claim_runs_the_instruction_that_was_claimed` |
| **H06** solapamiento de ejecuciones | Corregido | Una fecha recurrente que vence con otra ejecución activa se **omite** y queda explicada en el historial; una única **espera** | `a_recurring_date_that_falls_during_an_active_run_is_skipped_and_explained`, `a_one_off_date_waits_for_the_active_run_instead_of_overlapping` |
| **H07** estado tardío reabre tarea | Corregido | Transiciones monotónicas: lo terminal no se reabre y una huérfana solo acepta su desenlace (guarda también dentro del `UPDATE`) | `a_late_poll_never_reopens_a_cancelled_task` |
| **H08** cancelar sin ID remoto | Corregido | `cancel_requested_at` durable; sin transmitir se cierra localmente; si pudo transmitirse se reconcilia con la misma clave y se cancela; si el Broker no responde, el sondeo lo repite | 3 pruebas `a_question_cancelled_…`, `a_possibly_transmitted_…`, `a_cancellation_the_broker_could_not_confirm_…` |
| **H09** fallo al materializar | Corregido | Bloqueos de SQLite se reintentan; un resultado no materializable cierra la tarea con su motivo visible | `an_unmaterializable_result_closes_the_task_with_a_visible_reason` |
| **H10** borrador compartido | Corregido | `borradores.ts`: uno por conversación, persistido en el equipo (30 días, 200 como máximo, se borra al enviar) | `App.auditoria.test.tsx`, `borradores.test.ts` |
| **H11** respuestas tardías cambian la vista | Corregido | Cada carga lleva identidad; solo se pinta si sigue siendo la conversación elegida. Lo mismo en adjuntos, política y búsqueda | `App.auditoria.test.tsx` (2 casos) |
| **H12** error de refresco = error de envío | Corregido | Aceptar y refrescar están separados; «El mensaje se envió… Actualizar la vista» relee sin reenviar; identificador de operación del cliente ligado al texto exacto | `App.auditoria.test.tsx` (2 casos), `repeating_a_send_with_the_same_operation_returns_the_first_task` |
| **H13** editor de flujos pierde cambios | Corregido | Autoguardado local por flujo; al volver se recupera con aviso y opción de descartar; guardar lo retira, un fallo lo conserva | `WorkflowStudio.auditoria.test.tsx` (3 casos) |
| **H14** archivar sin salida | Corregido | Área **Archivo**: archivadas, proyectos archivados (vuelven con sus conversaciones) y papelera; «Borrar para siempre» solo en la papelera y con confirmación; búsqueda con «Incluir archivadas» | `conversaciones_auditoria` (4), `Recuperacion.test.tsx` (2) |
| **H15** eliminar programación borra historial | Corregido | «Retirar» conserva historial; «Borrar historial» es otra decisión solo para retiradas | `retiring_a_schedule_keeps_its_history_and_purging_is_a_separate_decision` |
| **H16** sin copia/restauración | Corregido | `respaldo.rs`: `VACUUM INTO` + adjuntos + manifiesto con SHA-256; verificación abriendo la copia en un directorio temporal; restauración al reiniciar que aparta (no borra) lo anterior y revoca carpetas autorizadas | `respaldo::tests` (2), `Recuperacion.test.tsx` (3) |
| **H17** paginación solo visual | Corregido | Rust devuelve el último lote (80) y páginas anteriores por cursor (`get_conversation_messages_before`) | `a_long_conversation_is_paged_in_rust_not_only_in_the_screen`, prueba de interfaz del chat largo |
| **H18** respuestas sin límite | Corregido | JSON del Broker 32 MB, Athena 16 MB, errores 64 KB, contados por trozos aunque no haya `Content-Length` | `an_endless_response_is_cut_at_the_local_budget`, `un_resultado_sin_fin_se_corta_en_el_presupuesto_local` |
| **H19** indexación sin límite | Corregido | Cola por base de datos con 4 fragmentos en curso; el lote se persiste en una transacción; el chat no espera en la cola | `document_indexing_keeps_a_bounded_number_of_tasks_in_flight` |
| **H20** arranques incoherentes | Corregido | Lanzador, inicio con Windows y app usan `/api/v1/auth/check`; sin Broker la app se abre sin conexión; la espera del inicio con Windows está acotada | `startup_script_…`, `test_the_launcher_opens_the_app_without_a_broker`, ejecución real del lanzador |
| **H21** garantías de capacidades | Corregido | Política documentada: se envía por compatibilidad y el turno lleva `unverified_capabilities`, que la interfaz muestra | `a_turn_sent_without_verified_capabilities_says_so` |
| **H22** búsqueda sin acentos y obsoleta | Corregido | Función SQL `chatygpt_fold` (minúsculas y sin diacríticos, carácter a carácter); fragmento de coincidencia; ir al mensaje encontrado; número de búsqueda para descartar respuestas viejas | `search_ignores_case_and_spanish_accents_and_shows_where_it_matched`, `App.auditoria.test.tsx` |
| **H23** guardar archivo generado | Corregido | Copias locales registradas (`task_artifact_copies`); «Conservar en el equipo», «Guardar como…», «Mostrar en el Explorador»; siguen visibles aunque el Broker pode el remoto; sin `alert` | `MessageArtifacts.test.tsx` (3), `a_kept_artifact_copy_stays_associated_with_its_task` |
| **H24** Enter durante IME | Corregido | `isImeComposition` (`isComposing` o `keyCode` 229) | `App.auditoria.test.tsx` |
| **H25** textos y guías desfasados | Corregido | Título por área (ya no «Fase 1 · Núcleo»), número real de métricas, guías con los nombres de área actuales, README y compatibilidad al día | revisión manual |
| **H26** calidad y release | Corregido en lo verificable | Áreas pesadas en carga diferida (paquete 510 → 471 kB, sin aviso de Vite); CI con Rust 1.97.1 y Node 24.11.1 fijos, `pnpm audit` y `cargo audit`; umbrales de cobertura subidos | CI, `pnpm build` |

## 4. Verificación

| Comprobación | Resultado |
|---|---|
| `cargo test --lib` | **373 aprobadas** (antes 338; 35 nuevas) |
| `cargo fmt --check` · `cargo clippy --all-targets -D warnings` | Correctos |
| `cargo audit` | 0 vulnerabilidades (8 avisos de crates sin mantenimiento, transitivos de Tauri) |
| `pnpm audit --prod --audit-level high` | Sin vulnerabilidades conocidas |
| Vitest | **354 aprobadas**, 34 ficheros (antes 319) |
| Cobertura frontend | Líneas 58,25 % · ramas 73,99 % · funciones 45,40 % (antes 56,20 / 72,61 / 41,53); umbrales subidos a 57 / 72 / 44 |
| `tsc -b` · `vite build` | Correctos; paquete principal 471 kB (antes 510 kB, sin aviso) |
| Python `unittest` | **44 aprobadas** (contratos de órdenes y confirmaciones incluidos) |
| `tauri build --no-bundle` | `chatygpt.exe` 0.3.0; huella registrada para el lanzador |

**Evidencias originales** (`evidence/`): describían el defecto, así que ahora deben fallar.
Las 6 de interfaz fallan todas. De las 5 de SQL fallan 3 (reapertura, cascada y
búsqueda); las otras 2 comprueban una consulta aislada cuya corrección está en la lógica
que la rodea —la fecha vencida se **omite** después de seleccionarla (H06) y la
recuperación **reutiliza** la tarea por su identidad (H05)—, cubiertas por las regresiones
de la tabla anterior.

## 5. Uso real de la aplicación

Se ejecutó el `chatygpt.exe` 0.3.0 sobre una **copia** del perfil real (33
conversaciones, esquema 23: la prueba recorrió también las migraciones 24 y 25 con datos
reales) contra el Broker real en `192.168.1.52:8765` (contrato 2.10). La interfaz se manejó
con ratón y teclado reales a través de WebView2 (CDP), con capturas de pantalla. Para no
tocar los datos del usuario se añadió `CHATYGPT_DATA_DIR`, que abre la aplicación sobre
otro perfil.

Comprobado como usuario:

- Ajustes → Comprobar conexión: «Credencial: aceptada por el Broker», destino y aviso de
  conexión sin cifrar (H04, H20).
- Borrador escrito en una conversación, cambio a otra (vacía) y vuelta (restaurado) (H10).
- Envío real: progreso en 0,4 s, respuesta en 17–26 s; navegar a otra conversación
  durante el turno ya no devuelve a la de origen (H11).
- Cancelación real a 1–1,5 s: el turno se cierra y no aparece respuesta después (H08).
- Búsqueda de «arbol», «ÁRBOL», «pena», «genealogico» encuentra «Árbol genealógico de la
  PEÑA»; los resultados en mensajes muestran su fragmento (H22).
- Archivar → área Archivo → Restaurar; la papelera muestra una conversación eliminada en
  julio que hasta ahora no se podía recuperar (H14).
- Editor de flujos: cambio sin guardar, ir a Chats y volver → recuperado con aviso;
  «Descartar cambios» vuelve a lo guardado (H13).
- Copia de seguridad del perfil (280 MB, 17 adjuntos) creada y verificada en 3,8 s (H16).
- Programación diaria creada, «Ejecutar ahora» completada con el Broker real, «Retirar»
  conserva el historial y solo ofrece «Borrar historial» (H15).
- GPT con perfil «confidencial, 1 USD» en un chat «uso personal, 0,10 USD»: se aplica
  Confidencial y 0,10 USD, con las tres notas visibles antes de enviar y la forma de
  responder bloqueada (H01).
- Flujo real «GPT confidencial → instrucción genérica → resultado»: la instrucción
  genérica salió como `confidential` y el lienzo marca «Solo modelos locales» (H02).
- Un artefacto real (`final.md`) conservado en el equipo, verificado por SHA-256 y
  asociado a su tarea (H23).
- El área Athena se carga bajo demanda y explica que el servicio no está disponible.

**Problemas de usabilidad que solo aparecieron usando la app, ya corregidos:**

| Problema | Corrección |
|---|---|
| Una respuesta cancelada por la persona se mostraba como «La tarea no pudo completarse · El Broker no proporcionó más detalles» | Estado propio «Respuesta cancelada», sin culpar al Broker |
| Las notas de la política efectiva solo se veían al desplegar «Opciones de ejecución» | Se muestran siempre que el chat no decide algo |
| Las superficies nuevas usaban fondos claros en tema oscuro | Reglas oscuras explícitas, cubiertas por `stylesContrast.test.js` |
| En un chat de proyecto con varios archivos, «Archivos del proyecto» dejaba la conversación en 72 px | Lista plegable con su recuento; altura máxima del compositor |
| A 980×680: barra horizontal (`body` exigía 980 px), panel de contexto ocupando un tercio y «Enviar» fuera de la vista | `min-width` 900 px; el panel se cierra por debajo de 1200 px; pie del compositor fijo y compacto |
| Por debajo de 1100 px se ocultaba el botón «Contexto»: cerrado el panel, no había forma de reabrirlo | El botón queda siempre visible |
| El formulario del GPT prometía «Siempre en este equipo» y que su perfil sustituía al del chat | Textos alineados con la regla real |
| La prioridad del GPT (50) aparecía como «normal» | Etiqueta válida para ambas escalas |

Medido tras las correcciones: a 980×680 no hay desplazamiento horizontal, el cuadro de
texto y «Enviar» están visibles y «Contexto» reabre el panel; a 1280×820, lista de
mensajes 216 px y compositor 469 px con el pie visible.

## 6. Límites que siguen abiertos

- **HTTPS con el Broker (H04)**: la aplicación lo avisa y lo muestra, pero no puede
  exigirlo mientras el Broker solo sirva HTTP. Requiere configurar TLS en el Broker.
- **IME real (H24)**: probado con eventos DOM; la guarda cubre `isComposing` y
  `keyCode 229`. Falta una prueba con un IME japonés o chino real en WebView2.
- **Lector de pantalla, escalado 150 %/zoom 200 % y aprobación visual** (§5 del informe):
  no se certificaron en esta sesión.
- **Diálogos nativos** (carpeta de copias, «Guardar como…»): se probó la orden de Rust por
  el puente real; el diálogo de Windows en sí no se pulsó automáticamente.
- **Instalación MSI/NSIS y CI remota**: la CI se actualizó, pero no se ha ejecutado en
  GitHub desde aquí.
- **Búsqueda y «ñ»**: al ignorar diacríticos, «pena» encuentra también «peña». Es la misma
  regla que ya usaban los demás buscadores de la aplicación.
