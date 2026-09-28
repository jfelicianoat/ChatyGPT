# Auditoría de intención, implementación y usabilidad de ChatyGPT

**Fecha:** 28 de septiembre de 2026. **Versión:** 0.2.0; SQLite 24. **Base:** `d0473ddc69b59bb01d500aef0e510e944c14f92a`.

## Dictamen

**ChatyGPT implementa gran parte del producto que describe su documentación, pero todavía presenta fallos importantes de privacidad efectiva, recuperación y conservación del trabajo.** La aplicación tiene una base técnica valiosa y una batería amplia de pruebas. Sin embargo, esas pruebas no cubren suficientemente las transiciones entre operaciones, conversaciones, nodos y estados de fallo.

La prioridad debe ser estabilizar esas transiciones antes de ampliar funcionalidades. No hace falta sustituir Tauri, Rust, React o SQLite. Hace falta que las garantías prometidas —control de privacidad, ejecución durable, cancelación, contexto trazable y recuperación— se mantengan también cuando hay latencia, navegación, reinicios o errores parciales.

Se registran **26 hallazgos y mejoras necesarias: 11 P1, 14 P2 y 1 P3**. No todos son vulnerabilidades ni todos tienen el mismo nivel de demostración. Las fichas distinguen las reproducciones, el análisis estático y las carencias de producto.

**Resultados obtenidos:** 338 pruebas Rust, 319 pruebas frontend y 43 pruebas Python aprobadas; comprobación de tipos, formato, Clippy estricto y construcción del frontend correctos. Además se han ejecutado **11 comprobaciones específicas de auditoría**: seis sobre componentes React y cinco sobre SQL extraído del código. Estas once comprobaciones reproducen comportamientos problemáticos actuales; que pasen significa que el problema se ha confirmado, no que esté corregido.

**Conclusión de uso:** resulta razonable seguir trabajando con la app como producto local en evolución, pero no dar por cerradas sus garantías de privacidad, recuperación y ausencia de duplicados hasta resolver los P1 y probarlos contra servicios reales.

## 1. Alcance y calidad de la evidencia

Se han contrastado README, CURRENT_STATE, ARCHITECTURE, BROKER_COMPATIBILITY, la remediación del 13 de septiembre, las guías de GPTs, el registro de diseño y los apartados relevantes de las verificaciones y auditorías históricas. La documentación histórica se ha tratado como antecedente, no como especificación vigente.

La revisión del código se ha centrado en la interfaz principal, flujos, comandos Tauri, tareas, programaciones, persistencia, contexto, red, archivos, Athena, arranque y pruebas. Graphify se utilizó para orientación: su índice contiene ubicaciones anteriores a la división de módulos, por lo que las referencias del informe se verificaron contra los archivos actuales. `codebase-memory-mcp` no estaba disponible entre las herramientas de esta sesión.

No se ha modificado código de producto ni abierto la base personal de la aplicación. Las pruebas añadidas viven bajo esta carpeta de auditoría y usan datos sintéticos. No se han enviado prompts a modelos, generado coste cloud, explotado destinos externos ni modificado servicios.

**Límites:** no se ha realizado una sesión visual sobre la ventana nativa Tauri, una instalación MSI/NSIS interactiva, una prueba con lector de pantalla, un corte real del proceso durante escritura ni una ejecución real completa contra Broker/Athena. La evaluación de usabilidad combina código, estilos y pruebas DOM; no certifica la apariencia o accesibilidad final de WebView2. Tampoco constituye una revisión exhaustiva de vulnerabilidades de dependencias.

Leyenda de evidencia:

- **R-UI:** reproducción ejecutando componentes React con el puente a servicios simulado.
- **R-SQL:** reproducción sobre SQLite en memoria con migraciones reales y consultas extraídas del código. No equivale a ejecutar todo el runtime Rust.
- **E:** defecto o divergencia identificado por seguimiento del código; falta reproducción integral.
- **G:** carencia de producto, mantenibilidad o verificación, con evidencia de su alcance.

Prioridades: **P1**, resolver antes de confiar datos sensibles o trabajo desatendido; **P2**, corregir para uso cotidiano fiable; **P3**, claridad y mantenimiento documental. No se ha constatado un incidente de seguridad real.

## 2. Qué pretendía el proyecto y qué existe realmente

| Intención documentada | Implementación encontrada | Valoración |
|---|---|---|
| Aplicación Windows con datos locales | Tauri, Rust, SQLite en AppLocalData, React mediante IPC | Alineada; el lanzador habitual condiciona el acceso a la disponibilidad del Broker |
| Persistir antes de enviar | Turnos y snapshots transaccionales; envío asíncrono e idempotencia | Buena base; quedan huecos entre scheduler, creación y asociación de tareas |
| Recuperar sin duplicar | Recuperación de tareas, adjuntos, flujos y claims | Parcial; no cubre todos los puntos de interrupción |
| Privacidad elegida por la persona | Clasificación, perfiles GPT y protección de recuerdos sensibles | Los controles visibles y la política efectiva pueden divergir; los flujos no propagan la clasificación de sus salidas |
| Secretos custodiados por Windows | DPAPI y credenciales fuera de SQLite | Alineada en almacenamiento; el destino Broker predeterminado usa HTTP por LAN |
| Herramientas con autorización | Decisiones persistidas antes del efecto, permisos separados, huellas | Implementación sustancial; la protección de destinos web deja un hueco de resolución DNS |
| GPTs versionados y conocimiento privado | Revisiones, perfiles, permisos, duplicación y transporte restringidos | Ampliamente implementado; falta explicar en el chat el perfil efectivo |
| Contexto trazable y acotado | Ventanas, resúmenes aprobados, fragmentos y snapshots | Implementado; los límites y cambios de modo no siempre se explican antes del envío |
| Automatizaciones durables | Claims, recurrencias IANA, agenda, historial | Hay solapamientos y un hueco de duplicación tras reinicio; eliminar programación borra su historial |
| Conversaciones largas fluidas | Se limita lo renderizado a 80 mensajes inicialmente | La consulta y transferencia siguen cargando el historial completo |
| Archivo y eliminación controlados | Archivado y borrado lógico | Falta un recorrido para consultar/restaurar archivados y una política operativa de purga y copia de seguridad |
| Resultados generados utilizables | Lista y descarga con comprobación de huella | El guardado termina en una ruta interna y no crea una referencia durable visible al archivo |
| Athena como servicio independiente | Cliente autenticado, SSE, permisos, historial y modelos | Separación correcta; respuestas JSON y artefactos aún se acumulan sin límite explícito de tamaño |
| Aplicación comprensible y accesible | Navegación por áreas, diálogos, atajos, progreso y errores legibles | Mejoras reales; subsisten carreras de navegación, pérdida de borradores y un fallo de composición IME |
| Calidad comprobada | CI, pruebas Rust/React/Python y cobertura | Buena base automatizada; falta evidencia de release y de los fallos entre operaciones |

