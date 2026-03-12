//! Markdown rendering for terminal output.
//!
//! Converts markdown text to styled ratatui `Line`s with basic formatting:
//! headers, bold, italic, code blocks, and inline code.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};
use super::style_tokens;

/// Renders markdown text into styled terminal lines.
pub struct MarkdownRenderer;

impl MarkdownRenderer {
    /// Render markdown text into a vector of styled lines.
    pub fn render(text: &str) -> Vec<Line<'static>> {
        let mut lines = Vec::new();
        let mut in_code_block = false;

        for raw_line in text.lines() {
            if raw_line.starts_with("```") {
                in_code_block = !in_code_block;
                if in_code_block {
                    // Code block start — show language hint if present
                    let lang = raw_line.trim_start_matches('`').trim();
                    if !lang.is_empty() {
                        lines.push(Line::from(Span::styled(
                            format!("--- {lang} ---"),
                            Style::default().fg(style_tokens::GREY),
                        )));
                    }
                }
                continue;
            }

            if in_code_block {
                lines.push(Line::from(Span::styled(
                    raw_line.to_string(),
                    Style::default().fg(style_tokens::CODE_FG).bg(style_tokens::CODE_BG),
                )));
                continue;
            }

            // Headers
            if let Some(header) = raw_line.strip_prefix("### ") {
                lines.push(Line::from(Span::styled(
                    header.to_string(),
                    Style::default()
                        .fg(style_tokens::HEADING_3)
                        .add_modifier(Modifier::BOLD),
                )));
            } else if let Some(header) = raw_line.strip_prefix("## ") {
                lines.push(Line::from(Span::styled(
                    header.to_string(),
                    Style::default()
                        .fg(style_tokens::HEADING_2)
                        .add_modifier(Modifier::BOLD),
                )));
            } else if let Some(header) = raw_line.strip_prefix("# ") {
                lines.push(Line::from(Span::styled(
                    header.to_string(),
                    Style::default()
                        .fg(style_tokens::HEADING_1)
                        .add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                )));
            } else if raw_line.starts_with("- ") || raw_line.starts_with("* ") {
                // Bullet list
                let content = &raw_line[2..];
                lines.push(Line::from(vec![
                    Span::styled("  * ", Style::default().fg(style_tokens::BULLET)),
                    Span::raw(render_inline(content)),
                ]));
            } else {
                // Regular text with inline formatting
                lines.push(render_inline_line(raw_line));
            }
        }

        lines
    }
}

/// Render inline formatting (bold, italic, code) in a single line.
fn render_inline_line(text: &str) -> Line<'static> {
    // Simple approach: split by backtick pairs for inline code
    let spans = parse_inline_spans(text);
    Line::from(spans)
}

/// Render inline text stripping markdown markers (for use inside list items, etc.).
fn render_inline(text: &str) -> String {
    text.replace("**", "").replace("__", "").replace('`', "")
}

/// Parse inline spans handling backtick code and bold markers.
fn parse_inline_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        // Look for inline code
        if let Some(code_start) = remaining.find('`') {
            // Add text before the backtick
            if code_start > 0 {
                spans.extend(parse_bold_spans(&remaining[..code_start]));
            }

            let after_start = &remaining[code_start + 1..];
            if let Some(code_end) = after_start.find('`') {
                let code = &after_start[..code_end];
                spans.push(Span::styled(
                    code.to_string(),
                    Style::default()
                        .fg(style_tokens::CODE_FG)
                        .add_modifier(Modifier::BOLD),
                ));
                remaining = &after_start[code_end + 1..];
            } else {
                // No closing backtick — treat rest as plain text
                spans.extend(parse_bold_spans(remaining));
                break;
            }
        } else {
            spans.extend(parse_bold_spans(remaining));
            break;
        }
    }

    if spans.is_empty() {
        spans.push(Span::raw(String::new()));
    }

    spans
}

/// Parse bold markers (**text**) within a segment of text.
fn parse_bold_spans(text: &str) -> Vec<Span<'static>> {
    let mut spans = Vec::new();
    let mut remaining = text;

    while !remaining.is_empty() {
        if let Some(bold_start) = remaining.find("**") {
            if bold_start > 0 {
                spans.push(Span::raw(remaining[..bold_start].to_string()));
            }
            let after_start = &remaining[bold_start + 2..];
            if let Some(bold_end) = after_start.find("**") {
                let bold_text = &after_start[..bold_end];
                spans.push(Span::styled(
                    bold_text.to_string(),
                    Style::default().add_modifier(Modifier::BOLD),
                ));
                remaining = &after_start[bold_end + 2..];
            } else {
                spans.push(Span::raw(remaining.to_string()));
                break;
            }
        } else {
            spans.push(Span::raw(remaining.to_string()));
            break;
        }
    }

    spans
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_plain_text() {
        let lines = MarkdownRenderer::render("Hello world");
        assert_eq!(lines.len(), 1);
    }

    #[test]
    fn test_headers() {
        let lines = MarkdownRenderer::render("# Title\n## Subtitle\n### Section");
        assert_eq!(lines.len(), 3);
    }

    #[test]
    fn test_code_block() {
        let md = "```rust\nfn main() {}\n```";
        let lines = MarkdownRenderer::render(md);
        // lang hint + code line = 2
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_bullet_list() {
        let md = "- item one\n- item two";
        let lines = MarkdownRenderer::render(md);
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_inline_code() {
        let spans = parse_inline_spans("use `tokio` for async");
        assert!(spans.len() >= 3);
    }

    #[test]
    fn test_bold_text() {
        let spans = parse_bold_spans("this is **bold** text");
        assert!(spans.len() >= 3);
    }
}
