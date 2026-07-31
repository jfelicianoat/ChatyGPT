---
type: "implementation"
date: "2026-07-25T06:51:43.841952+00:00"
question: "Continua con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "chunk_markdown()", "BrokerClient", "attachment_runtime.rs", "chat_request()", "document_chunk_selection_is_relevant_bounded_and_traceable()", ".prepare_chat_turn()"]
---

# Q: Continua con el desarrollo

## Answer

Implementada selección trazable y acotada de fragmentos para adjuntos convertidos: descarga Markdown del Broker, fragmentación local, recuperación de adjuntos anteriores, selección de hasta 8 fragmentos y 24000 caracteres, omisión del archivo completo y fuentes visibles en el inspector de contexto. Validado con 33 pruebas Rust, 7 TypeScript, Clippy, compilación web y ejecutable Tauri release.

## Outcome

- Signal: useful

## Source Nodes

- Database
- chunk_markdown()
- BrokerClient
- attachment_runtime.rs
- chat_request()
- document_chunk_selection_is_relevant_bounded_and_traceable()
- .prepare_chat_turn()