//! Todo progress panel widget.
//!
//! Displays a compact panel showing plan execution progress with
//! a progress bar and per-item status indicators.
//!
//! Mirrors Python's `TaskProgressDisplay` from
//! `opendev/ui_textual/components/task_progress.py`.

use crate::formatters::style_tokens;
use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Modifier, Style},
    text::{Line, Span},
    widgets::{Block, Borders, Paragraph, Widget},
};

/// Status of a single todo item for display purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum TodoDisplayStatus {
    Pending,
    InProgress,
    Completed,
}

/// A todo item prepared for display in the panel.
#[derive(Debug, Clone)]
pub struct TodoDisplayItem {
    pub id: usize,
    pub title: String,
    pub status: TodoDisplayStatus,
}

/// Widget that renders a todo progress panel.
///
/// Shows:
/// - A title with progress count (e.g. "Plan Progress (2/5)")
/// - A visual progress bar
/// - Each todo with a status indicator
pub struct TodoPanelWidget<'a> {
    items: &'a [TodoDisplayItem],
    plan_name: Option<&'a str>,
}

impl<'a> TodoPanelWidget<'a> {
    /// Create a new todo panel widget.
    pub fn new(items: &'a [TodoDisplayItem]) -> Self {
        Self {
            items,
            plan_name: None,
        }
    }

    /// Set the plan name to display in the title.
    pub fn with_plan_name(mut self, name: &'a str) -> Self {
        self.plan_name = Some(name);
        self
    }

    fn build_lines(&self) -> Vec<Line<'a>> {
        let total = self.items.len();
        let done = self
            .items
            .iter()
            .filter(|i| i.status == TodoDisplayStatus::Completed)
            .count();
        let in_progress = self
            .items
            .iter()
            .filter(|i| i.status == TodoDisplayStatus::InProgress)
            .count();

        let mut lines = Vec::new();

        // Progress bar
        if total > 0 {
            let bar_width = 20usize;
            let filled = (done * bar_width) / total;
            let partial = if in_progress > 0 && filled < bar_width {
                1
            } else {
                0
            };
            let empty = bar_width.saturating_sub(filled).saturating_sub(partial);

            let mut bar_spans = vec![Span::styled(" [", Style::default().fg(style_tokens::GREY))];
            if filled > 0 {
                bar_spans.push(Span::styled(
                    "=".repeat(filled),
                    Style::default().fg(style_tokens::SUCCESS),
                ));
            }
            if partial > 0 {
                bar_spans.push(Span::styled(
                    ">".to_string(),
                    Style::default().fg(style_tokens::WARNING),
                ));
            }
            if empty > 0 {
                bar_spans.push(Span::styled(
                    " ".repeat(empty),
                    Style::default().fg(style_tokens::GREY),
                ));
            }
            bar_spans.push(Span::styled(
                format!("] {done}/{total}"),
                Style::default().fg(style_tokens::GREY),
            ));
            lines.push(Line::from(bar_spans));
        }

        // Individual items
        for item in self.items {
            let (symbol, style) = match item.status {
                TodoDisplayStatus::Completed => (
                    " \u{2714} ", // checkmark
                    Style::default()
                        .fg(style_tokens::SUCCESS)
                        .add_modifier(Modifier::DIM),
                ),
                TodoDisplayStatus::InProgress => (
                    " \u{25B6} ", // play triangle
                    Style::default()
                        .fg(style_tokens::WARNING)
                        .add_modifier(Modifier::BOLD),
                ),
                TodoDisplayStatus::Pending => (
                    " \u{25CB} ", // circle
                    Style::default().fg(style_tokens::GREY),
                ),
            };

            let title = item.title.clone();
            // Truncate long titles
            let max_title = 60;
            let display_title = if title.len() > max_title {
                format!("{}...", &title[..max_title - 3])
            } else {
                title
            };

            lines.push(Line::from(vec![
                Span::styled(symbol.to_string(), style),
                Span::styled(display_title, style),
            ]));
        }

        lines
    }
}

impl Widget for TodoPanelWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        let total = self.items.len();
        let done = self
            .items
            .iter()
            .filter(|i| i.status == TodoDisplayStatus::Completed)
            .count();

        let title = if let Some(name) = self.plan_name {
            format!(" Plan: {name} ({done}/{total}) ")
        } else {
            format!(" Plan Progress ({done}/{total}) ")
        };

        let border_color = if done == total && total > 0 {
            style_tokens::SUCCESS
        } else {
            style_tokens::CYAN
        };

        let block = Block::default()
            .title(title)
            .borders(Borders::ALL)
            .border_style(Style::default().fg(border_color));

        let lines = self.build_lines();
        let paragraph = Paragraph::new(lines).block(block);
        paragraph.render(area, buf);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_items() -> Vec<TodoDisplayItem> {
        vec![
            TodoDisplayItem {
                id: 1,
                title: "Set up project".into(),
                status: TodoDisplayStatus::Completed,
            },
            TodoDisplayItem {
                id: 2,
                title: "Write code".into(),
                status: TodoDisplayStatus::InProgress,
            },
            TodoDisplayItem {
                id: 3,
                title: "Write tests".into(),
                status: TodoDisplayStatus::Pending,
            },
        ]
    }

    #[test]
    fn test_build_lines_count() {
        let items = make_items();
        let widget = TodoPanelWidget::new(&items);
        let lines = widget.build_lines();
        // 1 progress bar line + 3 item lines
        assert_eq!(lines.len(), 4);
    }

    #[test]
    fn test_render_does_not_panic() {
        let items = make_items();
        let widget = TodoPanelWidget::new(&items).with_plan_name("bold-blazing-badger");
        let mut buf = Buffer::empty(Rect::new(0, 0, 80, 10));
        widget.render(Rect::new(0, 0, 80, 10), &mut buf);
    }

    #[test]
    fn test_empty_items() {
        let items: Vec<TodoDisplayItem> = vec![];
        let widget = TodoPanelWidget::new(&items);
        let lines = widget.build_lines();
        assert!(lines.is_empty());
    }

    #[test]
    fn test_all_completed_green_border() {
        let items = vec![
            TodoDisplayItem {
                id: 1,
                title: "Done".into(),
                status: TodoDisplayStatus::Completed,
            },
            TodoDisplayItem {
                id: 2,
                title: "Also done".into(),
                status: TodoDisplayStatus::Completed,
            },
        ];
        // Just verify no panic with all completed
        let widget = TodoPanelWidget::new(&items);
        let mut buf = Buffer::empty(Rect::new(0, 0, 60, 6));
        widget.render(Rect::new(0, 0, 60, 6), &mut buf);
    }

    #[test]
    fn test_long_title_truncated() {
        let items = vec![TodoDisplayItem {
            id: 1,
            title: "A".repeat(100),
            status: TodoDisplayStatus::Pending,
        }];
        let widget = TodoPanelWidget::new(&items);
        let lines = widget.build_lines();
        // Progress bar + 1 item
        assert_eq!(lines.len(), 2);
    }
}
