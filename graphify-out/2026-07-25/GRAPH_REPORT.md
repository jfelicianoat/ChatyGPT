# Graph Report - .  (2026-07-25)

## Corpus Check
- cluster-only mode — file stats not available

## Summary
- 645 nodes · 1826 edges · 62 communities (31 shown, 31 thin omitted)
- Extraction: 99% EXTRACTED · 1% INFERRED · 0% AMBIGUOUS · INFERRED: 13 edges (avg confidence: 0.81)
- Token cost: 0 input · 0 output

## Graph Freshness
- Built from commit: `5e463b61`
- Run `git rev-parse HEAD` and compare to check if the graph is stale.
- Run `graphify update .` after code changes (no API cost).

## Community Hubs (Navigation)
- Database
- lib.rs
- domain.ts
- BrokerClient
- task_runtime.rs
- verify_broker.py
- export.rs
- attachment_runtime.rs
- tauri.conf.json
- devDependencies
- compilerOptions
- .connect
- contracts.rs
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
- Failed PDF Ingestion
- build_icon
- Application Mark
- tsconfig.json
- AppError
- Connection
- Self
- TaskAccepted
- TaskState
- Value
- ConversationView
- ConversationSummaryOverview
- ConversationView
- LocalTaskSnapshot
- MemorySearchView
- BrokerClient
- ConversationSummaryOverview
- LocalTaskSnapshot
- MemorySearchView
- AttachmentRecord
- AttachmentView
- BrokerCapabilities
- BrokerClient
- BrokerDiagnostic
- ConversationMessage
- Database
- ExportReport
- MemoryItemView
- Option
- Path
- PathBuf
- Result
- String
- Vec

## God Nodes (most connected - your core abstractions)
1. `Database` - 109 edges
2. `AppState` - 46 edges
3. `BrokerClient` - 31 edges
4. `cleanup()` - 25 edges
5. `test_database()` - 23 edges
6. `compilerOptions` - 17 edges
7. `chat_request()` - 17 edges
8. `spawn_submission_and_poll()` - 15 edges
9. `BrokerTaskRecord` - 14 edges
10. `completed_semantic_search_prepares_chat_with_ranked_memory_and_trace()` - 14 edges

## Surprising Connections (you probably didn't know these)
- `Recoverable Two-stage Semantic Chat` --semantically_similar_to--> `Two-stage Semantic Memory Flow`  [INFERRED] [semantically similar]
  docs/PHASE_2_VERIFICATION.md → README.md
- `Safe Markdown Export` --semantically_similar_to--> `Atomic Markdown Export`  [INFERRED] [semantically similar]
  docs/PHASE_1_VERIFICATION.md → README.md
- `Future Local Python Automation Service` --semantically_similar_to--> `Narrow Python Sidecar`  [INFERRED] [semantically similar]
  services/automation/README.md → docs/ARCHITECTURE.md
- `BrokerProbeTests` --uses--> `BrokerProbe`  [INFERRED]
  tests/test_broker_probe.py → scripts/verify_broker.py
- `ContractHandler` --uses--> `BrokerProbe`  [INFERRED]
  tests/test_broker_probe.py → scripts/verify_broker.py

## Import Cycles
- None detected.

## Hyperedges (group relationships)
- **Durable Local-first Task Lifecycle** — readme_durable_chat_flow, docs_architecture_persist_before_send, docs_architecture_explicit_recovery, docs_phase_0_verification_idempotent_broker_probe, docs_phase_1_verification_visible_startup_recovery [INFERRED 0.85]
- **Semantic Memory Pipeline** — readme_semantic_memory_flow, docs_phase_2_verification_local_embeddings, docs_phase_2_verification_semantic_memory_search, docs_phase_2_verification_semantic_chat_workflow, docs_phase_2_verification_context_inspector [EXTRACTED 1.00]
- **Layered Desktop Architecture** — apps_desktop_index_react_root, docs_architecture_react_ui, docs_architecture_rust_application_core, docs_architecture_ai_broker_adapter, docs_architecture_sqlite_app_local_data [EXTRACTED 1.00]

## Communities (62 total, 31 thin omitted)

