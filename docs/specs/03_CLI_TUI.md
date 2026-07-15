# 3 — CLI & TUI Experience (Commands, Sessions, Review)

Source: https://app.notion.com/p/5712f16eea1145c6ac569f234e4274c3

> ⌨️ **Kendra CLI — Doc 3: CLI & TUI Experience**

## 1. Command contract
- `kendra plan` — produce a planner-only execution graph with **no file edits**.
- `kendra run` — execute against an approved plan.

Slash commands:
`/plan` · `/build` · `/review` · `/approve` · `/revert` · `/rewind` · `/model` · `/tools` · `/memory` · `/mcp`.

## 2. Sessions
- stable session IDs; resumable planner state
- TUI shows: session ID, permission mode, model, tool scope, checkpoint state
- chaptered + branchable sessions (task graph)

## 3. Rewind & navigation
`/rewind <n>` backed by checkpoints + shadow commits.

## 4. Diff review & approval
- no edits without reviewable diff (unless policy)
- shell/git/network routed through permission engine
- approvals persisted and auditable

## 5. Configuration surfacing
- `KENDRA.md` upward traversal
- `kendra.jsonc` policy/model/memory/tool-scope
- inspectable effective config

## 6. Acceptance criteria (Release 1 cut)
- planner-only graph without edits
- resume session
- always-visible TUI status
- `/rewind` branches/restores
- diffs before edits; gated tools; persisted approvals
- correct `KENDRA.md`/`kendra.jsonc` resolution
