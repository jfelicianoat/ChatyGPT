---
type: "architecture"
date: "2026-07-30T21:03:56.811078+00:00"
question: "How should ChatyGPT extend durable schedules with recurrence, editing and terminal notifications?"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "App()", "start_chat_turn", "LocalTaskSnapshot"]
---

# Q: How should ChatyGPT extend durable schedules with recurrence, editing and terminal notifications?

## Answer

Expanded from graph vocabulary: [task, tasks, repeatable, event, events, windows, application, app, database, runtime, claims, history]. Implemented daily/weekly next_run advancement in the same IMMEDIATE claim transaction, preserving local Windows wall time via SQLite localtime/utc and skipping missed dates. Editing preserves scheduled_runs, resets future claim state and requires confirmation. App() tracks persisted run transitions and projects terminal states to permission-gated WebView2 notifications without making alerts the source of truth.

## Outcome

- Signal: useful

## Source Nodes

- Database
- App()
- start_chat_turn
- LocalTaskSnapshot