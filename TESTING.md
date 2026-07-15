# KendraCLI Testing & Verification Protocol

This document serves as the single source of truth for testing procedures, functionality documentation, and the verification log for KendraCLI implementation phases.

## 1. Standard Testing Procedures

Every phase must undergo the following validation levels before being marked as "Done" in the backlog:

### 1.1. Unit Tests
*   **Goal:** Validate isolated logic (schemas, parsing, small algorithm units).
*   **Procedure:** Run `cargo test -p <crate> --lib`. All unit tests must pass.
*   **Requirement:** New features MUST include unit tests covering edge cases.

### 1.2. Integration Tests
*   **Goal:** Validate interactions between components within a crate or between two crates.
*   **Procedure:** Run `cargo test -p <crate> --test <test_name>`.
*   **Requirement:** Mock dependencies (e.g., registries, approval channels) to simulate component interactions.

### 1.3. End-to-End (E2E) Tests
*   **Goal:** Validate the full user experience from TUI input through agent orchestration to final output/side-effect.
*   **Procedure:** Execute scenario-based scripts using `kendra --continue` or fresh sessions.
*   **Requirement:** Verify side effects (e.g., file writes) and UI feedback (e.g., approval prompt rendering).

---

## 2. Functionality & Verification Log

### Phase 5: Platform Extensions & Foundational Improvements

| Phase/Epic | Functionality | Status | Verified Date | Verification Details |
| :--- | :--- | :--- | :--- | :--- |
| **Epic F.1** | Tool Approval Diff Preview | Done | 2026-06-10 | Diff injected into `ToolApprovalRequest`; TUI renders diff in approval popup. |
| **Epic F.2** | Permission Engine & Policy | Done | 2026-06-10 | Implemented `kendra.policy.json` loader, Permission Modes mapping, and audit logging. |
| **Epic F.5** | Memory/Context (Lean-Ctx) | Done | 2026-06-10 | Implemented `StructuralContext` integration into `ContextCompactor`. |
| **Epic F.8/F.9** | Observability & Telemetry | Done | 2026-06-10 | Implemented `ObservabilityManager` and `FileTelemetryExporter` with universal headers. |
| **Epic F.6** | Durable Orchestration | In Progress | 2026-06-11 | Defined `OrchestrationState` and `GovernedOrchestrator` durable state machine. |

#### Verification for Epic F.6
1. **Unit Test:** Run `cargo test -p kendra-runtime --lib orchestration::tests` to verify state machine transitions and disk persistence.
2. **Integration Test:**
   - Start a session and verify that `<session_id>_orchestration.json` is created in the state directory.
   - Trigger a phase transition (e.g., Explore -> Plan) and verify the JSON file reflects the new phase.
   - Simulate a crash, restart the session with `--continue`, and verify the agent resumes in the correct phase.

#### Verification for Epic F.1
1. **Unit Test:** Run `cargo test -p kendra-tui` to verify the `ApprovalController` diff support.
2. **E2E Test:** Observe the approval popup in the TUI; verify that a "Diff Preview:" section is visible with green/red lines.

#### Verification for Epic F.2
1. **Unit Test:** Run `cargo test -p kendra-runtime --lib permissions::tests` to verify rule evaluation, JSON loading, and audit log generation.
2. **Integration Test:** Observe audit log entry for denied commands.

#### Verification for Epic F.5
1. **Unit Test:** Run `cargo test -p kendra-context` to verify structural context awareness in summarization and token counting.
2. **Integration Test:** Verify that the resulting summary message includes "## Project Context" and "## Dependencies" headers.

#### Verification for Epic F.8/F.9
1. **Unit Test:** Run `cargo test -p kendra-runtime --lib observability::tests` to verify telemetry collection, enrichment, and redaction.
2. **Integration Test:** Verify that events are enriched with headers and saved to the file sink in NDJSON format.

---

## 3. Functionality Documentation Reference

Detailed documentation of functionality resides in `docs/specs/`. Please consult these documents for technical architecture and behavioral definitions:

*   **Persona System:** `docs/specs/phase-5-platform-extensions.md`
*   **Marketplace Integration:** `docs/specs/phase-5-comprehensive-plan.md`
*   **Core Architecture:** `docs/specs/02_ARCHITECTURE.md`
*   **TUI/CLI Contract:** `docs/specs/03_CLI_TUI.md`
*   **Observability Config:** `docs/specs/OBSERVABILITY_CONFIG.md`
