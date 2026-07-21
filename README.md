# ChatyGPT

Aplicación de escritorio Windows, local-first, para conversar mediante AI Broker
sin acoplar la interfaz a su API HTTP.

## Estado

Fase 1 en curso. La base durable y el primer corte de organización local incluyen:

- shell Tauri 2 + React + TypeScript;
- SQLite local con migración inicial y recuperación de tareas activas;
- adaptador tipado de AI Broker 2.5;
- diagnóstico de salud y capacidades sin crear inferencias;
- recorrido durable opcional: persistir, enviar, sondear, cancelar y recuperar;
- conversaciones y mensajes persistentes con compositor multi-turno;
- snapshot trazable de la ventana de contexto utilizada por cada turno;
- materialización idempotente del resultado remoto como mensaje asistente;
- creación, renombrado, archivado y eliminación lógica de conversaciones;
- proyectos locales con asociación reversible de conversaciones;
- búsqueda por título y contenido de mensajes;
- confirmaciones explícitas y auditoría para operaciones de ciclo de vida;
- recuperación visual de una tarea pendiente al reabrir su conversación;
- adjuntos reutilizables con copia local administrada, SHA-256 y deduplicación;
- selección nativa y arrastre de archivos, subida en streaming y seguimiento de ingesta;
- envío al chat solo cuando Broker AI confirma el `file_id` como `ready`;
- fuentes documentales trazables bajo cada respuesta, derivadas de los adjuntos enviados;
- modo de herramientas opt-in con confirmación individual antes de cualquier acción local;
- primera herramienta de cliente: renombrar la conversación solo tras autorización visible;
- decisiones y resultados de herramientas persistidos antes de reanudar AI Broker;
- exportación Markdown mediante diálogo nativo, con fuentes documentales y sin rutas internas;
- escritura atómica, comprobación SHA-256, detección de cambios externos y auditoría del exportado;
- ejecución opcional de Python en el sandbox desechable de Broker AI, habilitada para un solo turno;
- comprobación redundante de la capacidad `sandbox_run_code` antes de persistir y enviar la tarea;
- inspector de actividad reciente con descripciones legibles y severidad visual;
- proyección segura de auditoría que excluye prompts, tokens, rutas, hashes y JSON técnico;
- aviso global de recuperación al reiniciar, con recuento de tareas y adjuntos reanudados;
- acceso directo desde el aviso a cada conversación recuperada;
- fixture contractual local-only y sin coste cloud;
- pruebas ejecutables con la biblioteca estándar de Python.

El recorrido de conversación sigue
`persistir turno y contexto → crear tarea → sondear → materializar respuesta`.
La petición HTTP se realiza en segundo plano después del commit local y se
reintenta con la misma clave idempotente ante errores transitorios.

## Desarrollo

El entorno Windows auditado ya dispone de Rust estable y de las dependencias
JavaScript. Se han verificado TypeScript, Vite, Cargo y una construcción Tauri
de producción. Para desarrollo:

```powershell
pnpm.cmd install
pnpm.cmd typecheck
pnpm.cmd test
pnpm.cmd tauri dev
```

Las pruebas de fundamentos, que no requieren dependencias externas:

```powershell
python -m unittest discover -s tests -v
```

Verificación contractual contra una instancia real:

```powershell
python scripts\verify_broker.py --base-url http://127.0.0.1:8765
python scripts\verify_broker.py --base-url http://127.0.0.1:8765 --smoke-task
```

El segundo comando crea una tarea `single`, `local_only`, con proveedores cloud
deshabilitados y coste máximo cero. Repite el mismo POST para comprobar
idempotencia y sondea hasta estado terminal.

Configuración no secreta:

- `CHATYGPT_BROKER_BASE_URL`, por defecto `http://127.0.0.1:8765`.

Para la instancia personal verificada en `A9_Mega`, antes de iniciar Tauri:

```powershell
$env:CHATYGPT_BROKER_BASE_URL = "http://192.168.1.52:8765"
```

Dentro de la app, primero se usa **Comprobar conexión**. Cuando Broker AI está
listo, se puede crear una conversación y enviar el primer mensaje.

Una conversación se guarda desde **Exportar Markdown**. Si el archivo ya
existe, Windows solicita confirmación antes de reemplazarlo. ChatyGPT escribe
el resultado de forma atómica y verifica su huella antes de declararlo
completado.

La opción **Código aislado · un turno** solo se habilita cuando Broker AI
publica la capacidad `sandbox_run_code`. El permiso se consume tras el siguiente
envío; la ejecución sucede en el contenedor restringido del Broker, nunca en el
proceso de ChatyGPT ni con acceso a los archivos del equipo.

Secreto de transición:

- `AI_BROKER_ADMIN_TOKEN`, leído solo del entorno y enviado como
  `x-admin-token`. No se persiste ni se registra.

Antes de distribuir la aplicación se sustituirá el entorno por Windows
Credential Manager o Stronghold, tras decidir el modelo de desbloqueo.

## Documentación

- [Arquitectura y plan](docs/ARCHITECTURE.md)
- [Evidencias de Fase 0](docs/PHASE_0_VERIFICATION.md)
- [Evidencias de Fase 1](docs/PHASE_1_VERIFICATION.md)
- [Contrato local AI Broker 2.5](contracts/broker/2.5/single-task.request.json)
