---
type: "query"
date: "2026-07-27T18:24:46.723510+00:00"
question: "Ok, continua"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "attachment_runtime.rs", "start_attachment_semantic_index()", ".select_attachment_chunks_hybrid()", "AttachmentView", "semantic_workflow_uses_memory()"]
---

# Q: Ok, continua

## Answer

Expanded from original query via graph vocab: [attachment, chunk, embedding, index, record, recover, search, semantic, status, task, vectors, workflow]. Implemented progressive attachment-chunk embeddings using existing durable broker_tasks and embedding_records, one active task per document with restart recovery and SHA validation. Added automatic chat_document_search workflows for indexed attachments, hybrid cosine-plus-lexical selection with compatibility checks and lexical fallback, opt-in isolation from personal memory, visible progress and retry states, regression tests, and documentation.

## Outcome

- Signal: useful

## Source Nodes

- Database
- attachment_runtime.rs
- start_attachment_semantic_index()
- .select_attachment_chunks_hybrid()
- AttachmentView
- semantic_workflow_uses_memory()