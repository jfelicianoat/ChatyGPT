---
type: "query"
date: "2026-07-30T04:59:46.135436+00:00"
question: "Ok, sigue con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "App()", ".prepare_chat_turn()", ".prepare_semantic_chat_turn()", "chat_request()", "ContextSnapshotView", "SemanticChatWorkflow", "ConversationView"]
---

# Q: Ok, sigue con el desarrollo

## Answer

Expanded from original query via vocab: [gpt, version, versioned, conversation, task, context, snapshot, prepare, request, select, selected, persisted]. Implemented selection of a personal GPT per conversation, durable schema 12 persistence, exact version snapshot by value for normal and semantic chat workflows, explicit Broker prompt metadata, visible custom_gpt context trace, and deny-by-default tool behavior. Verified with Rust, TypeScript, Python, clippy, production build and a staged Windows executable.

## Outcome

- Signal: useful

## Source Nodes

- Database
- App()
- .prepare_chat_turn()
- .prepare_semantic_chat_turn()
- chat_request()
- ContextSnapshotView
- SemanticChatWorkflow
- ConversationView