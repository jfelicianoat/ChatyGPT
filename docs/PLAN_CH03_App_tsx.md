# CH-03 · Partir `App.tsx` (plan de cambios del 13-nov-2026)

## Hecho el 14-nov-2026

`App.tsx` ha pasado de 5.193 a 4.697 líneas. Se han extraído nueve tarjetas de presentación a `src/paneles/`, copiando el JSX tal cual y recibiendo el estado y los manejadores como props:

- `TarjetaProyectos`
- `TarjetaEstadoYBroker`
- `TarjetaApariencia`
- `TarjetaImagenesDocumentos`
- `TarjetaRendimiento`
- `TarjetaCredencialBroker`
- `TarjetaCarpetasAutorizadas`
- `TarjetaPruebaInferencia`
- `TarjetaActividad`

Pruebas: `paneles/TarjetasAjustes.test.tsx` (8 casos). Verificado: `pnpm typecheck`, `pnpm test:coverage` (384 tests, umbral cumplido), `pnpm build`, los tests de Python (contrato de interfaz) y el exe 0.4.19 regenerado.

### Corte 1 · Adjuntos (`adjuntos/`)

`App.tsx` baja a ~3.920 líneas. Nuevo `adjuntos/useAdjuntos.ts`: estado de ingesta (`attachments`, `projectFiles`, `draftAttachmentIds`, reintentos de contexto e índice semántico), captura de pantalla con recorte, cámara, y los cinco efectos que eran solo de adjuntos (limpieza al desmontar, vídeo de la cámara, cámara al cambiar de conversación, arrastrar-soltar y sondeo de ingesta). La zona del compositor (cámara, vista previa de captura, fila de botones, biblioteca del proyecto y chips) viaja a `adjuntos/TarjetaAdjuntos.tsx` tal cual, como fragmento: el DOM no cambia.

Pruebas: `adjuntos/TarjetaAdjuntos.test.tsx` (6 casos). Verificado: `pnpm typecheck`, `pnpm test:coverage` (390 tests, umbral cumplido), `pnpm build`, tests de Python (45, vía `python -m unittest discover -s tests`, como la CI; hizo falta `pip install -r tests/requirements.txt` para `jsonschema`).

### Corte 2 · Compositor (`compositor/`)

`App.tsx` baja a ~3.680 líneas. Nuevo `compositor/useCompositor.ts`: el borrador por conversación con su `useLayoutEffect` (H10: nunca se ve el borrador de otra conversación), la política efectiva de ejecución con su señal de refresco (H01/H04: privacidad, coste y destino calculados en Rust) y el teclado del compositor (H24: Enter no envía con IME en curso). El desplegable «Opciones de ejecución» (privacidad, estrategia, prioridad, límite de coste, profundidad y map-reduce) viaja a `compositor/TarjetaOpcionesEjecucion.tsx` tal cual, con las etiquetas de estrategia y clasificación; `routingFixedByGpt` se calcula ahora dentro de la tarjeta. Los atajos globales de teclado (Ctrl+N, Ctrl+F, Ctrl+Shift+M, Alt+1, `/` y `?`) siguen en `App.tsx`: despachan navegación de toda la aplicación, no del compositor.

`sendTurn` sigue en `App.tsx`: orquesta el turno (diagnóstico de sandbox, tarea activa, recarga), no solo la redacción; comparte el almacén de borradores vía `compositor.draftsRef`.

Pruebas: `compositor/TarjetaOpcionesEjecucion.test.tsx` (5 casos). Verificado: `pnpm typecheck`, `pnpm test:coverage` (395 tests, umbral cumplido), `pnpm build`, tests de Python (45).

### Corte 3 · Confirmaciones de herramientas (`confirmaciones/`)

Nuevo `confirmaciones/useConfirmaciones.ts`: las decisiones por llamada, el aviso de ocupado, el reset cuando cambia el turno (o sus llamadas pendientes) y `submitToolDecisions` (reanudar el turno, recargar navegación y conversación). La sección «ChatyGPT quiere realizar una acción» viaja a `confirmaciones/TarjetaConfirmacionHerramientas.tsx` tal cual; recibe el hook entero y el turno pausado.

Pruebas: `confirmaciones/TarjetaConfirmacionHerramientas.test.tsx` (5 casos). Verificado junto con el corte 2: `pnpm typecheck`, `pnpm test:coverage`, `pnpm build`, tests de Python.

### Corte 4 · Lista de mensajes (`lista/`)

