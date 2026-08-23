# Estado vigente de ChatyGPT

Última revisión contra el código: **23 de agosto de 2026**.

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
  concesiones separadas, denegadas por defecto y confirmadas de forma durable.
- Las rutas se resuelven canónicamente y las escrituras sensibles usan comprobación de
  huella y reemplazo atómico.

## Compatibilidad externa

- **AI Broker:** el cliente conserva el cuerpo de petición estable de 2.8, acepta
  respuestas aditivas 2.9 y lee `served_by`, `models_used` y `fallback_used` cuando están
  presentes. También mantiene compatibilidad de lectura con tareas anteriores. Véase
  [BROKER_COMPATIBILITY.md](BROKER_COMPATIBILITY.md).
- **Athena:** wire protocol 1. La aplicación comprueba `/v1/health`, consume runs y eventos,
  resuelve aprobaciones y puede consultar `/v1/profiles`, `/v1/models` y memoria cuando el
  despliegue los ofrece. Un catálogo de modelos ausente significa que el despliegue usa un
  modelo fijo; no es un error.

## Capacidades principales implementadas

- conversaciones, proyectos, búsqueda, adjuntos, contexto documental y memoria opt-in;
- GPTs personales versionados, permisos, conocimiento privado, importación y exportación;
- investigación profunda, fuentes trazables y exportación Markdown/Obsidian;
- automatizaciones locales durables, calendario proyectado e inicio con Windows;
- captura de pantalla y webcam iniciada por la persona;
- sandbox de Broker por turno y herramientas locales siempre confirmadas;
- área Athena con historial, estado, permisos, revisión, modelo por run y reconexión.

## Límites que deben seguir declarándose

- Una capacidad cubierta por pruebas no equivale a una validación manual en el perfil
  Windows final.
- La disponibilidad real de modelos, proveedores, sandbox y herramientas depende del
  despliegue consultado.
- La selección explícita de un modelo Athena solo aparece si el servicio publica más de
  una opción permitida; Athena rechaza nombres no ofrecidos.
- El empaquetado, firma y pruebas con servicios reales deben registrarse como evidencias de
  release, no inferirse del código.

## Comprobaciones

```powershell
pnpm.cmd test
pnpm.cmd typecheck
pnpm.cmd build
cargo test --manifest-path apps/desktop/src-tauri/Cargo.toml
python -m unittest discover -s tests -v
```

