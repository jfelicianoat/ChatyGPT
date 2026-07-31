---
type: "implementation"
date: "2026-07-25T21:24:48.677480+00:00"
question: "Continua con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["domain.ts", "domain.test.ts", "App()", "AttachmentView", "attachment_runtime.rs"]
---

# Q: Continua con el desarrollo

## Answer

Expanded from graph vocabulary: [attachment, error, failed, retry, message, view, domain, guidance]. Implementada recuperación visible para errores de conversión de adjuntos: attachmentFailureGuidance reconoce el límite de páginas del Broker, muestra cifras reales y una solución; attachmentStatusLabel traduce estados técnicos; App presenta la guía en la tarjeta y contextualiza el reintento. Validado con 9 pruebas TypeScript, 33 Rust, typecheck, Vite, Clippy y Tauri release.

## Outcome

- Signal: useful

## Source Nodes

- domain.ts
- domain.test.ts
- App()
- AttachmentView
- attachment_runtime.rs