No se consideran defectos actuales la ausencia de un sidecar Python, la falta de una carpeta de fixtures 2.10 o las limitaciones marcadas explícitamente como históricas. El scheduler actual es Rust y el contrato del Broker admite extensiones aditivas.

## 3. Hallazgos prioritarios

### H01 · P1 · La privacidad y el coste mostrados pueden no ser los que se aplican — E

**Problema.** El chat muestra y permite editar `conversation.executionPreferences`, pero la petición sustituye esas preferencias por el perfil del GPT cuando existe. Por ejemplo, un chat puede mostrar «Solo en este equipo» y presupuesto cero mientras el GPT seleccionado tiene un perfil `public` con presupuesto superior. La herencia de contexto previamente local puede endurecer la política, pero no resuelve el caso de un primer turno sin ese contexto.

La precedencia del GPT está documentada; el defecto es que la interfaz sigue presentando los valores del chat como controles operativos sin proyectar el resultado de esa precedencia.

**Cambio necesario.** Calcular una política efectiva única en Rust y mostrarla antes de enviar, indicando origen de cada valor: chat, GPT, sensibilidad o contexto heredado. Si el GPT manda, bloquear o distinguir claramente los controles heredados. Una elección explícita de privacidad o límite de gasto debe conservarse como restricción o requerir una aceptación visible para ampliarla.

**Aceptación.** GPT público + chat local; GPT con coste 1 USD + chat con coste cero; cambio de GPT y uso de recuerdos sensibles. Lo visible debe coincidir con la petición enviada.

**Evidencia:** [apps/desktop/src/App.tsx:3550](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:3550>), [apps/desktop/src/App.tsx:3918](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:3918>), [apps/desktop/src-tauri/src/task_runtime/peticiones.rs:339](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime/peticiones.rs:339>), [apps/desktop/src-tauri/src/task_runtime/peticiones.rs:776](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime/peticiones.rs:776>).

### H02 · P1 · Los flujos pierden la clasificación de los datos entre nodos — E

**Problema.** Las salidas de nodos se transportan como cadenas. Cada nodo posterior calcula su privacidad a partir de su propio perfil y recuerdos, sin heredar la clasificación de las salidas que recibe. Un nodo A local que usa conocimiento sensible puede alimentar un nodo B sin ese conocimiento, cuya petición vuelve a `internal` o a un perfil menos restrictivo.

**Impacto.** Una transformación o resumen de información protegida puede quedar autorizado para modelos cloud. La política de herencia implementada para conversaciones no está aplicada al grafo de ejecución.

**Cambio necesario.** Transportar y persistir junto a cada salida su clasificación y procedencia; calcular el máximo restrictivo de todas las entradas en cada nodo, incluidas aprobaciones, bifurcaciones, recuperaciones y reintentos. Nunca degradarlo por cambiar de GPT.

**Aceptación.** Flujo entrada → GPT con recuerdo sensible → instrucción genérica → resultado. Todas las peticiones posteriores deben mantener `local_only`, también después de reiniciar.

**Evidencia:** [apps/desktop/src-tauri/src/workflow_runtime.rs:298](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/workflow_runtime.rs:298>), [apps/desktop/src-tauri/src/workflow_runtime.rs:391](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/workflow_runtime.rs:391>), [apps/desktop/src-tauri/src/workflow_runtime.rs:704](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/workflow_runtime.rs:704>), [apps/desktop/src-tauri/src/workflow_runtime.rs:869](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/workflow_runtime.rs:869>). No se ha enviado información real para demostrar la salida a cloud.

### H03 · P1 · La validación DNS no fija el destino de la conexión — E

**Problema.** Investigación y APIs resuelven el dominio, comprueban las direcciones obtenidas y después entregan la URL original al cliente HTTP. El cliente puede resolverla de nuevo. No se fija la conexión a las direcciones que superaron la comprobación.

**Impacto.** La comprobación de red privada puede eludirse si la resolución cambia entre validación y conexión. Para investigación, que admite HTTP y recibe URLs procedentes del modelo/web, es una frontera relevante. En HTTPS interviene además la validación del certificado; no debe suponerse el mismo alcance de explotación en ambos casos.

**Cambio necesario.** Resolver y validar una vez por destino/salto, fijar esas direcciones en el cliente o conector y definir una política explícita de proxies. Revisar también rangos especiales no globales que no cubren únicamente `is_private` e `is_loopback`.

**Aceptación.** Resolver controlado que primero devuelve una IP pública y luego una privada; la segunda no debe recibir ninguna conexión. Repetir con redirección y proxy.

