---
type: "architecture"
date: "2026-07-30T20:25:31.049494+00:00"
question: "How should ChatyGPT implement the first durable local scheduler slice without duplicating its broker task system?"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "App()", "LocalTaskSnapshot", "BrokerClient", "start_chat_turn"]
---

# Q: How should ChatyGPT implement the first durable local scheduler slice without duplicating its broker task system?

## Answer

Expanded from graph vocabulary: claims, conversation, database, history, local, migration, runtime, task, tasks, broker. Implemented ScheduledTaskView and ScheduledRunView over the existing scheduled_tasks/scheduled_runs schema; claim_due_scheduled_task uses an IMMEDIATE transaction and unique claim_key; scheduler_runtime delegates due prompts to start_chat_turn and reconciles terminal broker task status. App() exposes creation, confirmation, pause/resume and history in Inicio.

## Outcome

- Signal: useful

## Source Nodes

- Database
- App()
- LocalTaskSnapshot
- BrokerClient
- start_chat_turn