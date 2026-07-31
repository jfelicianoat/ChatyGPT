---
type: "implementation"
date: "2026-07-25T21:39:56.138376+00:00"
question: "Continua con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "AttachmentView", "attachment_runtime.rs", "App", "domain.ts"]
---

# Q: Continua con el desarrollo

## Answer

Expansión: attachment, context, chunk, ingestion, status, error, retry, view, database, markdown, ready, recoverable. Se añadió un estado durable e independiente para la preparación del contexto documental, recuento visible de fragmentos y reintento sin volver a subir el archivo. La interfaz sondea pending/preparing y conserva la subida lista ante fallos locales.

## Outcome

- Signal: useful

## Source Nodes

- Database
- AttachmentView
- attachment_runtime.rs
- App
- domain.ts