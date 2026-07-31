---
type: "architecture"
date: "2026-07-30T20:07:15.132868+00:00"
question: "Ok, sigue con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["App()", "AttachmentView", "attachment_runtime.rs", "permissions"]
---

# Q: Ok, sigue con el desarrollo

## Answer

Expanded from graph vocab: app, image, attachment, permissions, preview, import. Se añadió fotografía mediante webcam desde App(): permiso solicitado solo por gesto explícito, vídeo sin audio, indicador activo, parada de tracks al fotografiar/cancelar/navegar/cerrar, errores accionables y vista previa confirmable. La foto reutiliza import_captured_image y attachment_runtime.rs. 75 Rust, 42 UI y 15 Python pasaron; release staged.

## Outcome

- Signal: useful

## Source Nodes

- App()
- AttachmentView
- attachment_runtime.rs
- permissions