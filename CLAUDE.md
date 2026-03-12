# CLAUDE.md

This file provides guidance to Claude Code when working with code in this repository.

## Build & Development Commands

```bash
# Build the entire workspace
cargo build --workspace

# Run all tests
cargo test --workspace

# Type/lint checks
cargo check --workspace
cargo clippy --workspace -- -D warnings

# Format code
cargo fmt --all

# Run a specific crate's tests
cargo test -p opendev-cli

# Build release binary
cargo build --release -p opendev-cli

# Web UI (React/Vite frontend)
cd web-ui && npm ci && npm run build
```

## Architecture Overview

OpenDev is a Rust workspace with 21 crates under `crates/`. The binary entry point is `opendev-cli`.

```text
crates/
  opendev-cli         ← Binary entry point (clap CLI)
  opendev-tui         ← Terminal UI (ratatui + crossterm)
  opendev-web         ← Web backend (axum + WebSocket)
  opendev-repl        ← REPL / interactive loop
  opendev-agents      ← Main agent + subagents, prompt templates
  opendev-runtime     ← Runtime services (approval, cost, modes)
  opendev-config      ← Hierarchical config loading
  opendev-models      ← Shared data types and models
  opendev-http        ← HTTP client, auth rotation, provider adapters
  opendev-context     ← Context engineering (compaction, validated messages)
  opendev-history     ← Session persistence and management
  opendev-memory      ← Memory systems (embeddings, reflection, playbook)
  opendev-tools-core  ← Tool registry and dispatch
  opendev-tools-impl  ← Tool implementations (bash, edit, file ops, web)
  opendev-tools-lsp   ← LSP integration and language servers
  opendev-tools-symbol← AST-based symbol navigation
  opendev-mcp         ← Model Context Protocol integration
  opendev-channels    ← Channel routing
  opendev-hooks       ← Hook system
  opendev-plugins     ← Plugin manager
  opendev-docker      ← Docker runtime support
```

## Key Patterns

- **Workspace dependencies**: Shared deps declared in root `Cargo.toml` `[workspace.dependencies]`
- **Async runtime**: Tokio with full features
- **Error handling**: `thiserror` for library errors, `anyhow` for application errors
- **Serialization**: `serde` + `serde_json` throughout
- **HTTP**: `reqwest` for client, `axum` for server
- **TUI**: `ratatui` + `crossterm`
- **CLI**: `clap` with derive
- **Home directory**: Use `dirs-next` (not `dirs`)
- **Prompt templates**: Embedded via `include_str!()` in `opendev-agents/src/prompts/embedded.rs`
- **Tests**: Use `tempfile::TempDir`, call `.canonicalize()` for macOS symlink resolution

## Agent Design

**CRITICAL:** Never hard-code if/else branching logic to handle LLM conversation flows. The LLM must decide the next step at each turn — not static conditionals. Design agent loops so the model reasons and chooses actions dynamically.

**CRITICAL:** When crafting system prompts, never use table format. Tables are poorly parsed by LLMs and waste tokens. Use plain prose, bullet lists, or structured sections instead.

## Web UI

The React/Vite frontend lives in `web-ui/` and is served by the `opendev-web` crate. Build with `npm run build` from `web-ui/`.

## Migration Reference

Historical migration documentation is in `migration_docs/`. The original Python implementation is archived at https://github.com/opendev-to/opendev-py.

## Code Style

- Run `cargo fmt` before committing
- Run `cargo clippy` and fix all warnings
- Follow standard Rust naming conventions (snake_case functions, CamelCase types)
