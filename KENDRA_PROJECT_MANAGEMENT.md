# KendraCLI: Project Management & Status

## 1. Project Overview
- **Product Name:** KendraCLI
- **Base Engine:** kendra (kendratui)
- **Strategy:** Branded Fork
- **Goal:** A professional-grade, terminal-native AI coding agent with expert team extensions.

---

## 2. Current Work (Status: ACTIVE)
- [x] Comprehensive architectural review.
- [x] Rebranding impact analysis (mapping files/crates).
- [x] Migration strategy defined (Branded Fork).
- [x] Documentation of reasoning and session history.

---

## 3. Planned Work (Roadmap)

### Phase 1: Global Branding & Identity
- [x] **String Replacement:** "kendra" -> "KendraCLI" across all documentation and comments.
- [x] **System Naming:** "kendra" -> "kendra" in binary names and scripts.
- [x] **Documentation Update:** Refactor `README.md`, `GEMINI.md`, and architectural docs.

### Phase 2: Configuration & Pathing Migration
- [x] **Directory Change:** Update `APP_DIR_NAME` to `.kendra`.
- [x] **Compatibility Layer:** Implement fallback logic to check for `~/.opendev` if `~/.kendra` is missing.
- [x] **Env Vars:** Update `kendra_` to `KENDRA_` with fallback support for legacy variables (`OPENDEV_`).
- [x] **Migration Logic:** Ensure `~/.opendev` configs are migrated to `~/.kendra` on first run.

### Phase 3: Frontend UI Refresh
- [x] **Component Branding:** Update text in `AppNavBar`, `TopBar`, and `WelcomeScreen`.
- [x] **Asset Update:** Replace icons and logos in `web-ui/public/` (alt text and names).
- [x] **Build Pipeline:** Update `package.json` names and output directories.

### Phase 4: Crate Refactoring
- [x] **Crate Renaming:** Rename all 21 crates (e.g., `opendev-agents` -> `kendra-agents`).
- [x] **Dependency Mapping:** Update all internal `Cargo.toml` path references.
- [x] **Binary Output:** Configure `kendra-cli` to output the `kendra` binary.


### Phase 5: Platform Extensions
- [x] **Expert Teams:** Implement agent personas for Security, Frontend, and Backend.
- [ ] **Marketplace Integration:** Finalize plugin discovery and installation.

---

## 4. Status Logs

| Date | Category | Description | Status |
| :--- | :--- | :--- | :--- |
| 2026-06-08 | Research | Architectural mapping of 21 crates. | COMPLETED |
| 2026-06-08 | Strategy | Branded Fork approach approved. | COMPLETED |
| 2026-06-08 | Management | Creation of History and Project Management logs. | IN PROGRESS |

---

## 5. Maintenance Note
**Once a module or phase is built and tested, I will prompt to update these files with the new status and technical details.**

