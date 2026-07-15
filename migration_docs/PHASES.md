# Migration Phases

## Phase 1: Core Data Models and Configuration

**Status**: Not started
**Estimated effort**: ~2.5K LOC Python → ~3K LOC Rust
**Dependencies**: None (leaf of dependency graph)

### What to migrate

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/models/message.py` | `kendra-models/src/message.rs` | ChatMessage, ToolCall, Role, InputProvenance |
| `KendraCLI/models/session.py` | `kendra-models/src/session.rs` | Session, SessionMetadata, Channel enum |
| `KendraCLI/models/config.py` | `kendra-models/src/config.rs` | AppConfig, PermissionConfig, PlaybookConfig |
| `KendraCLI/models/file_change.py` | `kendra-models/src/file_change.rs` | FileChange, FileChangeType enum |
| `KendraCLI/models/operation.py` | `kendra-models/src/operation.rs` | WriteResult, EditResult |
| `KendraCLI/models/user.py` | `kendra-models/src/user.rs` | User model |
| `KendraCLI/models/api.py` | `kendra-models/src/api.rs` | API request/response types |
| `KendraCLI/models/message_validator.py` | `kendra-models/src/validator.rs` | Validation rules |
| `KendraCLI/core/paths.py` | `kendra-config/src/paths.rs` | Path constants |
| `KendraCLI/config/models.py` | `kendra-config/src/lib.rs` | ModelInfo, ProviderInfo |
| `KendraCLI/config/models_dev_loader.py` | `kendra-config/src/models_dev.rs` | models.dev API cache |

### Key decisions
- All model structs derive `serde::Serialize, serde::Deserialize`
- Enums use `strum` for string conversion (matching Python enum string values)
- `chrono::DateTime<Utc>` replaces Python `datetime`
- `uuid::Uuid` replaces Python `uuid4()`
- PyO3 `#[pyclass]` annotations on all public types for bridge

### Deliverables
- `kendra-models` crate with all data types
- `kendra-config` crate with hierarchical config loading
- PyO3 module exposing both crates to Python
- Round-trip serialization tests for all types
- Compatibility test: load existing session JSON files

---

## Phase 2: HTTP Client, Auth, and API Adapters

**Status**: Not started
**Estimated effort**: ~3K LOC Python → ~2.5K LOC Rust
**Dependencies**: Phase 1 (models)

### What to migrate

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/core/agents/components/api/http_client.py` | `kendra-http/src/client.rs` | reqwest wrapper, retry, interrupt |
| `KendraCLI/core/agents/components/api/auth_rotation.py` | `kendra-http/src/rotation.rs` | API key rotation |
| `KendraCLI/core/agents/components/api/base_adapter.py` | `kendra-http/src/adapters/base.rs` | ProviderAdapter trait |
| `KendraCLI/core/agents/components/api/anthropic_adapter.py` | `kendra-http/src/adapters/anthropic.rs` | Anthropic adapter |
| `KendraCLI/core/agents/components/api/openai_responses_adapter.py` | `kendra-http/src/adapters/openai.rs` | OpenAI adapter |
| `KendraCLI/core/auth/credentials.py` | `kendra-http/src/auth.rs` | CredentialStore |
| `KendraCLI/core/auth/user_store.py` | `kendra-http/src/auth.rs` | User storage |

### Key decisions
- `reqwest` with `rustls-tls` (no OpenSSL dependency)
- Interrupt via `tokio_util::sync::CancellationToken` + `tokio::select!` (replaces polling `_should_interrupt`)
- Retry with exponential backoff via custom logic (simpler than Python's polling loop)
- Auth file at `~/.kendra/auth.json` with `std::fs::set_permissions` (mode 0600)

### Deliverables
- `kendra-http` crate with full HTTP client
- Provider adapters for Anthropic and OpenAI
- Credential store with atomic writes
- Mock server tests (wiremock crate)
- Integration test with live API call (gated by env var)

---

## Phase 3: Context Engineering Core

**Status**: Not started
**Estimated effort**: ~15K LOC Python → ~12K LOC Rust
**Dependencies**: Phase 1 (models), Phase 2 (HTTP for LLM-powered compaction)

### What to migrate

**kendra-context crate:**

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/core/context_engineering/compaction.py` | `kendra-context/src/compaction.rs` | Staged compaction |
| `KendraCLI/core/context_engineering/validated_message_list.py` | `kendra-context/src/validated_list.rs` | ValidatedMessageList |
| `KendraCLI/core/context_engineering/message_pair_validator.py` | `kendra-context/src/pair_validator.rs` | Message pair repair |
| `KendraCLI/core/context_engineering/context_picker/` | `kendra-context/src/context_picker.rs` | Dynamic context selection |

