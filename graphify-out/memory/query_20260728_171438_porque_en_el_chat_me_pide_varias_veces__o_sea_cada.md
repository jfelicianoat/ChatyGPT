---
type: "debugging"
date: "2026-07-28T17:14:38.462875+00:00"
question: "Porque en el chat me pide varias veces, o sea cada vez que mando una petición, autorización para cambiar el nombre del chat?"
contributor: "graphify"
outcome: "useful"
source_nodes: ["chat_request()", "resolve_tool_calls()", "Database", ".recent_context()"]
---

# Q: Porque en el chat me pide varias veces, o sea cada vez que mando una petición, autorización para cambiar el nombre del chat?

## Answer

Vocabulario ampliado: approve, confirmation, conversation, permissions, rename, request, result, results, tool, tools. La causa era que chat_request ofrecía rename_conversation en todos los turnos con Herramientas activado; el modelo podía inventar una petición de renombrado y volver a solicitar confirmación. Se corrigió con un filtro determinista que solo expone la herramienta cuando el mensaje actual pide explícitamente renombrar o cambiar el título; los mensajes normales vuelven a estrategia single. Las pruebas de regresión cubren ambos casos.

## Outcome

- Signal: useful

## Source Nodes

- chat_request()
- resolve_tool_calls()
- Database
- .recent_context()