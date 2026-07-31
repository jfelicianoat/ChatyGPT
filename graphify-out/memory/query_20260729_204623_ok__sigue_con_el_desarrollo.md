---
type: "query"
date: "2026-07-29T20:46:23.823670+00:00"
question: "Ok, sigue con el desarrollo"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "ProjectKnowledgeOverview", "ConversationSummary", "App()", "attachmentSelectionOnConversationOpen()"]
---

# Q: Ok, sigue con el desarrollo

## Answer

Expanded from original query via graph vocabulary: [project, overview, file, attachment, conversation, chat, open, select, source, links]. Added privacy-preserving navigation from every reusable project file to the active same-project conversations that actually use it. ProjectKnowledgeOverview now exposes fileUsages derived from conversation_attachments, the UI shows titled chat links and reuses normal conversation opening, tests prove two linked chats remain intact after removing the project-library relation, and documentation plus the Windows release were updated.

## Outcome

- Signal: useful

## Source Nodes

- Database
- ProjectKnowledgeOverview
- ConversationSummary
- App()
- attachmentSelectionOnConversationOpen()