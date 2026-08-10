# Arquitectura y plan de producto

Fecha de auditoría: 2026-07-26.

## 1. Estado real del repositorio y el entorno

### Comprobado

- `ChatyGPT` estaba vacía al comenzar.
- El workspace contiene `AI_Broker`, su código, documentación, pruebas y una
  instancia SQLite local.
- AI Broker está implementado con FastAPI, Pydantic y SQLite.
- Existen Node 24.11.1, pnpm 11.9.0, Python 3.14.0, uv 0.11.7 y Git 2.47.0.
- Rust estable está instalado mediante rustup y Cargo compila el proyecto.
- La política de PowerShell impide ejecutar `npm.ps1`; `pnpm.cmd` sí arranca.
- Las dependencias JavaScript quedaron instaladas con pnpm 11.9.0.
- El virtualenv de AI Broker referencia un intérprete inexistente; el Python del
  sistema carga FastAPI 0.128.0 y Pydantic 2.12.5, pero no `pytest`.
- AI Broker inicialmente no estaba ejecutándose. Después se verificó una
  instancia real en `A9_Mega` mediante un probe ejecutado en esa máquina.
- Git rechazó la inspección por propiedad dudosa del directorio superior. No se
  cambió la configuración global del usuario.

### No verificado

- Interacción manual con la ventana Tauri desde el perfil del usuario final; la
  sesión aislada de Codex no puede crear la ventana o su directorio de perfil.
- Cancelación real y recuperación de una tarea tras reinicio.
- Disponibilidad real de modelos, Docker y sandbox.
- Empaquetado MSI/NSIS y firma.

## 2. Capacidades verificadas de AI Broker

La evidencia procede del código local (`app/main.py`, `app/schemas.py`,
`app/admin_auth.py`), README, documentación de ingesta/sandbox y fixtures.

| Capacidad | Estado | Evidencia local |
|---|---|---|
| Contrato | Revisado estáticamente | contrato cliente 2.7 y comprobación reproducible de OpenAPI |
| Crear tarea | Revisado estáticamente | `POST /api/v1/tasks`, 202 o 200 por idempotencia |
| Consultar tarea | Revisado estáticamente | `GET /api/v1/tasks/{task_id}` |
| Cancelar | Revisado estáticamente | `DELETE /api/v1/tasks/{task_id}` |
| Reanudar tools | Revisado estáticamente | `POST /api/v1/tasks/{task_id}/tool_results` |
| Estados | Revisado estáticamente | `waiting_for_memory` continúa sondeándose y se explica como espera recuperable; solo `completed`, `failed` y `cancelled` son terminales |
| Ingesta | Revisado estáticamente | `POST /api/v1/files`, polling y Markdown |
| Modelos/capacidades | Revisado estáticamente | endpoints `/models`, `/models/availability`, `/models/context`, `/capabilities` |
| Embeddings | Revisado estáticamente | `inference_kind=embedding`, estrategia `single`, salida JSON |
| Autenticación | Revisado estáticamente | cabecera `x-admin-token` cuando hay token configurado |
| Idempotencia | Revisado estáticamente | `idempotency_key` + hash; conflicto HTTP 409 |
| Sandbox | Revisado estáticamente | `run_code` opt-in y `SANDBOX_DISABLED` si no está habilitado |
| OpenAPI real | Verificado manualmente (alcance) | endpoint consultado por el probe en A9 |
| Integración real | Pendiente de repetir con 2.7 | ejecutar el diagnóstico con el Broker actualizado y su token en memoria |

La semántica de cancelación observada es una solicitud de cancelación. No se
presupone que una operación remota en curso termine de forma instantánea.

## 3. Arquitectura propuesta

```text
React (vista y estado efímero)
          │ comandos tipados Tauri
          ▼
Rust application core
  ├─ casos de uso y permisos
  ├─ scheduler de polling / leases
  ├─ adaptador AI Broker 2.7
  ├─ repositorios SQLite
  ├─ exportador atómico al vault
  └─ gestor del sidecar Python
          │
          ├──────── HTTP local ────────► AI Broker (sin modificar)
          │
          ├──────── SQLite ────────────► AppLocalData (fuente de verdad)
          │
          ├──────── IPC autenticado ───► Python sidecar (cuando sea necesario)
          │
          └──────── exportación ───────► Vault/Google Drive (proyección)
```

Decisiones:

1. **Rust es el proceso de aplicación.** Posee persistencia, red, permisos,
   secretos y ciclo de vida. React no llama directamente a AI Broker ni abre
   SQLite.
2. **SQLite vive en `AppLocalData`.** Se usa WAL, claves foráneas, timeout de
   bloqueo y migraciones transaccionales. No vive dentro del vault ni de Google
   Drive.
3. **El vault es una proyección.** Un único exportador usa identificadores
   estables, hashes, temporales y reemplazo atómico; un conflicto nunca modifica
   SQLite.
4. **Python es un sidecar estrecho.** Se añadirá para automatizaciones y trabajo
   documental que lo justifique, con protocolo versionado. No forma parte del
   camino crítico del chat básico.
5. **Los secretos no cruzan React.** En el slice actual solo se admite lectura
   desde entorno. El backend seguro definitivo será Credential Manager o
   Stronghold; SQLite restringe `app_settings` a valores públicos.
6. **Polling por lease.** Una única operación local puede poseer cada tarea. Los
   intervalos crecen con backoff y jitter, se reducen tras un cambio real y se
   detienen en estados terminales.
