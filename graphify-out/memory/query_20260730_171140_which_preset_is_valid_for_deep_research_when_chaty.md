---
type: "debugging"
date: "2026-07-30T17:11:40.125192+00:00"
question: "Which preset is valid for Deep Research when ChatyGPT uses Broker execution strategy agent?"
contributor: "graphify"
outcome: "corrected"
correction: "Corrected the initial Deep Research implementation from agent/slow to agent/fast and added a regression assertion matching the Broker contract error."
---

# Q: Which preset is valid for Deep Research when ChatyGPT uses Broker execution strategy agent?

## Answer

The captured Broker 422 proves agent only accepts preset fast. Deep Research must use strategy=agent, preset=fast; depth comes from the research plan and max_iterations, while preset=slow remains valid only for mixture_of_agents.

## Outcome

- Signal: corrected
- Correction: Corrected the initial Deep Research implementation from agent/slow to agent/fast and added a regression assertion matching the Broker contract error.