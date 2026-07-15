# Kendra CLI docset (export for Gemini CLI)

This folder is a clean export of the Notion docs as plain Markdown files, ready to be fed into Gemini CLI (or any repo-ingestion pipeline).

## Contents
- `manifest.json` — machine-readable index of files and source page URLs
- `00_INDEX.md` — Kendra CLI index
- `01_VISION.md`
- `02_ARCHITECTURE.md`
- `03_CLI_TUI.md`
- `04_PERMISSIONS_GOVERNANCE.md`
- `05_TOOLING_MCP_EXTENSIBILITY.md`
- `06_MEMORY_CONTEXT_SESSION.md`
- `07_ROADMAP.md`
- `10_COMPETITORS.md`
- `11_FACTORY_AI.md`

## Notes on formatting
- Notion mentions (e.g. `<mention-page .../>`) are preserved as text.
- Notion tables are preserved as `<table>` blocks.

## Suggested Gemini CLI prompt pattern
- Treat files as normative specs.
- Implement Release 1 acceptance criteria first.
- Produce a repo skeleton aligned to the six-layer architecture (Doc 2).
