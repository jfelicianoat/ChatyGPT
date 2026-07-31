---
type: "architecture"
date: "2026-07-30T20:15:02.036353+00:00"
question: "Where should portable image cropping integrate with screen and camera attachments in ChatyGPT?"
contributor: "graphify"
outcome: "useful"
source_nodes: ["windows", "image", "attachment", "import", "path", "app"]
---

# Q: Where should portable image cropping integrate with screen and camera attachments in ChatyGPT?

## Answer

Implemented in the shared ScreenCapturePreview path: normalizeCropSelection and cropCapturedFrame transform the in-memory preview before importCapturedImage. This preserves the existing Rust validation, deduplication and ingestion flow for screen and camera images. Native ms-screenclip callback remains gated on future MSIX packaging.

## Outcome

- Signal: useful

## Source Nodes

- windows
- image
- attachment
- import
- path
- app