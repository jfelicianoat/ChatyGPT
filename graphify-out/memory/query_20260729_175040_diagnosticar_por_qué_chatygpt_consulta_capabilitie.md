---
type: "query"
date: "2026-07-29T17:50:40.719521+00:00"
question: "Diagnosticar por qué ChatyGPT consulta capabilities en localhost y pierde el sandbox"
contributor: "graphify"
outcome: "useful"
source_nodes: ["BrokerClient", ".from_environment()", ".capabilities()", "AppState", "diagnose_broker()"]
---

# Q: Diagnosticar por qué ChatyGPT consulta capabilities en localhost y pierde el sandbox

## Answer

Expanded from original query via vocab: [broker, client, capabilities, diagnose, diagnostic, environment, token, url, bootstrap, state]. BrokerClient toma CHATYGPT_BROKER_BASE_URL al crear AppState. El BAT sí fijaba 192.168.1.52, pero el ejecutable directo caía a 127.0.0.1. No existe caché persistente de capabilities: App la consulta al arrancar y bajo demanda, y start_chat_turn vuelve a consultarla antes de sandbox. Se corrigió el fallback a 192.168.1.52 y el BAT ahora valida capabilities con el token y lanza el ejecutable release.

## Outcome

- Signal: useful

## Source Nodes

- BrokerClient
- .from_environment()
- .capabilities()
- AppState
- diagnose_broker()