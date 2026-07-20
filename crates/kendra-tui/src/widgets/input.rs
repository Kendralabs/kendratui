//! User input/prompt widget.

use ratatui::{
    buffer::Buffer,
    layout::Rect,
    style::{Color, Modifier, Style},
    text::{Line, Span},
    widgets::{Paragraph, Widget},
};
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use crate::formatters::style_tokens;

/// Convert a title to kebab-case display: lowercase, spaces→dashes, strip special chars.
fn to_kebab_display(title: &str) -> String {
    let lower = title.to_lowercase();
    let mut result = String::with_capacity(lower.len());
    let mut last_was_dash = true;
    for ch in lower.chars() {
        if ch.is_ascii_alphanumeric() {
            result.push(ch);
            last_was_dash = false;
        } else if !last_was_dash {
            result.push('-');
            last_was_dash = true;
        }
    }
    if result.ends_with('-') {
        result.pop();
    }
    result
}

/// Widget for the user input area.
pub struct InputWidget<'a> {
    buffer: &'a str,
    cursor: usize,
    mode: &'a str,
    user_msg_count: usize,
    bg_result_count: usize,
    activity_tag: Option<&'a str>,
}

impl<'a> InputWidget<'a> {
    pub fn new(
        buffer: &'a str,
        cursor: usize,
        mode: &'a str,
        user_msg_count: usize,
        bg_result_count: usize,
        activity_tag: Option<&'a str>,
    ) -> Self {
        Self {
            buffer,
            cursor,
            mode,
            user_msg_count,
            bg_result_count,
            activity_tag,
        }
    }
}

