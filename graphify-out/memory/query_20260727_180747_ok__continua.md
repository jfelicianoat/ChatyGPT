---
type: "query"
date: "2026-07-27T18:07:47.201849+00:00"
question: "Ok, continua"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "attachment_runtime.rs", "chunk_markdown()", ".select_attachment_chunks()", "AttachmentView", "document_selection_includes_nearby_context_after_relevant_chunks"]
---

# Q: Ok, continua

## Answer

Expanded from original query via graph vocab: [attachment, chunk, chunks, document, embedding, index, progress, ready, search, select, semantic, context]. Implemented the next Phase 2 document slice: chunk_markdown now prefers paragraph and sentence boundaries, Database.select_attachment_chunks adds nearby context after relevant matches while preserving limits, AttachmentView derives indexedCharacters from SQLite, and the existing attachment card exposes consultable characters plus an explicit token estimate. Added regression tests and kept hybrid chunk embeddings as the next slice rather than claiming lexical retrieval is semantic.

## Outcome

- Signal: useful

## Source Nodes

- Database
- attachment_runtime.rs
- chunk_markdown()
- .select_attachment_chunks()
- AttachmentView
- document_selection_includes_nearby_context_after_relevant_chunks