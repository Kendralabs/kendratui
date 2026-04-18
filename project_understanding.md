# Comprehensive Project Understanding: OpenDev

This document synthesizes information from `README.md`, `GEMINI.md`, `docs/architecture.md`, `migration_docs/ARCHITECTURE.md`, `docs/providers.md`, `docs/subagent-execution-model.md`, and `docs/agent-framework-refactoring.md` to provide a holistic view of the OpenDev project.

## 1. Architecture Overview

OpenDev is an open-source, terminal-native coding agent built as a **compound AI system** within a **layered Rust workspace**. It's designed as an operating environment for cooperating agent workflows, rather than a monolithic chatbot.

### Design Thesis
1.  **Runtime, not just a prompt wrapper:** It manages sessions, history, interruptions, and cost tracking.
2.  **Compound AI:** Different models from various providers can be bound to specific workflow slots (Execution, Thinking, Compaction, Critique, Vision).
3.  **First-class tools:** Tools are integral execution primitives.
4.  **Separation of Concerns:** User interaction, agent reasoning, and infrastructure are isolated into distinct Rust crates.

### Core Components and Layers

The project is structured into five main layers, each corresponding to a set of Rust crates:

-   **Interaction Layer:** User-facing interfaces (`opendev-cli`, `opendev-tui`, `opendev-web`, `opendev-repl`, `opendev-channels`).
-   **Orchestration Layer:** The control center for agents, context, configuration, and runtime management (`opendev-agents`, `opendev-context`, `opendev-config`, `opendev-runtime`).
-   **Tool Platform:** Provides the action system with various tool implementations (`opendev-tools-core`, `opendev-tools-impl`, `opendev-tools-lsp`, `opendev-tools-symbol`, `opendev-mcp`, `opendev-plugins`, `opendev-hooks`).
-   **Provider + Persistence Layer:** Handles external and durable state, including LLM provider adapters and session history (`opendev-http`, `opendev-history`, `opendev-memory`, `opendev-docker`).
-   **Shared Types:** Common data models and types used across crates (`opendev-models`).

### Runtime Request Flow

A user request progresses through six stages:
1.  **User Input:** Via CLI, TUI, Web UI, REPL, or external channels.
2.  **Config + Session Resolution:** Establishes working directory, project instructions, configuration, session identity, and active model bindings.
3.  **Prompt + Context Construction:** Dynamically assembles prompts from templates, instructions, tool descriptions, and conversation history, often with compaction.
4.  **Agent ReAct Loop:** The core execution engine (`think` -> `select tools` -> `run tools` -> `observe results` -> `continue or finish`). This is where autonomy resides, implemented in `opendev-agents`.
5.  **Tool Execution + Provider Calls:** Agents can invoke built-in tools, MCP-discovered tools, LSP/symbol tools, or external model providers via `opendev-http`.
6.  **State Persistence + UI Events:** Progress is streamed to the UI, session history is recorded, costs are tracked, and snapshots are persisted for resumability.

### Subagent Architecture

-   Subagents are **logical child agents**, not separate OS processes or dedicated threads.
-   They are executed as **async tasks/futures** within the main `opendev` process, scheduled by Tokio.
-   Each subagent has its own isolated state (prompt, task, message history, tool allowlist, permissions, cancellation token).
-   They run concurrently when multiple `spawn_subagent` calls are emitted in the same model response.
-   Future plans include enhancements for subagent lifecycle, team systems, and worktree isolation.

## 2. Code Structure and Implementation Details

### Rust Workspace Layout

The `crates/` directory contains numerous Rust crates, each representing a distinct module as described in the architectural layers. Key crates include:

