---
type: "query"
date: "2026-08-12T16:30:53.094445+00:00"
question: "el flujo que he creado no me está funcionando, averigua porque"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "BrokerClient"]
---

# Q: el flujo que he creado no me está funcionando, averigua porque

## Answer

Expanded from original query via graph vocab: [workflow, runtime, node, run, broker, database]. The real persisted workflow run failed at its custom_gpt node before a remote task id was created. The Broker returned HTTP 403 ADMIN_AUTH_REQUIRED. Client_API says this means the admin credential must be renewed, commonly because the Broker rotates its token on restart. Input completed and Result was skipped only as a downstream consequence. Workflow uses BrokerClient.create_task, the same authorized client path as chat tasks, so graph topology and GPT configuration are not the root cause.

## Outcome

- Signal: useful

## Source Nodes

- Database
- BrokerClient