//! Handler for thinking/reasoning content.
//!
//! Mirrors `opendev/core/context_engineering/tools/handlers/thinking_handler.py`.
//!
//! Responsibilities:
//! - Capture thinking blocks from model responses
//! - Format thinking traces for display
//! - Blend self-critique into responses at HIGH thinking level

use std::collections::HashMap;

use serde_json::Value;

use super::traits::{HandlerResult, PreCheckResult, ToolHandler};

/// Handler for the Think tool.
pub struct ThinkingHandler;

impl ThinkingHandler {
    /// Create a new thinking handler.
    pub fn new() -> Self {
        Self
    }

    /// Format thinking content for display.
    pub fn format_thinking(content: &str) -> String {
        if content.is_empty() {
            return String::new();
        }

        let mut result = String::with_capacity(content.len() + 40);
        result.push_str("--- thinking ---\n");
        result.push_str(content.trim());
        result.push_str("\n--- end thinking ---");
        result
    }

    /// Extract a summary line from thinking content.
    pub fn summarize(content: &str, max_words: usize) -> String {
        let words: Vec<&str> = content.split_whitespace().collect();
        if words.len() <= max_words {
            words.join(" ")
        } else {
            let mut summary: String = words[..max_words].join(" ");
            summary.push_str("...");
            summary
        }
    }
}

impl Default for ThinkingHandler {
    fn default() -> Self {
        Self::new()
    }
}

impl ToolHandler for ThinkingHandler {
    fn handles(&self) -> &[&str] {
        &["Think", "think"]
    }

    fn pre_check(
        &self,
        _tool_name: &str,
        _args: &HashMap<String, Value>,
    ) -> PreCheckResult {
        PreCheckResult::Allow
    }

    fn post_process(
        &self,
        _tool_name: &str,
        _args: &HashMap<String, Value>,
        output: Option<&str>,
        error: Option<&str>,
        success: bool,
    ) -> HandlerResult {
        // Format thinking output with delimiters.
        let formatted = output.map(|o| Self::format_thinking(o));

        HandlerResult {
            output: formatted,
            error: error.map(|s| s.to_string()),
            success,
            meta: Default::default(),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_format_thinking() {
        let content = "Let me analyze this step by step.";
        let formatted = ThinkingHandler::format_thinking(content);
        assert!(formatted.starts_with("--- thinking ---"));
        assert!(formatted.contains("step by step"));
        assert!(formatted.ends_with("--- end thinking ---"));
    }

    #[test]
    fn test_format_thinking_empty() {
        assert!(ThinkingHandler::format_thinking("").is_empty());
    }

    #[test]
    fn test_summarize_short() {
        let content = "brief thought";
        assert_eq!(ThinkingHandler::summarize(content, 10), "brief thought");
    }

    #[test]
    fn test_summarize_long() {
        let content = "this is a very long thinking process that goes on and on";
        let summary = ThinkingHandler::summarize(content, 5);
        assert_eq!(summary, "this is a very long...");
    }

    #[test]
    fn test_post_process_formats_output() {
        let h = ThinkingHandler::new();
        let args = HashMap::new();
        let result = h.post_process("Think", &args, Some("reasoning here"), None, true);
        assert!(result.output.unwrap().contains("--- thinking ---"));
    }

    #[test]
    fn test_handles() {
        let h = ThinkingHandler::new();
        assert!(h.handles().contains(&"Think"));
        assert!(h.handles().contains(&"think"));
    }
}
