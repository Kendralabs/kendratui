# OpenDev Project Documentation

This document provides a comprehensive overview of the OpenDev project, including its purpose, architecture, development guidelines, and how to build and run it.

## Project Overview

OpenDev is an open-source, terminal-native coding agent built as a compound AI system. It leverages a structured ensemble of agents and workflows, each independently bound to a user-configured Large Language Model (LLM). This modular approach allows for fine-grained control over cost, latency, and capability trade-offs for different workflows (Execution, Thinking, Compaction, Self-Critique, Vision).

The project is primarily written in **Rust** for its performance and memory efficiency, enabling fast startup times and low memory consumption. It also includes a **React/Vite**-based Web UI for visual monitoring and remote sessions.

Key features include:
- **Blazing fast, ultra lightweight:** Written in Rust, offering superior performance compared to alternatives.
- **Proactive, not reactive:** Designed for autonomous planning, execution, and iteration.
- **Multi-provider, multi-model:** Supports 9 LLM providers (OpenAI, Anthropic, Fireworks, Google, Groq, Mistral, DeepInfra, OpenRouter, Azure OpenAI) with independent model assignment to different workflow slots.
- **TUI + Web UI:** Provides both a terminal user interface for power users and a web interface for visual monitoring.
- **Agent Fleet:** Enables parallel execution by spawning multiple sub-agents concurrently, each with its own LLM binding, context, and tools.

## Building and Running

The OpenDev project consists of a Rust backend and a React/Vite frontend.

### Rust Backend (CLI)

To build the Rust CLI from source:

```bash
git clone https://github.com/opendev-to/opendev.git
cd opendev
cargo build --release -p opendev-cli
# The binary will be located at target/release/opendev (or opendev.exe on Windows)
```

To run the interactive TUI:

```bash
opendev
```

To run the Web UI:

```bash
opendev run ui
```

For single-prompt (non-interactive) execution:

```bash
opendev -p "explain this codebase"
```

To resume the most recent session:

```bash
opendev --continue
```

### Web UI (Frontend)

The frontend is a React/Vite application located in the `web-ui/` directory.

To build the Web UI:

```bash
cd web-ui && npm ci && npm run build
```

## Development Conventions

### Code Quality

-   **Type Checking:**
    ```bash
    cargo check --workspace
    ```
-   **Linting:**
    ```bash
    cargo clippy --workspace
    ```
-   **Formatting:**
    ```bash
    cargo fmt --all
    ```

### Testing

To run all tests for the Rust workspace:

```bash
cargo test --workspace
```

To run tests for a specific crate (e.g., `opendev-cli`):

```bash
cargo test -p opendev-cli
```

### LLM Configuration (Multi-Provider Support)

OpenDev supports multiple LLM providers and allows assigning different models to specific workflow slots. Configuration is done via environment variables or in `~/.opendev/settings.json`.

Example `settings.json` for mixing providers:

```json
{
  "model_provider": "anthropic",
  "model": "claude-sonnet-4-20250514",
  "model_thinking_provider": "openai",
  "model_thinking": "o3"
}
```

The available workflow slots are:
-   **Normal:** Primary execution model.
-   **Thinking:** Complex reasoning and planning.
-   **Compact:** Context summarization.
-   **Critique:** Self-critique of agent reasoning.
-   **VLM:** Vision/image processing.

Refer to `docs/providers.md` for detailed provider setup and advanced configuration.

### MCP Integration

OpenDev uses the Model Context Protocol (MCP) for dynamic tool discovery, enabling connection to external tools and data sources.

```bash
opendev mcp list
opendev mcp add myserver uvx mcp-server-sqlite
opendev mcp enable/disable myserver
```
