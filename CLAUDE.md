# CLAUDE.md

This file provides guidance to Claude Code (claude.ai/code) when working with code in this repository.

## Project Overview

**kendra** is a blazingly-fast, terminal-native **compound AI coding agent** built in Rust. It's fundamentally different from traditional single-model chatbots — it's a **multi-agent orchestration platform** where five independent workflow slots can bind to different LLM providers and models simultaneously:

- **Execution** slot: Main coding/reasoning model (e.g., Claude Opus)
- **Thinking** slot: Deep reasoning model (e.g., GPT-o3)
- **Critique** slot: Self-review and verification
- **Compaction** slot: Context summarization (70% → 99% reduction)
- **Vision** slot: Image understanding

This compound architecture enables fine-grained optimization: use expensive models where needed, lightweight models for context compression, and different providers for different strengths. kendra orchestrates these slots automatically, routing work to the right model for the job.

**Key characteristics:**
- **Ultra-lightweight:** 4.3ms startup, 9.4MB RAM, 18MB binary (128x faster, 30x smaller than alternatives)
- **Multi-provider:** Supports 9 LLM providers simultaneously (OpenAI, Anthropic, Fireworks, Google, Groq, Mistral, DeepInfra, OpenRouter, Azure)
- **Autonomous execution:** Runs ReAct loop (think → select tools → execute → observe → continue) with subagent spawning
- **Parallel agent fleets:** Multiple subagents run concurrently as Tokio tasks, each with independent LLM bindings
- **Rich tool ecosystem:** 25+ built-in tools + LSP integration (35 languages) + MCP support + Docker sandboxing

## Tech Stack

**Language:** Rust (edition 2024, min 1.94)

**Core Runtime:** Tokio 1.0 (async, zero-interpreter overhead)

**User Interfaces:**
- **TUI:** Ratatui 0.30 + Crossterm 0.29 (terminal rendering + event loop)
- **Web:** Axum 0.8 + WebSocket (real-time streaming, remote access)
- **CLI:** Clap 4.0 (command-line parsing)

**Data & Serialization:** Serde 1.0, ts-rs 10.0 (TypeScript bindings for frontend), Serde JSON

**Web Frontend:** React 18.3, Vite 5.1, TypeScript 5.4, TailwindCSS 3.4, Zustand (state), XYFlow (diagrams)

**Error & Observability:** Thiserror, Anyhow, Tracing 0.1, Tracing-subscriber 0.3

**Testing:** Criterion 0.5 (benchmarks), Tempfile 3 (test isolation)

**Networking:** Reqwest 0.13 (HTTP), custom auth rotation for provider APIs

## Build & Development Commands

```bash
# Build entire workspace
cargo build --workspace

# Build release binary (outputs to target/release/kendra)
cargo build --release -p kendra-cli

# Run all tests (skips doc-tests)
cargo test --workspace --lib --tests

# Run tests for a specific crate
cargo test -p kendra-tui

# Run a single test
cargo test -p kendra-tui test_render_thinking_expanded

# Type/lint checks (must pass in CI)
cargo check --workspace
cargo clippy --workspace -- -D warnings

# Format code (required before commit)
cargo fmt --all

# Auto-rebuild on file changes (requires cargo-watch: cargo install cargo-watch)
cargo watch -x 'build --release -p kendra-cli'

# Smoke test with real LLM (requires OPENAI_API_KEY or similar)
echo "hello" | kendra -p "hello"

# Web UI development (requires Node.js 18+)
cd web-ui && npm ci && npm run build

# Clean build artifacts (reclaims 60GB+ of incremental build cache)
cargo clean --profile dev
```

## Architecture Overview

kendra is a **21-crate Rust workspace** (edition 2024) organized into 5 architectural layers:

### Layer 1: User Interfaces
- **kendra-cli** – Binary entry point (Clap CLI dispatcher)
- **kendra-tui** – Terminal UI (full interactive session management via Ratatui)
- **kendra-web** – Axum web server + WebSocket streaming (real-time agent output, remote access)
- **kendra-repl** – REPL interface (query enhancement, @file injection)
- **kendra-channels** – Message routing abstraction

