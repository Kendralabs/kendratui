# KendraCLI: Session History & Reasoning Log

## Session Date: Monday, 8 June 2026

### 1. Project Initiation & Research
**User Request:** "Review the codebase"
**Reasoning:** To understand the architecture of kendra (kendratui) before any modifications.
**Actions:** Invoked `codebase_investigator` sub-agent.
**Internal Analysis:**
- Mapped 21 Rust crates under `crates/`.
- Identified `kendra-agents` as the ReAct loop engine.
- Identified `kendra-web` as the bridge between the Rust backend and the React frontend.
- Identified `kendra-config` as the central manager for paths and naming.
- Recognized the use of Event Sourcing in `kendra-history` and WebSocket synchronization for user approvals.

### 2. Rebranding & Platform Strategy
**User Request:** "I want to make this the kendracli and replace outward brand, and extend the whole platform. tell me what will need to be done for that?"
**Reasoning:** Transition from a general tool (kendra) to a specific product (KendraCLI).
**Internal Analysis:**
- Rebranding requires a multi-layered approach:
    - **Identity:** String replacements ("kendra" -> "KendraCLI").
    - **Filesystem:** Directory migration (`.kendra` -> `.kendra`).
    - **Architecture:** Crate renaming and dependency refactoring.
    - **UI:** Asset replacement and component updates.
- Extension focuses on:
    - **Expert Teams:** Specializing agents for specific tasks.
    - **Plugin Ecosystem:** Enhancing WASM and MCP support.

### 3. Repository Strategy
**Question:** "Is it better to create a fork for this and a new repo?"
**Analysis & Recommendation:**
- Recommended a **Branded Fork**.
- **Pros:** Keeps the link to the powerful kendra engine for upstream updates (bug fixes, new LLM providers) while allowing local divergence for branding and unique extensions.
- **User Decision:** Approved the "Branded Fork" approach.

### 4. Technical Mapping for Rebranding
**Findings:**
- `crates/kendra-config/src/paths.rs`: Contains the hardcoded `.kendra` directory and `kendra_` environment variables.
### 2. Phase 1 & 2 Execution: Global Branding & Compatibility
**Goal:** Implement branding changes while ensuring zero disruption for existing users.
**Actions:**
- **Path Refactoring:** Updated `kendra-config/src/paths.rs` to prioritize `~/.kendra` and `KENDRA_DIR` with a fallback to legacy `.kendra` paths.
- **Env Var Aliasing:** Updated `kendra-config/src/loader/env_overrides.rs` to support `KENDRA_` prefixed variables with legacy `kendra_` fallback.
- **Global Documentation Rebranding:** Systems-wide replacement of "kendra" with "KendraCLI" and "kendra" with "kendra" in `README.md`, `GEMINI.md`, and 21 crates.
- **CLI Refresh:** Updated `kendra-cli` binary name, help text, and startup logs.
- **Verification:** Updated unit tests in `kendra-config` to verify the dual-path and prioritized environment variable lookup logic.
### 3. Phase 3 Execution: Web UI Refresh
**Goal:** Align the React frontend with the KendraCLI brand identity.
**Actions:**
- **Component Rebranding:** Updated `WelcomeScreen`, `AppNavBar`, `TopBar`, `Header`, and `LandingPage` with "KendraCLI" and "KENDRA" branding.
- **Metadata Update:** Renamed package to `kendra-web-ui` in `package.json` and updated page title in `index.html`.
- **Trace Adapter Refactoring:** Updated `adapter.ts` and `trace.ts` to use `KendraCLIChatMessage` and `kendra-` prefixed UUIDs for stability.
- **Asset Alignment:** Updated all `alt` text for logos and icons to reflect the new brand.
### 5. Final Remediation & Integrity Audit
**Goal:** Ensure 100% brand consistency and resolve any residual "opendev" references.
**Actions:**
- **Audit:** Conducted a comprehensive search for remaining "opendev" and "OpenDev" strings.
- **Install Scripts:** Updated `scripts/install.sh` to reference the new KendraCLI binary and repository URLs.
- **Frontend Config:** Updated `web-ui/vite.config.ts` output directory to `kendra/web/static`.
- **CodeWiki:** Updated hardcoded path references in `DocumentationViewer.tsx` to align with the new `kendra-*` structure.
- **Filesystem Cleanup:** Renamed the root `opendev/` directory to `kendra/`.
**Reasoning:** Manual audit was necessary due to automated toolchain limitations. These final steps eliminate residual references and consolidate the brand identity across the entire repository.

## Session Date: Friday, 12 June 2026

### 1. Rebranding Integrity & Path Migration
**Goal:** Finalize Phase 2 and ensure robust legacy support.
**Actions:**
- **Path Fix:** Corrected `LEGACY_APP_DIR_NAME` to `.opendev` and fixed a shadowing bug in `Paths::new`.
- **Migration Logic:** Implemented one-time directory migration from `~/.opendev` to `~/.kendra`.
- **Env Var Fallback:** Expanded `env_overrides.rs` to support `KENDRA_`, `kendra_`, and `OPENDEV_` prefixes.
- **Global Cleanup:** Updated `LICENSE`, `Cargo.toml`, and system templates to replace remaining "opendev" and "kendra" references with "KendraCLI".
**Reasoning:** Discovered that previous rebranding attempts left inconsistencies and broken fallback logic. Ensuring a seamless transition for existing users is critical for adoption.

### 2. Expert Team Implementation
**Goal:** Implement Phase 5 specialized agent personas.
**Actions:**
- **Role Definition:** Added `Security`, `Frontend`, and `Backend` variants to `AgentRole` and `SubagentType`.
- **Prompt Engineering:** Created specialized Markdown templates for each expert persona with tailored workflows and guidelines.
- **System Integration:** Registered the new templates in `embedded.rs` and wired them into `SubagentManager` as built-in agents.
- **Tool Mapping:** Assigned specialized tool allowlists (e.g., `osv_scan` for Security expert).
**Reasoning:** Extending the platform with expert personas enables more efficient delegation and higher-quality results for domain-specific tasks.

