# Resolución de la auditoría QA del 14 de octubre de 2026

**Informe auditado:** `D:\Desarrollo\docs\Pruebas\Auditoria_ChatyGPT_2026-10-14` (15 incidencias,
observaciones menores y 16 mejoras). **Versión resultante:** 0.4.1 · **Esquema SQLite:** 26
(sin migración nueva). **Base:** `68a63de` (0.4.0).

## 1. ¿Era correcta la auditoría?

Sí, y en dos puntos se quedaba corta:

- **INC-01** tenía una segunda causa que el informe no vio: `target-next\release\chatygpt.exe`
  era el mismo binario 0.3.0 del 3-oct, y `Arrancar ChatyGPT.bat` lo **copiaba** encima del
  release en cada arranque sin retirarlo. Aunque se hubiera recompilado, el siguiente
  arranque habría vuelto a instalar la 0.3.0.
- **INC-11** citaba 3 puntos de contraste. Un medidor sobre la app real (todas las áreas,
  diálogos, conversación, historial de GPT, automatización con ejecuciones y calendario, en
  los dos temas) encontró **unos 60 textos** por debajo de 4,5:1, el peor a 1,04:1.
- **INC-05**: la ficha del GPT no tenía tres defectos sueltos sino uno: la columna de
  botones (`auto`) se quedaba con todo el ancho y el contenido de la ficha entera —chips,
  descripción e historial— caía a ~70 px.

La hipótesis de INC-14 era correcta y se confirmó contra el Broker real (ver abajo).

## 2. Resolución por incidencia

