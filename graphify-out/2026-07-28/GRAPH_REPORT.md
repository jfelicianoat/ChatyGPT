# Graph Report - ChatyGPT  (2026-07-28)

## Corpus Check
- 63 files · ~58,476 words
- Verdict: corpus is large enough that graph structure adds value.

## Summary
- 810 nodes · 2231 edges · 87 communities (41 shown, 46 thin omitted)
- Extraction: 100% EXTRACTED · 0% INFERRED · 0% AMBIGUOUS · INFERRED: 4 edges (avg confidence: 0.57)
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

## God Nodes (most connected - your core abstractions)
1. `AppError` - 199 edges
2. `Database` - 124 edges
3. `AppState` - 51 edges
4. `BrokerClient` - 41 edges
5. `cleanup()` - 33 edges
6. `test_database()` - 31 edges
7. `chat_request()` - 22 edges
8. `App()` - 20 edges
9. `compilerOptions` - 17 edges
10. `spawn_submission_and_poll()` - 16 edges

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

## Communities (87 total, 46 thin omitted)

### Community 0 - "lib.rs"
Cohesion: 0.11
Nodes (74): approve_conversation_summary(), AppState, archive_conversation(), archive_project(), bootstrap_app(), BootstrapReport, cancel_local_task(), create_conversation() (+66 more)

### Community 1 - "Database"
Cohesion: 0.08
Nodes (18): AttachmentRecord, ConversationSummary, ConversationSummaryInput, cosine_similarity(), Database, decode_embedding(), MemoryOverview, Connection (+10 more)

### Community 2 - "domain.ts"
Cohesion: 0.09
Nodes (51): App(), describeError(), dialogCopy(), DialogState, Loadable, MemoryEditDraft, AttachmentContextSummary, AttachmentFailureGuidance (+43 more)

### Community 3 - "task_runtime.rs"
Cohesion: 0.19
Nodes (22): chunk_markdown(), converted_markdown_is_split_into_bounded_chunks_without_losing_content(), converted_markdown_prefers_document_boundaries(), copy_into_managed_storage(), import_attachment(), import_hashes_and_deduplicates_managed_copy(), ImportedFile, is_permanent() (+14 more)

### Community 4 - "String"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Ok, continua, Source Nodes

### Community 5 - "BrokerClient"
Cohesion: 0.05
Nodes (74): BrokerCapabilities, FileAccepted, FileState, Option, String, Value, Vec, TaskAccepted (+66 more)

### Community 6 - "Q: Estoy subiendo un pdf de 2000 y pico paginas, en el broker he puesto un limite de 5000 paginas, sin embargo, la subida me da error: El PDF supera el límite de páginas. Tiene 2.204 páginas y el Broker admite 2.000 por conversión."
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Estoy subiendo un pdf de 2000 y pico paginas, en el broker he puesto un limite de 5000 paginas, sin embargo, la subida me da error: El PDF supera el límite de páginas. Tiene 2.204 páginas y el Broker admite 2.000 por conversión., Source Nodes

### Community 7 - "verify_broker.py"
Cohesion: 0.15
Nodes (15): BaseHTTPRequestHandler, RuntimeError, BrokerProbe, Check, main(), Any, Verificación reproducible y sin persistencia de secretos para AI Broker 2.6., resolve_token() (+7 more)

### Community 8 - "attachment_runtime.rs"
Cohesion: 0.05
Nodes (39): 0A. Base ejecutable — en curso, 0B. Contrato Broker — en curso, 0C. Recuperación, 0D. Seguridad y observabilidad, 0E. Calidad y distribución, 10. Criterios de aceptación de Fase 0, 11. Primer slice vertical, 1. Estado real del repositorio y el entorno (+31 more)

### Community 9 - "export.rs"
Cohesion: 0.13
Nodes (36): Result, atomic_copy(), atomic_write(), atomic_write_replaces_file_and_hashes_final_bytes(), export_conversation(), export_conversation_to_obsidian(), export_detects_external_changes_and_requires_overwrite_confirmation(), ExportReport (+28 more)

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
Cohesion: 0.08
Nodes (72): approved_edited_summary_compacts_context_without_deleting_messages(), attachment_exposes_durable_document_context_progress_and_chunk_count(), attachment_is_deduplicated_and_reused_across_conversations(), AttachmentChunkEmbeddingInput, AttachmentView, audit_inspector_exposes_only_safe_presentation_fields(), audit_presentation(), AuditEventView (+64 more)

### Community 15 - "Q: Porque en el chat me pide varias veces, o sea cada vez que mando una petición, autorización para cambiar el nombre del chat?"
Cohesion: 0.40
Nodes (4): Answer, Outcome, Q: Porque en el chat me pide varias veces, o sea cada vez que mando una petición, autorización para cambiar el nombre del chat?, Source Nodes

### Community 16 - ".connect"
Cohesion: 0.21
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

## Knowledge Gaps
- **198 isolated node(s):** `$schema`, `identifier`, `description`, `windows`, `permissions` (+193 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **46 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Work-memory lessons

**Preferred sources** — corroborated by past sessions; start here.
- `Database` (13× useful, score=12.446354316)
- `attachment_runtime.rs` (7× useful, score=6.668731954)
- `App()` (4× useful, score=3.734217132) _(code changed — re-verify)_
- `chat_request()` (3× useful, score=2.830197004) _(code changed — re-verify)_
- `BrokerCapabilities` (2× useful, score=1.910736642)
- `chunk_markdown()` (2× useful, score=1.89610418)
- `BrokerClient` (2× useful, score=1.876180947)
- `domain.ts` (2× useful, score=1.868238223) _(code changed — re-verify)_

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `AppError` connect `Database` to `lib.rs`, `task_runtime.rs`, `BrokerClient`, `export.rs`, `mod.rs`?**
  _High betweenness centrality (0.172) - this node is a cross-community bridge._
- **Why does `Database` connect `Database` to `lib.rs`, `task_runtime.rs`, `BrokerClient`, `export.rs`, `mod.rs`?**
  _High betweenness centrality (0.030) - this node is a cross-community bridge._
- **Why does `chat_request()` connect `BrokerClient` to `Database`, `mod.rs`?**
  _High betweenness centrality (0.011) - this node is a cross-community bridge._
- **What connects `$schema`, `identifier`, `description` to the rest of the system?**
  _214 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `lib.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.10628844839371156 - nodes in this community are weakly interconnected._
- **Should `Database` be split into smaller, more focused modules?**
  _Cohesion score 0.08440721649484537 - nodes in this community are weakly interconnected._
- **Should `domain.ts` be split into smaller, more focused modules?**
  _Cohesion score 0.09220779220779221 - nodes in this community are weakly interconnected._