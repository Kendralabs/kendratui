//! Core tool traits and types.
//!
//! Defines the `BaseTool` async trait that all tools implement, along with
//! `ToolResult` (execution outcome) and `ToolContext` (session state passed
//! to tool handlers).

use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;

/// Errors that can occur during tool execution.
#[derive(Debug, thiserror::Error)]
pub enum ToolError {
    #[error("Tool execution failed: {0}")]
    Execution(String),

    #[error("Invalid parameters: {0}")]
    InvalidParams(String),

    #[error("Tool not found: {0}")]
    NotFound(String),

    #[error("Permission denied: {0}")]
    PermissionDenied(String),

    #[error("Interrupted by user")]
    Interrupted,

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("{0}")]
    Other(String),
}

/// Result of a tool execution.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ToolResult {
    /// Whether the tool executed successfully.
    pub success: bool,
    /// Tool output text (for successful results).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Error message (for failed results).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Additional metadata (tool-specific).
    #[serde(default, skip_serializing_if = "HashMap::is_empty")]
    pub metadata: HashMap<String, serde_json::Value>,
}

impl ToolResult {
    /// Create a successful result.
    pub fn ok(output: impl Into<String>) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            metadata: HashMap::new(),
        }
    }

    /// Create a successful result with metadata.
    pub fn ok_with_metadata(
        output: impl Into<String>,
        metadata: HashMap<String, serde_json::Value>,
    ) -> Self {
        Self {
            success: true,
            output: Some(output.into()),
            error: None,
            metadata,
        }
    }

    /// Create a failed result.
    pub fn fail(error: impl Into<String>) -> Self {
        Self {
            success: false,
            output: None,
            error: Some(error.into()),
            metadata: HashMap::new(),
        }
    }

    /// Create a result from a ToolError.
    pub fn from_error(err: ToolError) -> Self {
        Self::fail(err.to_string())
    }
}

/// Execution context passed to tool handlers.
///
/// Carries session state, configuration, and working directory so tools
/// can resolve paths, check permissions, and access shared resources.
#[derive(Debug, Clone)]
pub struct ToolContext {
    /// Working directory for path resolution.
    pub working_dir: PathBuf,
    /// Whether the caller is a subagent (may restrict some operations).
    pub is_subagent: bool,
    /// Optional session ID for session-scoped operations.
    pub session_id: Option<String>,
    /// Arbitrary context values for tool-specific needs.
    pub values: HashMap<String, serde_json::Value>,
}

impl ToolContext {
    /// Create a new tool context with a working directory.
    pub fn new(working_dir: impl Into<PathBuf>) -> Self {
        Self {
            working_dir: working_dir.into(),
            is_subagent: false,
            session_id: None,
            values: HashMap::new(),
        }
    }

    /// Set the subagent flag.
    pub fn with_subagent(mut self, is_subagent: bool) -> Self {
        self.is_subagent = is_subagent;
        self
    }

    /// Set the session ID.
    pub fn with_session_id(mut self, session_id: impl Into<String>) -> Self {
        self.session_id = Some(session_id.into());
        self
    }

    /// Insert a context value.
    pub fn with_value(mut self, key: impl Into<String>, value: serde_json::Value) -> Self {
        self.values.insert(key.into(), value);
        self
    }
}

impl Default for ToolContext {
    fn default() -> Self {
        Self::new(std::env::current_dir().unwrap_or_default())
    }
}

/// Base trait for all tools.
///
/// Tools implement this trait to provide:
/// - Identity (name, description)
/// - Parameter schema (JSON Schema for LLM tool-use)
/// - Async execution
#[async_trait::async_trait]
pub trait BaseTool: Send + Sync + std::fmt::Debug {
    /// Unique tool name used for dispatch.
    fn name(&self) -> &str;

    /// Human-readable description shown to the LLM.
    fn description(&self) -> &str;

    /// JSON Schema describing the tool's parameters.
    ///
    /// Returns a JSON object with `type`, `properties`, and `required` fields
    /// following the JSON Schema specification.
    fn parameter_schema(&self) -> serde_json::Value;

    /// Execute the tool with the given arguments and context.
    async fn execute(
        &self,
        args: HashMap<String, serde_json::Value>,
        ctx: &ToolContext,
    ) -> ToolResult;
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_tool_result_ok() {
        let result = ToolResult::ok("file contents here");
        assert!(result.success);
        assert_eq!(result.output.as_deref(), Some("file contents here"));
        assert!(result.error.is_none());
        assert!(result.metadata.is_empty());
    }

    #[test]
    fn test_tool_result_ok_with_metadata() {
        let mut meta = HashMap::new();
        meta.insert("lines".into(), serde_json::json!(42));
        let result = ToolResult::ok_with_metadata("output", meta);
        assert!(result.success);
        assert_eq!(result.metadata.get("lines"), Some(&serde_json::json!(42)));
    }

    #[test]
    fn test_tool_result_fail() {
        let result = ToolResult::fail("file not found");
        assert!(!result.success);
        assert!(result.output.is_none());
        assert_eq!(result.error.as_deref(), Some("file not found"));
    }

    #[test]
    fn test_tool_result_from_error() {
        let err = ToolError::NotFound("read_file".into());
        let result = ToolResult::from_error(err);
        assert!(!result.success);
        assert!(result.error.as_ref().unwrap().contains("read_file"));
    }

    #[test]
    fn test_tool_result_serde_roundtrip() {
        let result = ToolResult::ok("hello");
        let json = serde_json::to_string(&result).unwrap();
        let deserialized: ToolResult = serde_json::from_str(&json).unwrap();
        assert!(deserialized.success);
        assert_eq!(deserialized.output.as_deref(), Some("hello"));
    }

    #[test]
    fn test_tool_context_builder() {
        let ctx = ToolContext::new("/tmp/project")
            .with_subagent(true)
            .with_session_id("sess-123")
            .with_value("key", serde_json::json!("value"));

        assert_eq!(ctx.working_dir, PathBuf::from("/tmp/project"));
        assert!(ctx.is_subagent);
        assert_eq!(ctx.session_id.as_deref(), Some("sess-123"));
        assert_eq!(ctx.values.get("key"), Some(&serde_json::json!("value")));
    }

    #[test]
    fn test_tool_context_default() {
        let ctx = ToolContext::default();
        assert!(!ctx.is_subagent);
        assert!(ctx.session_id.is_none());
    }

    #[test]
    fn test_tool_error_display() {
        let err = ToolError::InvalidParams("missing file_path".into());
        assert_eq!(err.to_string(), "Invalid parameters: missing file_path");
    }
}