| Inc. | Causa raíz | Corrección | Prueba |
|---|---|---|---|
| **INC-14** (alta) | El Broker 2.11 publica un fallo terminal como `progress: {"phase": "failed"}` **sin contadores**; `TaskState::from_contract_value` los exigía siempre, rechazaba el desenlace como «contrato inesperado» y el sondeo seguía para siempre. El simulador devolvía `failed` *con* contadores, por eso ninguna prueba lo vio. | `broker/contracts.rs`: los contadores solo se exigen en estados no terminales. `task_runtime/herramientas.rs`: tres lecturas seguidas que no cumplen el contrato cierran la tarea (`orphaned`) con el motivo visible, en vez de un bucle silencioso. El simulador devuelve ahora la forma literal del Broker. | `runtime_contract_accepts_terminal_failure_without_progress_counters` (respuesta literal capturada al Broker), `tests_auditoria_1014.rs` (cancelar un turno que el Broker da por fallido; error de contrato persistente). Ambas fallan sin la corrección. **En vivo**: el turno zombi del perfil del auditor (`task_c6a938d…`, 312 errores de sondeo) se resolvió al abrirlo con 0.4.1: «La tarea no pudo completarse» con el motivo real. |
| **INC-01** (alta) | La huella de fuentes se registró sin que el exe se hubiera reconstruido; además el exe «preparado» de `target-next` se reinstalaba en cada arranque. | `Needs-ChatyGPTBuild.ps1`: un exe más antiguo que la fuente más reciente siempre pide compilar, y `-Record` se niega a registrar sobre él. El `.bat` **mueve** (no copia) la actualización preparada y borra la huella antes de compilar. | `test_build_detector_never_accepts_an_executable_older_than_its_sources` ejecuta el script real (falla con el script anterior). Detector sobre el estado real del 18-oct: devolvía «1» y `-Record` rehusó. |
| **INC-15** | Nada impedía dos procesos sobre el mismo perfil. | `instancia.rs`: candado exclusivo `chatygpt.lock` en el directorio de datos (`File::try_lock`, sin dependencias), tomado antes de abrir la base o reanudar nada. La segunda instancia muestra un aviso nativo y termina. Otro `CHATYGPT_DATA_DIR` sigue pudiendo abrirse. | Prueba unitaria del candado. **En vivo**: la segunda instancia abrió el aviso, no abrió interfaz y terminó al cerrarlo; log `app.instance_already_running`. |
| **INC-02** | Sin token, `/auth/check` responde 401 y se rotulaba «rechaza la credencial guardada», con una explicación de rotación que no se cumple. | `diagnose()` distingue `missing`; el banner tiene texto para «falta» y «rechazada» sin la excusa de la rotación. | `diagnosis_reports_a_rejected_credential_instead_of_ready` ampliada; `App.auditoria1014.test.tsx`. **En vivo** con perfil vacío: «Falta la credencial de Broker AI…». |
| **INC-03** | Guardar no consultaba al Broker. | Tras guardar se diagnostica y el aviso dice si el Broker la acepta, la rechaza o no responde (`credentialSavedNotice`). | Prueba de dominio. **En vivo** contra el Broker real: token inventado → «la rechaza»; token real → «la acepta» y desaparece el banner. |
| **INC-04** | La barra lateral se desplazaba con la página. | `.sidebar` fija (`sticky`, `100vh`, desplazamiento propio). | Prueba de CSS. **En vivo**: Ajustes desplazado al final y la navegación sigue visible. |
| **INC-05** | Ver §1. El botón «Autorizar carpeta para modificar» estaba además fuera de sitio: en la cabecera «Proyectos» de la barra lateral. | Ficha del GPT en una columna (botones debajo). El botón pasa a Ajustes → Carpetas autorizadas, junto a su gemelo «…para lectura», que es donde se pintan sus errores. | Prueba de CSS. **En vivo**: medidor sin cajas aplastadas; captura del historial de versiones a ancho completo. |
| **INC-06** | Rótulos históricos de fase y una única frase de cabecera para todas las áreas. | Fuera «Fase 2/3»; cada área tiene su frase. | Revisión en vivo. |
| **INC-07** | 24 `window.confirm` nativos repartidos por hooks y paneles. | `confirmacion.tsx`: `confirmAction()` devuelve una promesa y un único anfitrión pinta el modal propio (foco en «Cancelar», Escape cancela, foco atrapado). «Borrar para siempre» y «Restaurar versión» con su verbo y marca destructiva. | `App.confirmations.test.tsx` reescritas sobre el modal y afirmando que no se abre el nativo. **En vivo**: «Ejecutar ahora» pidió confirmación con el modal y ejecutó de verdad. |
| **INC-08** | Perfiles, modelos e historial solo consultaban al montarse. | «Volver a comprobar» y «Guardar y conectar» los remontan (`key`) y limpian el aviso. | `AthenaArea.reconexion.test.tsx` (falla sin la corrección). **En vivo** con Athena aislado: caído → errores; arrancado → «Volver a comprobar» → «Athena está disponible» y selector de perfiles, sin salir del área. |
| **INC-09** | Se mostraba el texto de `reqwest` (inglés, URL completa). | `error::describe_network_failure`, usado por Broker y Athena: destino y qué comprobar, en español. | Prueba con conexión rechazada real. **En vivo**: «no se pudo conectar con 127.0.0.1:8770; comprueba que el servicio está arrancado…». |
| **INC-10** | La propuesta se calculaba al arrancar y el planificador no se desmonta al cambiar de área. | `freshScheduledLocalTime` renueva una propuesta vencida al entrar en Automatizaciones; una fecha futura elegida se respeta. | Prueba unitaria y `App.auditoria1014.test.tsx` (falla sin el efecto). La primera versión vivía en el montaje del panel y la prueba en vivo demostró que no corría: se movió a `App`. |
| **INC-11** | Superficies que conservaban fondo claro con texto claro en el tema oscuro (o al revés) y una familia de grises secundarios demasiado claros en el claro. | Reglas oscuras para tarjeta de actividad, modal, historial de automatizaciones, calendario, chips de GPT, avisos de seguridad, flujos; grises secundarios oscurecidos; rótulos de la barra lateral aclarados. | Medidor de contraste en vivo: **0 textos por debajo de 4,5:1** en las 8 áreas y en las vistas interiores, en los dos temas. Pruebas de CSS que fijan las reglas. |
| **INC-12** | El límite de 120 caracteres existía (Rust y `maxLength`), pero al pegar se recortaba sin avisar. | Contador «n / 120 caracteres» bajo el campo, fuera de la etiqueta. | Suite de interfaz. |
| **INC-13** | La ruta canónica (con `\\?\`) se usa también como clave de deduplicación en la base, así que no se cambia en Rust. | Se normaliza solo al mostrarla; el aviso se retira a los 15 s y al cambiar de área. | Prueba de `withoutExtendedPathPrefix`. |
| Menores | Estado `generating` en inglés en el banner de recuperación; calendario «Jueves, 15 De Octubre»; barra horizontal en RECIENTES. | `taskStatusLabel`; solo la inicial en mayúscula; `overflow-x: hidden` en la barra lateral. | Prueba de la etiqueta del calendario. **En vivo**: ningún elemento de la barra lateral con desplazamiento horizontal. |

## 3. Verificación

- `cargo fmt`, `cargo clippy --all-targets -- -D warnings`, `cargo test`: **398** pruebas.
- `pnpm typecheck`, `pnpm test:coverage`: **372** pruebas, umbral de cobertura cumplido.
- `python -m unittest discover -s tests`: **45** pruebas. `git diff --check` limpio.
- Prueba de uso real con el ejecutable 0.4.1 sobre **copias** del perfil del usuario y del
  perfil del auditor (`CHATYGPT_DATA_DIR`), manejado por CDP con clics y teclado; Broker real
  `192.168.1.52:8765` y Athena aislado en `127.0.0.1:8770`.

## 4. Lo que no se ha tocado

- Los escenarios PEND/BLOQ de la matriz (captura de pantalla, webcam, restauración completa,
  Obsidian, flujo publicado) no eran incidencias y no se han vuelto a auditar.
