//! User input/prompt widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Block, BorderType, Paragraph, Widget},
};

use crate::formatters::style_tokens;

/// Widget for the user input area.
pub struct InputWidget<'a> {
    buffer: &'a str,
    cursor: usize,
    agent_active: bool,
    mode: &'a str,
}

impl<'a> InputWidget<'a> {
    pub fn new(buffer: &'a str, cursor: usize, agent_active: bool, mode: &'a str) -> Self {
        Self {
            buffer,
            cursor,
            agent_active,
            mode,
        }
    }
}

impl Widget for InputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (border_color, placeholder) = if self.agent_active {
            (style_tokens::GOLD, " Agent is thinking... (ESC to interrupt)")
        } else {
            (style_tokens::ACCENT, " Type a message...")
        };

        let block = Block::bordered()
            .border_type(BorderType::Rounded)
            .border_style(Style::default().fg(border_color))
            .title(Span::styled(
                format!(" {} > ", self.mode),
                Style::default()
                    .fg(border_color)
                    .add_modifier(Modifier::BOLD),
            ));

        let display_text = if self.buffer.is_empty() {
            Line::from(Span::styled(
                placeholder,
                Style::default().fg(style_tokens::SUBTLE),
            ))
        } else {
            // Show buffer with cursor indicator
            let before = &self.buffer[..self.cursor];
            let cursor_char = self
                .buffer
                .get(self.cursor..self.cursor + 1)
                .unwrap_or(" ");
            let after = if self.cursor < self.buffer.len() {
                &self.buffer[self.cursor + 1..]
            } else {
                ""
            };

            Line::from(vec![
                Span::raw(before.to_string()),
                Span::styled(
                    cursor_char.to_string(),
                    Style::default()
                        .fg(Color::Black)
                        .bg(Color::White),
                ),
                Span::raw(after.to_string()),
            ])
        };

        let paragraph = Paragraph::new(display_text).block(block);
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_input_widget_creation() {
        let _widget = InputWidget::new("hello", 3, false, "NORMAL");
    }

    #[test]
    fn test_input_widget_empty() {
        let _widget = InputWidget::new("", 0, false, "NORMAL");
    }

    #[test]
    fn test_input_widget_agent_active() {
        let _widget = InputWidget::new("", 0, true, "NORMAL");
    }
}
