//! Directory/search result formatter.
//!
//! Formats Glob and Grep tool output as file lists with counts.

use ratatui::{
    style::Style,
    text::{Line, Span},
};

use super::base::{FormattedOutput, ToolFormatter};
use super::style_tokens;

/// Formatter for Glob/Grep search results.
pub struct DirectoryFormatter;

/// Maximum number of result lines to display before truncating.
const MAX_RESULTS: usize = 40;

impl ToolFormatter for DirectoryFormatter {
    fn format<'a>(&self, tool_name: &str, output: &str) -> FormattedOutput<'a> {
        let label = match tool_name {
            "Glob" | "list_files" => "matching files",
            "Grep" | "search" => "matching results",
            _ => "results",
        };

        let all_lines: Vec<&str> = output.lines().filter(|l| !l.trim().is_empty()).collect();
        let total = all_lines.len();

        let header = Line::from(vec![
            Span::styled("  🔍 ".to_string(), Style::default().fg(style_tokens::CYAN)),
            Span::styled(
                format!("{total} {label}"),
                Style::default().fg(style_tokens::CYAN),
            ),
        ]);

        let display_count = total.min(MAX_RESULTS);
        let body: Vec<Line<'a>> = all_lines[..display_count]
            .iter()
            .map(|line| {
                Line::from(vec![
                    Span::styled("    ".to_string(), Style::default()),
                    Span::styled(line.to_string(), Style::default().fg(style_tokens::PRIMARY)),
                ])
            })
            .collect();

        let footer = if total > MAX_RESULTS {
            let remaining = total - MAX_RESULTS;
            Some(Line::from(Span::styled(
                format!("  ... and {remaining} more"),
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

    fn handles(&self, tool_name: &str) -> bool {
        matches!(tool_name, "Glob" | "Grep" | "list_files" | "search")
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_handles() {
        let f = DirectoryFormatter;
        assert!(f.handles("Glob"));
        assert!(f.handles("Grep"));
        assert!(f.handles("list_files"));
        assert!(f.handles("search"));
        assert!(!f.handles("Bash"));
    }

    #[test]
    fn test_format_glob() {
        let f = DirectoryFormatter;
        let output = "src/main.rs\nsrc/lib.rs\ntests/test.rs";
        let result = f.format("Glob", output);

        let header_text: String = result.header.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header_text.contains("3 matching files"));
        assert_eq!(result.body.len(), 3);
        assert!(result.footer.is_none());
    }

    #[test]
    fn test_format_grep() {
        let f = DirectoryFormatter;
        let output = "src/main.rs:10:fn main()\nsrc/lib.rs:5:pub mod foo";
        let result = f.format("Grep", output);

        let header_text: String = result.header.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header_text.contains("2 matching results"));
    }

    #[test]
    fn test_format_truncation() {
        let f = DirectoryFormatter;
        let lines: Vec<String> = (0..60).map(|i| format!("file_{i}.rs")).collect();
        let output = lines.join("\n");
        let result = f.format("Glob", &output);

        assert_eq!(result.body.len(), MAX_RESULTS);
        let footer = result.footer.unwrap();
        let footer_text: String = footer.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(footer_text.contains("20 more"));
    }

    #[test]
    fn test_empty_lines_filtered() {
        let f = DirectoryFormatter;
        let output = "file1.rs\n\nfile2.rs\n\n";
        let result = f.format("Glob", output);

        let header_text: String = result.header.spans.iter().map(|s| s.content.as_ref()).collect();
        assert!(header_text.contains("2 matching files"));
    }
}