`App.tsx` queda en ~3.060 líneas. Nuevo `lista/useListaMensajes.ts`: la ventana visible de mensajes, la paginación de Rust con anteponida sin salto (H17), los dos `useLayoutEffect` de scroll (seguir al último y reponer la posición tras anteponer) y el salto a un resultado de búsqueda (H22), incluido el reintento de páginas hasta encontrar el mensaje. Los refs que también escriben el envío y la apertura de conversaciones (`followConversationScrollRef`, `pendingMessageFocusRef`) se quedan en `App.tsx` y entran al hook como opciones. Todo el bloque `<div className="message-list">` (mensajes, progreso real, investigación profunda, avisos de consenso, metadatos, artefactos vía `MessageArtifacts`, contexto utilizado y fuentes) viaja a `lista/ListaMensajes.tsx` tal cual.

Pruebas: `lista/ListaMensajes.test.tsx` (5 casos). Verificado junto con el corte 5: `pnpm typecheck`, `pnpm test:coverage`, `pnpm build`, tests de Python.

### Corte 5 · Diálogos y barra lateral (`dialogos/`, `navegacion/`)

`App.tsx` queda en ~2.800 líneas (desde 5.193: −46 %). Nuevo `dialogos/useDialogos.ts`: el diálogo abierto, su valor, la ocupación, el ref que evita que Escape cierre mientras se envía, `openDialog` y el interruptor de `submitDialog` (crear/renombrar/instrucciones/archivar proyecto, renombrar/archivar/borrar conversación y probar un GPT en un chat real). El panel ya extraído (`paneles/Dialogo.tsx`) recibe el estado desde el hook con destructuring, sin tocar sus llamadas. La barra lateral entera (marca, nueva conversación, búsqueda, espacios, proyectos, conversaciones con resultados, acciones del proyecto y pie de estado) viaja a `navegacion/BarraLateral.tsx` como tarjeta presentacional: la búsqueda y la navegación no tienen estado propio nuevo, entran como props.

Pruebas: `dialogos/useDialogos.test.tsx` (5 casos, con `renderHook` y plataforma simulada) y `navegacion/BarraLateral.test.tsx` (4 casos).

Verificación del cierre (14-nov-2026): `pnpm typecheck`, `pnpm test:coverage` (414 tests, umbral cumplido), `pnpm build`, tests de Python (45) y el exe 0.4.19 regenerado con la huella de `scripts/Needs-ChatyGPTBuild.ps1` grabada (el empaquetado MSI falla en local por falta de un `.ico`, ajeno a estos cambios; el ejecutable de `target/release` sí se produce).

### Revisión del cierre (14-nov-2026)

Revisión independiente de los cinco cortes, en dos partes.

**Lógica.** Comparación mecánica de cada línea de los hooks y tarjetas nuevos contra el `App.tsx` de `HEAD`: lo único que no es copia literal son las firmas de props, el retorno de los hooks y los renombres de variables (`executionPolicy` → `policy`, `conversation.value` → `conversation`). Ningún hook queda detrás de un `return` temprano, y la tarjeta de confirmación se pinta con la misma condición que antes. El exe 0.4.19 lleva la interfaz actual (`index-CvSPIipA.js`).

**Uso real**, con el exe sobre una copia del perfil (`CHATYGPT_DATA_DIR`) manejada por CDP y con el Broker real:

- las ocho áreas de la barra lateral se abren, y Ajustes muestra las nueve tarjetas;
- el borrador es propio de cada conversación (H10) y el scroll arranca en el último mensaje;
- en un chat sembrado con 200 mensajes, «Mostrar 50 mensajes anteriores» antepone sin mover la lectura (H17: el ancla pasa de 131 a 130 px), y buscar una palabra del mensaje 5 carga las páginas que faltan y lo resalta (H22);
- funcionan renombrar, Escape, crear proyecto, instrucciones, archivar proyecto y conversación (aparece en Archivo) y «Probar» un GPT;
- funcionan Ctrl+N, opciones de ejecución y política efectiva, chips de adjuntos, «Ver contexto utilizado» y «Ver ficheros generados»;
- turnos reales: con modelo local responde «listo», y con código aislado calcula 123×457 = 56211. Un proveedor de nube agotó los 600 s del Broker; la app lo mostró y liberó el compositor.

- adjuntos en vivo: cámara (WebView2 con dispositivo simulado), captura de pantalla con recorte, selector de ficheros nativo y arrastrar y soltar. Un `.txt` adjunto llega al modelo: responde con la clave que contiene y cita el fichero como fuente.

