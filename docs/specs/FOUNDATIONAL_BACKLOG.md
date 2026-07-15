# KendraCLI Foundational Backlog

This file tracks the foundational and architectural work items based on the project specification documents (`docs/specs`).

## Epic: Core Harness & Governance (Release 1 & 2) [Jira: KCLI-8]
| Story | Description | Priority | Status |
| :--- | :--- | :--- | :--- |
| **F.1** | **Core TUI/CLI Architecture:** Finalize stable terminal contracts and diff/review engine. [Jira: KCLI-13] | High | ToDo |
| **F.2** | **Permission Engine:** Implement layered governance based on policy files. [Jira: KCLI-14] | High | In Progress |
| | F.2.1 | Design policy file (`kendra.policy.json`) loading and parsing | Done |
| | F.2.2 | Implement Permission Modes (`default`, `plan`, `auto`, `bypass`) | Done |
| | F.2.3 | Integrate audit logging for permission denials/prompts | Done |
| **F.3** | **Hook Engine:** Develop the infrastructure for pre/post-operation hooks. [Jira: KCLI-15] | Medium | ToDo |
| **F.4** | **Headless/CI Support:** Enable non-interactive execution (`kendra exec`) for CI environments. [Jira: KCLI-16] | Medium | ToDo |

## Epic: Context & Orchestration (Release 2 & 3) [Jira: KCLI-9]
| Story | Description | Priority | Status |
| :--- | :--- | :--- | :--- |
| **F.5** | **Memory/Context Layer:** Implement 'lean-ctx' for efficient codebase understanding. [Jira: KCLI-17] | High | Done |
| **F.6** | **Durable Orchestration:** Implement governance for planner → worker → validator loops. [Jira: KCLI-18] | High | ToDo |
| **F.7** | **Streaming SDK:** Develop JSON-RPC streaming interface for external surface integration. [Jira: KCLI-19] | Medium | ToDo |

## Epic: Observability & Telemetry (Release 2) [Jira: KCLI-10]
| Story | Description | Priority | Status |
| :--- | :--- | :--- | :--- |
| **F.8** | **Native Telemetry Collector:** Integrate ubiquitous event collection via `EventBus`. [Jira: KCLI-20] | High | Done |
| **F.9** | **Pluggable Exporters:** Implement exporters (JSON, OTEL) for observability sink. [Jira: KCLI-21] | High | Done |

## Release Plan
1.  **Release 1 (Terminal Trust):** Stabilize TUI, Diff/Review engine.
2.  **Release 2 (Harness Maturity):** Tool registry, Permissions/Policies, Hook engine, CI support, Memory/Context.
3.  **Release 3 (Governed Breadth):** SDK, Durable orchestration, CI automation.

## Testing Strategy
*   **Unit Tests:** Core engine logic, policy evaluator, context compaction, hook execution.
*   **Integration Tests:** TUI/CLI interaction flows, policy file enforcement, context window management.
*   **E2E Tests:** Headless CI workflows, full planning-execution-verification lifecycle for complex tasks.
