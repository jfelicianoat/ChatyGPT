---
type: "architecture"
date: "2026-07-31T04:47:44.986937+00:00"
question: "Continue ChatyGPT development with readable scheduled-run details and auditable history export"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "App()", "export.rs", "atomic_write", "domain.ts"]
---

# Q: Continue ChatyGPT development with readable scheduled-run details and auditable history export

## Answer

Expanded from graph vocabulary: [export, result, error, history, task, tasks, run, dialog, path, file, database, app]. Added scheduledRunDetail in domain.ts to extract known textual result/error fields and avoid raw JSON in App(). Added Database.scheduled_history_export_rows so export applies the same status/date filters across the complete durable history, then export_scheduled_history writes a structured UTF-8 text file atomically, verifies SHA-256, prevents unconfirmed overwrite and records an audit event. Reused the native Windows save-dialog pattern.

## Outcome

- Signal: useful

## Source Nodes

- Database
- App()
- export.rs
- atomic_write
- domain.ts