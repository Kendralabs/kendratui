//! Railway/clack-style rendering primitives for the setup wizard.
//!
//! Mirrors `opendev/setup/wizard_ui.py`.

use crossterm::style::{Attribute, Color, Print, ResetColor, SetAttribute, SetForegroundColor};
use crossterm::terminal;
use std::io::{self, Write};

// ── Colors ─────────────────────────────────────────────────────────────────

const ACCENT: Color = Color::Rgb {
    r: 130,
    g: 160,
    b: 255,
};
const SUCCESS_COLOR: Color = Color::Rgb {
    r: 106,
    g: 209,
    b: 143,
}; // #6ad18f
const ERROR_COLOR: Color = Color::Rgb {
    r: 255,
    g: 92,
    b: 87,
}; // #ff5c57
const WARNING_COLOR: Color = Color::Rgb {
    r: 255,
    g: 179,
    b: 71,
}; // #ffb347
const DIM: Color = Color::Rgb {
    r: 122,
    g: 134,
    b: 145,
}; // #7a8691

// ── Rail characters ────────────────────────────────────────────────────────

const RAIL_START: char = '┌';
const RAIL_BAR: char = '│';
const RAIL_END: char = '└';
const RAIL_STEP: char = '◇';
const RAIL_TEE: char = '├';
const RAIL_DASH: char = '─';
const RAIL_BOX_TR: char = '╮';
const RAIL_BOX_BR: char = '╯';

// ── Helpers ────────────────────────────────────────────────────────────────

/// Print the rail bar character in accent color.
fn print_accent_char(w: &mut impl Write, ch: char) -> io::Result<()> {
    crossterm::execute!(w, SetForegroundColor(ACCENT), Print(ch), ResetColor)
}

/// Print a string in accent color.
fn print_accent(w: &mut impl Write, s: &str) -> io::Result<()> {
    crossterm::execute!(w, SetForegroundColor(ACCENT), Print(s), ResetColor)
}

fn dashes(n: usize) -> String {
    RAIL_DASH.to_string().repeat(n)
}

fn term_width() -> usize {
    terminal::size().map(|(w, _)| w as usize).unwrap_or(80)
}

// ── Public API ─────────────────────────────────────────────────────────────

/// Render the opening block of the wizard.
pub fn rail_intro(title: &str, lines: &[&str]) {
    let mut w = io::stdout();
    let _ = writeln!(w);
    // ┌  Title
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_START);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetAttribute(Attribute::Bold),
        Print(title),
        SetAttribute(Attribute::Reset)
    );
    let _ = writeln!(w);
    // │
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = writeln!(w);
    // │  line
    for line in lines {
        let _ = write!(w, "  ");
        let _ = print_accent_char(&mut w, RAIL_BAR);
        let _ = writeln!(w, "  {line}");
    }
    // │
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = writeln!(w);
}

/// Render the closing block of the wizard.
pub fn rail_outro(message: &str) {
    let mut w = io::stdout();
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_END);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetAttribute(Attribute::Bold),
        Print(message),
        SetAttribute(Attribute::Reset)
    );
    let _ = writeln!(w);
    let _ = writeln!(w);
}

/// Render a step heading with a diamond marker.
pub fn rail_step(title: &str, step_label: Option<&str>) {
    let mut w = io::stdout();
    let _ = writeln!(w);
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_STEP);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetAttribute(Attribute::Bold),
        Print(title),
        SetAttribute(Attribute::Reset)
    );
    if let Some(label) = step_label {
        let _ = crossterm::execute!(
            w,
            SetForegroundColor(DIM),
            Print(format!(" {}{} Step {}", RAIL_DASH, RAIL_DASH, label)),
            ResetColor
        );
    }
    let _ = writeln!(w);
}

/// Render an info box attached to the rail.
pub fn rail_info_box(title: &str, lines: &[String], step_label: Option<&str>) {
    let mut w = io::stdout();

    let label = match step_label {
        Some(l) => format!(" {}{} Step {}", RAIL_DASH, RAIL_DASH, l),
        None => String::new(),
    };
    let header_text = format!("{title}{label}");

    let max_line_len = lines.iter().map(|l| l.len()).max().unwrap_or(0);
    let total_w = max_line_len
        .max(header_text.len() + 8)
        .max(52)
        .min(term_width().saturating_sub(4));

    // Top: ◇  Title ──────╮
    let top_dashes_count = total_w.saturating_sub(header_text.len() + 7).max(1);
    let _ = writeln!(w);
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_STEP);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetAttribute(Attribute::Bold),
        Print(&header_text),
        SetAttribute(Attribute::Reset),
        Print(" ")
    );
    let _ = print_accent(&mut w, &format!("{}{}", dashes(top_dashes_count), RAIL_BOX_TR));
    let _ = writeln!(w);

    // Empty line
    let inner_spaces = " ".repeat(total_w.saturating_sub(5));
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = write!(w, "{inner_spaces} ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = writeln!(w);

    // Content lines
    for line in lines {
        let pad = " ".repeat(total_w.saturating_sub(line.len() + 7).max(1));
        let _ = write!(w, "  ");
        let _ = print_accent_char(&mut w, RAIL_BAR);
        let _ = write!(w, "  {line}{pad} ");
        let _ = print_accent_char(&mut w, RAIL_BAR);
        let _ = writeln!(w);
    }

    // Empty line
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = write!(w, "{inner_spaces} ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = writeln!(w);

    // Bottom: ├──────────╯
    let bottom_dashes_count = total_w.saturating_sub(4);
    let _ = write!(w, "  ");
    let _ = print_accent(
        &mut w,
        &format!("{}{}{}", RAIL_TEE, dashes(bottom_dashes_count), RAIL_BOX_BR),
    );
    let _ = writeln!(w);

    // │
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = writeln!(w);
}

/// Render the selected answer below a step.
pub fn rail_answer(value: &str) {
    let mut w = io::stdout();
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = writeln!(w, "  {value}");
}

/// Render a success message on the rail.
pub fn rail_success(message: &str) {
    let mut w = io::stdout();
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetForegroundColor(SUCCESS_COLOR),
        Print(format!("✓ {message}")),
        ResetColor
    );
    let _ = writeln!(w);
}

