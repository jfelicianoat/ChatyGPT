# Arquitectura y plan de producto

Fecha de auditoría: 2026-07-19.

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
| Contrato | Revisado estáticamente | `/api/v1/capabilities` declara versión `2.5` |
| Crear tarea | Revisado estáticamente | `POST /api/v1/tasks`, 202 o 200 por idempotencia |
| Consultar tarea | Revisado estáticamente | `GET /api/v1/tasks/{task_id}` |
| Cancelar | Revisado estáticamente | `DELETE /api/v1/tasks/{task_id}` |
| Reanudar tools | Revisado estáticamente | `POST /api/v1/tasks/{task_id}/tool_results` |
| Estados | Revisado estáticamente | 15 estados; terminales `completed`, `failed`, `cancelled` |
| Ingesta | Revisado estáticamente | `POST /api/v1/files`, polling y Markdown |
| Modelos/capacidades | Revisado estáticamente | endpoints `/models`, `/models/availability`, `/models/context`, `/capabilities` |
| Embeddings | Revisado estáticamente | `inference_kind=embedding`, estrategia `single`, salida JSON |
| Autenticación | Revisado estáticamente | cabecera `x-admin-token` cuando hay token configurado |
| Idempotencia | Revisado estáticamente | `idempotency_key` + hash; conflicto HTTP 409 |
| Sandbox | Revisado estáticamente | `run_code` opt-in y `SANDBOX_DISABLED` si no está habilitado |
| OpenAPI real | Verificado manualmente (alcance) | endpoint consultado por el probe en A9 |
| Integración real | Verificado manualmente | contrato 2.5, 73 modelos, HTTP 202/200 idempotente y tarea completada |

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
  ├─ adaptador AI Broker 2.5
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
│  └─ broker/2.5/                # fixtures contractuales trazables
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

- Backend definitivo de secretos y rotación.
- Logs estructurados con redacción y correlation IDs.
- Confirmaciones y carpetas autorizadas.
- Feature flags locales.

### 0E. Calidad y distribución

- Unitarias Rust/TypeScript, integración SQLite/Broker y E2E.
- Presupuestos de rendimiento instrumentados.
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

Memoria visible y opt-in, embeddings, recuperación semántica, resúmenes
jerárquicos y documentos largos. Toda recuperación conserva procedencia, razón,
score y acceso a la fuente original.

### Fase 3

GPTs personalizados versionados, editor guiado, importación/exportación y
matriz de permisos realmente aplicada antes de ejecutar herramientas.

### Fase 4

Deep Research como workflow durable, captura/webcam y scheduler local con
claim keys, zonas horarias, historial, confirmación previa y notificaciones.

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

1. Elegir Credential Manager nativo o Stronghold. Se recomienda Credential
   Manager para una app Windows personal sin contraseña maestra adicional.
2. Confirmar si AI Broker siempre será loopback o también LAN/TLS.
3. Obtener el OpenAPI vivo y comprobar si el endpoint expone eventos de tarea o
   solo el snapshot agregado.
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
