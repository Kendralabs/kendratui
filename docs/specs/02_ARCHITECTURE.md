# 2 — Architecture & Subsystems (Six-Layer Model)

Source: https://app.notion.com/p/3a90a9140cd44e40a1dac87392fd3d92

> 🏗️ **Kendra CLI — Doc 2: Architecture & Subsystems**

## 1. Six-layer architecture
### Layer 1 — Kernel
Planner, executor, memory engine, compaction engine, event reminders.

### Layer 2 — Harness
Tool registry, permission engine, policy engine, hook engine, checkpoint service, shadow-commit service, sandbox service, MCP client, A2A client, sub-agent runner, loop detector, audit logger.

### Layer 3 — Execution
Shell runtime, PTY shell, filesystem tools, git tools, web tools, browser agent, LSP adapters, verification pipelines, worktree manager.

### Layer 4 — Experience
CLI/TUI, IDE extensions, web dashboard, review UI, share/export artifacts, session chapters.

### Layer 5 — Ecosystem
Extension registry, plugin packaging, skill bundles, MCP packages, partner adapters.

### Layer 6 — Control
Config scopes, org policy, integration permissions, evaluation harness, telemetry, audit export.

## 2. Layer map
<table fit-page-width="true" header-row="true">
<tr>
<td>Layer</td>
<td>Owns</td>
<td>Primary concern</td>
</tr>
<tr>
<td>1 Kernel</td>
<td>Planner, executor, memory, compaction, reminders</td>
<td>Reasoning & plan/execute separation</td>
</tr>
<tr>
<td>2 Harness</td>
<td>Registry, permissions, policy, hooks, checkpoints, sandbox, MCP/A2A, sub-agents, audit</td>
<td>Trust, safety, capability brokering</td>
</tr>
<tr>
<td>3 Execution</td>
<td>Shell/PTY, FS, git, web/browser, LSP, verification, worktrees</td>
<td>Side-effects & isolation</td>
</tr>
<tr>
<td>4 Experience</td>
<td>CLI/TUI, IDE, web, review UI, session chapters</td>
<td>Developer experience</td>
</tr>
<tr>
<td>5 Ecosystem</td>
<td>Extension registry, packaging, skills, MCP packages</td>
<td>Extensibility & distribution</td>
</tr>
<tr>
<td>6 Control</td>
<td>Config scopes, org policy, eval, telemetry, audit export</td>
<td>Governance & operations</td>
</tr>
</table>

## 3. Mapping to the Kendra Agent Harness
<table fit-page-width="true" header-row="true">
<tr>
<td>Kendra CLI harness component</td>
<td>Agent Harness subsystem</td>
</tr>
<tr>
<td>Model routing/fallback</td>
<td>Model binding (single · router · cascade)</td>
</tr>
<tr>
<td>Canonical tool registry</td>
<td>Tool registry</td>
</tr>
<tr>
<td>Layered permission engine</td>
<td>Permission engine</td>
</tr>
<tr>
<td>Hook engine (PreToolUse, PostToolUse, …)</td>
<td>Hook engine</td>
</tr>
<tr>
<td>Checkpoint + shadow-commit services</td>
<td>Checkpoint service</td>
</tr>
<tr>
<td>Sandbox / worktree / container</td>
<td>Sandbox service</td>
</tr>
<tr>
<td>MCP client (scoped)</td>
<td>MCP client</td>
</tr>
<tr>
<td>Sub-agent runner + A2A client</td>
<td>Sub-agent runner</td>
</tr>
<tr>
<td>Audit logger (signed, OTEL)</td>
<td>Audit logger</td>
</tr>
</table>

## 4. Capability matrix (current → target)
<table fit-page-width="true" header-row="true">
<tr>
<td>Capability</td>
<td>Current strength</td>
<td>Gap</td>
<td>Target</td>
</tr>
<tr>
<td>Agent architecture</td>
<td>Dual-agent planner/executor, lazy tool discovery, adaptive compaction, event reminders, project memory</td>
<td>Limited user-facing exposure</td>
<td>Plan mode, task graph, explicit checkpoints, session chapters, branchable sessions</td>
</tr>
<tr>
<td>CLI usability</td>
<td>Emerging CLI/TUI</td>
<td>No mature stable commands, rewind, share/export</td>
<td>Claude-level ergonomics: plan/run, resume, rewind, diff, verify, config inspect</td>
</tr>
<tr>
<td>Permission control</td>
<td>Safety-focused foundation</td>
<td>No explicit layered engine / policy UX</td>
<td>Deny-first engine, six modes, project/org policy files, logged decisions</td>
</tr>
<tr>
<td>Hooks & automation</td>
<td>Unclear</td>
<td>No lifecycle interception</td>
<td>Full hook system across tool/stop/compaction events</td>
</tr>
<tr>
<td>Tooling model</td>
<td>Strong harness potential</td>
<td>No canonical registry/schemas</td>
<td>Central registry with dispatch, budgets, audit logging</td>
</tr>
<tr>
<td>Memory & context</td>
<td>Strong architectural base</td>
<td>No project instruction model / inbox / patching</td>
<td>KENDRA.md, layered config, four-tier memory, inbox review, checkpointing</td>
</tr>
<tr>
<td>Model/provider</td>
<td>Specialized routing</td>
<td>No polished abstraction/switch UX</td>
<td>Model-agnostic routing, fallback, live switch, air-gapped local mode</td>
</tr>
<tr>
<td>Tool scoping & MCP</td>
<td>Lazy discovery</td>
<td>No productized scoping/wildcards</td>
<td>Scoped MCP/tool enablement by repo/path/task/role, schema-on-demand</td>
</tr>
<tr>
<td>Workspace isolation</td>
<td>Aligned with safe execution</td>
<td>No worktree/container model</td>
<td>Worktrees, shadow git commits, optional containers</td>
</tr>
<tr>
<td>IDE & web</td>
<td>Terminal-centric</td>
<td>No IDE ext / web control plane</td>
<td>CLI, VS Code, JetBrains, web dashboard, mobile-safe review</td>
</tr>
<tr>
<td>Integrations</td>
<td>Minimal</td>
<td>No GitHub Actions/Jira/Slack/Notion/Sentry</td>
<td>Engineering-system integration for delegated tasks, CI review, promotion</td>
</tr>
<tr>
<td>Governance & audit</td>
<td>Partial by intent</td>
<td>No tamper-evident audit / policy-as-code</td>
<td>Signed audit log, org policy, OTEL, role-based approvals, eval harness</td>
</tr>
<tr>
<td>Agent interop</td>
<td>None</td>
<td>No remote-agent delegation</td>
<td>A2A-compatible discovery, auth, audited delegation</td>
</tr>
<tr>
<td>Ecosystem</td>
<td>Early concepts</td>
<td>No installable packaging/trust controls</td>
<td>Marketplace of skills, hooks, MCP servers, sub-agents</td>
</tr>
</table>
