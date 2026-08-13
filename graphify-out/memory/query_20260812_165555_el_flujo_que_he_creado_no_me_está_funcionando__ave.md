---
type: "query"
date: "2026-08-12T16:55:55.690777+00:00"
question: "el flujo que he creado no me está funcionando, averigua porque"
contributor: "graphify"
outcome: "useful"
source_nodes: ["WorkflowRunView", "BrokerClient", "execute_run"]
---

# Q: el flujo que he creado no me está funcionando, averigua porque

## Answer

Expanded from original query via graph vocabulary: workflow run broker error validation persistence. Persisted flow is a valid published DAG from Entrada to GPT personal to Resultado. First run failed at GPT personal because Broker returned HTTP 403 ADMIN_AUTH_REQUIRED. After broker credential was stored, the next run completed all three nodes in about 9 seconds. The semantic mismatch comes from the frozen custom GPT instruction asking for a greeting and the meaning of life, which dominates the upstream input. Current network test also finds host reachable but TCP port 8765 closed.

## Outcome

- Signal: useful

## Source Nodes

- WorkflowRunView
- BrokerClient
- execute_run