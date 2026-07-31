---
type: "query"
date: "2026-07-27T15:08:38.470954+00:00"
question: "Revisar trazabilidad documental: abrir o localizar la fuente original de cualquier fragmento recuperado, seguridad, estados, accesibilidad y concurrencia"
contributor: "graphify"
outcome: "useful"
source_nodes: ["Database", "App", "context_source_path_must_exist_inside_managed_storage", "document_chunk_selection_is_relevant_bounded_and_traceable"]
---

# Q: Revisar trazabilidad documental: abrir o localizar la fuente original de cualquier fragmento recuperado, seguridad, estados, accesibilidad y concurrencia

## Answer

Expanded from original query via vocab: [context, source, sources, references, snapshot, chunk, attachment, file, path, open, task, explorer]. El flujo Database.task_context -> context_source_file -> validated_managed_source_path -> reveal_context_source cumple referencia opaca, relación task/snapshot/source/chunk/attachment, canonicalización bajo almacenamiento administrado y Explorer /select sin ejecutar. Hallazgo: toggleTaskContext aplica respuestas asíncronas sin comprobar que el taskId sigue activo; una carga anterior puede reabrir o reemplazar el panel tras cerrar/cambiar de respuesta. Añadir guard por taskId o token de solicitud. Estados de revelar fuente y ausencia local son explícitos; pruebas de dominio y Rust pasan.

## Outcome

- Signal: useful

## Source Nodes

- Database
- App
- context_source_path_must_exist_inside_managed_storage
- document_chunk_selection_is_relevant_bounded_and_traceable