/// Render an error message on the rail.
pub fn rail_error(message: &str) {
    let mut w = io::stdout();
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetForegroundColor(ERROR_COLOR),
        Print(format!("✖ {message}")),
        ResetColor
    );
    let _ = writeln!(w);
}

/// Render a warning message on the rail.
pub fn rail_warning(message: &str) {
    let mut w = io::stdout();
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetForegroundColor(WARNING_COLOR),
        Print(format!("⚠ {message}")),
        ResetColor
    );
    let _ = writeln!(w);
}

/// Render a blank rail line.
pub fn rail_separator() {
    let mut w = io::stdout();
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = writeln!(w);
}

/// Y/n prompt on the rail. Returns bool.
pub fn rail_confirm(prompt_text: &str, default: bool) -> io::Result<bool> {
    let mut w = io::stdout();
    let hint = if default { "(Y/n)" } else { "(y/N)" };

    let _ = writeln!(w);
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_STEP);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetAttribute(Attribute::Bold),
        Print(prompt_text),
        SetAttribute(Attribute::Reset),
        Print(format!(" {hint}"))
    );
    let _ = writeln!(w);

    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_BAR);
    let _ = write!(w, "  > ");
    w.flush()?;

    let mut buf = String::new();
    io::stdin().read_line(&mut buf)?;
    let answer = buf.trim().to_lowercase();

    if answer.is_empty() {
        return Ok(default);
    }
    Ok(answer.starts_with('y'))
}

/// Text input on the rail. Password mode reads char-by-char with `*` echo.
pub fn rail_prompt(prompt_text: &str, password: bool) -> io::Result<String> {
    let mut w = io::stdout();

    let _ = writeln!(w);
    let _ = write!(w, "  ");
    let _ = print_accent_char(&mut w, RAIL_STEP);
    let _ = crossterm::execute!(
        w,
        Print("  "),
        SetAttribute(Attribute::Bold),
        Print(prompt_text),
        SetAttribute(Attribute::Reset)
    );
    let _ = writeln!(w);

    if password {
        let _ = write!(w, "  ");
        let _ = print_accent_char(&mut w, RAIL_BAR);
        let _ = write!(w, "  > ");
        w.flush()?;
        read_password()
    } else {
        let _ = write!(w, "  ");
        let _ = print_accent_char(&mut w, RAIL_BAR);
        let _ = write!(w, "  > ");
        w.flush()?;
        let mut buf = String::new();
        io::stdin().read_line(&mut buf)?;
        Ok(buf.trim().to_string())
    }
}

/// Read a password with `*` echo using crossterm raw mode.
fn read_password() -> io::Result<String> {
    use crossterm::event::{self, Event, KeyCode, KeyModifiers};

    terminal::enable_raw_mode()?;
    let mut password = String::new();
    let mut stdout = io::stdout();

    loop {
        if let Event::Key(key_event) = event::read()? {
            match key_event.code {
                KeyCode::Enter => {
                    terminal::disable_raw_mode()?;
                    let _ = writeln!(stdout);
                    return Ok(password);
                }
                KeyCode::Backspace => {
                    if password.pop().is_some() {
                        let _ = write!(stdout, "\x08 \x08");
                        stdout.flush()?;
                    }
                }
                KeyCode::Char('c') if key_event.modifiers.contains(KeyModifiers::CONTROL) => {
                    terminal::disable_raw_mode()?;
                    let _ = writeln!(stdout);
                    return Ok(String::new());
                }
                KeyCode::Esc => {
                    terminal::disable_raw_mode()?;
                    let _ = writeln!(stdout);
                    return Ok(String::new());
                }
                KeyCode::Char(c) => {
                    password.push(c);
                    let _ = write!(stdout, "*");
                    stdout.flush()?;
                }
                _ => {}
            }
        }
    }
}

/// Render a summary table in info-box style.
pub fn rail_summary_box(
    title: &str,
    rows: &[(&str, &str)],
    extra_lines: Option<&[String]>,
) {
    let mut content_lines: Vec<String> = Vec::new();
    for (label, value) in rows {
        content_lines.push(format!("{:<12}{}", label, value));
    }
    if let Some(extras) = extra_lines {
        content_lines.push(String::new());
        content_lines.extend(extras.iter().cloned());
    }
    rail_info_box(title, &content_lines, None);
}

