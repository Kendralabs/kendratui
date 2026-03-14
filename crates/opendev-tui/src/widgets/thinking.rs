//! Thinking trace and self-critique display widget.
//!
//! Mirrors the Python thinking/critique display from `ThinkingMixin` and
//! the TUI's `on_thinking` / `on_critique` callbacks. Renders thinking traces
//! and critique feedback in the conversation view.

use ratatui::{
    style::{Modifier, Style},
    text::{Line, Span},
};

use crate::formatters::style_tokens::{self, Indent};

/// Thinking display phase.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ThinkingPhase {
    /// Initial thinking trace.
    Thinking,
    /// Self-critique of the thinking trace.
    Critique,
    /// Refined thinking after incorporating critique.
    Refinement,
}

impl std::fmt::Display for ThinkingPhase {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Thinking => write!(f, "Thinking"),
            Self::Critique => write!(f, "Critique"),
            Self::Refinement => write!(f, "Refined Thinking"),
        }
    }
}

/// A thinking trace block ready for display.
#[derive(Debug, Clone)]
pub struct ThinkingBlock {
    pub phase: ThinkingPhase,
    pub content: String,
    /// Whether this block is collapsed in the UI.
    pub collapsed: bool,
}

/// Render a thinking block into styled lines for the conversation widget.
///
/// Matches the Python rendering: `⟡ first line\n  continuation lines`
/// with dim italic styling and no phase label prefix.
pub fn render_thinking_block(block: &ThinkingBlock) -> Vec<Line<'static>> {
    let mut lines = Vec::new();

    let phase_color = match block.phase {
        ThinkingPhase::Thinking => style_tokens::PHASE_THINKING,
        ThinkingPhase::Critique => style_tokens::PHASE_CRITIQUE,
        ThinkingPhase::Refinement => style_tokens::PHASE_REFINEMENT,
    };

    let content_lines: Vec<&str> = block.content.lines().collect();

    if block.collapsed {
        // Collapsed: show icon + first line (or empty) with collapse indicator
        let first = content_lines.first().copied().unwrap_or("");
        lines.push(Line::from(vec![
            Span::styled(
                format!("{} ", style_tokens::THINKING_ICON),
                Style::default().fg(phase_color),
            ),
            Span::styled(
                format!("+ {first}"),
                Style::default()
                    .fg(phase_color)
                    .add_modifier(Modifier::ITALIC),
            ),
        ]));
    } else {
        // Expanded: icon + first line, then indented continuation
        for (i, content_line) in content_lines.iter().enumerate() {
            if i == 0 {
                lines.push(Line::from(vec![
                    Span::styled(
                        format!("{} ", style_tokens::THINKING_ICON),
                        Style::default().fg(phase_color),
                    ),
                    Span::styled(
                        content_line.to_string(),
                        Style::default()
                            .fg(phase_color)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            } else {
                lines.push(Line::from(vec![
                    Span::raw(Indent::CONT),
                    Span::styled(
                        content_line.to_string(),
                        Style::default()
                            .fg(phase_color)
                            .add_modifier(Modifier::ITALIC),
                    ),
                ]));
            }
        }
    }

    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_thinking_phase_display() {
        assert_eq!(ThinkingPhase::Thinking.to_string(), "Thinking");
        assert_eq!(ThinkingPhase::Critique.to_string(), "Critique");
        assert_eq!(ThinkingPhase::Refinement.to_string(), "Refined Thinking");
    }

    #[test]
    fn test_render_thinking_expanded() {
        let block = ThinkingBlock {
            phase: ThinkingPhase::Thinking,
            content: "I should first read the file\nthen edit it".to_string(),
            collapsed: false,
        };
        let lines = render_thinking_block(&block);
        // First line (icon + content) + 1 continuation line = 2 lines
        assert_eq!(lines.len(), 2);
    }

    #[test]
    fn test_render_thinking_collapsed() {
        let block = ThinkingBlock {
            phase: ThinkingPhase::Critique,
            content: "The approach misses error handling".to_string(),
            collapsed: true,
        };
        let lines = render_thinking_block(&block);
        // Only collapsed header line
        assert_eq!(lines.len(), 1);
    }
}
