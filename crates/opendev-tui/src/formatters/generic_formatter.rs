//! Generic fallback formatter.
//!
//! Attempts JSON pretty-print; otherwise shows plain text with truncation.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::base::{FormattedOutput, ToolFormatter, truncate_lines};
use super::style_tokens;

/// Fallback formatter for any tool not handled by a specific formatter.
pub struct GenericFormatter;

/// Maximum lines before truncation.
const MAX_LINES: usize = 60;

impl ToolFormatter for GenericFormatter {
    fn format<'a>(&self, tool_name: &str, output: &str) -> FormattedOutput<'a> {
        let header = Line::from(vec![
            Span::styled(
                "  ⚙ ".to_string(),
                Style::default().fg(style_tokens::PRIMARY),
            ),
            Span::styled(
                tool_name.to_string(),
                Style::default().fg(style_tokens::PRIMARY),
            ),
        ]);

        // Try to pretty-print as JSON
        let display_text = if let Ok(parsed) = serde_json::from_str::<serde_json::Value>(output) {
            match serde_json::to_string_pretty(&parsed) {
                Ok(pretty) => pretty,
                Err(_) => output.to_string(),
            }
        } else {
            output.to_string()
        };

        let truncated = truncate_lines(&display_text, MAX_LINES);
        let total = display_text.lines().count();

        let body: Vec<Line<'a>> = truncated
            .lines()
            .map(|line| Line::from(Span::raw(format!("    {line}"))))
            .collect();

        let footer = if total > MAX_LINES {
            Some(Line::from(Span::styled(
                format!("  ... {total} total lines (truncated)"),
                Style::default().fg(style_tokens::SUBTLE),
            )))
        } else {
            None
        };

        FormattedOutput {
            header,
            body,
            footer,
        }
    }

    fn handles(&self, _tool_name: &str) -> bool {
        // Generic formatter handles everything as a fallback.
        true
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles_anything() {
        let f = GenericFormatter;
        assert!(f.handles("anything"));
        assert!(f.handles("random_tool"));
        assert!(f.handles(""));
    }

    #[test]
    fn test_format_plain_text() {
        let f = GenericFormatter;
        let result = f.format("some_tool", "hello world\nsecond line");

        let header_text: String = result
            .header
            .spans
            .iter()
            .map(|s| s.content.as_ref())
            .collect();
        assert!(header_text.contains("some_tool"));
        assert_eq!(result.body.len(), 2);
        assert!(result.footer.is_none());
    }

    #[test]
    fn test_format_json_pretty() {
        let f = GenericFormatter;
        let json = r#"{"key":"value","nested":{"a":1}}"#;
        let result = f.format("api_call", json);

        // Body should have multiple lines (pretty-printed)
        assert!(result.body.len() > 1);

        // Check that body contains the key
        let body_text: String = result
            .body
            .iter()
            .flat_map(|l| l.spans.iter().map(|s| s.content.as_ref().to_string()))
            .collect::<Vec<_>>()
            .join("\n");
        assert!(body_text.contains("key"));
        assert!(body_text.contains("value"));
    }

    #[test]
    fn test_format_invalid_json_fallback() {
        let f = GenericFormatter;
        let output = "not json {broken";
        let result = f.format("tool", output);
        assert_eq!(result.body.len(), 1);
    }

    #[test]
    fn test_format_truncation() {
        let f = GenericFormatter;
        let lines: Vec<String> = (0..100).map(|i| format!("line {i}")).collect();
        let output = lines.join("\n");
        let result = f.format("tool", &output);

        assert!(result.footer.is_some());
    }
}
