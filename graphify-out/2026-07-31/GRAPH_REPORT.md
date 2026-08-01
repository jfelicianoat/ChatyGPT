# Graph Report - ChatyGPT  (2026-07-31)

## Corpus Check
- 120 files · ~106,741 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 1317 nodes · 3780 edges · 144 communities (94 shown, 50 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 8 edges (avg confidence: 0.61)
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
- contracts.rs
- mod.rs
- appearance.ts
- .attachment_view
- .list_scheduled_task_templates
- Q: Ok, sigue con el desarrollo

## God Nodes (most connected - your core abstractions)
1. `AppError` - 324 edges
2. `Database` - 187 edges
3. `AppState` - 89 edges
4. `cleanup()` - 50 edges
5. `App()` - 49 edges
6. `test_database()` - 48 edges
7. `BrokerClient` - 46 edges
8. `chat_request()` - 23 edges
9. `chat_request_with_project_instruction()` - 20 edges
10. `export_conversation_to_obsidian()` - 18 edges

## Surprising Connections (you probably didn't know these)
- `BrokerProbeTests` --uses--> `BrokerProbe`  [INFERRED]
  tests/test_broker_probe.py → scripts/verify_broker.py
- `ContractHandler` --uses--> `BrokerProbe`  [INFERRED]
  tests/test_broker_probe.py → scripts/verify_broker.py
- `App()` --indirect_call--> `loadAppearancePreference()`  [INFERRED]
  apps/desktop/src/App.tsx → apps/desktop/src/appearance.ts
- `App()` --indirect_call--> `task()`  [INFERRED]
  apps/desktop/src/App.tsx → apps/desktop/src/domain.test.ts
- `pnpm Monorepo Workspace Layout` --conceptually_related_to--> `Desktop HTML Entrypoint`  [EXTRACTED]
  pnpm-workspace.yaml → apps/desktop/index.html

## Import Cycles
- None detected.

## Communities (144 total, 50 thin omitted)

### Community 0 - "lib.rs"
Cohesion: 0.07
Nodes (129): approve_conversation_summary(), AppState, archive_conversation(), archive_project(), bootstrap_app(), BootstrapReport, cancel_local_task(), cancel_scheduled_run() (+121 more)

### Community 1 - "Database"
Cohesion: 0.08
Nodes (20): BrokerTaskRecord, ContextMessage, ContextSourceFile, ConversationExecutionPreferences, CustomGptContext, Database, ProjectInstructionContext, Connection (+12 more)

### Community 2 - "domain.ts"
Cohesion: 0.09
Nodes (35): App(), describeError(), dialogCopy(), loadSchedulerReadNotifications(), persistSchedulerReadNotifications(), scheduledRunLabel(), schedulerReadNotificationsExist(), AttachmentContextSummary (+27 more)

### Community 3 - "task_runtime.rs"
Cohesion: 0.13
Nodes (33): advance_semantic_chat(), cancel_task(), deep_research_request(), deterministic_jitter(), document_chunk_embedding_request_is_local_only_and_traceable(), embedding_request(), is_permanent(), jitter_is_bounded_and_stable() (+25 more)

### Community 4 - "String"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, continua, Source Nodes

### Community 5 - "BrokerClient"
Cohesion: 0.20
Nodes (26): chunk_markdown(), converted_markdown_is_split_into_bounded_chunks_without_losing_content(), converted_markdown_prefers_document_boundaries(), copy_into_managed_storage(), import_attachment(), import_captured_image(), import_custom_gpt_attachment(), import_hashes_and_deduplicates_managed_copy() (+18 more)

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
Cohesion: 0.16
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
Cohesion: 0.08
Nodes (28): audit_presentation(), AuditEventView, ContextSnapshotView, ContextSourceView, ConversationMessage, ConversationSource, ConversationView, CustomGptConfiguration (+20 more)

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
Nodes (41): approved_edited_summary_compacts_context_without_deleting_messages(), attachment_exposes_durable_document_context_progress_and_chunk_count(), attachment_is_deduplicated_and_reused_across_conversations(), audit_inspector_exposes_only_safe_presentation_fields(), broker_progress_is_persisted_for_the_visible_task_snapshot(), cleanup(), completed_semantic_search_prepares_chat_with_ranked_memory_and_trace(), completed_turn_materializes_attachment_sources_on_assistant_message() (+33 more)

### Community 89 - "domain.ts"
Cohesion: 0.07
Nodes (32): BrokerTaskStatus, ComposerErrorGuidance, ContextSourceView, ConversationMessage, ConversationSource, ConversationSummaryRevision, CustomGptExportReport, CustomGptImportReport (+24 more)

### Community 90 - "Vec"
Cohesion: 0.18
Nodes (8): cosine_similarity(), custom_gpt_portable_knowledge_is_explicit_filtered_and_quarantined(), CustomGptPortableExport, decode_embedding(), MemoryItemView, MemorySearchResultView, MemorySearchView, SemanticMemoryMatch

### Community 91 - "startup.rs"
Cohesion: 0.17
Nodes (26): Result, atomic_write(), current_token(), disable(), enable(), enabling_requires_explicit_confirmation_before_mutating_windows(), powershell_literal(), protect_token() (+18 more)

### Community 92 - ".create_custom_gpt_with_starters"
Cohesion: 0.14
Nodes (15): AttachmentChunkEmbeddingInput, conversation_custom_gpt_selection_and_task_version_are_durable(), custom_gpt_edits_create_immutable_versions_without_tool_permissions(), custom_gpt_files_follow_the_selected_gpt_without_sticky_chat_links(), CustomGptImportReport, CustomGptToolPermissions, CustomGptView, default_execution_priority() (+7 more)

### Community 93 - "screenCapture.ts"
Cohesion: 0.28
Nodes (11): canvasBlob(), captureDisplayName(), CapturedScreenFrame, captureScreenFrame(), captureVideoFrame(), constrainedCaptureSize(), cropCapturedFrame(), CropSelection (+3 more)

### Community 94 - ".create_memory_item"
Cohesion: 0.13
Nodes (14): completed_memory_embedding_is_stored_with_model_and_dimensions(), context_inspector_explains_the_sources_used_by_a_chat_turn(), ConversationSummary, editing_memory_preserves_or_invalidates_its_index_by_content(), memory_is_opt_in_scoped_and_user_controllable(), MemoryOverview, project_knowledge_overview_composes_only_the_selected_project_sources(), ProjectFileUsageView (+6 more)

### Community 95 - "MarkdownContent.tsx"
Cohesion: 0.33
Nodes (9): isBlockStart(), isTableDivider(), MarkdownContent(), MarkdownContentProps, renderBlocks(), renderInline(), renderParagraph(), safeWebUrl() (+1 more)

### Community 96 - ".open"
Cohesion: 0.11
Nodes (30): defaultScheduledLocalTime(), DialogState, Loadable, MemoryEditDraft, scheduledLocalTimeValue(), ScreenCapturePreview, AttachmentView, AuditEventView (+22 more)

### Community 97 - ".semantic_memory_matches"
Cohesion: 0.19
Nodes (12): FileAccepted, BrokerClient, Option, Path, Result, start(), Client, HeaderValue (+4 more)

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

### Community 137 - "scheduledCalendarOccurrences"
Cohesion: 0.15
Nodes (24): AttachmentRecord, approved_memory_is_visible_in_request_and_absent_without_items(), chat_request(), chat_request_with_project_instruction(), chat_routing_delegates_provider_selection_for_internal_context(), chat_routing_keeps_sensitive_memory_local_only(), ChatExecutionOptions, collaborative_analysis_uses_the_selected_depth_without_invalid_map_reduce() (+16 more)

### Community 138 - "contracts.rs"
Cohesion: 0.18
Nodes (10): BrokerCapabilities, FileState, HashMap, Option, String, Value, Vec, TaskAccepted (+2 more)

### Community 139 - "mod.rs"
Cohesion: 0.20
Nodes (8): BrokerDiagnostic, polling_is_bounded_and_backed_off(), PollPolicy, Default, Self, String, Value, Vec

### Community 140 - "appearance.ts"
Cohesion: 0.27
Nodes (9): AppearancePreference, applyAppearancePreference(), loadAppearancePreference(), normalizeAppearancePreference(), persistAppearancePreference(), resolveAppearance(), ResolvedAppearance, subscribeToSystemAppearance() (+1 more)

### Community 143 - "Q: Ok, sigue con el desarrollo"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, sigue con el desarrollo, Source Nodes

## Knowledge Gaps
- **318 isolated node(s):** `$schema`, `identifier`, `description`, `windows`, `permissions` (+313 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **50 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Work-memory lessons

**Preferred sources** — corroborated by past sessions; start here.
- `Database` (33× useful, score=31.184690855)
- `App()` (31× useful, score=29.811575217) _(code changed — re-verify)_
- `domain.ts` (13× useful, score=12.555971134)
- `attachment_runtime.rs` (10× useful, score=9.176637032)
- `BrokerClient` (7× useful, score=6.647370716)
- `chat_request()` (7× useful, score=6.497064501)
- `domain.test.ts` (5× useful, score=4.731231893)
- `platform.ts` (4× useful, score=3.909404377)
- `export.rs` (4× useful, score=3.881906423)
- `.capabilities()` (3× useful, score=2.87316862)

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppError` connect `Database` to `lib.rs`, `.semantic_memory_matches`, `.conversation_summary_overview`, `task_runtime.rs`, `BrokerClient`, `String`, `export.rs`, `scheduledCalendarOccurrences`, `mod.rs`, `.attachment_view`, `.list_scheduled_task_templates`, `mod.rs`, `Vec`, `startup.rs`, `.create_custom_gpt_with_starters`, `.create_memory_item`?**
  _High betweenness centrality (0.187) - this node is a cross-community bridge._
- **Why does `Database` connect `Database` to `lib.rs`, `.semantic_memory_matches`, `.conversation_summary_overview`, `task_runtime.rs`, `BrokerClient`, `String`, `export.rs`, `.attachment_view`, `.list_scheduled_task_templates`, `mod.rs`, `Vec`, `.create_custom_gpt_with_starters`, `.create_memory_item`?**
  _High betweenness centrality (0.018) - this node is a cross-community bridge._
- **Why does `AppState` connect `lib.rs` to `.semantic_memory_matches`, `Database`?**
  _High betweenness centrality (0.006) - this node is a cross-community bridge._
- **What connects `$schema`, `identifier`, `description` to the rest of the system?**
  _334 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `lib.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.06766134628730049 - nodes in this community are weakly interconnected._
- **Should `Database` be split into smaller, more focused modules?**
  _Cohesion score 0.08160192713038242 - nodes in this community are weakly interconnected._
- **Should `domain.ts` be split into smaller, more focused modules?**
  _Cohesion score 0.09309309309309309 - nodes in this community are weakly interconnected._