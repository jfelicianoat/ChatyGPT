# Graph Report - ChatyGPT  (2026-07-31)

## Corpus Check
- 117 files · ~105,838 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1300 nodes · 3749 edges · 138 communities (89 shown, 49 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 7 edges (avg confidence: 0.59)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `5e463b61`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- lib.rs
- Database
- domain.ts
- task_runtime.rs
- String
- BrokerClient
- Q: Estoy subiendo un pdf de 2000 y pico paginas, en el broker he puesto un limite de 5000 paginas, sin embargo, la subida me da error: El PDF supera el límite de páginas. Tiene 2.204 páginas y el Broker admite 2.000 por conversión.
- verify_broker.py
- attachment_runtime.rs
- export.rs
- tauri.conf.json
- devDependencies
- compilerOptions
- contracts.rs
- mod.rs
- Q: Porque en el chat me pide varias veces, o sea cada vez que mando una petición, autorización para cambiar el nombre del chat?
- .connect
- Phase 0 Verification Evidence
- Rust Application Core
- compilerOptions
- Phase 1 Verification Evidence
- ChatyGPT
- main.json
- ChatyGPT Product Architecture
- Recoverable Two-stage Semantic Chat
- build_report
- Desktop HTML Entrypoint
- ConversationExecutionPreferences
- Failed PDF Ingestion
- build_icon
- Application Mark
- tsconfig.json
- AppError
- Connection
- Default
- String
- TaskAccepted
- TaskState
- ConversationView
- ConversationView
- BrokerCapabilities
- AttachmentRecord
- AttachmentView
- BrokerClient
- BrokerDiagnostic
- BrokerTaskRecord
- ContextMessage
- ConversationExecutionPreferences
- ConversationMessage
- ConversationSummaryOverview
- Database
- ExportReport
- LocalTaskSnapshot
- MemorySearchView
- Option
- Path
- PathBuf
- Result
- SelectedAttachmentChunk
- String
- Connection
- ToolDecision
- Value
- Vec
- Phase 1 Verification Evidence
- Project and Conversation Lifecycle
- Privacy-safe Audit Inspector
- Safe Markdown Export
- Durable Tool Confirmation
- Visible Startup Recovery
- Per-response Context Inspector
- Local Memory Embeddings
- Phase 2 Verification Evidence
- Recoverable Two-stage Semantic Chat
- Durable Semantic Memory Search
- Sensitive Memory Privacy Guard
- AI Broker 2.5 Integration
- Durable Chat Flow
- Local-first Windows Desktop App
- Atomic Markdown Export
- One-turn Isolated Python Execution
- Two-stage Semantic Memory Flow
- Authenticated Versioned Sidecar Protocol
- mod.rs
- domain.ts
- Vec
- startup.rs
- .create_custom_gpt_with_starters
- screenCapture.ts
- .create_memory_item
- MarkdownContent.tsx
- .open
- .semantic_memory_matches
- .conversation_summary_overview
- Evidencias de Fase 3
- Evidencias de Fase 4
- Q: Cuando lanzo una petici?n, la ventana donde aparece la respuesta no hace scroll para que se vea esa ultima respuesta, siempre se queda en la pregunta o instruccion lanzada, y tengo que mover a mano el scroll para ver la respuesta. Esto no puede ser as?. Otra cosa es que vaya apareciendo una respuesta y me interese ver algo que est? mas arriba y me muevo hasta ese punto, en ese caso el scroll no debe ser automatico y volver a la ultima respuesta , sino quedarse donde lo he puesto
- Q: Diagnosticar por qué ChatyGPT consulta capabilities en localhost y pierde el sandbox
- Q: El error de sandbox sigue apareciendo y está escondido abajo a la izquierda
- Q: Parece ser que hay una divergencia en los parametros de comunicación, mira a ver si lo puedes arreglar
- Q: Continua con el desarrollo, pero despues de anadir un calculo visible del tiempo que tarda cada respuesta en el chat
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Continuar Fase 3 con conocimiento privado por GPT personal
- Q: Continuar Fase 3 con archivos de conocimiento privados por GPT
- Q: Ok, sigue con el desarrollo. Pero ten en cuenta que los resultados me los estás presentando en Markdown, y me gustaría verlos en texto normal, manteniendo el formato descrito en el markdown
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Where should portable image cropping integrate with screen and camera attachments in ChatyGPT?
- Q: How should ChatyGPT implement the first durable local scheduler slice without duplicating its broker task system?
- Q: How should ChatyGPT extend durable schedules with recurrence, editing and terminal notifications?
- Q: Continue ChatyGPT development with retryable failed scheduled runs and an in-app notification center
- Q: Continue ChatyGPT development with cancellation of active scheduled runs and history filters
- Q: Continue ChatyGPT development with readable scheduled-run details and auditable history export
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- Q: Ok, sigue con el desarrollo
- cameraCapture.ts
- Q: How should ChatyGPT safely export and import portable custom GPT knowledge?
- Q: How should the first durable Deep Research slice integrate with ChatyGPT's existing task lifecycle?
- Q: Which preset is valid for Deep Research when ChatyGPT uses Broker execution strategy agent?
- Q: How can ChatyGPT persist Deep Research web sources without inventing unsupported Broker agent iteration fields?
- scheduledCalendarOccurrences

## God Nodes (most connected - your core abstractions)
1. `AppError` - 324 edges
2. `Database` - 187 edges
3. `AppState` - 89 edges
4. `cleanup()` - 50 edges
5. `test_database()` - 48 edges
6. `BrokerClient` - 46 edges
7. `App()` - 45 edges
8. `chat_request()` - 23 edges
9. `chat_request_with_project_instruction()` - 20 edges
10. `export_conversation_to_obsidian()` - 18 edges

## Surprising Connections (you probably didn't know these)
- `BrokerProbeTests` --uses--> `BrokerProbe`  [INFERRED]
  tests/test_broker_probe.py → scripts/verify_broker.py
- `ContractHandler` --uses--> `BrokerProbe`  [INFERRED]
  tests/test_broker_probe.py → scripts/verify_broker.py
- `App()` --indirect_call--> `task()`  [INFERRED]
  apps/desktop/src/App.tsx → apps/desktop/src/domain.test.ts
- `pnpm Monorepo Workspace Layout` --conceptually_related_to--> `Desktop HTML Entrypoint`  [EXTRACTED]
  pnpm-workspace.yaml → apps/desktop/index.html
- `pnpm Monorepo Workspace Layout` --conceptually_related_to--> `Future Local Python Automation Service`  [EXTRACTED]
  pnpm-workspace.yaml → services/automation/README.md

## Import Cycles
- None detected.

## Communities (138 total, 49 thin omitted)

### Community 0 - "lib.rs"
Cohesion: 0.08
Nodes (131): AppError, String, approve_conversation_summary(), AppState, archive_conversation(), archive_project(), bootstrap_app(), BootstrapReport (+123 more)

### Community 1 - "Database"
Cohesion: 0.07
Nodes (17): AttachmentRecord, ContextSourceFile, ConversationSummary, Database, project_instructions_are_scoped_and_visible_in_the_exact_task_context(), project_knowledge_overview_composes_only_the_selected_project_sources(), projects_search_and_lifecycle_are_audited(), ProjectSummary (+9 more)

### Community 2 - "domain.ts"
Cohesion: 0.09
Nodes (50): App(), defaultScheduledLocalTime(), describeError(), dialogCopy(), DialogState, Loadable, loadSchedulerReadNotifications(), MemoryEditDraft (+42 more)

### Community 3 - "task_runtime.rs"
Cohesion: 0.09
Nodes (56): advance_semantic_chat(), approved_memory_is_visible_in_request_and_absent_without_items(), cancel_task(), chat_request(), chat_request_with_project_instruction(), chat_routing_delegates_provider_selection_for_internal_context(), chat_routing_keeps_sensitive_memory_local_only(), ChatExecutionOptions (+48 more)

### Community 4 - "String"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, continua, Source Nodes

### Community 5 - "BrokerClient"
Cohesion: 0.06
Nodes (56): chunk_markdown(), converted_markdown_is_split_into_bounded_chunks_without_losing_content(), converted_markdown_prefers_document_boundaries(), copy_into_managed_storage(), import_attachment(), import_captured_image(), import_custom_gpt_attachment(), import_hashes_and_deduplicates_managed_copy() (+48 more)

### Community 6 - "Q: Estoy subiendo un pdf de 2000 y pico paginas, en el broker he puesto un limite de 5000 paginas, sin embargo, la subida me da error: El PDF supera el límite de páginas. Tiene 2.204 páginas y el Broker admite 2.000 por conversión."
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Estoy subiendo un pdf de 2000 y pico paginas, en el broker he puesto un limite de 5000 paginas, sin embargo, la subida me da error: El PDF supera el límite de páginas. Tiene 2.204 páginas y el Broker admite 2.000 por conversión., Source Nodes

### Community 7 - "verify_broker.py"
Cohesion: 0.15
Nodes (15): BaseHTTPRequestHandler, RuntimeError, BrokerProbe, Check, main(), Any, Verificación reproducible y sin persistencia de secretos para AI Broker 2.7., resolve_token() (+7 more)

### Community 8 - "attachment_runtime.rs"
Cohesion: 0.05
Nodes (39): 0A. Base ejecutable — en curso, 0B. Contrato Broker — en curso, 0C. Recuperación, 0D. Seguridad y observabilidad, 0E. Calidad y distribución, 10. Criterios de aceptación de Fase 0, 11. Primer slice vertical, 1. Estado real del repositorio y el entorno (+31 more)

### Community 9 - "export.rs"
Cohesion: 0.10
Nodes (59): ConversationExportMetadata, ProjectExportMetadata, atomic_copy(), atomic_write(), atomic_write_replaces_file_and_hashes_final_bytes(), bounded_calendar_text(), export_conversation(), export_conversation_to_obsidian() (+51 more)

### Community 10 - "tauri.conf.json"
Cohesion: 0.08
Nodes (23): app, security, windows, withGlobalTauri, build, beforeBuildCommand, beforeDevCommand, devUrl (+15 more)

### Community 11 - "devDependencies"
Cohesion: 0.08
Nodes (23): dependencies, react, react-dom, @tauri-apps/api, devDependencies, @tauri-apps/cli, @types/react, @types/react-dom (+15 more)

### Community 12 - "compilerOptions"
Cohesion: 0.11
Nodes (18): compilerOptions, allowJs, allowSyntheticDefaultImports, esModuleInterop, forceConsistentCasingInFileNames, isolatedModules, jsx, lib (+10 more)

### Community 13 - "contracts.rs"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continua con el desarrollo, Source Nodes

### Community 14 - "mod.rs"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Al quitar uno de los dos libros que he cargado en el chat, y al hacer la pregunta "Cuantos temas tiene?", el chat sigue pensando que tiene los dos libros y me pregunta por cual de ellos me refiero, Source Nodes

### Community 15 - "Q: Porque en el chat me pide varias veces, o sea cada vez que mando una petición, autorización para cambiar el nombre del chat?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Porque en el chat me pide varias veces, o sea cada vez que mando una petición, autorización para cambiar el nombre del chat?, Source Nodes

### Community 16 - ".connect"
Cohesion: 0.18
Nodes (5): BuildConfigurationTests, ContractFixtureTests, MigrationTests, Connection, Path

### Community 19 - "compilerOptions"
Cohesion: 0.22
Nodes (8): compilerOptions, allowImportingTsExtensions, composite, module, moduleResolution, noEmit, skipLibCheck, include

### Community 22 - "main.json"
Cohesion: 0.33
Nodes (5): description, identifier, permissions, $schema, windows

### Community 25 - "build_report"
Cohesion: 0.60
Nodes (5): broker_health(), build_report(), compact_error(), main(), Path

### Community 26 - "Desktop HTML Entrypoint"
Cohesion: 0.33
Nodes (6): Desktop HTML Entrypoint, Desktop Main TypeScript Module, React Root Mount Point, esbuild Install Script Allowlist, pnpm Monorepo Workspace Layout, Future Local Python Automation Service

### Community 27 - "ConversationExecutionPreferences"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continua con el desarrollo, Source Nodes

### Community 28 - "Failed PDF Ingestion"
Cohesion: 0.50
Nodes (4): AI Broker Health Snapshot, ChatyGPT Attachment Diagnostic, Docling Ingestion Engine, Failed PDF Ingestion

### Community 29 - "build_icon"
Cohesion: 0.67
Nodes (3): Image, build_icon(), main()

### Community 30 - "Application Mark"
Cohesion: 0.67
Nodes (3): Application Mark, Stylized Letter C, White Circular Dot

### Community 34 - "AppError"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continua con el desarrollo, Source Nodes

### Community 36 - "Connection"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: He intentado adjunta un pdf grande y no deja de fallar, Source Nodes

### Community 37 - "Default"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continua con el desarrollo, Source Nodes

### Community 38 - "String"
Cohesion: 0.09
Nodes (36): AttachmentChunkEmbeddingInput, BrokerTaskRecord, ContextMessage, ContextSnapshotView, ContextSourceView, ConversationExecutionPreferences, ConversationMessage, ConversationSource (+28 more)

### Community 39 - "TaskAccepted"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continua con el desarrollo, Source Nodes

### Community 40 - "TaskState"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: He hecho cambios en el Broker AI que afecta a tu comunicación con el, Source Nodes

### Community 41 - "ConversationView"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Haz los cambios necesarios para que, siguiendo el contrato del ChatyGPT y su proposito, aproveche las nuevas posiblidades que da el Broker AI, Source Nodes

### Community 42 - "ConversationView"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Muy bien, continua el desarrollo donde lo dejamos, el desarrollo del planning inicial que te pasé al principio de la conversación, Source Nodes

### Community 43 - "BrokerCapabilities"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Revisar trazabilidad documental: abrir o localizar la fuente original de cualquier fragmento recuperado, seguridad, estados, accesibilidad y concurrencia, Source Nodes

### Community 44 - "AttachmentRecord"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, continua con el desarrollo, Source Nodes

### Community 45 - "AttachmentView"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, continua, Source Nodes

### Community 46 - "BrokerClient"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, continua, Source Nodes

### Community 88 - "mod.rs"
Cohesion: 0.14
Nodes (40): approved_edited_summary_compacts_context_without_deleting_messages(), attachment_exposes_durable_document_context_progress_and_chunk_count(), attachment_is_deduplicated_and_reused_across_conversations(), audit_inspector_exposes_only_safe_presentation_fields(), audit_presentation(), AuditEventView, broker_progress_is_persisted_for_the_visible_task_snapshot(), cleanup() (+32 more)

### Community 89 - "domain.ts"
Cohesion: 0.07
Nodes (45): AttachmentView, BootstrapReport, BrokerDiagnostic, BrokerTaskStatus, ContextSnapshotView, ContextSourceView, ConversationExecutionPreferences, ConversationMessage (+37 more)

### Community 90 - "Vec"
Cohesion: 0.12
Nodes (12): AttachmentView, custom_gpt_knowledge_is_private_and_independent_from_global_memory(), custom_gpt_portable_knowledge_is_explicit_filtered_and_quarantined(), CustomGptConfiguration, CustomGptPortableExport, MemoryItemView, ProjectFileUsageView, ProjectKnowledgeOverview (+4 more)

### Community 91 - "startup.rs"
Cohesion: 0.17
Nodes (26): Result, atomic_write(), current_token(), disable(), enable(), enabling_requires_explicit_confirmation_before_mutating_windows(), powershell_literal(), protect_token() (+18 more)

### Community 92 - ".create_custom_gpt_with_starters"
Cohesion: 0.17
Nodes (11): conversation_custom_gpt_selection_and_task_version_are_durable(), custom_gpt_edits_create_immutable_versions_without_tool_permissions(), custom_gpt_files_follow_the_selected_gpt_without_sticky_chat_links(), custom_gpt_starters_and_portable_json_round_trip_safely(), CustomGptImportReport, CustomGptToolPermissions, CustomGptView, semantic_chat_persists_the_turn_before_requesting_its_query_embedding() (+3 more)

### Community 93 - "screenCapture.ts"
Cohesion: 0.28
Nodes (11): canvasBlob(), captureDisplayName(), CapturedScreenFrame, captureScreenFrame(), captureVideoFrame(), constrainedCaptureSize(), cropCapturedFrame(), CropSelection (+3 more)

### Community 94 - ".create_memory_item"
Cohesion: 0.31
Nodes (5): completed_memory_embedding_is_stored_with_model_and_dimensions(), editing_memory_preserves_or_invalidates_its_index_by_content(), memory_is_opt_in_scoped_and_user_controllable(), MemoryOverview, stale_embedding_result_cannot_replace_an_edited_memory_index()

### Community 95 - "MarkdownContent.tsx"
Cohesion: 0.33
Nodes (9): isBlockStart(), isTableDivider(), MarkdownContent(), MarkdownContentProps, renderBlocks(), renderInline(), renderParagraph(), safeWebUrl() (+1 more)

### Community 96 - ".open"
Cohesion: 0.25
Nodes (4): default_execution_priority(), Path, Self, AsRef

### Community 97 - ".semantic_memory_matches"
Cohesion: 0.38
Nodes (4): cosine_similarity(), decode_embedding(), MemorySearchResultView, MemorySearchView

### Community 99 - "Evidencias de Fase 3"
Cohesion: 0.40
Nodes (4): Evidencias de Fase 3, Matriz del corte, Siguiente fase, Verificación automática

### Community 100 - "Evidencias de Fase 4"
Cohesion: 0.40
Nodes (4): Evidencias de Fase 4, Matriz del primer corte, Siguiente corte, Verificación automática

### Community 101 - "Q: Cuando lanzo una petici?n, la ventana donde aparece la respuesta no hace scroll para que se vea esa ultima respuesta, siempre se queda en la pregunta o instruccion lanzada, y tengo que mover a mano el scroll para ver la respuesta. Esto no puede ser as?. Otra cosa es que vaya apareciendo una respuesta y me interese ver algo que est? mas arriba y me muevo hasta ese punto, en ese caso el scroll no debe ser automatico y volver a la ultima respuesta , sino quedarse donde lo he puesto"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Cuando lanzo una petici?n, la ventana donde aparece la respuesta no hace scroll para que se vea esa ultima respuesta, siempre se queda en la pregunta o instruccion lanzada, y tengo que mover a mano el scroll para ver la respuesta. Esto no puede ser as?. Otra cosa es que vaya apareciendo una respuesta y me interese ver algo que est? mas arriba y me muevo hasta ese punto, en ese caso el scroll no debe ser automatico y volver a la ultima respuesta , sino quedarse donde lo he puesto, Source Nodes

### Community 102 - "Q: Diagnosticar por qué ChatyGPT consulta capabilities en localhost y pierde el sandbox"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Diagnosticar por qué ChatyGPT consulta capabilities en localhost y pierde el sandbox, Source Nodes

### Community 103 - "Q: El error de sandbox sigue apareciendo y está escondido abajo a la izquierda"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: El error de sandbox sigue apareciendo y está escondido abajo a la izquierda, Source Nodes

### Community 104 - "Q: Parece ser que hay una divergencia en los parametros de comunicación, mira a ver si lo puedes arreglar"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Parece ser que hay una divergencia en los parametros de comunicación, mira a ver si lo puedes arreglar, Source Nodes

### Community 105 - "Q: Continua con el desarrollo, pero despues de anadir un calculo visible del tiempo que tarda cada respuesta en el chat"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continua con el desarrollo, pero despues de anadir un calculo visible del tiempo que tarda cada respuesta en el chat, Source Nodes

### Community 106 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 107 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 108 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 109 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 110 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 111 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 112 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 113 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 114 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 115 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 116 - "Q: Continuar Fase 3 con conocimiento privado por GPT personal"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continuar Fase 3 con conocimiento privado por GPT personal, Source Nodes

### Community 117 - "Q: Continuar Fase 3 con archivos de conocimiento privados por GPT"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continuar Fase 3 con archivos de conocimiento privados por GPT, Source Nodes

### Community 118 - "Q: Ok, sigue con el desarrollo. Pero ten en cuenta que los resultados me los estás presentando en Markdown, y me gustaría verlos en texto normal, manteniendo el formato descrito en el markdown"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo. Pero ten en cuenta que los resultados me los estás presentando en Markdown, y me gustaría verlos en texto normal, manteniendo el formato descrito en el markdown, Source Nodes

### Community 119 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 120 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 121 - "Q: Where should portable image cropping integrate with screen and camera attachments in ChatyGPT?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Where should portable image cropping integrate with screen and camera attachments in ChatyGPT?, Source Nodes

### Community 122 - "Q: How should ChatyGPT implement the first durable local scheduler slice without duplicating its broker task system?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: How should ChatyGPT implement the first durable local scheduler slice without duplicating its broker task system?, Source Nodes

### Community 123 - "Q: How should ChatyGPT extend durable schedules with recurrence, editing and terminal notifications?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: How should ChatyGPT extend durable schedules with recurrence, editing and terminal notifications?, Source Nodes

### Community 124 - "Q: Continue ChatyGPT development with retryable failed scheduled runs and an in-app notification center"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continue ChatyGPT development with retryable failed scheduled runs and an in-app notification center, Source Nodes

### Community 125 - "Q: Continue ChatyGPT development with cancellation of active scheduled runs and history filters"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continue ChatyGPT development with cancellation of active scheduled runs and history filters, Source Nodes

### Community 126 - "Q: Continue ChatyGPT development with readable scheduled-run details and auditable history export"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Continue ChatyGPT development with readable scheduled-run details and auditable history export, Source Nodes

### Community 127 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 128 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 129 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 130 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 131 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

### Community 133 - "Q: How should ChatyGPT safely export and import portable custom GPT knowledge?"
Cohesion: 0.50
Nodes (3): Answer, Outcome, Q: How should ChatyGPT safely export and import portable custom GPT knowledge?

### Community 134 - "Q: How should the first durable Deep Research slice integrate with ChatyGPT's existing task lifecycle?"
Cohesion: 0.50
Nodes (3): Answer, Outcome, Q: How should the first durable Deep Research slice integrate with ChatyGPT's existing task lifecycle?

### Community 135 - "Q: Which preset is valid for Deep Research when ChatyGPT uses Broker execution strategy agent?"
Cohesion: 0.50
Nodes (3): Answer, Outcome, Q: Which preset is valid for Deep Research when ChatyGPT uses Broker execution strategy agent?

### Community 136 - "Q: How can ChatyGPT persist Deep Research web sources without inventing unsupported Broker agent iteration fields?"
Cohesion: 0.50
Nodes (3): Answer, Outcome, Q: How can ChatyGPT persist Deep Research web sources without inventing unsupported Broker agent iteration fields?

## Knowledge Gaps
- **315 isolated node(s):** `$schema`, `identifier`, `description`, `windows`, `permissions` (+310 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **49 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Work-memory lessons

**Preferred sources** — corroborated by past sessions; start here.
- `Database` (32× useful, score=30.196684404) _(code changed — re-verify)_
- `App()` (30× useful, score=28.823030623) _(code changed — re-verify)_
- `domain.ts` (13× useful, score=12.560891987) _(code changed — re-verify)_
- `attachment_runtime.rs` (10× useful, score=9.180233479)
- `chat_request()` (7× useful, score=6.499610787)
- `BrokerClient` (6× useful, score=5.64974776) _(code changed — re-verify)_
- `domain.test.ts` (5× useful, score=4.733086126)
- `platform.ts` (4× useful, score=3.910936525) _(code changed — re-verify)_
- `export.rs` (4× useful, score=3.883427794)
- `.capabilities()` (3× useful, score=2.874294653) _(code changed — re-verify)_

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppError` connect `lib.rs` to `.open`, `Database`, `.conversation_summary_overview`, `.semantic_memory_matches`, `task_runtime.rs`, `BrokerClient`, `String`, `export.rs`, `mod.rs`, `Vec`, `startup.rs`, `.create_custom_gpt_with_starters`, `.create_memory_item`?**
  _High betweenness centrality (0.190) - this node is a cross-community bridge._
- **Why does `Database` connect `Database` to `.open`, `.semantic_memory_matches`, `.conversation_summary_overview`, `lib.rs`, `task_runtime.rs`, `BrokerClient`, `String`, `export.rs`, `mod.rs`, `Vec`, `.create_custom_gpt_with_starters`, `.create_memory_item`?**
  _High betweenness centrality (0.018) - this node is a cross-community bridge._
- **Why does `AppState` connect `lib.rs` to `Database`, `BrokerClient`?**
  _High betweenness centrality (0.006) - this node is a cross-community bridge._
- **What connects `$schema`, `identifier`, `description` to the rest of the system?**
  _331 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `lib.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.07631017843115251 - nodes in this community are weakly interconnected._
- **Should `Database` be split into smaller, more focused modules?**
  _Cohesion score 0.07394957983193277 - nodes in this community are weakly interconnected._
- **Should `domain.ts` be split into smaller, more focused modules?**
  _Cohesion score 0.08563134978229318 - nodes in this community are weakly interconnected._