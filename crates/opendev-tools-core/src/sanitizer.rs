//! Tool result sanitization — truncates large outputs before they enter LLM context.

use std::collections::HashMap;
use tracing::debug;

/// Truncation strategy for a tool's output.
#[derive(Debug, Clone)]
pub enum TruncationStrategy {
    /// Keep the beginning of the text.
    Head,
    /// Keep the end of the text (most recent output).
    Tail,
    /// Keep beginning and end, cut the middle.
    HeadTail {
        /// Proportion of max_chars allocated to the head (0.0..1.0).
        head_ratio: f64,
    },
}

/// Per-tool truncation configuration.
#[derive(Debug, Clone)]
pub struct TruncationRule {
    pub max_chars: usize,
    pub strategy: TruncationStrategy,
}

impl TruncationRule {
    pub fn head(max_chars: usize) -> Self {
        Self {
            max_chars,
            strategy: TruncationStrategy::Head,
        }
    }

    pub fn tail(max_chars: usize) -> Self {
        Self {
            max_chars,
            strategy: TruncationStrategy::Tail,
        }
    }

    pub fn head_tail(max_chars: usize, head_ratio: f64) -> Self {
        Self {
            max_chars,
            strategy: TruncationStrategy::HeadTail { head_ratio },
        }
    }
}

/// Maximum characters for error messages.
const ERROR_MAX_CHARS: usize = 2000;

/// Default truncation rule for MCP tools.
fn mcp_default_rule() -> TruncationRule {
    TruncationRule::head(8000)
}

/// Built-in default rules by tool name.
fn default_rules() -> HashMap<String, TruncationRule> {
    let mut rules = HashMap::new();
    rules.insert("run_command".into(), TruncationRule::tail(8000));
    rules.insert("read_file".into(), TruncationRule::head(15000));
    rules.insert("search".into(), TruncationRule::head(10000));
    rules.insert("list_files".into(), TruncationRule::head(10000));
    rules.insert("fetch_url".into(), TruncationRule::head(12000));
    rules.insert("web_search".into(), TruncationRule::head(10000));
    rules.insert("git".into(), TruncationRule::head_tail(12000, 0.7));
    rules.insert("browser".into(), TruncationRule::head(5000));
    rules.insert("get_session_history".into(), TruncationRule::tail(15000));
    rules.insert("memory_search".into(), TruncationRule::head(10000));
    rules
}

/// Sanitizes tool results by applying truncation rules.
///
/// Integrates as a single pass before results enter the message history,
/// preventing context bloat from large tool outputs.
#[derive(Debug)]
pub struct ToolResultSanitizer {
    rules: HashMap<String, TruncationRule>,
}

impl ToolResultSanitizer {
    /// Create with default rules.
    pub fn new() -> Self {
        Self {
            rules: default_rules(),
        }
    }

    /// Create with custom per-tool character limit overrides.
    ///
    /// Custom limits override the default max_chars but keep the default strategy.
    pub fn with_custom_limits(custom_limits: HashMap<String, usize>) -> Self {
        let mut rules = default_rules();
        for (tool_name, max_chars) in custom_limits {
            if let Some(existing) = rules.get(&tool_name) {
                rules.insert(
                    tool_name,
                    TruncationRule {
                        max_chars,
                        strategy: existing.strategy.clone(),
                    },
                );
            } else {
                rules.insert(tool_name, TruncationRule::head(max_chars));
            }
        }
        Self { rules }
    }

