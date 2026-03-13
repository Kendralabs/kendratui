//! Core traits for tool handler middleware.

use std::collections::HashMap;

use serde_json::Value;

/// Result of a pre-execution check.
#[derive(Debug, Clone)]
pub enum PreCheckResult {
    /// Allow execution to proceed.
    Allow,
    /// Deny execution with a reason.
    Deny(String),
    /// Modify the arguments before execution.
    ModifyArgs(HashMap<String, Value>),
}

/// Metadata attached to a handler result.
#[derive(Debug, Clone, Default)]
pub struct HandlerMeta {
    /// Files that were changed by this tool execution.
    pub changed_files: Vec<String>,
    /// Whether this was a background/server command.
    pub is_background: bool,
    /// Operation ID for audit/undo tracking.
    pub operation_id: Option<String>,
}

/// Result of post-execution processing.
#[derive(Debug, Clone)]
pub struct HandlerResult {
    /// The (potentially modified) tool output.
    pub output: Option<String>,
    /// The (potentially modified) error message.
    pub error: Option<String>,
    /// Whether the tool succeeded.
    pub success: bool,
    /// Handler metadata.
    pub meta: HandlerMeta,
}

/// Trait for tool handler middleware.
///
/// Handlers sit between the REPL and tool execution, providing
/// pre-check (approval), post-processing (formatting), and
/// side-effect management (file tracking, operation logging).
pub trait ToolHandler: Send + Sync {
    /// Tool names this handler manages.
    fn handles(&self) -> &[&str];

    /// Pre-execution check. Called before the tool runs.
    ///
    /// Can approve, deny, or modify the tool call arguments.
    fn pre_check(&self, tool_name: &str, args: &HashMap<String, Value>) -> PreCheckResult {
        let _ = (tool_name, args);
        PreCheckResult::Allow
    }

    /// Post-execution processing. Called after the tool runs.
    ///
    /// Can modify the output, attach metadata, or trigger side effects.
    fn post_process(
        &self,
        tool_name: &str,
        args: &HashMap<String, Value>,
        output: Option<&str>,
        error: Option<&str>,
        success: bool,
    ) -> HandlerResult {
        HandlerResult {
            output: output.map(|s| s.to_string()),
            error: error.map(|s| s.to_string()),
            success,
            meta: HandlerMeta {
                changed_files: self.extract_changed_files(tool_name, args),
                ..Default::default()
            },
        }
    }

    /// Extract file paths changed by this tool (for artifact tracking).
    fn extract_changed_files(
        &self,
        _tool_name: &str,
        _args: &HashMap<String, Value>,
    ) -> Vec<String> {
        Vec::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct TestHandler;

    impl ToolHandler for TestHandler {
        fn handles(&self) -> &[&str] {
            &["test_tool"]
        }
    }

    #[test]
    fn test_default_pre_check_allows() {
        let handler = TestHandler;
        let args = HashMap::new();
        match handler.pre_check("test_tool", &args) {
            PreCheckResult::Allow => {}
            other => panic!("Expected Allow, got {:?}", other),
        }
    }

    #[test]
    fn test_default_post_process_passes_through() {
        let handler = TestHandler;
        let args = HashMap::new();
        let result = handler.post_process("test_tool", &args, Some("output"), None, true);
        assert!(result.success);
        assert_eq!(result.output.as_deref(), Some("output"));
        assert!(result.meta.changed_files.is_empty());
    }
}
