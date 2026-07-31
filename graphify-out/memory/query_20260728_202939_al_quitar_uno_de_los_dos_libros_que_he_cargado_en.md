---
type: "debugging"
date: "2026-07-28T20:29:39.508104+00:00"
question: "Al quitar uno de los dos libros que he cargado en el chat, y al hacer la pregunta \"Cuantos temas tiene?\", el chat sigue pensando que tiene los dos libros y me pregunta por cual de ellos me refiero"
contributor: "graphify"
outcome: "useful"
source_nodes: ["chat_request()", "App()", "remove_attachment()", "Database", ".recent_context()", "AttachmentView"]
---

# Q: Al quitar uno de los dos libros que he cargado en el chat, y al hacer la pregunta "Cuantos temas tiene?", el chat sigue pensando que tiene los dos libros y me pregunta por cual de ellos me refiero

## Answer

Expansi?n desde el vocabulario del grafo: [attachment, attachments, remove, selected, conversation, context, messages, turn, source, chunks]. La reproducci?n real mostr? un solo conversation_attachment activo (math-deep.pdf), mientras conversation_history_json a?n conten?a Mark Minervini. La causa fue que chat_request conservaba correctamente el historial, pero no declaraba cu?l era el conjunto documental vigente; adem?s, al abrir la conversaci?n App() dejaba todos los adjuntos existentes sin seleccionar. Se corrigi? a?adiendo active_attachment_scope_json, que excluye como candidatos los archivos retirados y resuelve referencias singulares contra el ?nico archivo activo, y seleccionando al abrir solo los adjuntos que siguen vinculados. Cubierto por regresiones Rust y TypeScript.

## Outcome

- Signal: useful

## Source Nodes

- chat_request()
- App()
- remove_attachment()
- Database
- .recent_context()
- AttachmentView