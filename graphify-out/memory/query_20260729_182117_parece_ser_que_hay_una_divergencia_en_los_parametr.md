---
type: "query"
date: "2026-07-29T18:21:17.051995+00:00"
question: "Parece ser que hay una divergencia en los parametros de comunicación, mira a ver si lo puedes arreglar"
contributor: "graphify"
outcome: "useful"
source_nodes: ["BrokerClient", ".capabilities()", "BrokerCapabilities", "contracts.rs", "sandbox_is_explicit_and_requires_broker_capability()"]
---

# Q: Parece ser que hay una divergencia en los parametros de comunicación, mira a ver si lo puedes arreglar

## Answer

Expanded from original query via graph vocab: [broker, capabilities, client, contract, contracts, diagnostic, file, ingestion, sandbox]. DFS traced BrokerClient.capabilities() into BrokerCapabilities in contracts.rs and the sandbox validation path. Reproduction proved grouped ingestion_formats failed with invalid type map, expected sequence. Fixed the Rust contract to HashMap<String, Vec<String>>, retained serde defaults and additive-field tolerance, updated the 2.7 probe and fixture, and verified all suites plus a release build.

## Outcome

- Signal: useful

## Source Nodes

- BrokerClient
- .capabilities()
- BrokerCapabilities
- contracts.rs
- sandbox_is_explicit_and_requires_broker_capability()