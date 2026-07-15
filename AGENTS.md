# AGENTS.md

This file provides guidance to coding agents when working with code in this repository.

## Build & Development Commands

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests (skip doc-tests that have 0 tests)
cargo test --workspace --lib --tests

# Type/lint checks
cargo check --workspace
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all

# Run a specific crate's tests
cargo test -p kendra-tui

# Run a single test by name
cargo test -p kendra-tui test_render_thinking_expanded

# Build and install release binary
cargo build --release -p kendra-cli
# Binary outputs to target/release/kendra (not kendra-cli)

# Auto-rebuild on file changes (requires cargo-watch)
cargo watch -x 'build --release -p kendra-cli'

# Web UI (React/Vite frontend)
cd web-ui && npm ci && npm run build
```

## Architecture Overview

kendra is a Rust workspace (edition 2024) with 21 crates under `crates/`. It is an open-source AI coding agent that spawns parallel agents, each bound to the LLM of your choice. The binary entry point is `kendra-cli`.

### Crate Map

```text
crates/
  kendra-cli         ← Binary entry point (clap CLI, dispatches to TUI/REPL/subcommands)
  kendra-tui         ← Terminal UI (ratatui + crossterm, async event loop)
  kendra-web         ← Web backend (axum + WebSocket, broadcasts agent events)
  kendra-repl        ← REPL loop, query enhancement (@file injection), message preparation
  kendra-agents      ← ReAct loop, thinking/critique phases, prompt composition
  kendra-runtime     ← Runtime services (approval, cost tracking, modes)
  kendra-config      ← Hierarchical config loading (project > user > env > defaults)
  kendra-models      ← Shared data types and models
  kendra-http        ← HTTP client, auth rotation, provider adapters (Anthropic, OpenAI, etc.)
  kendra-context     ← Context engineering (compaction stages, message validation)
  kendra-history     ← Session persistence (JSON per project, atomic writes)
  kendra-memory      ← Memory systems (embeddings, reflection, playbook)
  kendra-tools-core  ← Tool registry, BaseTool trait, dispatch
  kendra-tools-impl  ← 30+ tool implementations (bash, edit, file ops, web, agents)
  kendra-tools-lsp   ← LSP integration and language servers
  kendra-tools-symbol← AST-based symbol navigation
  kendra-mcp         ← Model Context Protocol integration
  kendra-channels    ← Channel routing
  kendra-hooks       ← Hook system
  kendra-plugins     ← Plugin manager
  kendra-docker      ← Docker runtime support
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

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Follow standard Rust naming conventions (snake_case functions, CamelCase types)
- **NEVER** add `Co-Authored-By` lines (e.g. `Co-Authored-By: Claude ...`) to commit messages

## Code Organization

- **Tests belong in separate files.** Do not put `#[cfg(test)] mod tests` inline in source files. Place unit tests in a sibling `tests.rs` or `tests/` directory and integration tests in `tests/`.
- **Keep source files focused.** When a module grows large, split it into submodules with a `mod.rs` or named modules. Each file should have a single clear responsibility.
- **New code follows existing structure.** Before adding a new file, check how the surrounding crate is organized and match that pattern.

