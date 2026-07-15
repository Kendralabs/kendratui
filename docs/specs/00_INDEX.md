# Kendra CLI — Definitive Documentation (Index)

Source: https://app.notion.com/p/59b6f84ad3f94f4a87171ee64339f8b5

> ⌨️ **Kendra CLI — Definitive Documentation**
> Single source of truth for **Kendra CLI**: a high-trust, open, agentic software-development platform built on a dual-agent planner/executor kernel and a governed **Agent Harness**.
> Version **v1.1.0** · Status: **Draft** · Owner: Kendra Fabric · Backend contract: KACP + KOrchestrator + KMCP

## Purpose
Define Kendra CLI as a first-class Kendra Labs surface: what it is, the architecture that differentiates it, how it binds to the Kendra **Agent Harness**, and how it is delivered across releases. This page is the index and is normative for terminology; each sub-document is normative for its scope.

## Definition
**Kendra CLI** is a daily terminal coding agent that pairs an explicit **planner/executor** kernel with a production-grade **harness** (tool registry, permission engine, policy engine, hook engine, checkpoints, shadow commits, sandbox, MCP client, sub-agent runner, loop detector, audit logger). It is usable from the terminal and extensible across IDE, web, and CI surfaces, and governable for enterprise teams through runtime controls and auditability.
In short: **Agent = Model + Harness**, and Kendra CLI is the terminal-native expression of that harness over a dual-agent kernel.

## Differentiation (the kernel to preserve)
- **Explicit planner/executor separation** — a non-negotiable architectural advantage over prompt-only plan modes.
- **Context discipline for long-running tasks** — adaptive compaction, event reminders, output offloading.
- **Inspectable, open architecture** — diffs, hooks, checkpoints, shadow commits, memory proposals, and verification status are visible at every important step.

## Product principles
1. **Preserve the kernel** — expose planner/executor through commands, reviews, logs, task graphs, and chaptered sessions.
2. **Make trust visible** — permissions, diffs, hooks, checkpoints, memory proposals, and verification are always inspectable.
3. **Separate runtime from experience** — a stable, policy-aware, auditable harness; an experience layer that expands across CLI/IDE/web/CI.
4. **Optimize for bounded autonomy** — long-running automation only inside explicit permission modes, tool scopes, policy files, sandboxing, checkpoints, loop detection, and verification ladders.
5. **Prefer portable standards** — MCP for tools, A2A-style protocols for delegation, OTEL for telemetry, packageable extension bundles for ecosystem growth.

## Platform alignment
- Agent Harness: <mention-page url="https://app.notion.com/p/f6ce5c7e4f4746b0a5dcc56e4c1ef5b8"/>, authoring: <mention-page url="https://app.notion.com/p/138ab22bc27d434a8e7212ba1c688178"/>
- KACP: profile registry + governance tiers
- KOrchestrator: durable runs + checkpoints + sub-agent semantics
- KMCP: tool-calling layer; KIAM identity

## Document set
1. <mention-page url="https://app.notion.com/p/8f5e97de147c48489819fa1f42d9b715"/>
2. <mention-page url="https://app.notion.com/p/3a90a9140cd44e40a1dac87392fd3d92"/>
3. <mention-page url="https://app.notion.com/p/5712f16eea1145c6ac569f234e4274c3"/>
4. <mention-page url="https://app.notion.com/p/21bb7cf7efaf45d9807a9ab3e8e0d089"/>
5. <mention-page url="https://app.notion.com/p/c281c6f242d7473b845517cae313acbe"/>
6. <mention-page url="https://app.notion.com/p/c73a2a2dc13e427ab19c87270abaf80f"/>
7. <mention-page url="https://app.notion.com/p/a413ba0d31b24d4a8639c126e8f6200f"/>

## Competitors
- <mention-page url="https://app.notion.com/p/6b849ab1b80441979285ea7662efccc3"/>
	- <mention-page url="https://app.notion.com/p/136d4eb4ffed4cb78edb5050814b35b4"/>
