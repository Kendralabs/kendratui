# Phase 5: Platform Extensions Comprehensive Plan

## 1. Overview
This specification details the implementation plan for Phase 5 of KendraCLI, focusing on the Persona System and Marketplace Integration.

## 2. Epic A: Expert Agent Teams (Persona System)
*Goal: Provide specialized coding assistance and tools via dynamic persona loading.*

### Story A.1: Persona Configuration Schema
*   **Description:** Define a schema for persona definitions to ensure consistency.
*   **Tasks:**
    *   **A.1.1:** Design `team.json` schema (`system_prompt`, `enabled_tools`, `context_scopes`).
    *   **A.1.2:** Implement JSON validation logic in `kendra-config`.
*   **Dependency:** `kendra-config` path management.

### Story A.2: Dynamic Prompt Injection
*   **Description:** Refactor agent runtime to inject persona-specific prompts.
*   **Tasks:**
    *   **A.2.1:** Modify agent loop to read current persona from `AppState`.
    *   **A.2.2:** Prepend `system_prompt` to LLM turn history dynamically.
*   **Dependency:** Story A.1.

### Story A.3: Persona-Specific Tool Binding
*   **Description:** Intersect persona tools with available tools.
*   **Tasks:**
    *   **A.3.1:** Refactor `AgentManager` to intersect MCP tool registry with `enabled_tools` array.
*   **Dependency:** Story A.2.

---

## 3. Epic B: Marketplace Integration (Plugin System)
*Goal: Enable extensibility via community plugins.*

### Story B.1: Marketplace Index Schema
*   **Description:** Define the registry for plugin discovery.
*   **Tasks:**
    *   **B.1.1:** Define `registry.json` schema (plugin ID, description, version, install URL).

### Story B.2: Plugin Discovery CLI
*   **Description:** Implement CLI interface for plugin management.
*   **Tasks:**
    *   **B.2.1:** Implement `kendra marketplace list/search`.
    *   **B.2.2:** Implement `kendra marketplace install`.
*   **Dependency:** Story B.1, `kendra-mcp`.

### Story B.3: Secure Sandboxed Tool Execution
*   **Description:** Force plugin execution via sandbox.
*   **Tasks:**
    *   **B.3.1:** Update `kendra-mcp` to wrap plugin calls in `sandbox_exec`.
*   **Dependency:** `kendra-sandbox`.

---

## 4. Build & Release Plan
1.  **Milestone 1 (Persona Core):** Epic A stories.
2.  **Milestone 2 (Marketplace Core):** Epic B.1 & B.2.
3.  **Milestone 3 (Sandboxing):** Epic B.3 (Security Hardening).

## 5. Testing Strategy
*   **Unit Tests:** Test schema validation (`kendra-config`), prompt injection logic (`kendra-agents`), and registry parsing (`kendra-mcp`).
*   **Integration Tests:** Verify persona switching in a simulated TUI session; test marketplace installation end-to-end against a mock registry.
*   **E2E Tests:** Execute a complex workflow with a specialized persona (e.g., Security Team scanning a directory) and a marketplace plugin, verifying output and sandbox compliance.