**Evidencia:** [apps/desktop/src-tauri/src/research_tools.rs:122](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/research_tools.rs:122>), [apps/desktop/src-tauri/src/research_tools.rs:311](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/research_tools.rs:311>), [apps/desktop/src-tauri/src/research_tools.rs:410](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/research_tools.rs:410>). Reqwest ofrece fijación explícita mediante [`resolve_to_addrs`](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.resolve_to_addrs); [`no_proxy`](https://docs.rs/reqwest/latest/reqwest/struct.ClientBuilder.html#method.no_proxy) permite impedir el uso automático de proxies. Hallazgo estático; no se ha ejecutado una explotación.

### H04 · P1 · HTTP por LAN y «Solo en este equipo» comunican garantías incompatibles — E

**Problema.** El destino predeterminado del Broker es una dirección LAN con esquema HTTP. Las peticiones administrativas incorporan `x-admin-token`, y prompts y resultados utilizan ese transporte. DPAPI protege la credencial guardada, pero no cifra la conexión. Además, la opción `local_only` se presenta como «Solo en este equipo», aunque la aplicación envía al Broker configurado, que puede estar en otro equipo.

**Cambio necesario.** HTTPS con validación de certificado para destinos no loopback, configuración visible del servidor y explicación precisa: «solo modelos locales del Broker» si esa es la garantía contractual. Reservar «este equipo» para un destino realmente local comprobado. No sustituir HTTP por TLS desactivando la verificación de certificados.

**Aceptación.** Configuración LAN HTTP advertida o bloqueada según política; certificado inválido rechazado; UI identifica el destino real. Confirmar también qué significa “local” en el despliegue del Broker.

**Evidencia:** [apps/desktop/src-tauri/src/broker/mod.rs:24](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/broker/mod.rs:24>), [apps/desktop/src-tauri/src/broker/mod.rs:313](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/broker/mod.rs:313>), [scripts/Start-ChatyGPT.ps1:27](<D:/Desarrollo/Proyectos TFM/ChatyGPT/scripts/Start-ChatyGPT.ps1:27>), [apps/desktop/src/App.tsx:3587](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:3587>).

### H05 · P1 · Una interrupción del scheduler puede repetir un trabajo ya creado — E + R-SQL parcial

**Problema.** `dispatch_claim` crea el turno/flujo y arranca su ejecución antes de enlazarlo al `scheduled_run`. Si el proceso se interrumpe entre ambos pasos, queda una tarea válida y una claim sin enlace. La recuperación vuelve a despachar la claim; el chat crea nuevos UUID y nueva clave idempotente. La clave de la programación no deduplica esa segunda tarea de chat.

La reproducción SQL confirma que la consulta de recuperación devuelve esa claim aunque ya exista una tarea no enlazada. No simula el corte del proceso completo. Además, la recuperación lee el payload actual de la programación, no un snapshot específico de la ejecución.

**Cambio necesario.** Crear el trabajo local y enlazar la ejecución en una transacción antes de despachar, o usar una bandeja durable de operaciones con clave derivada del run y enlace único. Congelar instrucción/destino en la ejecución. Recuperar por identidad estable, no crear otra petición.

**Aceptación.** Inyectar fallos antes y después de cada commit/despacho; debe quedar un solo trabajo por run con la misma instrucción.

**Evidencia:** [apps/desktop/src-tauri/src/scheduler_runtime.rs:7](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/scheduler_runtime.rs:7>), [apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:11](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:11>), [apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:350](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:350>), [apps/desktop/src-tauri/src/task_runtime.rs:479](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:479>).

### H06 · P2 · La ejecución por fecha puede solaparse con otra de la misma programación — R-SQL

La reclamación automática selecciona una programación vencida sin excluir runs `claimed` o `running`. «Ejecutar ahora» y «Reintentar» sí aplican esa exclusión. Un lanzamiento manual próximo a la hora programada puede coincidir con el automático; también puede ocurrir con trabajos prolongados.

**Cambio necesario.** Aplicar la misma exclusión dentro de la transacción de reclamación y definir si una fecha que vence con una ejecución activa se omite, acumula o aplaza. Registrar esa decisión. Reforzar la invariancia en la base si corresponde.

**Aceptación.** Run manual activo y fecha automática vencida: nunca dos runs activos de esa programación. El retraso debe quedar explicado en el historial.

**Evidencia:** [apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:44](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:44>), [apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:292](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/programacion_ejecucion.rs:292>).

### H07 · P1 · Un estado remoto tardío puede reabrir una tarea terminada — R-SQL + E

`record_remote_state` actualiza incondicionalmente estado, resultado y fecha terminal. Una lectura iniciada antes de cancelar puede llegar después de la cancelación y sustituir `cancelled/terminal` por `generating/polling`, incluso poniendo `terminal_at` a NULL. La reproducción usa exactamente el UPDATE de producción.

Hay campos de lease en el esquema, pero no se encontró su adquisición/renovación en el runtime revisado. El sondeo puede convivir con cancelación y con otros iniciadores de polling. La protección de estados terminales ya aplicada a workflows no está generalizada a tareas Broker.

**Cambio necesario.** Transiciones monotónicas y guardas transaccionales, orden/versionado de snapshots cuando el contrato lo permita, un propietario de sondeo por tarea y comprobación de que el worker sigue vigente antes de aplicar datos.

**Aceptación.** GET antiguo resuelto después de DELETE terminal, dos workers y reinicio. Estado terminal, resultado y tiempo final deben permanecer estables.

**Evidencia:** [apps/desktop/src-tauri/src/db/tareas.rs:87](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/tareas.rs:87>), [apps/desktop/src-tauri/src/db/tareas.rs:135](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/tareas.rs:135>), [apps/desktop/src-tauri/src/task_runtime/herramientas.rs:975](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime/herramientas.rs:975>), [apps/desktop/src-tauri/src/task_runtime/herramientas.rs:1172](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime/herramientas.rs:1172>).

### H08 · P1 · No puede cancelarse un envío que aún no tiene ID remoto — E

La cancelación exige `remote_task_id`. Si el Broker está caído o la creación se reintenta por transporte, se devuelve un error y el bucle de envío continúa. No hay una intención durable de cancelación que ese bucle respete. La persona puede perder el control sobre una pregunta que acaba ejecutándose cuando vuelve la conexión.

**Cambio necesario.** Persistir `cancel_requested` desde cualquier estado. Antes del envío, detenerlo; si la creación ya pudo ser aceptada, reconciliar con la misma identidad y cancelar el trabajo remoto. Distinguir «pendiente de enviar», «cancelación solicitada» y «cancelado».

**Aceptación.** Enviar sin Broker, cancelar, restaurar conexión y reiniciar: la pregunta no debe ejecutarse después. La incertidumbre de una petición ya transmitida debe seguir visible hasta resolverse.

**Evidencia:** [apps/desktop/src-tauri/src/task_runtime/herramientas.rs:975](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime/herramientas.rs:975>), [apps/desktop/src-tauri/src/task_runtime/herramientas.rs:1133](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime/herramientas.rs:1133>).

### H09 · P1 · Un fallo al materializar el resultado detiene el worker sin cerrar la tarea — E

Si `record_remote_state` falla, el sondeo retorna inmediatamente. Esa operación incluye interpretar/persistir resúmenes, embeddings y respuestas. Un resultado formalmente recibido pero no materializable, o un error de almacenamiento, puede dejar la tarea local activa sin worker que la avance.

**Cambio necesario.** Separar adquisición y materialización, registrar el fallo recuperable o contractual y mostrar una acción concreta. Reintentar fallos transitorios de SQLite; cerrar y explicar resultados inválidos. No abandonar silenciosamente una tarea que la UI considera pendiente.

**Aceptación.** Resumen completado sin Markdown, embedding inválido y bloqueo temporal de SQLite: cada caso termina en recuperación o error visible, sin espera indefinida.

**Evidencia:** [apps/desktop/src-tauri/src/task_runtime/herramientas.rs:1191](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime/herramientas.rs:1191>), [apps/desktop/src-tauri/src/db/tareas.rs:243](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/tareas.rs:243>), [apps/desktop/src-tauri/src/db/tareas.rs:294](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/tareas.rs:294>).

### H10 · P1 · El borrador de una conversación aparece en otra — R-UI

`draft` es un estado global de App. Abrir otra conversación no lo guarda por conversación ni lo vacía. Se ha reproducido escribir en A, abrir B y encontrar el mismo texto listo para enviar allí. Los adjuntos se reinicializan por separado, por lo que texto y contexto pueden dejar de corresponderse.

**Cambio necesario.** Borrador identificado por conversación: texto, adjuntos seleccionados y opciones temporales coherentes. Guardado local recuperable, con política explícita de retención; no mover el texto a otro chat implícitamente.

**Aceptación.** A conserva su borrador y B tiene el suyo; volver a A lo restaura. Repetir con proyectos, GPTs, adjuntos y reinicio.

**Evidencia:** [apps/desktop/src/App.tsx:306](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:306>), [apps/desktop/src/App.tsx:1607](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:1607>).

### H11 · P2 · Las respuestas tardías desplazan la navegación elegida — R-UI

Se han reproducido dos escenarios: abrir B lentamente, abrir C y recibir finalmente B; y enviar en A, navegar a B y regresar involuntariamente a A cuando el envío termina. `loadConversation` aplica resultados sin comprobar la selección vigente. Los intervalos de adjuntos también pueden aplicar respuestas de una conversación anterior.

**Cambio necesario.** Identificar cada selección/carga, descartar respuestas obsoletas y mantener trabajos por conversación. Una actualización de fondo puede cambiar los datos de A sin convertir A en la pantalla activa.

**Aceptación.** Resolver cargas en todas las permutaciones de orden; nunca cambiar el destino después de la última decisión de la persona.

**Evidencia:** [apps/desktop/src/App.tsx:674](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:674>), [apps/desktop/src/App.tsx:933](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:933>), [apps/desktop/src/App.tsx:1851](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:1851>).

### H12 · P1 · Un error de refresco se trata como error de envío y facilita duplicados — R-UI

El mismo `try/catch` abarca el envío aceptado, la recarga del chat y la navegación. Si falla una recarga, se restaura el texto como borrador y se presenta el turno como error aunque ya exista la tarea. Un segundo envío genera otra clave y otro trabajo.

**Cambio necesario.** Separar la aceptación durable de la actualización visual. Tras aceptar el turno, no restaurar el texto como no enviado. Mostrar «enviado; no se pudo actualizar» y reintentar lectura. Añadir un identificador de operación del cliente que sobreviva a reintentos inciertos.

**Aceptación.** Resolver envío y rechazar lectura posterior: un solo mensaje/tarea y posibilidad de refrescar sin reenviar.

**Evidencia:** [apps/desktop/src/App.tsx:1840](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:1840>), [apps/desktop/src-tauri/src/task_runtime.rs:479](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:479>).

### H13 · P1 · Cambiar de flujo descarta trabajo sin guardar — R-UI

El editor mantiene `dirty`, pero `openWorkflow` reemplaza el documento y lo pone a `false` sin guardar ni preguntar. Se ha reproducido modificar una descripción, cambiar de flujo y volver: los cambios desaparecen y no hubo confirmación. Cambiar de área desmonta también el editor.

**Cambio necesario.** Autoguardado durable de borradores o protección de salida con Guardar/Descartar/Seguir editando; restauración tras cierre. Mantener separadas la versión publicada y la edición local.

**Aceptación.** Cambio de flujo, cambio de área, cierre y fallo de guardado no pierden cambios silenciosamente.

**Evidencia:** [apps/desktop/src/WorkflowStudio.tsx:70](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/WorkflowStudio.tsx:70>), [apps/desktop/src/WorkflowStudio.tsx:208](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/WorkflowStudio.tsx:208>), [apps/desktop/src/WorkflowStudio.tsx:461](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/WorkflowStudio.tsx:461>).

## 4. Correcciones funcionales y de experiencia

### H14 · P2 · Archivar carece de una salida utilizable — G

Las listas y búsquedas excluyen archivados, y no hay comandos registrados para listarlos/restaurarlos. Los mensajes se conservan, pero la persona no tiene un recorrido normal para recuperarlos. En proyectos, además, archivar desvincula las conversaciones; restaurar en el futuro no bastaría para reconstruir automáticamente la agrupación perdida.

**Cambio necesario.** Área Archivo, búsqueda inclusiva opcional y restauración; papelera diferenciada. Conservar las relaciones del proyecto al archivar o registrar información suficiente para revertirlas. No llamar reversible a una acción que exige manipular SQLite.

**Aceptación.** Archivar y restaurar recupera mensajes, agrupación y contexto pertinente. Es una carencia ya reconocida en la verificación histórica de Fase 1, que sigue abierta.

**Evidencia:** [apps/desktop/src-tauri/src/db/conversaciones.rs:44](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/conversaciones.rs:44>), [apps/desktop/src-tauri/src/db/conversaciones.rs:295](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/conversaciones.rs:295>), [apps/desktop/src-tauri/src/db/proyectos.rs:317](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/proyectos.rs:317>), [apps/desktop/src-tauri/src/lib.rs:203](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/lib.rs:203>).

### H15 · P2 · Eliminar una programación borra también sus ejecuciones — R-SQL

La eliminación física de `scheduled_tasks` activa `ON DELETE CASCADE` en `scheduled_runs`. La reproducción confirma la desaparición de un run completado. Esto reduce la utilidad del historial durable, especialmente para verificar qué se ejecutó y qué falló.

**Cambio necesario.** Retirar/archivar programaciones preservando sus ejecuciones. Si se ofrece borrar el historial, hacerlo como acción explícita separada, con consecuencias y alcance visibles. Conservar snapshots del nombre, instrucción y destino en cada ejecución cuando se necesiten para auditoría.

**Aceptación.** Eliminar una programación impide futuras ejecuciones y mantiene sus resultados consultables; purgarlos exige otra decisión.

**Evidencia:** [apps/desktop/src-tauri/src/db/programacion.rs:378](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/programacion.rs:378>), [apps/desktop/src-tauri/migrations/0001_initial.sql:264](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/migrations/0001_initial.sql:264>).

### H16 · P2 · Falta una estrategia operativa de copia, restauración y retención — G

La base local es la fuente de verdad, mientras Markdown y Obsidian son proyecciones. No se encontró un recorrido de backup/restauración integral ni mantenimiento de borrados físicos. Exportar chats no conserva por sí solo GPTs, versiones, permisos, programaciones, contexto y relaciones.

**Cambio necesario.** Backup consistente con WAL, adjuntos y manifiesto de versiones; restauración validada en un directorio temporal; política de retención de adjuntos, snapshots y tareas. Tratar DPAPI y permisos como capacidades que pueden exigir nueva autorización al migrar de cuenta/equipo. Definir borrado físico y explicar qué permanece tras borrar un recuerdo o una conversación.

**Aceptación.** Restaurar en un perfil limpio sin perder relaciones ni reutilizar permisos indebidamente. Comprobar integridad y recuperación tras fallos de copia. No se ha constatado corrupción actual; es una carencia de recuperación.

**Evidencia:** [docs/ARCHITECTURE.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/docs/ARCHITECTURE.md>), [apps/desktop/src-tauri/src/db/apertura.rs](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/apertura.rs>), [apps/desktop/src-tauri/src/db/conversaciones.rs:318](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/conversaciones.rs:318>), [apps/desktop/src-tauri/src/lib.rs](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/lib.rs>).

### H17 · P2 · La paginación del chat solo reduce el renderizado — E

La interfaz muestra inicialmente 80 mensajes, pero `conversation_view` consulta y devuelve todos los mensajes, citas e investigaciones. Mostrar otros 50 amplía una ventana sobre datos ya cargados. El coste de SQL, memoria, serialización IPC y transferencia continúa creciendo con el historial.

**Cambio necesario.** Paginación real por cursor/secuencia en Rust, fuentes correspondientes a esa página y consultas de resumen separadas. Mantener exportación completa como operación independiente. No afirmar una mejora de tiempo de apertura solo por reducir nodos DOM.

**Aceptación.** Abrir un historial sintético grande devuelve inicialmente un lote acotado; medir tiempo, bytes y memoria, conservar el punto de lectura al añadir páginas.

**Evidencia:** [apps/desktop/src-tauri/src/db/turnos.rs:371](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/turnos.rs:371>), [apps/desktop/src-tauri/src/db/turnos.rs:469](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/turnos.rs:469>), [apps/desktop/src/App.tsx:1754](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:1754>).

### H18 · P2 · Quedan respuestas de red sin límite explícito de tamaño — E

El lector JSON general del Broker usa `bytes()`. Athena hace lo mismo para JSON y errores, y descarga artefactos con `text()`. El límite del parser SSE y los límites de archivos del Broker no protegen estos caminos. Un historial legítimamente grande o una respuesta defectuosa puede consumir memoria excesiva; el timeout no es un presupuesto de bytes.

**Cambio necesario.** Lectura incremental con límites por endpoint, verificación de tamaño real además de `Content-Length`, paginación de listados y errores legibles. Descargar artefactos grandes a disco cuando corresponda.

**Aceptación.** Respuestas sin Content-Length y cuerpos por encima del límite se interrumpen antes de acumularlos completos.

**Evidencia:** [apps/desktop/src-tauri/src/broker/mod.rs:320](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/broker/mod.rs:320>), [apps/desktop/src-tauri/src/athena/mod.rs:251](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/athena/mod.rs:251>), [apps/desktop/src-tauri/src/athena/mod.rs:286](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/athena/mod.rs:286>), [apps/desktop/src-tauri/src/athena/mod.rs:756](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/athena/mod.rs:756>).

### H19 · P2 · La indexación documental lanza todos los fragmentos sin límite local de concurrencia — E

El README describe una sola tarea activa por documento. La implementación prepara todos los fragmentos y llama a `spawn_submission_and_poll` para cada uno. El Broker puede limitar inferencias, pero el cliente crea sus workers y peticiones sin una cola local acotada. Además, el lote se prepara mediante operaciones individuales; una interrupción intermedia no equivale a un lote completo transaccional.

**Cambio necesario.** Cola durable con concurrencia global y por documento, prioridades para preguntas interactivas y recuperación de lotes incompletos. Alinear la documentación con el comportamiento elegido. Evitar que un documento grande retrase la persistencia visible del turno al registrar sus dependencias.

**Aceptación.** Documento con cientos/miles de fragmentos: número acotado de peticiones simultáneas, UI utilizable y reanudación completa sin duplicados.

**Evidencia:** [README.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/README.md>), [apps/desktop/src-tauri/src/task_runtime.rs:148](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:148>), [apps/desktop/src-tauri/src/task_runtime.rs:196](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:196>), [apps/desktop/src-tauri/src/task_runtime.rs:375](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:375>).

### H20 · P2 · El arranque manual y automático tienen comportamientos inconsistentes — E

El lanzador habitual aborta si no puede validar la credencial del Broker, impidiendo abrir desde ese recorrido los datos locales cuando el servicio está caído. El script de inicio con Windows espera indefinidamente y consulta `/capabilities`, mientras el lanzador manual explica y usa otro endpoint porque capacidades no prueba la credencial. Una credencial incorrecta puede superar una comprobación y fallar en la otra.

**Cambio necesario.** Abrir la app en modo sin conexión; diagnosticar y reconectar en segundo plano. Unificar comprobación de autenticación y mensajes. Acotar esperas o hacer visible su estado. Separar arrancar la interfaz, conectar Broker y supervisar Athena.

**Aceptación.** Broker apagado, credencial incorrecta y arranque con Windows: los datos siguen accesibles y cada fallo tiene una explicación consistente. No se ha modificado el registro de Windows para esta revisión.

**Evidencia:** [scripts/Start-ChatyGPT.ps1:165](<D:/Desarrollo/Proyectos TFM/ChatyGPT/scripts/Start-ChatyGPT.ps1:165>), [apps/desktop/src-tauri/src/startup.rs:220](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/startup.rs:220>), [apps/desktop/src-tauri/src/startup.rs:277](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/startup.rs:277>).

### H21 · P2 · Las garantías documentadas de capacidades previas son más fuertes que el código — E

La documentación describe validación redundante del sandbox antes de persistir y rechazo de investigación si no pueden verificarse capacidades. El código captura el error de capacidades y continúa: para sandbox delega en el Broker; para investigación fabrica un plan no verificado. El fallo puede aparecer después de crear el turno.

**Cambio necesario.** Decidir explícitamente entre rechazar de forma conservadora o permitir compatibilidad con validación remota. Si se permite continuar, reflejar «capacidad no comprobada», las garantías perdidas y el posible rechazo posterior. Actualizar README, arquitectura y pruebas a una misma política.

**Aceptación.** `/capabilities` inaccesible, sandbox retirado y herramientas de investigación ausentes: resultado previsto y mensaje coherente. No se ha demostrado ejecución sin autorización de la persona; la divergencia afecta a la comprobación de disponibilidad y al momento del fallo.

**Evidencia:** [apps/desktop/src-tauri/src/task_runtime.rs:334](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:334>), [apps/desktop/src-tauri/src/task_runtime.rs:366](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:366>), [apps/desktop/src-tauri/src/task_runtime.rs:413](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/task_runtime.rs:413>), [docs/BROKER_COMPATIBILITY.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/docs/BROKER_COMPATIBILITY.md>), [README.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/README.md>).

### H22 · P2 · La búsqueda de chats es inconsistente con el idioma y admite resultados obsoletos — R-SQL + E

`LIKE ... COLLATE NOCASE` no implementa por sí mismo normalización española completa. Se ha confirmado que un chat llamado «Árbol» no aparece buscando «árbol» ni «arbol», aunque sí con «Árbol». Otros buscadores de la aplicación prometen ignorar acentos. Además, el debounce cancela el temporizador pendiente, pero no descarta respuestas ya iniciadas de una consulta anterior.

**Cambio necesario.** Normalización Unicode y de diacríticos uniforme, índice apropiado y número de versión de búsqueda para ignorar respuestas antiguas. Mostrar coincidencia en el mensaje y permitir ir al punto encontrado, no solo a la conversación.

**Aceptación.** Árbol/árbol/arbol, Ñ/ñ y resolución invertida de consultas. Medir búsquedas con volumen realista.

**Evidencia:** [apps/desktop/src-tauri/src/db/conversaciones.rs:65](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/db/conversaciones.rs:65>), [apps/desktop/src/App.tsx:900](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:900>).

### H23 · P2 · «Guardar» un archivo generado no completa un flujo de usuario durable — E

El botón descarga a la carpeta administrada y muestra una alerta con la ruta. No ofrece elegir destino ni mostrarlo en el Explorador. El comando devuelve una cadena de ruta sin registrar la asociación entre artefacto, mensaje y copia local. La próxima consulta vuelve a depender de la lista del Broker, incluso si ya se descargó una copia que después deja de estar disponible remotamente.

**Cambio necesario.** Persistir el artefacto y su asociación local; distinguir «Conservar en el chat», «Guardar como…» y «Mostrar archivo». Mantener acceso a la copia local tras expirar la remota. Ofrecer previsualización segura para formatos compatibles y estado de descarga por archivo.

**Aceptación.** Descargar, cerrar, retirar el artefacto remoto y reabrir: la copia conservada sigue accesible desde el mensaje.

**Evidencia:** [apps/desktop/src/MessageArtifacts.tsx:25](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/MessageArtifacts.tsx:25>), [apps/desktop/src/MessageArtifacts.tsx:48](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/MessageArtifacts.tsx:48>), [apps/desktop/src-tauri/src/comandos/tareas.rs:89](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/comandos/tareas.rs:89>), [apps/desktop/src-tauri/src/attachment_runtime.rs:429](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src-tauri/src/attachment_runtime.rs:429>).

### H24 · P2 · Enter envía durante la composición de texto IME — R-UI

El manejador del compositor comprueba Enter y Shift, pero no `isComposing`. Se ha reproducido el envío de una composición todavía activa. La protección existente en los atajos generales no alcanza a este manejador.

**Cambio necesario.** No enviar durante composición; comprobar el evento nativo y la compatibilidad necesaria con WebView2. Mantener Enter/Shift+Enter y documentar una opción de envío alternativa si se ofrece.

**Aceptación.** Enter confirma la composición y solo un Enter posterior envía. Verificar con un IME real además del test DOM.

**Evidencia:** [apps/desktop/src/App.tsx:3475](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:3475>), [apps/desktop/src/keyboard.ts:22](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/keyboard.ts:22>).

### H25 · P3 · La documentación y algunos textos de UI conservan estados anteriores — E

Hay guías que indican «Inicio → GPTs personales» o «Inicio → Tareas programadas», aunque la navegación actual presenta áreas propias. La ventana aún usa «Fase 1 · Núcleo» y la confirmación de métricas habla de cuatro cuando hay cinco. El README simplifica límites documentales que varían con el perfil. ARCHITECTURE mezcla decisiones vigentes con pendientes e hipótesis antiguas; los documentos históricos están mejor etiquetados, pero no todas las afirmaciones funcionales vigentes se han reconciliado.

**Cambio necesario.** Manual de usuario por recorridos, especificación normativa breve por capacidad, matriz requisito → código → prueba y changelog separado. Quitar detalles como SQLite, esquema y fases de las pantallas principales salvo diagnóstico. Actualizar el grafo de código después de reorganizaciones.

**Aceptación.** Una persona sigue cada guía sin traducir nombres de menús ni descubrir que la capacidad funciona de otro modo.

**Evidencia:** [docs/GPT_SCHEDULED_TASKS.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/docs/GPT_SCHEDULED_TASKS.md>), [docs/GPT_API_ACTIONS.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/docs/GPT_API_ACTIONS.md>), [apps/desktop/src/App.tsx:1588](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:1588>), [apps/desktop/src/App.tsx:2568](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx:2568>), [docs/ARCHITECTURE.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/docs/ARCHITECTURE.md>).

### H26 · P2 · La organización de UI y los controles de calidad no protegen suficientemente sus recorridos — G

La división del backend en `db/*` y `comandos/*` es una mejora real respecto a la auditoría antigua. No procede repetir que toda la persistencia siga en un único `db/mod.rs`. Sin embargo, App mantiene 4.591 líneas, numerosos estados y cargas coordinadas. El bundle JS construido pesa 510,34 kB minificado y Vite emite su aviso de tamaño; esto no demuestra por sí solo una lentitud, pero identifica una oportunidad de carga por áreas.

La cobertura frontend medida es 56,20 % de líneas y 41,53 % de funciones. El umbral global pasa aunque las carreras y pérdidas reproducidas no estén cubiertas. Las pruebas simuladas tampoco certifican arranque, escalado de Windows, instalación o servicios reales. CI usa toolchains flotantes y no se observan pasos de auditoría de dependencias; no se ha constatado una CVE concreta en esta sesión.

**Cambio necesario.** Extraer controladores por conversación/envío, archivos y navegación con identidades y estados explícitos; cargar áreas pesadas bajo demanda. Introducir tests de recorridos y fallos, versiones reproducibles de herramientas, análisis de dependencias y evidencias de release. No emprender una reescritura general para resolver errores localizados.

**Aceptación.** Los casos H01–H24 quedan cubiertos donde corresponde; CI prueba instalación/arranque y un smoke contractual controlado. Rendimiento medido antes/después, no inferido solo del número de líneas.

**Evidencia:** [apps/desktop/src/App.tsx](<D:/Desarrollo/Proyectos TFM/ChatyGPT/apps/desktop/src/App.tsx>), [vite.config.ts:24](<D:/Desarrollo/Proyectos TFM/ChatyGPT/vite.config.ts:24>), [.github/workflows/ci.yml](<D:/Desarrollo/Proyectos TFM/ChatyGPT/.github/workflows/ci.yml>), [design-qa.md](<D:/Desarrollo/Proyectos TFM/ChatyGPT/design-qa.md>).

## 5. Evaluación de usabilidad y validaciones visuales pendientes

La navegación por Chats, Proyectos, GPTs, Flujos, Athena, Automatizaciones y Ajustes tiene sentido para un usuario experimentado. Para una primera sesión, falta una presentación más clara de tres tareas distintas: conversar, ejecutar un flujo y encargar trabajo autónomo. Debe explicarse qué necesita cada una y qué permisos/costes implica antes de entrar en formularios extensos.

Los cambios prioritarios de experiencia son H01 y H04 —garantías comprensibles—, H10–H13 —continuidad del trabajo—, H14–H16 —recuperación— y H23 —entregables utilizables—. Un rediseño puramente visual no resolvería estos problemas.

Además deben validarse, sin presentarlos todavía como defectos visuales demostrados:

| Recorrido/condición | Qué comprobar | Resultado exigido |
|---|---|---|
| 980×680 y 1280×820 | Compositor, menú Más, panel de contexto y formularios | Acciones principales visibles; desplazamiento inequívoco |
| Escalado Windows 150 % y zoom 200 % | `body` tiene `min-width: 980px`; varios layouts estrechos están por debajo de ese umbral | No perder controles ni exigir precisión extrema |
| Tema oscuro | Estados de error, tablas, permisos y campos | Contraste comprobado en estilos resueltos, no solo en tokens |
| Teclado/lector de pantalla | Foco en diálogos, retorno de foco, canvas, cambios de ruta | Flujo completo sin ratón y sin foco invisible |
| Respuestas largas | `aria-live` en lista de mensajes, repetición de anuncios | Anunciar progreso y contenido nuevo sin releer todo el historial |
| Fallo de servicio | Arranque, envío, cancelación y recuperación | Cada estado explica qué ocurrió y qué puede hacer la persona |
| Primer uso | Credenciales, disponibilidad del modelo, diferencia Broker/Athena | Llegar al primer resultado con decisiones comprensibles |

La documentación `design-qa.md` ya reconoce pendiente la aprobación visual. Este informe no la da por superada mediante las pruebas DOM.

## 6. Qué conservar del desarrollo actual

- Separación entre React y el núcleo que custodia red, base de datos y credenciales.
- Transacciones y claves idempotentes para turnos ordinarios; materialización durable.
- Modelo explícito de permisos y persistencia de decisiones antes de ejecutar efectos.
- Snapshots de contexto, resúmenes revisables y trazabilidad de fragmentos.
- Versionado de GPTs y publicaciones de flujos con referencias autorizadas.
- Comprobación de huellas de archivos y protección de descargas autenticadas por origen.
- Pruebas contractuales con servicios simulados y distinción de errores de transporte/contrato.
- Separación del servicio Athena y reconstrucción de eventos SSE, con pruebas Unicode.

La remediación del 13 de septiembre resolvió problemas reales. Los hallazgos de este informe describen huecos restantes o garantías incompletas, y no invalidan indiscriminadamente aquel trabajo.

## 7. Plan de cambios, orden y criterios de salida

| Orden | Trabajo | Hallazgos | Tamaño relativo | Condición para darlo por terminado |
|---|---|---|---|---|
| 1 | Política efectiva de privacidad/coste y clasificación entre nodos | H01, H02, H04 | Medio/grande | UI, snapshots y peticiones coinciden; clasificación nunca se degrada |
| 2 | Conexiones a destinos validados | H03, H18 | Medio | Límites de bytes y destinos probados con servidores/resolver controlados |
| 3 | Estados durables de tarea y cancelación | H07, H08, H09 | Grande | Estados terminales monotónicos; cancelación previa al ID; ningún worker abandona una tarea activa sin explicación |
| 4 | Scheduler transaccional e idempotencia entre subsistemas | H05, H06, H15 | Grande | Fallos entre operaciones no duplican; ausencia de solapamientos; historial preservado |
| 5 | Borradores, navegación y confirmación de envío | H10, H11, H12, H13, H24 | Medio | Las seis reproducciones DOM se convierten en regresiones con comportamiento correcto |
| 6 | Recuperación de datos y entregables | H14, H16, H23 | Grande | Archivo restaurable, backup verificado y artefactos locales durables |
| 7 | Volumen y capacidad de respuesta | H17, H19, H22 | Medio/grande | Paginación real, concurrencia acotada y búsqueda Unicode estable |
| 8 | Arranque, coherencia documental y release | H20, H21, H25, H26 | Medio | Inicio sin conexión, política contractual única y evidencia Windows real |

“Tamaño relativo” no es una estimación de días. Depende del alcance acordado y de si se añaden migraciones. Las fases 1–5 pueden desarrollarse en cambios pequeños, cada uno con su prueba de regresión y revisión de compatibilidad de datos.

**Primera entrega recomendada:** política efectiva visible, propagación de clasificación, DNS fijado, cancelación durable, scheduler sin hueco de enlace y aislamiento de borradores/envíos. No mezclar en el mismo cambio una actualización grande de dependencias y una modificación de persistencia.

## 8. Verificaciones realizadas y cómo repetir las nuevas evidencias

| Comprobación | Resultado |
|---|---|
| Rust `cargo test --offline ... --lib` | 338 aprobadas |
| Rust `cargo fmt --check` | Correcto |
| Rust `cargo clippy --offline ... --all-targets -- -D warnings` | Correcto |
| Frontend Vitest | 30 archivos; 319 pruebas aprobadas |
| Cobertura frontend V8 | Líneas/sentencias 56,20 %; ramas 72,61 %; funciones 41,53 % |
| TypeScript `tsc -b --pretty false` | Correcto |
| Vite build | Correcto; aviso por bundle JS de 510,34 kB |
| Python `unittest discover -s tests -v` | 43 aprobadas |
| Evidencias de interfaz añadidas | 6 comportamientos defectuosos reproducidos |
| Evidencias SQL añadidas | 5 comportamientos confirmados con consultas reales |
| Ventana Tauri, instalación y servicios reales | No verificados en esta sesión |

La invocación inicial de `pnpm.cmd test` no pudo completar la comprobación de firma/descarga de la versión del gestor porque falló el acceso al registro. Se utilizaron después los ejecutables de Vitest, TypeScript y Vite ya instalados. **Ese mensaje no demuestra por sí mismo manipulación de dependencias**, y no se desactivaron las comprobaciones de firma.

Desde la raíz de ChatyGPT, las evidencias específicas se ejecutan con:

```powershell
rtk proxy node node_modules/vitest/vitest.mjs run docs/AUDIT_2026-09-28/evidence/ui.test.tsx --root .
rtk proxy python -X utf8 docs/AUDIT_2026-09-28/evidence/sql_checks.py
```

Los archivos son [docs/AUDIT_2026-09-28/evidence/ui.test.tsx](<D:/Desarrollo/Proyectos TFM/ChatyGPT/docs/AUDIT_2026-09-28/evidence/ui.test.tsx>) y [docs/AUDIT_2026-09-28/evidence/sql_checks.py](<D:/Desarrollo/Proyectos TFM/ChatyGPT/docs/AUDIT_2026-09-28/evidence/sql_checks.py>). Las aserciones describen el fallo existente y deberán cambiarse al incorporar la corrección. Las cinco pruebas SQL verifican consultas y restricciones; no deben anunciarse como cinco pruebas integrales del runtime.

## 9. Condición de aceptación del producto mejorado

La app podrá considerarse sustancialmente más fiable cuando la persona sepa **qué datos se envían, a dónde, con qué coste máximo y bajo qué perfil**, pueda **cambiar de tarea sin perder ni mezclar su trabajo**, y pueda **cancelar, cerrar y reabrir sin duplicados ni estados eternos**.

Ese resultado debe demostrarse con pruebas de fallos entre operaciones y recorridos reales en Windows, conservando los datos históricos de la versión 0.2.0. Añadir más funcionalidades antes de cerrar estas garantías aumentaría la superficie de fallos y la dificultad de explicarlos.