    /// Sanitize a tool result, truncating output if needed.
    ///
    /// Takes `success`, `output`, and `error` fields. Returns potentially
    /// truncated versions. The original strings are not mutated.
    pub fn sanitize(
        &self,
        tool_name: &str,
        success: bool,
        output: Option<&str>,
        error: Option<&str>,
    ) -> SanitizedResult {
        // Truncate error messages
        if !success {
            let truncated_error = error.map(|e| {
                if e.len() > ERROR_MAX_CHARS {
                    truncate_head(e, ERROR_MAX_CHARS)
                } else {
                    e.to_string()
                }
            });
            return SanitizedResult {
                output: output.map(String::from),
                error: truncated_error,
                was_truncated: false,
            };
        }

        let output_str = match output {
            Some(s) if !s.is_empty() => s,
            _ => {
                return SanitizedResult {
                    output: output.map(String::from),
                    error: error.map(String::from),
                    was_truncated: false,
                }
            }
        };

        let rule = match self.get_rule(tool_name) {
            Some(r) => r,
            None => {
                return SanitizedResult {
                    output: Some(output_str.to_string()),
                    error: None,
                    was_truncated: false,
                }
            }
        };

        if output_str.len() <= rule.max_chars {
            return SanitizedResult {
                output: Some(output_str.to_string()),
                error: None,
                was_truncated: false,
            };
        }

        let original_len = output_str.len();
        let truncated = apply_strategy(output_str, rule);
        let strategy_name = match &rule.strategy {
            TruncationStrategy::Head => "head",
            TruncationStrategy::Tail => "tail",
            TruncationStrategy::HeadTail { .. } => "head_tail",
        };

        let marker = format!(
            "\n\n[truncated: showing {} of {} chars, strategy={}]",
            truncated.len(),
            original_len,
            strategy_name
        );

        debug!(
            tool = tool_name,
            original = original_len,
            truncated = truncated.len(),
            strategy = strategy_name,
            "Truncated tool result"
        );

        SanitizedResult {
            output: Some(format!("{truncated}{marker}")),
            error: None,
            was_truncated: true,
        }
    }

    /// Look up the truncation rule for a tool.
    fn get_rule(&self, tool_name: &str) -> Option<&TruncationRule> {
        // Exact match first
        if let Some(rule) = self.rules.get(tool_name) {
            return Some(rule);
        }
        // MCP tools get a default rule
        if tool_name.starts_with("mcp__") {
            // Return a reference to a leaked static for MCP default.
            // This is fine since the sanitizer lives for the program's lifetime.
            None // We handle MCP separately below
        } else {
            None
        }
    }

    /// Sanitize with MCP fallback (returns owned result).
    pub fn sanitize_with_mcp_fallback(
        &self,
        tool_name: &str,
        success: bool,
        output: Option<&str>,
        error: Option<&str>,
    ) -> SanitizedResult {
        if success && tool_name.starts_with("mcp__") && self.get_rule(tool_name).is_none() {
            // Apply MCP default rule
            if let Some(output_str) = output {
                let rule = mcp_default_rule();
                if output_str.len() > rule.max_chars {
                    let truncated = apply_strategy(output_str, &rule);
                    let marker = format!(
                        "\n\n[truncated: showing {} of {} chars, strategy=head]",
                        truncated.len(),
                        output_str.len()
                    );
                    return SanitizedResult {
                        output: Some(format!("{truncated}{marker}")),
                        error: None,
                        was_truncated: true,
                    };
                }
            }
        }
        self.sanitize(tool_name, success, output, error)
    }
}

impl Default for ToolResultSanitizer {
    fn default() -> Self {
        Self::new()
    }
}

/// Result of sanitization.
#[derive(Debug, Clone)]
pub struct SanitizedResult {
    pub output: Option<String>,
    pub error: Option<String>,
    pub was_truncated: bool,
}

/// Apply a truncation strategy to text.
fn apply_strategy(text: &str, rule: &TruncationRule) -> String {
    match &rule.strategy {
        TruncationStrategy::Head => truncate_head(text, rule.max_chars),
        TruncationStrategy::Tail => truncate_tail(text, rule.max_chars),
        TruncationStrategy::HeadTail { head_ratio } => {
            truncate_head_tail(text, rule.max_chars, *head_ratio)
        }
    }
}

fn truncate_head(text: &str, max_chars: usize) -> String {
    text.chars().take(max_chars).collect()
}

fn truncate_tail(text: &str, max_chars: usize) -> String {
    let char_count = text.chars().count();
    if char_count <= max_chars {
        return text.to_string();
    }
    text.chars().skip(char_count - max_chars).collect()
}

