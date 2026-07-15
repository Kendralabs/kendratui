# Phase 5: Platform Extensions Requirements Plan

## 1. Overview
This specification outlines the requirements for Phase 5 of KendraCLI, focusing on "Expert Agent Teams" (Persona System) and "Marketplace Integration" (Plugin System).

## 2. Epic: Platform Extensibility & Specialization

### Story 1: Expert Agent Teams (Persona System)
*   **Description:** As a developer, I want to switch between specialized agent teams (Security, Frontend, Backend) so that I get domain-specific coding assistance and tools.
*   **Detailed Description:** This feature introduces "Team Personas". A persona consists of a set of configurations that modify the agent's behavior, instructions, and capabilities. Users should be able to trigger a change in persona via CLI flag or interactive session change, which updates the agent's system prompt and tool availability.
*   **Acceptance Criteria:**
    *   System can load valid persona configurations from `~/.kendra/teams/`.
    *   Switching personas immediately updates the agent's system prompt.
    *   Persona-specific MCP tools are automatically enabled/bound when the persona is active.
*   **Tasks:**
    *   **1.1 Persona Configuration Schema:** Create a JSON/YAML schema definition for `team.json` files. Must support `system_prompt`, `enabled_tools`, and `context_scopes` fields.
    *   **1.2 Dynamic Prompt Injection:** Refactor `kendra-agents` to read the active persona configuration and prepend the `system_prompt` to the LLM turn context.
    *   **1.3 Persona-specific Tool binding:** Update the `AgentManager` to intersect available MCP tools with the `enabled_tools` list defined in the persona.

### Story 2: Marketplace Integration (Plugin System)
*   **Description:** As a developer, I want to discover and install community-contributed tools so that I can extend KendraCLI's functionality.
*   **Detailed Description:** This introduces a centralized marketplace for MCP-based plugins. Users need a way to search, install, and manage these plugins safely. Plugins should be executed within the `kendra-sandbox` to ensure no unauthorized system access.
*   **Acceptance Criteria:**
    *   `kendra marketplace` CLI command allows searching for available plugins via a remote JSON index.
    *   `kendra marketplace install <name>` downloads and registers an MCP server configuration.
    *   Installed plugins run within the `kendra-sandbox` runtime.
*   **Tasks:**
    *   **2.1 Marketplace Index Schema:** Design a schema for a JSON-based registry (`registry.json`) that lists available plugins, their descriptions, and installation URLs.
    *   **2.2 Plugin Discovery CLI:** Implement CLI subcommands (`list`, `search`, `install`). Installation should automatically update the global `mcp.json` file.
    *   **2.3 Secure Sandboxed Tool Execution:** Ensure the `mcp` client configuration forces all plugin executions to pass through `sandbox_exec`.
