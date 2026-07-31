---
type: "query"
date: "2026-07-30T15:54:55.684617+00:00"
question: "Continuar Fase 3 con conocimiento privado por GPT personal"
contributor: "graphify"
outcome: "useful"
source_nodes: ["MemoryItemView", "active_memories_for_conversation", "semantic_memory_matches"]
---

# Q: Continuar Fase 3 con conocimiento privado por GPT personal

## Answer

Expanded from original query via graph vocabulary: [custom, gpt, memory, context, semantic]. El diseño reutiliza memory_items.custom_gpt_id, mantiene memory_overview limitado a custom_gpt_id IS NULL, resuelve solo el conocimiento del GPT seleccionado en active_memories_for_conversation y aplica el mismo aislamiento en semantic_memory_matches. MemoryItemView conserva customGptId/customGptName para que task_context etiquete la fuente como Conocimiento GPT. Resultado verificado con pruebas de aislamiento, compilación y ejecutable release.

## Outcome

- Signal: useful

## Source Nodes

- MemoryItemView
- active_memories_for_conversation
- semantic_memory_matches