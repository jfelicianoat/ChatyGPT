---
type: "architecture"
date: "2026-08-10T21:51:40.047771+00:00"
question: "¿Qué componentes deben adaptarse al contrato 2.7 del Broker AI?"
contributor: "graphify"
outcome: "useful"
---

# Q: ¿Qué componentes deben adaptarse al contrato 2.7 del Broker AI?

## Answer

BrokerCapabilities tolera campos aditivos; BrokerClient distingue salud de capacidades, aplica errores seguros y polling acotado; task_runtime construye adjuntos estrictos, estrategias compatibles y reintenta autenticación; db materializa assistant_content y métricas; App usa ingestion_formats, no bloquea capacidades no verificadas y presenta metadatos de respuesta.

## Outcome

- Signal: useful