### Community 0 - "Database"
Cohesion: 0.06
Nodes (72): approved_edited_summary_compacts_context_without_deleting_messages(), attachment_exposes_durable_document_context_progress_and_chunk_count(), attachment_is_deduplicated_and_reused_across_conversations(), AttachmentView, audit_inspector_exposes_only_safe_presentation_fields(), audit_presentation(), AuditEventView, BrokerTaskRecord (+64 more)

### Community 1 - "lib.rs"
Cohesion: 0.14
Nodes (66): approve_conversation_summary(), AppState, archive_conversation(), archive_project(), bootstrap_app(), BootstrapReport, cancel_local_task(), create_conversation() (+58 more)

### Community 2 - "domain.ts"
Cohesion: 0.11
Nodes (40): App(), describeError(), dialogCopy(), DialogState, Loadable, AttachmentContextSummary, AttachmentFailureGuidance, attachmentStatusLabel() (+32 more)

### Community 3 - "BrokerClient"
Cohesion: 0.12
Nodes (24): BrokerClient, BrokerDiagnostic, polling_is_bounded_and_backed_off(), PollPolicy, AppError, BrokerCapabilities, Option, Path (+16 more)

### Community 4 - "task_runtime.rs"
Cohesion: 0.15
Nodes (38): LocalTaskSnapshot, advance_semantic_chat(), approved_memory_is_visible_in_request_and_absent_without_items(), cancel_task(), chat_request(), chat_routing_allows_local_and_cloud_providers_for_normal_context(), chat_routing_keeps_sensitive_memory_local_only(), ChatExecutionOptions (+30 more)

### Community 5 - "verify_broker.py"
Cohesion: 0.15
Nodes (15): BaseHTTPRequestHandler, RuntimeError, BrokerProbe, Check, main(), Any, Verificación reproducible y sin persistencia de secretos para AI Broker 2.5., resolve_token() (+7 more)

### Community 6 - "export.rs"
Cohesion: 0.16
Nodes (22): AppError, Result, String, atomic_write(), atomic_write_replaces_file_and_hashes_final_bytes(), export_conversation(), export_detects_external_changes_and_requires_overwrite_confirmation(), ExportReport (+14 more)

### Community 7 - "attachment_runtime.rs"
Cohesion: 0.23
Nodes (23): chunk_markdown(), converted_markdown_is_split_into_bounded_chunks_without_losing_content(), copy_into_managed_storage(), import_attachment(), import_hashes_and_deduplicates_managed_copy(), ImportedFile, is_permanent(), poll_remote_file() (+15 more)

### Community 8 - "tauri.conf.json"
Cohesion: 0.08
Nodes (23): app, security, windows, withGlobalTauri, build, beforeBuildCommand, beforeDevCommand, devUrl (+15 more)

### Community 9 - "devDependencies"
Cohesion: 0.08
Nodes (23): dependencies, react, react-dom, @tauri-apps/api, devDependencies, @tauri-apps/cli, @types/react, @types/react-dom (+15 more)

### Community 10 - "compilerOptions"
Cohesion: 0.11
Nodes (18): compilerOptions, allowJs, allowSyntheticDefaultImports, esModuleInterop, forceConsistentCasingInFileNames, isolatedModules, jsx, lib (+10 more)

### Community 11 - ".connect"
Cohesion: 0.21
Nodes (5): BuildConfigurationTests, ContractFixtureTests, MigrationTests, Connection, Path

### Community 12 - "contracts.rs"
Cohesion: 0.27
Nodes (10): BrokerCapabilities, FileAccepted, FileState, Option, String, Value, Vec, TaskAccepted (+2 more)

### Community 13 - "Phase 0 Verification Evidence"
Cohesion: 0.20
Nodes (10): Explicit Task Recovery, Persist Before Send, Context Traceability, Durable Assistant Response, Idempotent Broker Probe, Phase 0 Verification Evidence, Transactional Chat Turn, Verified Desktop Toolchain (+2 more)

### Community 14 - "Rust Application Core"
Cohesion: 0.25
Nodes (9): AI Broker 2.5 Adapter, Polling by Lease, Narrow Python Sidecar, Rust Application Core, Secrets Do Not Cross React, SQLite in AppLocalData, Vault as Projection, Authenticated Versioned Sidecar Protocol (+1 more)