- confirmación de herramientas en vivo, con un GPT con permiso «Renombrar: confirmar», modelo preferido `qwen3.8:27b` y privacidad «Confidencial». Rust solo ofrece la herramienta ante una petición explícita: «Renombra esta conversación como…» la activa; «usa la herramienta…» no. Aparece la tarjeta, «Confirmar» está deshabilitado hasta decidir, y «Autorizar una vez» + confirmar cierra la tarjeta. La confirmación queda `allowed_once` y el chat pasa a llamarse «Prueba CH03 confirmada»; el modelo cierra con «Listo, el título del chat ahora es…».

Además se añadió la prueba `CH-03: un turno en pausa…` en `App.confirmations.test.tsx`: monta `App` entera, autoriza y comprueba que `resolveToolCalls` recibe la decisión. Se verificó que muerde: falla al invertir la decisión.

**Dos fallos previos encontrados al probar**, ajenos a CH-03 (estaban igual en `HEAD`) y corregidos en **0.4.20**:

1. **Vista previa de captura y foto en blanco, y recorte imposible** («No se pudo leer la imagen para recortarla»). La CSP de `tauri.conf.json` no permitía `blob:` en `img-src`. Se añadió solo ahí; `src/csp.test.js` lo fija y comprueba que `blob:` no se cuela en `default-src`, `connect-src` ni `script-src`.
2. **Ventana «No responde» mientras un diálogo de fichero o carpeta está abierto.** Los 11 comandos `pick_*` eran síncronos, y Tauri 2 los ejecuta en el hilo principal: se congelaba la ventana y también el sondeo de un turno en curso. Ahora usan `#[tauri::command(async)]`. Se probaron en vivo los 11, invocados como lo hace la interfaz: con cada diálogo abierto, la ventana no está colgada y Rust responde en 6–38 ms; al cancelar, la orden devuelve `null` o `[]` sin error. Importar un fichero por el selector sigue funcionando.
3. **«Esperando tu confirmación» después de haber confirmado.** El Broker sigue anunciando `waiting_for_tools` hasta su siguiente informe, aunque la tarea ya esté `queued`. Ahora `taskProgressSummary` solo dice «Esperando tu confirmación» si queda alguna llamada pendiente; si no, muestra el estado real de la tarea. Hay prueba nueva en `domain.test.ts`, verificada: falla sin la corrección. `tests/test_frontend_contract.py` reconoce ahora el atributo con argumentos (sin ese cambio, la prueba marcaba los 11 comandos como «sin declarar»).

Cierre: `cargo fmt`, 426 pruebas de Rust, `clippy -D warnings`, typecheck, 418 pruebas de interfaz con el umbral de cobertura, 45 de Python y el exe 0.4.20 regenerado con su huella.

## Lo que queda y en qué orden

Los cinco cortes del plan están hechos (14-nov-2026). Quedan en el componente la orquestación del turno (`sendTurn` y el ciclo del turno activo), la carga de conversación (`loadConversation`), la navegación y la búsqueda (con su sondeo de 250 ms), los GPT personales activos, el resumen y las áreas pesadas (Athena, WorkflowStudio). Cada una es un corte futuro con el mismo patrón si se quiere seguir reduciendo; no hay listas de ficheros que mantener porque los tests de `tests/test_frontend_contract.py` recorren `src/**`.

Mismo patrón que funcionó en septiembre (`programacion/`, `gpts/`, `memoria/`) y en los cortes 1-5: **hook + tarjeta**. El hook se llama donde estaba su estado y la tarjeta recibe `ReturnType<typeof useX>` en una sola prop (o las props que necesita cuando el estado lo comparten varios dueños, como los refs de scroll).

1. ~~**Adjuntos** (`useAdjuntos` + `TarjetaAdjuntos`)~~ — hecho (corte 1).
2. ~~**Compositor** (`useCompositor` + `TarjetaOpcionesEjecucion`)~~ — hecho (corte 2). Los atajos globales de teclado se quedan en `App.tsx` (despachan navegación de la aplicación).
3. ~~**Confirmaciones de herramientas** (`useConfirmaciones` + `TarjetaConfirmacionHerramientas`)~~ — hecho (corte 3).
4. ~~**Lista de mensajes** (`useListaMensajes` + `ListaMensajes`)~~ — hecho (corte 4).
5. ~~**Barra lateral** (`navegacion/BarraLateral`) y **diálogos** (`dialogos/useDialogos`)~~ — hecho (corte 5).

Reglas: un corte por PR, sin cambiar comportamiento. Cada corte deja `pnpm typecheck`, `pnpm test:coverage`, `pnpm build` y los tests de Python en verde, y añade una prueba de la pieza suelta. Los tests de `tests/test_frontend_contract.py` recorren `src/**`, así que no hay listas de ficheros que actualizar.
