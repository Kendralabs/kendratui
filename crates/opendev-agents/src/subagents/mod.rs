//! Subagent specifications and execution.

pub mod manager;
pub mod spec;

pub use manager::{
    NoopProgressCallback, SubagentManager, SubagentProgressCallback, SubagentRunResult,
    SubagentType,
};
pub use spec::{builtins, SubAgentSpec};