fn truncate_head_tail(text: &str, max_chars: usize, head_ratio: f64) -> String {
    let head_size = (max_chars as f64 * head_ratio) as usize;
    let tail_size = max_chars - head_size;
    let char_count = text.chars().count();

    let head: String = text.chars().take(head_size).collect();
    let tail: String = text.chars().skip(char_count.saturating_sub(tail_size)).collect();
    format!("{head}\n\n... [middle truncated] ...\n\n{tail}")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_no_truncation_short_output() {
        let sanitizer = ToolResultSanitizer::new();
        let result = sanitizer.sanitize("read_file", true, Some("short output"), None);
        assert!(!result.was_truncated);
        assert_eq!(result.output.as_deref(), Some("short output"));
    }

    #[test]
    fn test_truncation_head_strategy() {
        let sanitizer = ToolResultSanitizer::new();
        let long_output = "x".repeat(20000);
        let result = sanitizer.sanitize("read_file", true, Some(&long_output), None);
        assert!(result.was_truncated);
        let output = result.output.unwrap();
        assert!(output.contains("[truncated:"));
        assert!(output.contains("strategy=head"));
    }

    #[test]
    fn test_truncation_tail_strategy() {
        let sanitizer = ToolResultSanitizer::new();
        let long_output = "x".repeat(10000);
        let result = sanitizer.sanitize("run_command", true, Some(&long_output), None);
        assert!(result.was_truncated);
        let output = result.output.unwrap();
        assert!(output.contains("strategy=tail"));
    }

    #[test]
    fn test_truncation_head_tail_strategy() {
        let sanitizer = ToolResultSanitizer::new();
        let long_output = "x".repeat(15000);
        let result = sanitizer.sanitize("git", true, Some(&long_output), None);
        assert!(result.was_truncated);
        let output = result.output.unwrap();
        assert!(output.contains("strategy=head_tail"));
        assert!(output.contains("[middle truncated]"));
    }

    #[test]
    fn test_error_truncation() {
        let sanitizer = ToolResultSanitizer::new();
        let long_error = "e".repeat(5000);
        let result = sanitizer.sanitize("read_file", false, None, Some(&long_error));
        assert!(!result.was_truncated);
        let error = result.error.unwrap();
        assert!(error.len() <= ERROR_MAX_CHARS);
    }

    #[test]
    fn test_error_not_truncated_when_short() {
        let sanitizer = ToolResultSanitizer::new();
        let result = sanitizer.sanitize("read_file", false, None, Some("file not found"));
        assert_eq!(result.error.as_deref(), Some("file not found"));
    }

    #[test]
    fn test_no_rule_no_truncation() {
        let sanitizer = ToolResultSanitizer::new();
        let long_output = "x".repeat(50000);
        let result = sanitizer.sanitize("custom_tool", true, Some(&long_output), None);
        assert!(!result.was_truncated);
        assert_eq!(result.output.unwrap().len(), 50000);
    }

    #[test]
    fn test_mcp_fallback() {
        let sanitizer = ToolResultSanitizer::new();
        let long_output = "x".repeat(10000);
        let result = sanitizer.sanitize_with_mcp_fallback(
            "mcp__github__list",
            true,
            Some(&long_output),
            None,
        );
        assert!(result.was_truncated);
    }

    #[test]
    fn test_custom_limits() {
        let mut limits = HashMap::new();
        limits.insert("read_file".into(), 100);
        let sanitizer = ToolResultSanitizer::with_custom_limits(limits);

        let output = "x".repeat(200);
        let result = sanitizer.sanitize("read_file", true, Some(&output), None);
        assert!(result.was_truncated);
    }

    #[test]
    fn test_empty_output() {
        let sanitizer = ToolResultSanitizer::new();
        let result = sanitizer.sanitize("read_file", true, Some(""), None);
        assert!(!result.was_truncated);
    }

    #[test]
    fn test_none_output() {
        let sanitizer = ToolResultSanitizer::new();
        let result = sanitizer.sanitize("read_file", true, None, None);
        assert!(!result.was_truncated);
        assert!(result.output.is_none());
    }

    #[test]
    fn test_truncate_head() {
        assert_eq!(truncate_head("hello world", 5), "hello");
    }

    #[test]
    fn test_truncate_tail() {
        assert_eq!(truncate_tail("hello world", 5), "world");
    }

    #[test]
    fn test_truncate_head_tail() {
        let text = "abcdefghij";
        let result = truncate_head_tail(text, 6, 0.5);
        assert!(result.starts_with("abc"));
        assert!(result.ends_with("hij"));
        assert!(result.contains("[middle truncated]"));
    }
}