impl Widget for InputWidget<'_> {
    fn render(self, area: Rect, buf: &mut Buffer) {
        if area.height < 2 {
            return;
        }

        let accent = if self.mode == "PLAN" {
            style_tokens::GREEN_LIGHT
        } else {
            style_tokens::ACCENT
        };

        let placeholder = "Type a message...";

        // Row 0: separator line with embedded mode indicator
        // e.g. "── Normal (Shift+Tab) ──────────"
        let mode_label = match self.mode {
            "NORMAL" => "Normal",
            "PLAN" => "Plan",
            other => other,
        };
        let mode_text = format!(" {mode_label} ");
        let hint_text = "(Shift+Tab) ";
        let prefix_width = "── ".width(); // display width of prefix

        let queue_text = match (self.user_msg_count, self.bg_result_count) {
            (0, 0) => String::new(),
            (u, 0) => format!(
                "── {} message{} queued (ESC) ",
                u,
                if u == 1 { "" } else { "s" }
            ),
            (0, b) => format!("── {} result{} queued ", b, if b == 1 { "" } else { "s" }),
            (u, b) => format!("── {} queued (ESC) ", u + b),
        };

        let used = prefix_width + mode_text.width() + hint_text.width() + queue_text.width();
        let remaining_dashes = (area.width as usize).saturating_sub(used);

        let sep_style = Style::default().fg(accent);
        let mut spans = vec![
            Span::styled("── ", sep_style),
            Span::styled(
                mode_text,
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            ),
            Span::styled(hint_text, Style::default().fg(style_tokens::GREY)),
        ];
        if !queue_text.is_empty() {
            spans.push(Span::styled(
                queue_text,
                Style::default()
                    .fg(Color::Yellow)
                    .add_modifier(Modifier::BOLD),
            ));
        }
        if let Some(tag) = self.activity_tag {
            let tag_display = to_kebab_display(tag);
            let tag_section = format!(" {} ", tag_display);
            let trailing = "──";
            let tag_width = tag_section.width() + trailing.width();
            let fill = remaining_dashes.saturating_sub(tag_width);
            spans.push(Span::styled("─".repeat(fill), sep_style));
            spans.push(Span::styled(
                tag_section,
                Style::default().fg(Color::Black).bg(style_tokens::GOLD),
            ));
            spans.push(Span::styled(trailing, sep_style));
        } else {
            spans.push(Span::styled("─".repeat(remaining_dashes), sep_style));
        }
        let sep_line = Line::from(spans);
        // Pre-fill entire row with ─ so any rendering gap stays filled
        buf.set_string(
            area.left(),
            area.top(),
            "─".repeat(area.width as usize),
            sep_style,
        );
        buf.set_line(area.left(), area.top(), &sep_line, area.width);

        // Rows below separator: multiline input
        let text_height = area.height.saturating_sub(1);
        if text_height == 0 {
            return;
        }
        let text_area = Rect {
            x: area.x,
            y: area.y + 1,
            width: area.width,
            height: text_height,
        };

        if self.buffer.is_empty() {
            let prefix = Span::styled(
                "> ".to_string(),
                Style::default().fg(accent).add_modifier(Modifier::BOLD),
            );
            let content = vec![
                prefix,
                Span::styled(placeholder, Style::default().fg(style_tokens::SUBTLE)),
            ];
            Paragraph::new(Line::from(content)).render(text_area, buf);
        } else {
            let avail_width = (text_area.width as usize).saturating_sub(2).max(1);
            struct PhysicalLineInfo {
                text: String,
                cursor_idx: Option<usize>,
            }
            let mut physical_lines: Vec<PhysicalLineInfo> = Vec::new();

            let logical_lines: Vec<&str> = self.buffer.split('\n').collect();

            // Compute which logical line and column the cursor is on
            let mut cursor_logical_line = 0;
            let mut cursor_col = 0;
            let mut pos = 0;
            for (i, line) in logical_lines.iter().enumerate() {
                if self.cursor <= pos + line.len() {
                    cursor_logical_line = i;
                    cursor_col = self.cursor - pos;
                    break;
                }
                pos += line.len() + 1; // +1 for '\n'
                if i == logical_lines.len() - 1 {
                    cursor_logical_line = i;
                    cursor_col = line.len();
                }
            }

            for (i, logical_line) in logical_lines.iter().enumerate() {
                let is_cursor_line = i == cursor_logical_line;

                let mut current_text = String::new();
                let mut current_width = 0;
                let mut current_cursor = None;

                if logical_line.is_empty() {
                    physical_lines.push(PhysicalLineInfo {
                        text: String::new(),
                        cursor_idx: if is_cursor_line { Some(0) } else { None },
                    });
                } else {
                    let mut char_indices: Vec<(usize, char)> =
                        logical_line.char_indices().collect();
                    char_indices.push((logical_line.len(), '\0'));

                    for (byte_idx, ch) in char_indices {
                        let is_cursor_at_char = is_cursor_line && byte_idx == cursor_col;
                        if is_cursor_at_char {
                            current_cursor = Some(current_text.len());
                        }

                        if ch == '\0' {
                            break;
                        }

                        let ch_width = UnicodeWidthChar::width(ch).unwrap_or(1);
                        if current_width + ch_width > avail_width && !current_text.is_empty() {
                            physical_lines.push(PhysicalLineInfo {
                                text: std::mem::take(&mut current_text),
                                cursor_idx: current_cursor.take(),
                            });
                            current_width = 0;

                            if is_cursor_at_char {
                                current_cursor = Some(0);
                            }
                        }

                        current_text.push(ch);
                        current_width += ch_width;
                    }

                    physical_lines.push(PhysicalLineInfo {
                        text: current_text,
                        cursor_idx: current_cursor,
                    });
                }
            }

            let prefix_style = Style::default().fg(accent).add_modifier(Modifier::BOLD);
            let cursor_style = Style::default().fg(Color::Black).bg(Color::White);

            // Find the index of the physical line containing the cursor
            let mut cursor_phys_line_idx = 0;
            for (i, phys_line) in physical_lines.iter().enumerate() {
                if phys_line.cursor_idx.is_some() {
                    cursor_phys_line_idx = i;
                    break;
                }
            }

            // Calculate scrolling window (start_line) to ensure the cursor line is visible
            let start_line = if physical_lines.len() <= text_height as usize {
                0
            } else if cursor_phys_line_idx >= text_height as usize {
                cursor_phys_line_idx - text_height as usize + 1
            } else {
                0
            };

            let visible_lines = &physical_lines[start_line..(start_line + text_height as usize).min(physical_lines.len())];

            for (i, phys_line) in visible_lines.iter().enumerate() {
                let row = text_area.y + i as u16;
                let pfx = if start_line + i == 0 { "> " } else { "  " };

                if let Some(cursor_idx) = phys_line.cursor_idx {
                    let before = &phys_line.text[..cursor_idx];
                    let (cursor_char, after) = if cursor_idx < phys_line.text.len() {
                        let ch = phys_line.text[cursor_idx..].chars().next().unwrap();
                        let end = cursor_idx + ch.len_utf8();
                        (&phys_line.text[cursor_idx..end], &phys_line.text[end..])
                    } else {
                        (" ", "")
                    };
                    let spans = Line::from(vec![
                        Span::styled(pfx, prefix_style),
                        Span::raw(before.to_string()),
                        Span::styled(cursor_char.to_string(), cursor_style),
                        Span::raw(after.to_string()),
                    ]);
                    buf.set_line(text_area.x, row, &spans, text_area.width);
                } else {
                    let spans = Line::from(vec![
                        Span::styled(pfx, prefix_style),
                        Span::raw(phys_line.text.to_string()),
                    ]);
                    buf.set_line(text_area.x, row, &spans, text_area.width);
                }
            }
        }
    }
}

/// Calculate the number of wrapped/display lines that the user input buffer will occupy.
pub fn count_display_lines(buffer: &str, width: u16) -> usize {
    let avail_width = (width as usize).saturating_sub(2).max(1);
    let mut display_lines = 0;
    for line in buffer.split('\n') {
        let mut current_width = 0;
        let mut line_count = 1;
        for ch in line.chars() {
            let ch_width = UnicodeWidthChar::width(ch).unwrap_or(1);
            if current_width + ch_width > avail_width && current_width > 0 {
                line_count += 1;
                current_width = ch_width;
            } else {
                current_width += ch_width;
            }
        }
        display_lines += line_count;
    }
    display_lines
}

#[cfg(test)]
#[path = "input_tests.rs"]
mod tests;