-   `opendev-models`: Defines shared data types (messages, sessions, config).
-   `opendev-config`: Manages hierarchical configuration loading and paths.
-   `opendev-http`: Handles HTTP requests, authentication, and LLM provider adapters.
-   `opendev-context`: Implements context engineering, prompt compaction, and token monitoring.
-   `opendev-history`: Manages session persistence, indexing, undo functionality, and sidechain transcripts for subagents.
-   `opendev-memory`: Provides memory systems like ACE playbooks, embeddings, and reflection.
-   `opendev-tools-core`: Defines tool traits, registry, and contracts.
-   `opendev-tools-impl`: Contains concrete implementations of various tools (bash, file I/O, git, web, memory, subagent spawning).
-   `opendev-tools-lsp` & `opendev-tools-symbol`: Integrate LSP and AST-based symbol operations for code intelligence.
-   `opendev-agents`: Implements the core ReAct loop, prompt composition, and subagent management.
-   `opendev-mcp`: Integrates with the Model Context Protocol for external tool discovery.
-   `opendev-web`: Provides the Axum-based backend for the Web UI.
-   `opendev-tui`: Implements the terminal user interface using ratatui.
-   `opendev-cli`: The main binary entry point, dispatching to TUI/Web/REPL.

### Key Trait Definitions

-   `Tool`: Defines the interface for all tools (name, description, schema, `execute` method).
-   `UICallback`: Trait for UI event handling (e.g., `on_thinking_start`, `on_tool_call`, `on_cost_update`).
-   `BaseAgent`: Main agent structure, composing `HttpClient`, `LlmCaller`, `ToolRegistry`, etc.

### Async Model
-   Built on **Tokio** for asynchronous execution.
-   The ReAct loop runs as a Tokio task; tool executions are `async fn`.
-   `tokio::task::spawn_blocking` is used for blocking operations.
-   Interrupts are managed via `tokio_util::sync::CancellationToken`.

### Error Handling
-   `thiserror` is used for typed errors in library crates.
-   `anyhow` is used for ergonomic error propagation in the application crate.

### Logging
-   Uses the `tracing` crate for structured logging with spans, supporting per-crate filtering.

## 3. Functionality and Features

### Core Agent Capabilities
-   **Multi-Provider LLM Support:** Seamlessly integrates with 9 major LLM providers (OpenAI, Anthropic, Fireworks, Google, Groq, Mistral, DeepInfra, OpenRouter, Azure OpenAI) through a unified `opendev-http` layer.
-   **Configurable Workflow Models:** Allows binding different models to specific workflows (Normal, Thinking, Compact, Critique, VLM) for optimized performance and cost.
-   **Dynamic Context Engineering:** Constructs prompts dynamically, incorporating system templates, reminders, project instructions, tool schemas, and conversation history, with context compaction when history grows too long.
-   **ReAct Loop-based Autonomy:** The core agent logic follows a ReAct pattern for reasoning and tool use.
-   **Tool Execution:** Access to a rich set of built-in tools for file system operations, git, web browsing/searching/fetching, and specialized LSP/symbol interactions.
-   **Subagent Spawning:** Can spawn child agents for parallel or specialized tasks, with ongoing refactoring to enhance subagent lifecycle, communication, and worktree isolation.

### User Interfaces
-   **Terminal User Interface (TUI):** A feature-rich, interactive terminal interface for power users, built with `ratatui`.
-   **Web UI:** A React/Vite-based web interface for visual monitoring and remote interaction.

### Development and Operations
-   **Comprehensive Testing:** The project emphasizes unit, integration, and TUI unit tests, along with real simulation tests.
-   **Config System:** Hierarchical configuration (project > global > defaults) with environment variable overrides and secure credential storage.
-   **MCP Integration:** Supports dynamic tool discovery via the Model Context Protocol.

### Planned Enhancements (from `docs/agent-framework-refactoring.md`)
-   **TaskManager:** For managing the lifecycle of tasks (pending, running, completed, failed, killed) including background agents.
-   **Sidechain Transcripts:** Persistent history for subagents, stored in `~/.opendev/sessions/{parent_session_id}/agents/{agent_id}.jsonl`.
-   **Background Agent Execution:** Enhanced support for running agents in the background, including auto-backgrounding and mid-execution backgrounding via Ctrl+B.
-   **Agent Team System:** Implementation of agent teams with mailbox-based communication and a `TeamManager`.
-   **Git Worktree Isolation:** Providing isolated worktrees for agents to perform changes without affecting the main repository directly.
-   **Enhanced TUI Wiring:** Detailed plans for integrating all new backend features into the TUI, including task watchers, improved status displays, and keybindings.

This comprehensive overview provides a solid foundation for understanding the OpenDev project, its current state, and its future direction.