### Layer 2: Orchestration & Control
- **kendra-agents** – Core **ReAct loop**, subagent spawning, prompt composition, task state machine
- **kendra-context** – **Dynamic context engineering**: token monitoring, multi-stage compaction (70%→99%), message validation
- **kendra-config** – Hierarchical config loading (project > global > env > defaults), workflow slot binding
- **kendra-runtime** – Task lifecycle management, background agents, session orchestration, approval gates

### Layer 3: Tool Platform (Execution)
- **kendra-tools-core** – Tool registry, policy enforcement, sanitization, dispatch framework
- **kendra-tools-impl** – 25+ concrete tools: bash, file I/O, git, web (fetch/screenshot), memory, git-worktree isolation
- **kendra-tools-lsp** – LSP integration (35 languages: Python, TypeScript, Go, Rust, Java, C#, Ruby, PHP, C/C++, Swift, etc.)
- **kendra-tools-symbol** – AST-based code symbol navigation
- **kendra-mcp** – Model Context Protocol (discover/invoke external tools)
- **kendra-hooks** – Plugin lifecycle system (before/after agent execution)
- **kendra-plugins** – Plugin manager and extensibility
- **kendra-sandbox** – Docker runtime for isolated code execution

### Layer 4: Provider Integration & Persistence
- **kendra-http** – LLM provider adapters (OpenAI, Anthropic, Fireworks, Google, Groq, Mistral, DeepInfra, OpenRouter, Azure), auth token rotation, cost tracking
- **kendra-history** – Session persistence (JSON per project, atomic writes), conversation indexing, sidechain transcripts for subagents

### Layer 5: Shared Foundation
- **kendra-models** – Common data types (Message, Session, Config, Task), TypeScript bindings

### Crate Dependencies Map

```
kendra-cli (entry point)
  ├→ kendra-tui
  ├→ kendra-web
  ├→ kendra-repl
  ├→ kendra-config
  └→ kendra-runtime
      ├→ kendra-agents
      │   ├→ kendra-context
      │   ├→ kendra-tools-core
      │   └→ kendra-http
      ├→ kendra-tools-impl
      │   ├→ kendra-tools-lsp
      │   ├→ kendra-tools-symbol
      │   └→ kendra-sandbox
      ├→ kendra-history
      ├→ kendra-hooks
      ├→ kendra-plugins
      └→ kendra-models (all crates depend on this)
```

## Key Workflows & Concepts

### The ReAct Loop (Core Agent Behavior)
1. **Think** – Reason about the task (Thinking slot LLM, optional)
2. **Select Tools** – Choose tools based on task (main Execution slot)
3. **Execute** – Run selected tool (bash, edit, git, LSP, web, etc.)
4. **Observe** – Incorporate tool output back into context
5. **Critique** – Self-review output (Critique slot, optional)
6. **Continue** → Loop until task complete or token limit

### Workflow Slot Binding
Each agent's workflow slots can independently bind to different LLM providers:
```json
{
  "model_provider": "anthropic",
  "model": "claude-opus",
  "model_thinking_provider": "openai",
  "model_thinking": "o3",
  "model_critique_provider": "anthropic",
  "model_compact_provider": "openai",
  "model_vlm_provider": "anthropic"
}
```
This decoupling enables fine-grained cost/latency/capability optimization.

### Subagent Spawning & Fleets
- Agents can programmatically spawn child agents for parallel work
- Each subagent gets its own LLM binding, context window, tool access
- Child agents run as independent Tokio tasks
- Results aggregate back to parent
- Example: "Survey all 10 crates" → spawn 10 subagents, each explores 1 crate concurrently

### Context Engineering
Dynamic prompt construction with multi-stage compaction:
- Monitor tokens used vs. available
- Compaction strategies: summarize old messages, collapse tool results, truncate irrelevant context
- Target: fit maximum relevant context while preserving task coherence
- Achieve 70% → 99% compression ratios on repetitive conversations

### Session Persistence
- Conversations saved as JSON per project (`~/.kendra/sessions/`)
- Sidechain transcripts for subagents
- Full replay/resume capability
- Indexed for fast lookups

## Testing Strategy

**Organization:**
- **Unit tests** in `src/` with `#[cfg(test)]` or in `tests.rs` files
- **Integration tests** in crate-specific `tests/` directories
- **Benchmarks** using Criterion in `benches/` directories

**Running Tests:**
```bash
# Full workspace
cargo test --workspace --lib --tests

# Single crate
cargo test -p kendra-agents

# Single test
cargo test -p kendra-agents test_think_phase_routing

# With output
cargo test -p kendra-agents -- --nocapture

# Benchmarks (slower, shows detailed stats)
cargo bench -p kendra-agents --bench agent_bench
```

**CI Validation** (must all pass):
1. Format: `cargo fmt --all -- --check`
2. Type check: `cargo check --workspace` (Windows, macOS, Linux)
3. Lint: `cargo clippy --workspace -- -D warnings`
4. Tests: `cargo test --workspace --lib --tests` (Windows, macOS, Linux)

## Testing New Features

Before declaring a feature complete, **always verify end-to-end**:

```bash
# 1. Run unit/integration tests
cargo test --workspace --lib --tests

# 2. Pass linting
cargo clippy --workspace -- -D warnings

# 3. Rebuild release binary
cargo build --release -p kendra-cli

# 4. Test with real LLM (critical!)
echo "hello" | kendra -p "hello"
# OR launch interactive TUI for manual testing
kendra
```

This catches issues unit tests cannot: prompt composition, API payload format, TUI rendering, event flow, real LLM response handling.

## Configuration & Environment

**Config Hierarchy:**
1. Project-level config (auto-detected or specified)
2. Global user config (`~/.kendra/settings.json`)
3. Environment variables (OPENAI_API_KEY, ANTHROPIC_API_KEY, etc.)
4. Built-in defaults

**Key Environment Variables:**
```bash
OPENAI_API_KEY=...               # OpenAI provider
ANTHROPIC_API_KEY=...            # Anthropic provider
GOOGLE_API_KEY=...               # Google provider
GROQ_API_KEY=...                 # Groq provider
FIREWORKS_API_KEY=...            # Fireworks provider
OPENROUTER_API_KEY=...           # OpenRouter provider
# ... (similar for Mistral, DeepInfra, Azure)
```

**Session Data:**
```
~/.kendra/
  settings.json          # Main config
  sessions/              # Saved conversations per project
    project-name.json
```
## Post-Change Workflow

**CRITICAL:** After every fix or feature, you MUST complete ALL of these steps before considering the task done. Do NOT skip any step.

### 1. Unit & Integration Tests

```bash
cargo test --workspace --lib --tests
```

All tests must pass. If you added new logic, add unit tests covering it. If the change touches cross-crate behavior, add or update integration tests in the relevant `tests/integration.rs`.

### 2. Lint & Type Checks

```bash
cargo clippy --workspace -- -D warnings
cargo check --workspace
```

Zero warnings, zero errors.

### 3. Rebuild Release Binary

```bash
cargo build --release -p kendra-cli
```

The binary at `target/release/kendra` is symlinked to `~/.local/bin/kendra`, so rebuilding automatically updates the installed version.

### 4. Real Simulation Test on TUI

**CRITICAL:** After rebuilding, you MUST run the actual binary against a real LLM to verify the feature works end-to-end. The `OPENAI_API_KEY` environment variable is already set. Run:

```bash
echo "hello" | kendra -p "hello"
```

Or for interactive TUI testing, launch `kendra` and exercise the feature manually. This catches issues that unit tests cannot — prompt composition, API payload format, TUI rendering, event flow, and real LLM response handling.

**Do NOT consider a feature complete until you have verified it works in the real TUI with a real LLM response.**

### 5. Clean Debug Artifacts

```bash
cargo clean --profile dev
```

Reclaims disk space from incremental compilation (can grow to 60GB+). The next test/clippy run rebuilds in ~20-30s.

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Follow standard Rust naming conventions (snake_case functions, CamelCase types)
- **NEVER** add `Co-Authored-By` lines (e.g. `Co-Authored-By: Claude ...`) to commit messages

## Code Organization

- **Tests belong in separate files.** Do not put `#[cfg(test)] mod tests` inline in source files. Place unit tests in a sibling `tests.rs` or `tests/` directory and integration tests in `tests/`.
- **Keep source files focused.** When a module grows large, split it into submodules with a `mod.rs` or named modules. Each file should have a single clear responsibility.
- **New code follows existing structure.** Before adding a new file, check how the surrounding crate is organized and match that pattern.

