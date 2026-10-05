# Estado vigente de ChatyGPT

Última revisión general contra el código: **28 de septiembre de 2026**.
Actualización System 1 y contrato del broker: **3 de octubre de 2026**.

Versión de producto: **0.4.0** (visible en la barra lateral). Esquema SQLite: **26**
(`0026`: traza System 1 en sombra). La resolución de la auditoría
del 28 de septiembre está en
[AUDIT_2026-09-28/RESOLUCION.md](AUDIT_2026-09-28/RESOLUCION.md); la anterior, en
[REMEDIATION_2026-09-13.md](REMEDIATION_2026-09-13.md).

Este documento es la referencia breve de estado. `README.md` explica el producto,
`ARCHITECTURE.md` conserva el diseño y las decisiones, y los documentos `PHASE_*` son
evidencias históricas del corte que indican en su título; no describen por sí solos el
estado actual.

## Qué es

ChatyGPT es una aplicación de escritorio Windows, local-first, construida con Tauri 2,
Rust, React 19 y TypeScript. La interfaz React presenta estado y solicita comandos; el
núcleo Rust posee SQLite, red, secretos, filesystem, permisos, recuperación y operaciones
nativas.

## Dos rutas de ejecución independientes

1. **Chat normal:** ChatyGPT persiste el turno y su contexto, crea una tarea durable en
   AI Broker, sondea su estado y materializa una única respuesta.
2. **Encargo autónomo:** ChatyGPT actúa como cliente del servicio local de Athena mediante
   HTTP autenticado y eventos SSE. Athena posee el bucle agente, herramientas, permisos,
   delegados, verificación, recuperación y resultado. Esta ruta no sustituye al chat
   normal y no convierte a Athena en proveedor de modelos.

## Persistencia y seguridad

- SQLite en `AppLocalData` es la fuente de verdad; el vault de Obsidian es una proyección.
- React no recibe tokens ni realiza HTTP directo a Broker o Athena.
- La credencial de AI Broker y la del servicio Athena son distintas y se protegen con
  DPAPI para la cuenta de Windows.
- Lectura, escritura, modificación de archivos, herramientas y tareas programadas son
  concesiones separadas, denegadas por defecto. La decisión se persiste antes del efecto
  y cada resultado queda registrado, incluso cuando la operación falla.
- Una clasificación `local_only` se hereda al reutilizar respuestas o resúmenes, y las
  descargas autenticadas del Broker nunca siguen un origen distinto.
- Las rutas se resuelven canónicamente y las escrituras sensibles usan comprobación de
  huella y reemplazo atómico.

## Compatibilidad externa

- **AI Broker:** el cliente implementa los contratos aditivos 2.10 y 2.11, incluidos artefactos,
  exclusividad y evidencia de ejecución, y mantiene compatibilidad de lectura con tareas
  anteriores. Véase
  [BROKER_COMPATIBILITY.md](BROKER_COMPATIBILITY.md).
  La selección de contexto opcional mediante System 1 está desactivada por defecto;
  usa una rúbrica de tres niveles, conserva cada candidato cuyo juicio se rechaza,
  juzga en paralelo y evalúa el modo sombra fuera del turno. Véase
  [SYSTEM1_CONTEXT.md](SYSTEM1_CONTEXT.md).
- **Athena:** wire protocol 1. La aplicación comprueba `/v1/health`, consume runs y eventos,
  resuelve aprobaciones y puede consultar `/v1/profiles`, `/v1/models` y memoria cuando el
  despliegue los ofrece. Un catálogo de modelos ausente significa que el despliegue usa un
  modelo fijo; no es un error.

## Capacidades principales implementadas

- conversaciones, proyectos, búsqueda, adjuntos, contexto documental y memoria opt-in;
- GPTs personales versionados, permisos, conocimiento privado, importación y exportación;
- investigación profunda, fuentes trazables y exportación Markdown/Obsidian;
- automatizaciones locales durables, calendario proyectado e inicio con Windows;
- recurrencias calculadas en la zona IANA guardada y resúmenes capaces de avanzar por
  fragmentos trazables de mensajes que superan el lote;
- ficheros generados accesibles desde cada respuesta, descargados con límite y SHA-256;
- captura de pantalla y webcam iniciada por la persona;
- sandbox de Broker por turno y herramientas locales siempre confirmadas;
- área Athena con historial, estado, permisos, revisión, modelo por run y reconexión.

## Garantías añadidas en 0.3.0

- Una única política efectiva (privacidad más estricta, gasto menor) visible antes de
  enviar; la clasificación viaja entre nodos de un flujo.
- Estados de tarea monotónicos, cancelación durable sin identidad remota y cierre visible
  de resultados que no pueden guardarse.
- Identidad estable para turnos programados y reenvíos; sin solapamientos de ejecuciones.
- Borradores por conversación y del editor de flujos que no se pierden ni se mezclan.
- Archivo y papelera recuperables, copias de seguridad verificadas y restauración al
  reiniciar, copias locales de ficheros generados.
- Conexiones de investigación clavadas a la IP validada y respuestas con límite de tamaño.
- `CHATYGPT_DATA_DIR` abre la aplicación sobre otro perfil de datos (pruebas y copias).

## Límites que deben seguir declarándose

- Una capacidad cubierta por pruebas no equivale a una validación manual en el perfil
  Windows final.
- La disponibilidad real de modelos, proveedores, sandbox y herramientas depende del
  despliegue consultado.
- La selección explícita de un modelo Athena solo aparece si el servicio publica más de
  una opción permitida; Athena rechaza nombres no ofrecidos.
- CI construye MSI/NSIS y extrae el MSI como prueba estructural. La firma, la instalación
  interactiva y las pruebas con servicios reales deben registrarse como evidencias de release.

## Comprobaciones

```powershell
pnpm.cmd test
pnpm.cmd test:coverage
pnpm.cmd typecheck
pnpm.cmd build
cargo fmt --check --manifest-path apps/desktop/src-tauri/Cargo.toml
cargo clippy --manifest-path apps/desktop/src-tauri/Cargo.toml --all-targets -- -D warnings
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
python -m unittest discover -s tests -v
```
