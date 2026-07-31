---
type: "architecture"
date: "2026-07-30T16:45:06.353261+00:00"
question: "How should ChatyGPT safely export and import portable custom GPT knowledge?"
contributor: "graphify"
outcome: "useful"
---

# Q: How should ChatyGPT safely export and import portable custom GPT knowledge?

## Answer

Keep schemaVersion 1 configuration-only. Offer an explicit schemaVersion 2 export that includes only enabled normal textual knowledge; exclude sensitive data, disabled data, files, permissions, IDs, and indexes, and report every exclusion count. Accept v1/v2 imports, always deny tool permissions, import knowledge disabled for human review, and never import files.

## Outcome

- Signal: useful