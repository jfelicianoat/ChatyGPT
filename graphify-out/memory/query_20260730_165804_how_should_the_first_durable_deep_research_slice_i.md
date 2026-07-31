---
type: "architecture"
date: "2026-07-30T16:58:04.060878+00:00"
question: "How should the first durable Deep Research slice integrate with ChatyGPT's existing task lifecycle?"
contributor: "graphify"
outcome: "useful"
---

# Q: How should the first durable Deep Research slice integrate with ChatyGPT's existing task lifecycle?

## Answer

Use an explicit one-turn mode that validates the broker announces agent, web_search, and fetch_url before persistence. Build a multi-source agent request, persist a research_run keyed to the existing broker_task_id, materialize plan/research/synthesis steps, and update them transactionally from real broker phases. Reuse existing polling, recovery, cancellation, citations, attachments, and conversation rendering; do not simulate token streaming or progress.

## Outcome

- Signal: useful