# KendraCLI Phase 5 Tracking: Platform Extensions

This file tracks the implementation of Phase 5 features, mapping Epics, Stories, and Tasks.

## Epic A: Expert Agent Teams (Persona System) [Jira: KCLI-11]
*Goal: Specialized coding assistance via dynamic persona loading.*

| Story | Task | Description | Status |
| :--- | :--- | :--- | :--- |
| **A.1** | **A.1.1** | Design `team.json` schema (`system_prompt`, `enabled_tools`, `context_scopes`) [Jira: KCLI-22] | Done |
| | **A.1.2** | Implement JSON validation logic in `kendra-config` | Done |
| **A.2** | **A.2.1** | Modify agent loop to read current persona from `AppState` [Jira: KCLI-23] | TODO |
| | **A.2.2** | Prepend `system_prompt` to LLM turn history dynamically | TODO |
| **A.3** | **A.3.1** | Refactor `AgentManager` to intersect MCP tool registry with `enabled_tools` [Jira: KCLI-24] | TODO |

## Epic B: Marketplace Integration (Plugin System) [Jira: KCLI-12]
*Goal: Extensibility via community plugins.*

| Story | Task | Description | Status |
| :--- | :--- | :--- | :--- |
| **B.1** | **B.1.1** | Define `registry.json` schema (plugin ID, description, version, install URL) [Jira: KCLI-25] | TODO |
| **B.2** | **B.2.1** | Implement `kendra marketplace list/search` [Jira: KCLI-26] | TODO |
| | **B.2.2** | Implement `kendra marketplace install` | TODO |
| **B.3** | **B.3.1** | Update `kendra-mcp` to wrap plugin calls in `sandbox_exec` [Jira: KCLI-27] | TODO |

## Release Plan
1.  **Release 1 (Persona Core):** Epic A implementation.
2.  **Release 2 (Marketplace Core):** Epic B.1 & B.2.
3.  **Release 3 (Security & Hardening):** Epic B.3 (Sandboxing).

## Testing Plan
*   **Unit Tests:** Schema validation (`kendra-config`), Prompt injection (`kendra-agents`), Registry parsing (`kendra-mcp`).
*   **Integration Tests:** Persona switching in TUI, Plugin install against mock registry.
*   **E2E Tests:** Specialized persona scanning workflow, Marketplace plugin usage with sandbox compliance.