**kendra-history crate:**

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/core/context_engineering/history/session_manager/index.py` | `kendra-history/src/index.rs` | Session index |
| `KendraCLI/core/context_engineering/history/session_manager/listing.py` | `kendra-history/src/listing.rs` | Session listing |
| `KendraCLI/core/context_engineering/history/file_locks.py` | `kendra-history/src/file_locks.rs` | File locks (fd-lock) |
| `KendraCLI/core/context_engineering/history/snapshot.py` | `kendra-history/src/snapshot.rs` | Snapshots |

**kendra-memory crate:**

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/core/context_engineering/memory/playbook.py` | `kendra-memory/src/playbook.rs` | ACE playbook |
| `KendraCLI/core/context_engineering/memory/delta.py` | `kendra-memory/src/delta.rs` | Delta operations |
| `KendraCLI/core/context_engineering/memory/embeddings.py` | `kendra-memory/src/embeddings.rs` | Embedding cache |
| `KendraCLI/core/context_engineering/memory/selector.py` | `kendra-memory/src/selector.rs` | Semantic selector |
| `KendraCLI/core/context_engineering/memory/reflection/reflector.py` | `kendra-memory/src/reflector.rs` | Reflection |
| `KendraCLI/core/context_engineering/memory/roles.py` | `kendra-memory/src/roles.rs` | Roles |

### Key decisions
- Token counting via `tiktoken-rs` crate
- File locking via `fd-lock` crate (cross-platform)
- Session JSON read/write via `serde_json` (must match Python format exactly)

### Deliverables
- Three crates: `kendra-context`, `kendra-history`, `kendra-memory`
- Session file compatibility tests (load real files from ~/.kendra/sessions/)
- Compaction tests at various token thresholds
- File locking stress tests

---

## Phase 4: Tool System and Implementations

**Status**: Not started
**Estimated effort**: ~20K LOC Python → ~18K LOC Rust
**Dependencies**: Phases 1-3

### What to migrate

**kendra-tools-core:**

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/core/context_engineering/tools/implementations/base.py` | `kendra-tools-core/src/traits.rs` | BaseTool trait |
| `KendraCLI/core/context_engineering/tools/registry.py` | `kendra-tools-core/src/registry.rs` | ToolRegistry |
| `KendraCLI/core/context_engineering/tools/param_normalizer.py` | `kendra-tools-core/src/normalizer.rs` | Param normalization |
| `KendraCLI/core/context_engineering/tools/result_sanitizer.py` | `kendra-tools-core/src/sanitizer.rs` | Result truncation |
| `KendraCLI/core/context_engineering/tools/tool_policy.py` | `kendra-tools-core/src/policy.rs` | Tool policy |
| `KendraCLI/core/context_engineering/tools/parallel_policy.py` | `kendra-tools-core/src/parallel.rs` | Parallel policy |

**kendra-tools-impl:** All 20+ tool implementations from `tools/implementations/`

**kendra-tools-lsp:** All 39 language server configs from `tools/lsp/language_servers/`

**kendra-tools-symbol:** Symbol operations from `tools/symbol_tools/`

### Key decisions
- `BaseTool` becomes an `async_trait` in Rust
- Bash tool uses `tokio::process::Command` with output streaming
- Git operations via `git2` crate (libgit2 bindings)
- File search via `grep` crate or inline ripgrep integration
- Web fetch via `reqwest` + `scraper` for HTML parsing
- PDF via `lopdf` or `pdf-extract`

### Deliverables
- Four crates with all tools implemented
- Unit tests per tool with mocked dependencies
- Integration tests for bash, file ops, git (using temp dirs/repos)
- LSP tests against TypeScript and Python language servers

---

## Phase 5: Agent Layer and ReAct Loop

**Status**: Not started
**Estimated effort**: ~8.7K LOC Python → ~7K LOC Rust
**Dependencies**: Phases 1-4

### What to migrate

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/core/base/abstract/base_agent.py` | `kendra-agents/src/traits.rs` | BaseAgent trait |
| `KendraCLI/core/agents/main_agent/agent.py` | `kendra-agents/src/main_agent.rs` | MainAgent struct |
| `KendraCLI/core/agents/main_agent/llm_calls.py` | `kendra-agents/src/llm_calls.rs` | LLM call methods |
| `KendraCLI/core/agents/main_agent/run_loop.py` | `kendra-agents/src/react_loop.rs` | ReAct loop |
| `KendraCLI/core/agents/prompts/composition.py` | `kendra-agents/src/prompts/composer.rs` | PromptComposer |
| `KendraCLI/core/agents/prompts/loader.py` | `kendra-agents/src/prompts/loader.rs` | Template loader |
| `KendraCLI/core/agents/subagents/` | `kendra-agents/src/subagents/` | 8 subagent types |
| `KendraCLI/core/agents/components/response/cleaner.py` | `kendra-agents/src/response/cleaner.rs` | Response cleaner |

