---
type: "architecture"
date: "2026-07-30T21:26:36.318425+00:00"
question: "Continue ChatyGPT development with retryable failed scheduled runs and an in-app notification center"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "App()", "domain.ts", "lib.rs"]
---

# Q: Continue ChatyGPT development with retryable failed scheduled runs and an in-app notification center

## Answer

Expanded from graph vocabulary: [task, tasks, run, retry, failed, failure, history, database, app, events]. Implemented Database.retry_failed_scheduled_run as an IMMEDIATE transaction that inserts a new claimed scheduled_run with a unique claim key and increasing attempt while preserving the failed source run and rejecting concurrent retries. Added the Tauri command and App() controls to start the normal chat runtime. Added scheduledNotifications in domain.ts to derive the internal notification center from persisted terminal runs; only read markers live in local WebView storage.

## Outcome

- Signal: useful

## Source Nodes

- Database
- App()
- domain.ts
- lib.rs