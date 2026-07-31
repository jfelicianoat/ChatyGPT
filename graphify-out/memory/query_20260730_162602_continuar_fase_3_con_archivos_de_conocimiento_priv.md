---
type: "query"
date: "2026-07-30T16:26:02.143898+00:00"
question: "Continuar Fase 3 con archivos de conocimiento privados por GPT"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "AttachmentView", "start_chat_turn", "ready_attachments_for_turn", "select_attachment_chunks"]
---

# Q: Continuar Fase 3 con archivos de conocimiento privados por GPT

## Answer

Expanded from original query via graph vocabulary: [custom, gpt, attachment, ingestion, conversation, context, semantic]. Se añadió custom_gpt_files para asociar una copia deduplicada de AttachmentView al GPT sin insertar conversation_attachments. start_chat_turn resuelve ready_custom_gpt_file_ids_for_conversation en cada envío, la autorización de ready_attachments_for_turn y select_attachment_chunks acepta solo el GPT actualmente seleccionado, y context_sources conserva la razón Archivo de conocimiento del GPT personal seleccionado. El cambio o retirada del GPT elimina el archivo del siguiente turno sin modificar las fuentes históricas.

## Outcome

- Signal: useful

## Source Nodes

- Database
- AttachmentView
- start_chat_turn
- ready_attachments_for_turn
- select_attachment_chunks