---
type: "query"
date: "2026-08-12T16:51:46.635831+00:00"
question: "Cómo integrar aprobaciones humanas duraderas en los flujos gráficos de ChatyGPT"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "WorkflowNode", "ToolDecision"]
---

# Q: Cómo integrar aprobaciones humanas duraderas en los flujos gráficos de ChatyGPT

## Answer

Expanded from project vocabulary: [workflow, node, waiting, decision, approve, tool, runtime]. Implemented a durable approval workflow node by reusing the existing SQLite/run-state architecture. A ready approval node becomes waiting_approval without contacting Broker AI; independent branches continue and completed result outputs remain visible. The atomic decision accepts exactly one pending approval: approve copies the full pending input to output and resumes downstream nodes; reject fails only that branch and resumes propagation. waiting_approval survives database close/reopen, is cancellable, and decisions cannot be replayed.

## Outcome

- Signal: useful

## Source Nodes

- Database
- WorkflowNode
- ToolDecision