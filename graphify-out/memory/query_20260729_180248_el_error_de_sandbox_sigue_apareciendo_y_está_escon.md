---
type: "query"
date: "2026-07-29T18:02:48.370069+00:00"
question: "El error de sandbox sigue apareciendo y está escondido abajo a la izquierda"
contributor: "graphify"
outcome: "useful"
source_nodes: ["App()", "domain.ts", "domain.test.ts", "send_chat_turn()", ".capabilities()"]
---

# Q: El error de sandbox sigue apareciendo y está escondido abajo a la izquierda

## Answer

Expanded from original query via vocab: [app, broker, capabilities, diagnose, diagnostic, sandbox, error, send, attachment, state]. Root cause: App.sendTurn only refreshed diagnoseBroker when broker.state was not ready, so a ready state containing a stale negative sandboxRunCode blocked CSV execution without rechecking. The same branch called setNavigationError, rendering the failure in the sidebar. Fixed by refreshing whenever code is required and sandbox is unavailable, and by routing an actionable ComposerErrorGuidance into an assertive alert beside the composer with a retry action.

## Outcome

- Signal: useful

## Source Nodes

- App()
- domain.ts
- domain.test.ts
- send_chat_turn()
- .capabilities()