### Community 15 - "compilerOptions"
Cohesion: 0.22
Nodes (8): compilerOptions, allowImportingTsExtensions, composite, module, moduleResolution, noEmit, skipLibCheck, include

### Community 16 - "Phase 1 Verification Evidence"
Cohesion: 0.29
Nodes (7): Deny-by-default Permissions, Phase 1 Verification Evidence, Project and Conversation Lifecycle, Privacy-safe Audit Inspector, Safe Markdown Export, Durable Tool Confirmation, Atomic Markdown Export

### Community 17 - "ChatyGPT"
Cohesion: 0.29
Nodes (7): Opt-in Personal Memory, Phase 2 Verification Evidence, AI Broker 2.5 Integration, ChatyGPT, Durable Chat Flow, Local-first Windows Desktop App, One-turn Isolated Python Execution

### Community 18 - "main.json"
Cohesion: 0.33
Nodes (5): description, identifier, permissions, $schema, windows

### Community 19 - "ChatyGPT Product Architecture"
Cohesion: 0.33
Nodes (6): Initial Relational Domain Model, Four-phase Product Roadmap, ChatyGPT Product Architecture, React UI Layer, Tauri SQL Transactional Migrations, Tauri Vite Frontend Guidance

### Community 20 - "Recoverable Two-stage Semantic Chat"
Cohesion: 0.33
Nodes (6): Sensitive Memory Local Routing, Local Memory Embeddings, Recoverable Two-stage Semantic Chat, Durable Semantic Memory Search, Sensitive Memory Privacy Guard, Two-stage Semantic Memory Flow

### Community 21 - "build_report"
Cohesion: 0.60
Nodes (5): broker_health(), build_report(), compact_error(), main(), Path

### Community 22 - "Desktop HTML Entrypoint"
Cohesion: 0.40
Nodes (5): Desktop HTML Entrypoint, Desktop Main TypeScript Module, React Root Mount Point, esbuild Install Script Allowlist, pnpm Monorepo Workspace Layout

### Community 23 - "Failed PDF Ingestion"
Cohesion: 0.40
Nodes (5): AI Broker Health Snapshot, ChatyGPT Attachment Diagnostic, Docling Ingestion Engine, Failed PDF Ingestion, Durable Deduplicated Attachments

### Community 24 - "build_icon"
Cohesion: 0.67
Nodes (3): Image, build_icon(), main()

### Community 25 - "Application Mark"
Cohesion: 0.67
Nodes (3): Application Mark, Stylized Letter C, White Circular Dot

## Knowledge Gaps
- **99 isolated node(s):** `$schema`, `identifier`, `description`, `windows`, `permissions` (+94 more)
  These have ≤1 connection - possible missing edges or undocumented components.
- **31 thin communities (<3 nodes) omitted from report** — run `graphify query` to explore isolated nodes.

## Work-memory lessons

**Preferred sources** — corroborated by past sessions; start here.
- `attachment_runtime.rs` (3× useful, score=2.984429029) _(code changed — re-verify)_
- `App()` (3× useful, score=2.953353802) _(code changed — re-verify)_
- `chat_request()` (2× useful, score=1.962599625)

## Suggested Questions
_Questions this graph is uniquely positioned to answer:_

- **Why does `Database` connect `Database` to `lib.rs`, `task_runtime.rs`, `export.rs`, `attachment_runtime.rs`?**
  _High betweenness centrality (0.275) - this node is a cross-community bridge._
- **Why does `AppState` connect `lib.rs` to `Database`?**
  _High betweenness centrality (0.166) - this node is a cross-community bridge._
- **Why does `BrokerClient` connect `BrokerClient` to `task_runtime.rs`?**
  _High betweenness centrality (0.090) - this node is a cross-community bridge._
- **What connects `$schema`, `identifier`, `description` to the rest of the system?**
  _107 weakly-connected nodes found - possible documentation gaps or missing edges._
- **Should `Database` be split into smaller, more focused modules?**
  _Cohesion score 0.06319272125723739 - nodes in this community are weakly interconnected._
- **Should `lib.rs` be split into smaller, more focused modules?**
  _Cohesion score 0.13784021071115013 - nodes in this community are weakly interconnected._
- **Should `domain.ts` be split into smaller, more focused modules?**
  _Cohesion score 0.10505050505050505 - nodes in this community are weakly interconnected._