7. **Persistir antes de enviar.** La aplicación crea conversación, mensaje,
   `broker_task`, `idempotency_key` y snapshot de contexto en una transacción;
   solo después hace HTTP.
8. **Recuperación explícita.** Al arrancar, toda tarea local no terminal pasa a
   `recovery_pending`; se consulta por su `remote_task_id` o se reintenta la
   creación con la misma clave idempotente.
9. **Permisos deny-by-default.** Las acciones sensibles producen una
   `confirmation_request` visible y auditable. Las autorizaciones globales
   ambiguas no existen.

La [recomendación oficial de Tauri](https://v2.tauri.app/start/frontend/)
favorece Vite para SPAs React. La
[documentación oficial del plugin SQL](https://tauri.app/plugin/sql/) confirma
migraciones transaccionales. Este slice usa `rusqlite` en el núcleo para no
exponer consultas arbitrarias al webview; es una decisión de superficie de
ataque, no un cambio de stack.

## 4. Estructura de carpetas

```text
ChatyGPT/
├─ apps/
│  └─ desktop/
│     ├─ src/                    # React, vista y puertos tipados
│     └─ src-tauri/
│        ├─ capabilities/        # ACL mínima
│        ├─ migrations/          # esquema SQLite versionado
│        └─ src/
│           ├─ broker/           # contratos y adaptador HTTP
│           ├─ db/               # conexión, migración, recuperación
│           ├─ error.rs
│           └─ lib.rs            # composition root y comandos
├─ contracts/
│  ├─ broker/2.6/                # compatibilidad histórica
│  └─ broker/2.7/                # fixtures contractuales vigentes
├─ docs/
├─ packages/                     # reservado para contratos UI compartidos
├─ services/
│  └─ automation/                # sidecar Python futuro
└─ tests/                        # verificaciones sin dependencias externas
```

## 5. Modelo de datos inicial

El esquema evita documentos JSON gigantes como sustituto de relaciones. JSON se
reserva a snapshots inmutables, payloads de API y configuración versionada.

Relaciones principales:

- `Project 1 ── * Conversation`.
- `Conversation 1 ── * Message 1 ── * MessagePart`.
- `Conversation/Message ── * Attachment`; `Project * ── * Attachment` mediante
  `ProjectFile`.
- `Message 0..1 ── 1 BrokerTask 1 ── * BrokerTaskEvent`.
- `BrokerTask 1 ── * ToolCall 1 ── 0..1 ToolResult`.
- `Message 1 ── * Citation`.
- `BrokerTask 1 ── 0..1 ContextSnapshot 1 ── * ContextSource`.
- `Project/GPT 0..1 ── * MemoryItem`.
- `CustomGPT 1 ── * GPTVersion 1 ── * GPTToolPermission`.
- `ScheduledTask 1 ── * ScheduledRun`.
- `ResearchRun 1 ── * ResearchStep`.

Decisiones de ciclo de vida:

- El borrado de conversación es lógico primero (`deleted_at`) y físico mediante
  una operación de mantenimiento confirmada.
- Eventos, snapshots y auditoría son append-only a nivel de dominio.
- Adjuntos se deduplican por SHA-256; `broker_file_id` es único cuando existe.
- `claim_key` impide duplicar ejecuciones programadas.
- `idempotency_key` es única localmente antes de tocar la red.
- `app_settings` rechaza secretos por diseño.
- `Conversation.execution_preferences_json` conserva la clasificación, estrategia,
  preset, presupuesto y política de contexto largo como configuración versionada.
- `SemanticChatWorkflow.execution_preferences_json` captura esas preferencias al
  iniciar el turno para que la segunda etapa y una recuperación usen exactamente
  la misma decisión.
- `BrokerTask.progress_json` conserva la fase y el contador de invocaciones para
  reconstruir el progreso visible después de reiniciar.

## 6. Plan detallado de la Fase 0

### 0A. Base ejecutable — en curso

- Workspace, React/Vite, Tauri y ACL mínima.
- SQLite en AppLocalData, migración inicial e integrity checks.
- Pantalla de diagnóstico y estados honestos.

### 0B. Contrato Broker — en curso

- Generar tipos desde el OpenAPI real o comparar manualmente con Pydantic.
- Fixtures de 202/200/409/422, estados terminales, `waiting_for_tools` y errores.
- **Implementado en el slice durable:** persistir `broker_task` antes de `POST`.
- **Implementado:** polling con backoff, jitter y clasificación de errores.
- **Implementado:** cancelación como solicitud, sin prometer inmediatez.
- Pendiente: fixture automatizado desde el binario Tauri y lease multiworker.

### 0C. Recuperación

- **Implementado parcialmente:** matriz local/remoto para `created`, `submitting`, `polling`,
  `waiting_for_tools`, terminal y huérfana.
- **Implementado en código:** reenvío con la misma petición y clave cuando el
  proceso se interrumpe antes de persistir el 202.
- **Verificado localmente:** identidad remota, petición e idempotency key
  sobreviven a recuperación.
- Pendiente: prueba E2E cerrando el proceso real entre commit, POST y 202.

### 0D. Seguridad y observabilidad

- **Implementado:** custodia del token con DPAPI `CurrentUser` (`secrets.rs`),
  alta y rotación desde la interfaz, sustitución en caliente del cliente HTTP y
  variable de entorno degradada a vía de transición.
- **Implementado:** logs estructurados con redacción por construcción y
  correlation IDs (`logging.rs`). El registro solo admite recuentos, banderas,
  identificadores, códigos controlados y duraciones; no existe un campo de texto
  libre, de modo que un prompt, una ruta o un token no pueden escribirse ni por
  error. Detalle y evidencias en [Endurecimiento de Fase 0](PHASE_0_HARDENING.md).
- **Implementado:** confirmaciones durables (`confirmation_requests`) resueltas
  como `allowed_once`/`cancelled` antes de ejecutar, y carpetas autorizadas
  (`authorized_folders`) que gobiernan toda escritura de exportación.
- Feature flags locales.

### 0E. Calidad y distribución

- **Implementado:** unitarias Rust/TypeScript, integración SQLite e integración
  contra un Broker AI simulado que cubre envío, reintento idempotente, sondeo,
  espera de herramientas, cancelación, recuperación tras reinicio, diagnóstico
  y la ingesta completa de adjuntos —subida multipart, conversión, fragmentación
  y sus modos de fallo—. Además, una verificación estática del contrato entre
  `platform.ts` y las órdenes de Tauri, y de que ninguna acción sensible envía
  una confirmación que la persona no dio. Y pruebas de interfaz que montan la
  aplicación con `jsdom` y comprueban que las acciones sensibles no se ejecutan
  al cancelar. **Pendiente:** E2E sobre la aplicación empaquetada.
- **Implementado:** cobertura medida con `cargo-llvm-cov` (~78,8 % de líneas,
  con `task_runtime.rs` en ~81,5 %, `broker/mod.rs` en ~87,8 % y
  `attachment_runtime.rs` en ~82,6 %) y CI en `windows-latest` con umbral 77 que
  falla si baja. Desglose en
  [Endurecimiento de Fase 0](PHASE_0_HARDENING.md).
- **Implementado:** presupuestos de rendimiento instrumentados y visibles en
  **Inicio → Rendimiento** (migración `0017`, módulo `metrics.rs`). Miden
  arranque, apertura de conversación, búsqueda y respuesta de la interfaz.
- MSI/NSIS, firma, actualización y rollback.
- Matriz de Windows soportada.

## 7. Plan resumido de Fases 1–4

### Fase 1

Chat multi-turno, historial, proyectos, adjuntos, citas y herramientas. Primer
recorrido: crear conversación → persistir mensaje y snapshot → crear tarea →
polling → resultado → reinicio. Después archivos, búsqueda, sandbox y exportación
Markdown.

El recorrido base de conversación, mensajes, snapshot, polling y resultado ya
está implementado. El primer corte de Fase 1 añade búsqueda, proyectos,
renombrado, archivado y borrado lógico con auditoría. Una conversación con
tarea activa no puede ocultarse y las tareas pendientes vuelven a enlazarse a
la interfaz al reabrir el chat. Siguen pendientes adjuntos, citas, herramientas,
exportación Markdown y recuperación E2E cerrando el proceso real.

### Fase 2

Memoria visible y opt-in, embeddings, recuperación semántica durable y resúmenes
jerárquicos y documentos largos. Toda recuperación conserva procedencia, razón,
score y acceso a la fuente original.

El primer corte del exportador de bóveda proyecta cada conversación a una nota
Obsidian con frontmatter YAML, `chatygpt_id` estable, enlace al proyecto y
enlaces relativos a fuentes documentales. Los adjuntos se copian bajo nombres
estables y se verifican por SHA-256; una reexportación reutiliza copias idénticas
y exige confirmación ante cambios externos. La base SQLite no entra en la
bóveda.

La edición de una memoria actualiza contenido y ámbito en una transacción
auditada. Un cambio exclusivamente de categoría, sensibilidad o proyecto
conserva el embedding porque el espacio semántico no cambia. Un cambio textual
elimina el vector anterior y programa una nueva indexación; al materializar un
embedding se compara su SHA-256 con el texto vigente para descartar resultados
tardíos de versiones antiguas.

Los documentos convertidos por Broker se guardan como fragmentos locales
inmutables identificados por adjunto y ordinal. El fragmentador limita cada unidad
a 4.000 caracteres y busca primero un final de párrafo, frase o palabra después del
70 % del límite. La selección léxica prioriza las coincidencias y completa el
resultado con fragmentos vecinos para conservar encabezados y contexto inmediato;
se limita a ocho fragmentos y 24.000 caracteres por turno. El snapshot conserva texto,
posición, puntuación y motivo; el request omite el `broker_file` completo cuando
ya dispone de fragmentos, evitando duplicar contexto.

`project_files` funciona como biblioteca explícita de contexto compartido. Solo
puede guardar un adjunto ya vinculado a un chat del proyecto y solo puede
reutilizarse en conversaciones activas del mismo proyecto. La interfaz no lo
inyecta automáticamente: **Usar en este chat** crea la relación durable con la
conversación y lo activa para el siguiente turno. Retirarlo de la biblioteca no
rompe los chats que ya lo utilizan.

Las instrucciones del proyecto se guardan directamente en `projects` y se
capturan por valor al preparar cada tarea. En un flujo semántico, esa copia
también queda en `semantic_chat_workflows`, de modo que una edición posterior no
altera una petición pendiente. El prompt las delimita como instrucciones
configuradas por el usuario y `context_sources` las presenta como una fuente
propia, independiente de la memoria personal.

`ProjectKnowledgeOverview` es una composición de lectura, no una nueva fuente de
verdad. Reutiliza `ProjectSummary`, resuelve los adjuntos actuales de
`project_files` como `AttachmentView` y filtra `MemoryOverview` por el ámbito del
proyecto. La interfaz recibe así estados actuales de ingesta, fragmentación,
activación y sensibilidad sin sincronizaciones adicionales.

Los controles de mantenimiento de esa vista vuelven a validar el ámbito en
SQLite. `remove_project_file` borra únicamente la relación de biblioteca y
conserva `conversation_attachments`; `set_project_memory_item_enabled` exige que
el recuerdo pertenezca al proyecto recibido. Ambas operaciones son reversibles o
no destructivas y escriben eventos de auditoría sin incluir el contenido.

La navegación documental de la vista se deriva también en SQLite. Cada elemento
de `file_usages` contiene únicamente el identificador y título de las
conversaciones activas del mismo proyecto que aparecen en
`conversation_attachments`. La interfaz usa esos identificadores para reutilizar
el flujo normal de apertura del chat; no recibe rutas locales ni intenta inferir
el uso a partir del historial textual.

La búsqueda de la vista de conocimiento es una proyección efímera en React. La
función pura `filterProjectKnowledge` normaliza mayúsculas y diacríticos y filtra
los `AttachmentView` y `MemoryItemView` ya cargados. Cambiar la consulta o el
tipo visible no consulta al Broker, no escribe en SQLite y no modifica el
conjunto sobre el que actúan los controles persistentes.

La indexación semántica reutiliza `embedding_records` con
`source_type=attachment_chunk`. El planificador crea como máximo una tarea local
activa por adjunto y encadena el siguiente fragmento al alcanzar un estado
terminal. La idempotency key contiene el identificador y SHA-256 del fragmento;
un resultado tardío solo se materializa si ese hash sigue vigente. Al arrancar se
recuperan primero las tareas no terminales y después se continúa cada índice
incompleto.

Cuando un turno usa adjuntos con vectores, la consulta se vectoriza mediante el
workflow durable ya usado por la memoria. La selección híbrida pondera un 65 % la
similitud coseno y un 35 % la coincidencia léxica, exige modelo y dimensiones
compatibles y conserva el fallback léxico. El uso de recuerdos sigue siendo
opt-in: una consulta iniciada solo por documentos usa `chat_document_search` y no
recupera memoria personal.

El inspector entrega al frontend una referencia opaca por fragmento, nunca la
ruta local. Al solicitar **Mostrar archivo**, el backend vuelve a comprobar la
relación `tarea → snapshot → fuente → fragmento → adjunto`, canonicaliza la ruta
y exige que permanezca dentro del almacenamiento administrado de ChatyGPT.
Windows selecciona la copia en el Explorador, pero la aplicación no la ejecuta.
Si fue eliminada, el inspector conserva el extracto histórico y muestra que el
archivo local ya no está disponible.

El adjunto conserva dos máquinas de estado independientes: `ingestion_status`
describe su disponibilidad en Broker y `context_status` (`pending`,
`preparing`, `ready`, `unavailable` o `failed`) describe la preparación local.
El recuento de fragmentos y la suma de caracteres consultables se derivan de
SQLite, sin duplicar metadatos. La interfaz presenta ambos y una estimación
explícita de tokens, además de los fragmentos semánticos preparados y el modelo.
Un fallo local no invalida la subida ni la búsqueda léxica y puede reintentarse
usando el `broker_file_id` ya persistido.

El resumen conserva cada revisión en SQLite y separa los
estados `generating`, `draft` y `approved`. La ventana reciente solo sustituye
mensajes cubiertos por una revisión aprobada; el historial original permanece
intacto y el snapshot de cada tarea registra el resumen como fuente diferenciada.
Para historiales que exceden una petición, cada generación consolida el resumen
aprobado con el siguiente lote cronológico, limitado a 48.000 caracteres. La
secuencia cubierta avanza únicamente al aprobar el nuevo borrador y los mensajes
posteriores siguen entrando como ventana reciente.

### Fase 3

GPTs personalizados versionados, editor guiado, importación/exportación y
matriz de permisos realmente aplicada antes de ejecutar herramientas.

Los dos primeros cortes usan las tablas fundacionales `custom_gpts` y `gpt_versions`.
`custom_gpts.active_version_id` señala la revisión visible, mientras que una
edición inserta una configuración JSON nueva con `version_no` creciente y
conserva todas las filas anteriores. La configuración inicial declara
explícitamente `toolsEnabled=false`; no crea filas en `gpt_tool_permissions` y
los eventos de auditoría contienen solo el ID y el número de versión, nunca las
instrucciones.

Cada conversación puede seleccionar de forma reversible un `custom_gpt_id`.
Al preparar un turno, ChatyGPT resuelve una sola vez su versión activa y copia
por valor ID, nombre, número e instrucciones. La tarea final guarda el
`gpt_version_id`, el prompt contiene un bloque explícito de instrucciones y el
snapshot lo materializa como fuente `custom_gpt`. En los turnos con recuperación
semántica, esa copia se guarda antes de solicitar el embedding y se reutiliza al
crear la tarea de chat: editar el GPT durante la búsqueda no cambia el turno.
La configuración del GPT no activa herramientas automáticamente. Cada versión
materializa una matriz en `gpt_tool_permissions` para `run_code` y
`rename_conversation`, con efectos `deny` o `confirm`. La ausencia de una fila
equivale a denegación. `confirm` solo permite ofrecer la capacidad: Código
aislado conserva el consentimiento de un turno y Renombrar conversación conserva
la aprobación individual de la llamada.

La matriz se copia dentro del snapshot del GPT al preparar la tarea. ChatyGPT la
comprueba al construir la petición y de nuevo antes de ejecutar una herramienta
devuelta por el Broker. Una edición posterior no puede ampliar los permisos de
una tarea antigua.

La misma configuración versionada conserva hasta seis iniciadores de
conversación. Son ayudas de interfaz: al pulsarlos solo rellenan el compositor,
sin enviar ni ejecutar nada automáticamente. La exportación portable básica usa
`schemaVersion=1` y contiene únicamente nombre, descripción, instrucciones e
iniciadores. Una segunda acción deliberada usa `schemaVersion=2` y añade solo
conocimiento textual habilitado y clasificado como normal. Ambos formatos
excluyen IDs, permisos, herramientas y archivos; el enriquecido también cuenta
y comunica los elementos sensibles, desactivados y documentales que dejó fuera.
La importación rechaza campos desconocidos, archivos mayores de 256 KB y
versiones no compatibles; siempre crea un GPT local nuevo, con sus capacidades
denegadas y todo conocimiento recibido desactivado hasta la revisión humana.

El conocimiento textual propio de un GPT reutiliza `memory_items.custom_gpt_id`,
pero no forma parte de `memory_overview`: la memoria global y de proyecto sigue
mostrando únicamente filas con `custom_gpt_id IS NULL`. Al preparar un turno se
resuelve el GPT seleccionado y se anteponen solo sus elementos habilitados. Este
ámbito explícito funciona aunque el interruptor de memoria general esté apagado.
La recuperación semántica aplica la misma separación en SQL y el snapshot
durable etiqueta cada fuente como `Conocimiento GPT`, incluyendo el nombre de su
propietario. Así, tanto el prompt como el inspector pueden demostrar qué dato se
usó y por qué, sin confiar en una inferencia de la interfaz.

Los archivos privados usan `custom_gpt_files(custom_gpt_id, attachment_id)` y
mantienen una sola copia deduplicada en `attachments`. No se insertan en
`conversation_attachments`: al preparar cada turno se resuelven los archivos
`ready` del GPT que la conversación tenga seleccionado en ese instante. La misma
autorización se aplica a la carga del registro, la selección léxica/semántica de
fragmentos y la persistencia en `message_attachments`. Cambiar de GPT o retirar
un archivo modifica el siguiente conjunto efectivo sin alterar las fuentes
históricas de respuestas anteriores. El snapshot guarda además una razón
específica para que el inspector identifique el origen como conocimiento del GPT.

### Fase 4

Deep Research como workflow durable, captura/webcam y scheduler local con
claim keys, zonas horarias, historial, confirmación previa y notificaciones.

El primer corte activa Investigación profunda únicamente para el siguiente
turno. ChatyGPT valida que Broker AI anuncie `agent`, `web_search` y `fetch_url`
antes de persistir el mensaje. La petición ordena planificar, realizar y
contrastar búsquedas múltiples y sintetizar un informe con citas; las
herramientas auxiliares solo se incluyen si el Broker las anuncia. La estrategia
`agent` usa siempre `preset=fast`, que es el único preset que admite el contrato
actual; la profundidad procede del máximo de iteraciones y del propio plan, no
de un preset de `mixture_of_agents`.

`research_runs` conserva objetivo, conversación y `broker_task_id`, mientras
`research_steps` materializa planificación, investigación y síntesis. Las fases
remotas actualizan esas etapas dentro de la misma transacción que actualiza la
tarea. Por ello, la ficha de progreso, la cancelación y la recuperación tras
reinicio describen el mismo workflow durable y no crean un segundo trabajo.

Al completar una investigación, los enlaces HTTP(S) que aparecen realmente en
el Markdown final se normalizan y materializan en `citations`. Se eliminan
duplicados y fragmentos, y se rechazan URLs con credenciales, protocolos no web
o tamaños desproporcionados. No se infieren búsquedas desde estados técnicos:
si el Broker no expone una invocación con contrato estable, ChatyGPT no fabrica
un paso. Las fuentes persistidas se muestran bajo la respuesta y se conservan en
la exportación Markdown/Obsidian existente.

La interfaz conserva `message.text` como Markdown canónico y aplica el formato
solo durante el renderizado de los mensajes del asistente. `MarkdownContent`
construye elementos React sin usar HTML sin procesar: los títulos, listas,
tablas, citas y bloques de código son visuales, mientras que el HTML incluido en
una respuesta se muestra como texto y solo los enlaces HTTP(S) sin credenciales
se convierten en enlaces externos. La persistencia, el contexto enviado al
Broker y las exportaciones siguen utilizando el Markdown original.

La captura de pantalla se inicia únicamente desde un gesto explícito del usuario
mediante `getDisplayMedia`, por lo que WebView2 presenta su selector de pantalla
o ventana. Se toma un único fotograma, se limita a 2.560 píxeles y cuatro
megapíxeles, se codifica como JPEG y se detienen todas las pistas antes de mostrar
la vista previa. Nada se persiste ni se envía hasta pulsar **Adjuntar captura**.
Rust vuelve a validar tamaño y firma JPEG/PNG, calcula la huella, deduplica en el
almacenamiento administrado y reutiliza la ingesta normal de adjuntos. No se
conserva un permiso permanente de captura.

La vista previa permite delimitar con el puntero la zona que se desea conservar.
El recorte se ejecuta en memoria sobre un lienzo local, reemplaza la vista previa
y no crea ningún archivo ni adjunto hasta la confirmación del usuario. Se usa
esta alternativa portable porque el protocolo moderno `ms-screenclip` solo
puede devolver el resultado a una aplicación con identidad MSIX y URI de
redirección registrada; el lanzador de desarrollo abre deliberadamente el
ejecutable Tauri sin empaquetar. La integración con la Herramienta Recortes
nativa queda condicionada a incorporar y verificar una distribución MSIX.

La webcam sigue el mismo límite de consentimiento: `getUserMedia` solo se invoca
desde **Usar cámara**, solicita vídeo sin audio y mantiene el `MediaStream`
exclusivamente mientras la vista previa en vivo está abierta. Un indicador
visible señala ese estado. Fotografiar, cancelar, navegar a otra conversación o
cerrar la aplicación detiene todas las pistas. La imagen resultante usa la misma
compresión, revisión local, confirmación, validación Rust e ingesta que una
captura de pantalla; ChatyGPT no almacena vídeo ni una autorización propia de
cámara.

El primer corte del scheduler usa las relaciones `scheduled_tasks` y
`scheduled_runs` creadas en el esquema inicial. La interfaz convierte la hora
local elegida a UTC y conserva además la zona IANA que vio el usuario. Una tarea
solo se activa tras confirmación explícita. Al vencer, una transacción inmediata
crea un `scheduled_run` con `claim_key` única y desactiva la programación de una
sola ejecución antes de contactar con Broker AI. El runtime reutiliza
`start_chat_turn`, por lo que mensajes, contexto, idempotencia remota, polling y
recuperación siguen el mismo camino que un envío manual.

El historial reconcilia el estado terminal del `broker_task` sin copiar respuestas
al margen de la conversación. Si la aplicación está cerrada a la hora prevista,
la programación vencida se reclama al siguiente arranque. Este corte no instala
un servicio de Windows ni concede herramientas a la ejecución automática.

Las recurrencias `daily` y `weekly` avanzan `next_run_at` dentro de la misma
transacción que inserta la claim. El cálculo pasa por la zona local del sistema
antes de añadir el día o la semana y vuelve a UTC, de modo que conserva la hora
de pared cuando Windows cambia entre horario estándar y horario de verano. Si
se omitieron varias fechas mientras la app estaba cerrada, se crea una sola
ejecución atrasada y la siguiente fecha salta directamente al futuro.

Editar una programación conserva sus ejecuciones anteriores, reemplaza el
payload futuro y exige una confirmación nueva. No se permite editar o eliminar
mientras una ejecución está reclamada o activa. Los avisos usan el permiso
estándar de notificaciones expuesto por WebView2; son una proyección efímera de
una transición terminal ya persistida. Si Windows o WebView2 los deniegan, el
historial visible continúa siendo la fuente de verdad.

El reintento de una ejecución fallida inserta otro `scheduled_run` con una claim
única y un número de intento creciente. No reactiva ni desplaza la próxima fecha
de una recurrencia, no modifica el registro fallido y rechaza el reintento cuando
ya existe una ejecución reclamada o activa para la misma programación. La bandeja
interna se deriva de los estados terminales persistidos; solo la marca de lectura
es una preferencia local de interfaz y puede reconstruirse sin pérdida de dominio.

Cancelar una ejecución programada reutiliza `task_runtime::cancel_task`: primero
solicita la cancelación del `broker_task` remoto y persiste su estado local; solo
después marca el `scheduled_run` como cancelado y audita la acción. La operación
se ofrece únicamente cuando el run está `running` y tiene tarea local asociada,
por lo que una respuesta fallida del Broker no crea una cancelación ficticia. En
recurrencias, cancelar el intento actual no cambia `enabled` ni `next_run_at`.
Los filtros de historial son una proyección pura de los diez runs recientes que
acompañan a cada programación y nunca mutan ni eliminan registros.

El detalle visible extrae únicamente campos textuales conocidos del resultado
persistido (`message`, `result_markdown`, `text`, `detail` o `error.message`) y
ofrece una explicación segura cuando el Broker no devolvió detalle. La
exportación consulta todo el historial que coincide con los filtros —no solo los
diez runs de la tarjeta—, genera texto UTF-8, limita cada detalle a 4.000
caracteres, escribe de forma atómica, verifica SHA-256 y registra el destino,
hash, filtros y número de ejecuciones en auditoría.

La búsqueda de programaciones se resuelve como una proyección pura en el cliente
sobre nombre, título de conversación e instrucción; normaliza mayúsculas y
acentos, y no altera la consulta durable del historial. Las plantillas viven en
`scheduled_task_templates` (migración `0015`) y almacenan exclusivamente nombre,
prompt y recurrencia. No guardan conversación, fecha, zona horaria, estado
activo ni confirmación. Aplicar una plantilla rellena el borrador del formulario
y fuerza `confirmed=false`, de modo que reutilización y autorización permanecen
separadas. Crear y eliminar plantillas queda registrado en auditoría.

Duplicar una programación es una transformación de interfaz: copia nombre,
conversación, instrucción y recurrencia a un borrador, propone una fecha futura
nueva y fuerza `confirmed=false`; no crea registros hasta la confirmación final.
**Ejecutar ahora** sí crea un `scheduled_run` durable con claim única dentro de
una transacción inmediata, pero no actualiza `scheduled_tasks.enabled`,
`next_run_at` ni `last_claim_key`. La misma transacción rechaza la operación si
ya existe un run `claimed` o `running`, y el envío posterior reutiliza
`start_chat_turn` y la reconciliación ordinaria del scheduler.

Las tarjetas conservan diez runs recientes para refresco y avisos ligeros. El
historial completo usa `scheduled_run_page`, una consulta SQLite separada con
filtros de estado y periodo, orden validado, `LIMIT/OFFSET` y recuento total. El
cliente solicita páginas de 10, 25 o 50 registros y vuelve a la primera página
cuando cambia un filtro, el orden o el tamaño. La consulta limita páginas fuera
de rango a la última disponible y se actualiza mientras el panel está abierto,
sin convertir la lista principal en una carga no acotada.

La agenda de próximas automatizaciones es una proyección pura del cliente sobre
`scheduled_tasks.next_run_at`. Solo incluye tareas activas: conserva como
autoridad la primera fecha persistida y deriva las repeticiones diarias o
semanales hasta un horizonte máximo de 30 días. La interfaz etiqueta esas fechas
derivadas como proyecciones, muestra una única ejecución atrasada por tarea y
detecta conflictos únicamente entre tareas distintas separadas por 15 minutos o
menos. Abrir, cambiar el periodo o navegar a una conversación no reclama runs,
no modifica la recurrencia y no requiere una consulta adicional al Broker.

La exportación iCalendar recibe exclusivamente la proyección visible ya acotada
y vuelve a validar en Rust cantidad, campos, fechas UTC y extensión `.ics`.
Cada evento usa un UID derivado mediante SHA-256 para no publicar identificadores
internos. El contenido omite deliberadamente prompt, resultado, modelo y contexto;
las fechas se etiquetan como `DURABLE`, `PROJECTED` u `OVERDUE`. Las líneas se
escapan y pliegan según iCalendar, se escriben con CRLF mediante reemplazo atómico,
se verifica el hash final y la exportación queda auditada sin convertir el archivo
en fuente de verdad.

El inicio con la sesión de Windows es una capacidad explícita y reversible del
scheduler, no un servicio. `startup.rs` registra una orden en
`HKCU\Software\Microsoft\Windows\CurrentVersion\Run`, por lo que afecta solo al
usuario actual y no requiere elevación. Antes de registrarla exige confirmación
y cifra el token activo con DPAPI `CurrentUser`; el secreto no aparece en el
script PowerShell, la base SQLite, el frontend ni la auditoría. El script
descifra la credencial en memoria, espera hasta que `/api/v1/capabilities`
responda con autenticación válida, comprueba que no exista otra instancia y solo
entonces abre ChatyGPT con el entorno necesario. Al arrancar manualmente desde
el BAT, Rust renueva el blob DPAPI si la opción continúa activa, cubriendo una
rotación del token sin introducir otro almacén de configuración sensible.

La apariencia es una preferencia de presentación y no forma parte del dominio
SQLite. `appearance.ts` valida únicamente `system`, `light` o `dark`, persiste el
valor bajo una clave versionada de `localStorage` y proyecta el resultado sobre
`data-theme` en el elemento raíz. Un script mínimo en `index.html` ejecuta esa
misma resolución antes de cargar el bundle de React, por lo que la primera
pintura ya usa el tema correcto. Cuando la opción es `system`, la aplicación
escucha `prefers-color-scheme` y actualiza también `color-scheme` y el color de
la ventana sin reinicio. Un valor ausente o ilegible degrada de forma segura a
la configuración de Windows.

La navegación de teclado se resuelve en una función pura de `keyboard.ts` que
traduce eventos válidos a acciones de aplicación. Rechaza composición IME,
ventanas superpuestas y teclas simples cuando el destino es editable, evitando
interferir con el compositor. `App.tsx` mantiene referencias únicamente a los
destinos visibles —búsqueda y mensaje— y publica los atajos con
`aria-keyshortcuts` y una ayuda accesible. El documento usa landmarks separados
para navegación y contenido, incorpora un enlace de salto y gestiona las
ventanas con foco inicial, ciclo de Tab, cierre mediante Escape y restauración
del foco previo. Ninguna de estas acciones modifica la persistencia o el
contrato del Broker.

Investigación profunda y recuperación semántica ya no se excluyen. Lo que
faltaba no era código sino una política de recuperación para dos workflows
anidados, y esa política es un plan congelado: `deep_research_plan` valida las
capacidades del Broker y decide las herramientas **antes** de persistir el
mensaje, y el resultado se guarda en `semantic_chat_workflows.research_plan_json`
(migración `0018`). La segunda etapa y una recuperación tras reinicio aplican ese
plan con una función pura, sin red. Así, un Broker que retire una herramienta
mientras corre la etapa de embeddings no puede alterar una investigación que la
persona ya autorizó, y un fallo de capacidades se manifiesta como rechazo del
turno en vez de como un mensaje a medias en SQLite.

El expediente de investigación lo abre `insert_research_run_if_needed`, que
decide leyendo la petición ya construida en lugar de un parámetro aparte. Por
eso las dos rutas —directa y semántica— crean las mismas tres etapas y el mismo
evento `research.started`, y no puede existir una petición `deep_research` sin
su expediente asociado. El contexto recuperado por similitud no se descarta: los
recuerdos y fragmentos seleccionados forman parte del objetivo que se investiga.

La medición de rendimiento vive en `performance_samples` (migración `0017`) y en
`metrics.rs`. La tabla admite exclusivamente una métrica de un vocabulario
cerrado por CHECK, un entero de milisegundos acotado a diez minutos y una marca
de tiempo: no tiene ninguna columna capaz de contener un prompt, un título, una
ruta ni un identificador de dominio, de modo que medir no crea un segundo
registro de contenido personal. La retención se aplica dentro de la misma
transacción que inserta —las últimas 200 muestras por métrica—, así que no
existe un instante en el que la tabla supere el límite ni una tarea de
mantenimiento que pudiera no ejecutarse.

Los objetivos se comparan siempre con el percentil 95 calculado por rango más
cercano, que devuelve un valor realmente observado en lugar de interpolar uno
que nunca ocurrió. `meets_budget` es `Option<bool>`: una métrica sin muestras no
obtiene veredicto, ni cumplido ni incumplido. Los presupuestos iniciales
adoptados son 2.000 ms para el arranque, 400 ms para abrir una conversación,
300 ms para la búsqueda y 100 ms para la respuesta de la interfaz, y viven en un
único punto del código.

La instrumentación del cliente acumula duraciones en un búfer acotado y las
envía por lotes cada cinco segundos, de modo que medir no se convierte en un
coste de rendimiento. El arranque se cuenta desde que la vista web empieza a
cargar hasta que hay navegación y primera conversación en pantalla: no incluye
la creación del proceso ni de WebView2, que el frontend no puede observar. La
respuesta de la interfaz procede de la API de Event Timing filtrada por
`interactionId`, cuyo umbral mínimo de 16 ms deja fuera las interacciones más
rápidas; los percentiles resultantes son por tanto un límite superior y nunca
una cifra optimista. Ambas limitaciones se declaran junto a la medida en la
propia interfaz.

## 8. Riesgos técnicos principales

| Riesgo | Mitigación |
|---|---|
| Diferencias entre sandbox y perfil Windows real | Compilación automática + prueba manual de ventana desde el `.bat` |
| Contrato dinámico no contrastado en vivo | Bloquear cierre de 0B hasta capturar OpenAPI y fixtures reales |
| Doble creación tras crash | Persistencia previa + clave idempotente estable + reconciliación |
| Polling duplicado | Lease en SQLite con expiración y propietario |
| SQLite dentro de Drive | Ruta fija AppLocalData; solo exportaciones van al vault |
| Secretos en logs/DB | Puertos de secreto aislados, redacción y tests negativos |
| Sidecar Python huérfano | Ciclo de vida propiedad de Rust, heartbeat y shutdown acotado |
| Contexto creciente | Ventana + resumen + recuperación; snapshot exacto por tarea |
| Tool calling sensible | Confirmación persistida antes de ejecutar; deny por defecto |
| Cancelación tardía | Estado `cancel_requested` local futuro y polling hasta terminal |

## 9. Decisiones y suposiciones pendientes

1. El arranque automático usa DPAPI `CurrentUser`. Sigue pendiente decidir si
   futuras credenciales editables desde la interfaz usarán Credential Manager
   nativo o Stronghold.
2. Confirmar si AI Broker siempre será loopback o también LAN/TLS.
3. ~~Obtener el OpenAPI vivo y comprobar si el endpoint expone eventos de tarea
   o solo el snapshot agregado.~~ **Cerrada el 6-ago-2026.** El endpoint de
   cliente expone **solo el snapshot agregado**, y es deliberado: el detalle por
   paso llega por el *passthrough* de herramientas, no ampliando el API de
   cliente. Cuando el agente se pausa, `result.pending_tool_calls` trae cada
   llamada con su identificador, su nombre y sus argumentos ya deserializados
   —que es el detalle por subtarea— y `progress.agent_iteration` dice por qué
   vuelta del bucle va. El contrato queda fijado en
   `contracts/broker/2.7/task-state.response.json`, copia literal del esquema
   que publica el Broker, y validado por
   `tests/test_broker_task_state_contract.py`.
4. Confirmar modelos mínimos disponibles para el smoke test sin coste cloud.
5. Definir ubicación del vault y política de conflicto.
6. Definir política de retención/borrado físico.
7. Decidir si las actualizaciones serán firmadas y desde qué canal.

## 10. Criterios de aceptación de Fase 0

- Tauri inicia en Windows sin consola auxiliar.
- SQLite se crea fuera de carpetas sincronizadas.
- Migraciones son atómicas, repetibles y pasan `integrity_check` y
  `foreign_key_check`.
- Un token nunca se persiste ni aparece en logs.
- AI Broker se diagnostica mediante health + capabilities.
- Una tarea de prueba se persiste antes de enviarse.
- La misma operación reintentada no duplica la tarea.
- Polling no bloquea UI, aplica límites y termina en estados terminales.
- Un reinicio recupera tareas activas sin pérdida.
- Cancelación refleja la respuesta real del Broker.
- Existe evidencia automática y manual de arranque, cierre y reapertura.
- MSI/NSIS instala, inicia y desinstala correctamente.

## 11. Primer slice vertical

El slice implementado cubre:

1. inicio Tauri;
2. resolución de AppLocalData;
3. apertura y migración SQLite;
4. marcado de tareas activas como `recovery_pending`;
5. render de estado local;
6. diagnóstico manual de `/health/ready` y `/api/v1/capabilities`.
7. persistencia previa al `POST`;
8. almacenamiento del identificador remoto;
9. polling adaptable en segundo plano;
10. estados y resultado leídos desde SQLite;
11. cancelación explícita;
12. recuperación al arranque sin reintentar errores contractuales huérfanos.
13. creación y reapertura de conversaciones persistentes;
14. commit atómico del mensaje de usuario, respuesta pendiente, tarea y contexto;
15. envío del turno en segundo plano con reintento idempotente;
16. materialización única de la respuesta terminal como mensaje asistente.

La app no crea inferencia automáticamente: tanto la prueba durable como el envío
de un mensaje requieren una acción explícita de la persona usuaria.
