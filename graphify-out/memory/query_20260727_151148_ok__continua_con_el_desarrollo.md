---
type: "query"
date: "2026-07-27T15:11:48.279105+00:00"
question: "Ok, continua con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", ".task_context()", ".context_source_file()", "ContextSourceView", "ContextSourceFile", "copy_into_managed_storage"]
---

# Q: Ok, continua con el desarrollo

## Answer

Expanded from original query via graph vocab: [attachment, source, context, file, managed, open, path, snapshot, chunk, existing]. Implemented secure source traceability from the context inspector: ContextSourceView exposes an opaque reference, Database.task_context and Database.context_source_file validate the task-to-snapshot-to-source-to-chunk-to-attachment chain, reveal_context_source canonicalizes and confines paths to managed storage before Windows Explorer selects the file, and App ignores stale context loads. Added Rust and TypeScript regression tests and documented where to find and test Mostrar archivo.

## Outcome

- Signal: useful

## Source Nodes

- Database
- .task_context()
- .context_source_file()
- ContextSourceView
- ContextSourceFile
- copy_into_managed_storage