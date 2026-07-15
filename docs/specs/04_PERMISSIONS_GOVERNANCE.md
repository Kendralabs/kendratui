# 4 — Permissions, Policy & Governance

Source: https://app.notion.com/p/21bb7cf7efaf45d9807a9ab3e8e0d089

## 1. Layered permission engine
Deny always wins.

## 2. Permission modes
`default` · `acceptEdits` · `plan` · `auto` · `dontAsk` · `bypassPermissions`

## 3. Policy files
`kendra.policy.yaml`: allow/deny/ask + tool matchers + MCP wildcards.

## 4. Hook engine
`PreToolUse` · `PostToolUse` · `PostToolUseFailure` · `PermissionRequest` · `Stop` · `SubAgentStop`.

## 5. Enterprise governance (KACP alignment)
Low/Medium/High/Restricted + 4-eyes review + freeze windows + signed publishes.

## 6. Audit & telemetry
Signed audit log; export JSON/NDJSON; OTEL metrics.
