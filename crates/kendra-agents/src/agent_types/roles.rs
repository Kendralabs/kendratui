//! Predefined agent roles for common development tasks.

use serde::{Deserialize, Serialize};

/// Predefined agent roles for common development tasks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum AgentRole {
    /// General-purpose coding agent with full tool access.
    Code,
    /// Planning agent focused on architecture and task decomposition.
    Plan,
    /// Testing agent specialized in writing and running tests.
    Test,
    /// Build agent for compilation, linting, and CI tasks.
    Build,
    /// Security agent specialized in vulnerability research and audits.
    Security,
    /// Frontend agent expert in UI/UX, React, and CSS.
    Frontend,
    /// Backend agent expert in Rust, APIs, and systems design.
    Backend,
}

impl AgentRole {
    /// Return the default system prompt snippet for this role.
    pub fn default_system_prompt(&self) -> &'static str {
        match self {
            AgentRole::Code => {
                "You are a coding agent. Your primary job is to read, write, and edit \
                 source code. Use tools to explore the codebase, make targeted edits, \
                 and verify your changes compile. Focus on correctness and minimal diffs."
            }
            AgentRole::Plan => {
                "You are a planning agent. Analyze the user's request and break it into \
                 concrete, ordered steps. Identify files to change, dependencies between \
                 tasks, and potential risks. Do NOT make code changes yourself — produce \
                 a structured plan for execution agents."
            }
            AgentRole::Test => {
                "You are a testing agent. Your job is to write, run, and verify tests. \
                 Read the relevant source code, write comprehensive tests covering edge \
                 cases, run them, and report results. Fix any failing tests you introduce."
            }
            AgentRole::Build => {
                "You are a build agent. Your job is to compile the project, run linters, \
                 and fix any build or lint errors. Focus on making the project build \
                 cleanly with zero warnings."
            }
            AgentRole::Security => {
                "You are a security agent. Your job is to conduct security audits, \
                 identify vulnerabilities (OWASP Top 10, memory safety, etc.), and \
                 recommend remediations. Use tools to analyze source code, dependencies, \
                 and configurations for potential risks."
            }
            AgentRole::Frontend => {
                "You are a frontend agent. Your expertise is in UI/UX, React, TypeScript, \
                 Vite, and modern CSS. Your job is to implement and polish user interfaces, \
                 ensure responsiveness, and optimize frontend performance. Focus on \
                 clean component architecture and accessibility."
            }
            AgentRole::Backend => {
                "You are a backend agent. Your expertise is in Rust, Axum, database design, \
                 and API development. Your job is to implement robust, scalable backend \
                 services, handle data persistence, and ensure secure communication. \
                 Focus on type safety, error handling, and performance."
            }
        }
    }

    /// Return the default tool allowlist for this role.
    ///
    /// An empty vec means "all tools" (no restriction).
    pub fn default_tools(&self) -> Vec<String> {
        match self {
            AgentRole::Code => vec![], // all tools
            AgentRole::Plan => vec![
                "read_file".into(),
                "list_files".into(),
                "grep".into(),
                "find_symbol".into(),
                "find_referencing_symbols".into(),
                "web_search".into(),
                "task_complete".into(),
            ],
            AgentRole::Test => vec![
                "read_file".into(),
                "write_file".into(),
                "edit_file".into(),
                "list_files".into(),
                "grep".into(),
                "bash".into(),
                "task_complete".into(),
            ],
            AgentRole::Build => vec![
                "read_file".into(),
                "edit_file".into(),
                "bash".into(),
                "list_files".into(),
                "grep".into(),
                "task_complete".into(),
            ],
            AgentRole::Security => vec![
                "read_file".into(),
                "list_files".into(),
                "grep".into(),
                "web_search".into(),
                "osv_scan".into(),
                "task_complete".into(),
            ],
            AgentRole::Frontend => vec![], // all tools
            AgentRole::Backend => vec![],  // all tools
        }
    }
}

impl std::fmt::Display for AgentRole {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            AgentRole::Code => write!(f, "Code"),
            AgentRole::Plan => write!(f, "Plan"),
            AgentRole::Test => write!(f, "Test"),
            AgentRole::Build => write!(f, "Build"),
            AgentRole::Security => write!(f, "Security"),
            AgentRole::Frontend => write!(f, "Frontend"),
            AgentRole::Backend => write!(f, "Backend"),
        }
    }
}

#[cfg(test)]
#[path = "roles_tests.rs"]
mod tests;
