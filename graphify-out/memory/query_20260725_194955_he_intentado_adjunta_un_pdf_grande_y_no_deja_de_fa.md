---
type: "query"
date: "2026-07-25T19:49:55.361806+00:00"
question: "He intentado adjunta un pdf grande y no deja de fallar"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "attachment_runtime.rs", "upload_local_file()", "poll_remote_file()", ".update_attachment_ingestion()", "retry_attachment()", "FileState"]
---

# Q: He intentado adjunta un pdf grande y no deja de fallar

## Answer

Expanded from original query via graph vocab: [attachment, upload, ingestion, failed, error, retry, markdown, download, file]. Diagnóstico confirmado en la base durable: math-deep.pdf mide 24629575 bytes y falla en el Broker con CONVERSION_FAILED porque tiene 2204 páginas y Docling limita max_num_pages a 2000. El límite local de ChatyGPT es 512 MB, por lo que no es tamaño en bytes, timeout, LLM ni fragmentación: el Markdown nunca llega a crearse. Reintentar sin cambiar el PDF o el límite repite determinísticamente el mismo fallo.

## Outcome

- Signal: useful

## Source Nodes

- Database
- attachment_runtime.rs
- upload_local_file()
- poll_remote_file()
- .update_attachment_ingestion()
- retry_attachment()
- FileState