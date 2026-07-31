---
type: "architecture"
date: "2026-07-31T04:37:30.933987+00:00"
question: "Continue ChatyGPT development with cancellation of active scheduled runs and history filters"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "App()", "BrokerClient", "cancel_task()", "domain.ts"]
---

# Q: Continue ChatyGPT development with cancellation of active scheduled runs and history filters

## Answer

Expanded from graph vocabulary: [cancel, task, tasks, run, status, history, database, broker, app]. Reused task_runtime::cancel_task so the Broker cancellation and local broker task state are confirmed before Database.finish_scheduled_cancellation marks and audits the scheduled run. Cancellation is only exposed for running runs with a broker task and does not pause recurring schedules. Added filterScheduledRuns in domain.ts as a pure UI projection combining status with today, 7-day, 30-day or all-time windows.

## Outcome

- Signal: useful

## Source Nodes

- Database
- App()
- BrokerClient
- cancel_task()
- domain.ts