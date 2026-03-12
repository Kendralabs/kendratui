# OpenDev: Python to Rust Migration

## Overview

This directory contains the complete strategy and documentation for migrating the OpenDev AI coding assistant from Python (~120K LOC) to Rust. The React/Vite frontend (~15K LOC TypeScript) remains unchanged.

## Documents

| Document | Description |
|----------|-------------|
| [STRATEGY.md](./STRATEGY.md) | High-level migration strategy, goals, principles, and risks |
| [ARCHITECTURE.md](./ARCHITECTURE.md) | Rust workspace structure, crate design, and module layout |
| [PHASES.md](./PHASES.md) | Detailed 7-phase migration plan with source files and deliverables |
| [CRATE_MAPPING.md](./CRATE_MAPPING.md) | Python dependency → Rust crate mapping for every component |
| [AGENT_TEAMS.md](./AGENT_TEAMS.md) | Agent team structure, assignments, and parallelism timeline |
| [TESTING.md](./TESTING.md) | Testing and verification strategy for each phase |

## Quick Start

1. Read STRATEGY.md for the big picture
2. Read ARCHITECTURE.md to understand the Rust workspace layout
3. Read PHASES.md for what to build and in what order
4. Use AGENT_TEAMS.md to assign work to parallel agent teams
5. Follow TESTING.md to verify each phase before moving on

## Key Decisions

- **Incremental migration via PyO3**: Each Rust crate exposes Python bindings so the system remains functional at every stage
- **7 phases ordered by dependency depth**: Models → HTTP → Context → Tools → Agents → Web → TUI
- **React frontend unchanged**: The Rust web backend (axum) must match the existing FastAPI REST/WebSocket API exactly
- **Session compatibility**: Rust must read/write the same JSON session format as Python during the transition period