### Key decisions
- Mixin inheritance → composition (MainAgent holds HttpClient, LlmCaller, ToolRegistry as fields)
- Prompt templates: `include_str!` for built-in, runtime file loading for user customizations
- Subagent execution via `tokio::task::spawn` with restricted tool sets

### Deliverables
- `kendra-agents` crate
- Prompt composition snapshot tests
- Mock LLM response tests for ReAct loop
- Integration test with real API call

---

## Phase 6: Web Backend and MCP

**Status**: Not started
**Estimated effort**: ~8K LOC Python → ~6K LOC Rust
**Dependencies**: Phase 5 (agents)

### What to migrate

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/web/server.py` | `kendra-web/src/server.rs` | Axum app + middleware |
| `KendraCLI/web/websocket.py` | `kendra-web/src/websocket.rs` | WebSocket manager |
| `KendraCLI/web/state.py` | `kendra-web/src/state.rs` | Shared state |
| `KendraCLI/web/routes/*.py` | `kendra-web/src/routes/*.rs` | REST routes |
| `KendraCLI/core/context_engineering/mcp/manager/manager.py` | `kendra-mcp/src/manager.rs` | MCP manager |
| `KendraCLI/core/context_engineering/mcp/manager/transport.py` | `kendra-mcp/src/transport/` | MCP transports |
| `KendraCLI/core/channels/router.py` | `kendra-channels/src/router.rs` | Channel router |

### Key decisions
- Axum must expose identical REST/WebSocket API as FastAPI (React frontend unchanged)
- State shared via `Arc<RwLock<WebState>>` (replaces Python's module-level state)
- Static files served via `tower-http::services::ServeDir`
- MCP client built on reqwest (HTTP/SSE) and tokio::process (stdio)

### Deliverables
- Three crates: `kendra-web`, `kendra-mcp`, `kendra-channels`
- API compatibility tests (same requests, same responses)
- React frontend integration test
- MCP test against sqlite MCP server

### API Endpoints to Match

```
GET  /api/health
POST /api/chat/query
GET  /api/chat/messages
POST /api/chat/clear
POST /api/chat/interrupt
GET  /api/sessions
GET  /api/sessions/{id}
POST /api/sessions/{id}/resume
GET  /api/config
PUT  /api/config
GET  /api/config/models
WS   /ws
```

---

## Phase 7: TUI and CLI

**Status**: Not started
**Estimated effort**: ~42K LOC Python → ~30K LOC Rust
**Dependencies**: Phases 5-6

### What to migrate

| Python Source | Rust Target | Description |
|---------------|-------------|-------------|
| `KendraCLI/ui_textual/chat_app.py` | `kendra-tui/src/app.rs` | Main ratatui app |
| `KendraCLI/ui_textual/widgets/` | `kendra-tui/src/widgets/` | All widgets |
| `KendraCLI/ui_textual/controllers/` | `kendra-tui/src/controllers/` | All controllers |
| `KendraCLI/ui_textual/formatters_internal/` | `kendra-tui/src/formatters/` | Output formatters |
| `KendraCLI/ui_textual/callback_interface.py` | `kendra-tui/src/callback.rs` | UICallback trait |
| `KendraCLI/repl/repl.py` | `kendra-repl/src/repl.rs` | REPL loop |
| `KendraCLI/repl/query_processor.py` | `kendra-repl/src/query_processor.rs` | Query processing |
| `KendraCLI/repl/tool_executor.py` | `kendra-repl/src/tool_executor.rs` | Tool execution |
| `KendraCLI/repl/commands/` | `kendra-repl/src/commands/` | All commands |
| `KendraCLI/cli/main.py` | `kendra-cli/src/main.rs` | CLI entry (clap) |
| `KendraCLI/input/autocomplete/` | `kendra-tui/src/controllers/autocomplete.rs` | Autocomplete |

### Key decisions
- Textual (reactive CSS widgets) → ratatui (immediate-mode rendering) — different paradigm
- Rich markdown → termimad or pulldown-cmark with custom renderer
- prompt-toolkit → crossterm raw input handling
- Accept visual differences; prioritize feature parity

### Deliverables
- Three crates: `kendra-tui`, `kendra-repl`, `kendra-cli`
- The final `KendraCLI` binary
- Ratatui snapshot tests
- Full manual QA of all TUI features
- End-to-end test: start binary, type query, verify response




