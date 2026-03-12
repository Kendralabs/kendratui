//! Tool-type color coding for consistent display.
//!
//! Maps tool categories to colors. Mirrors the color scheme used
//! across the Python TUI's `style_tokens` and tool display utilities.

use ratatui::style::Color;

use super::style_tokens;

/// Tool category for color-coding purposes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ToolCategory {
    /// File read operations (read_file, read_pdf, list_files).
    FileRead,
    /// File write/edit operations (write_file, edit_file).
    FileWrite,
    /// Bash/command execution.
    Bash,
    /// Search operations (search, web_search).
    Search,
    /// Web operations (fetch_url, open_browser, screenshots).
    Web,
    /// Subagent/agent spawn operations.
    Agent,
    /// Symbol/LSP operations (find_symbol, rename_symbol).
    Symbol,
    /// MCP tool calls.
    Mcp,
    /// Plan/task management tools.
    Plan,
    /// Docker operations.
    Docker,
    /// User interaction (ask_user).
    UserInteraction,
    /// Notebook operations.
    Notebook,
    /// Unknown/other tools.
    Other,
}

/// Classify a tool name into its category.
pub fn categorize_tool(tool_name: &str) -> ToolCategory {
    match tool_name {
        "read_file" | "read_pdf" | "list_files" => ToolCategory::FileRead,
        "write_file" | "edit_file" | "patch_file" => ToolCategory::FileWrite,
        "run_command" | "bash_execute" | "Bash" => ToolCategory::Bash,
        "search" | "web_search" => ToolCategory::Search,
        "fetch_url" | "open_browser" | "capture_screenshot"
        | "capture_web_screenshot" => ToolCategory::Web,
        "spawn_subagent" | "get_subagent_output" => ToolCategory::Agent,
        "find_symbol" | "find_referencing_symbols" | "insert_before_symbol"
        | "insert_after_symbol" | "replace_symbol_body" | "rename_symbol" => {
            ToolCategory::Symbol
        }
        "present_plan" | "write_todos" | "update_todo" | "complete_todo"
        | "list_todos" | "clear_todos" | "task_complete" => ToolCategory::Plan,
        "ask_user" => ToolCategory::UserInteraction,
        "notebook_edit" => ToolCategory::Notebook,
        s if s.starts_with("mcp__") => ToolCategory::Mcp,
        s if s.starts_with("docker_") => ToolCategory::Docker,
        _ => ToolCategory::Other,
    }
}

/// Get the primary display color for a tool category.
pub fn tool_color(category: ToolCategory) -> Color {
    match category {
        ToolCategory::FileRead => style_tokens::BLUE_PATH,
        ToolCategory::FileWrite => style_tokens::SUCCESS,
        ToolCategory::Bash => style_tokens::WARNING,
        ToolCategory::Search => style_tokens::CYAN,
        ToolCategory::Web => style_tokens::HEADING_1,
        ToolCategory::Agent => style_tokens::CYAN,
        ToolCategory::Symbol => style_tokens::BLUE_BRIGHT,
        ToolCategory::Mcp => style_tokens::HEADING_1,
        ToolCategory::Plan => style_tokens::GREEN_LIGHT,
        ToolCategory::Docker => Color::Rgb(0, 206, 209),
        ToolCategory::UserInteraction => style_tokens::GOLD,
        ToolCategory::Notebook => style_tokens::BLUE_LIGHT,
        ToolCategory::Other => style_tokens::PRIMARY,
    }
}

/// Human-friendly display name for a tool.
///
/// Mirrors Python `_TOOL_DISPLAY_PARTS` — returns `(verb, label)`.
pub fn tool_display_parts(tool_name: &str) -> (&str, &str) {
    match tool_name {
        "read_file" => ("Read", "file"),
        "read_pdf" => ("Read", "pdf"),
        "write_file" => ("Write", "file"),
        "edit_file" => ("Edit", "file"),
        "list_files" => ("List", "files"),
        "search" => ("Search", "project"),
        "run_command" | "bash_execute" | "Bash" => ("Bash", "command"),
        "get_process_output" => ("Get Process Output", "process"),
        "list_processes" => ("List Processes", "processes"),
        "kill_process" => ("Kill Process", "process"),
        "fetch_url" => ("Fetch", "url"),
        "open_browser" => ("Open", "browser"),
        "capture_screenshot" => ("Capture Screenshot", "screenshot"),
        "capture_web_screenshot" => ("Capture Web Screenshot", "page"),
        "analyze_image" => ("Analyze Image", "image"),
        "write_todos" => ("Create", "todos"),
        "update_todo" => ("Update Todos", "todo"),
        "complete_todo" => ("Complete Todos", "todo"),
        "list_todos" => ("List Todos", "todos"),
        "clear_todos" => ("Clear Todos", "todos"),
        "spawn_subagent" => ("Spawn", "subagent"),
        "find_symbol" => ("Find Symbol", "symbol"),
        "find_referencing_symbols" => ("Find References", "symbol"),
        "replace_symbol_body" => ("Replace Symbol", "symbol"),
        "rename_symbol" => ("Rename Symbol", "symbol"),
        "present_plan" => ("Present Plan", "plan"),
        "notebook_edit" => ("Edit", "notebook"),
        "ask_user" => ("Ask", "user"),
        "web_search" => ("Search", "web"),
        "get_subagent_output" => ("Get Output", "subagent"),
        "task_complete" => ("Complete", "task"),
        "invoke_skill" => ("Skill", "skill"),
        _ if tool_name.starts_with("mcp__") => ("MCP", "tool"),
        _ if tool_name.starts_with("docker_") => ("Docker", "operation"),
        _ => ("Call", "tool"),
    }
}

/// Format a tool call with arguments for display.
///
/// Returns a string like `Read(/path/to/file.rs)` or `Bash(ls -la)`.
pub fn format_tool_call_display(
    tool_name: &str,
    args: &std::collections::HashMap<String, serde_json::Value>,
) -> String {
    let (verb, label) = tool_display_parts(tool_name);

    // Try to extract a meaningful summary from args
    let summary = extract_arg_summary(tool_name, args);
    if let Some(summary) = summary {
        return format!("{verb}({summary})");
    }

    // MCP tool: show server/tool format
    if tool_name.starts_with("mcp__") {
        let parts: Vec<&str> = tool_name.splitn(3, "__").collect();
        if parts.len() == 3 {
            return format!("MCP({}/{})", parts[1], parts[2]);
        }
    }

    // Fallback: verb(label)
    format!("{verb}({label})")
}

/// Extract a meaningful argument summary for display.
fn extract_arg_summary(
    tool_name: &str,
    args: &std::collections::HashMap<String, serde_json::Value>,
) -> Option<String> {
    if args.is_empty() {
        return None;
    }

    // Primary arg keys by tool
    let primary_keys: &[&str] = match tool_name {
        "read_file" | "read_pdf" | "write_file" | "edit_file" => &["file_path", "path"],
        "list_files" => &["path", "directory"],
        "search" => &["pattern", "query"],
        "run_command" | "bash_execute" | "Bash" => &["command"],
        "fetch_url" | "open_browser" | "capture_web_screenshot" => &["url"],
        "spawn_subagent" => &["description"],
        "find_symbol" | "rename_symbol" => &["name", "symbol"],
        _ => &["command", "file_path", "path", "url", "query", "pattern", "name"],
    };

    for key in primary_keys {
        if let Some(val) = args.get(*key) {
            if let Some(s) = val.as_str() {
                let display = s.replace('\n', " ");
                if display.len() > 80 {
                    return Some(format!("{}...", &display[..77]));
                }
                return Some(display);
            }
        }
    }

    None
}

/// Green gradient colors for nested tool spinner animation.
///
/// Matches Python `GREEN_GRADIENT` style tokens.
pub const GREEN_GRADIENT: &[Color] = &[
    Color::Rgb(0, 200, 80),
    Color::Rgb(0, 220, 100),
    Color::Rgb(0, 240, 120),
    Color::Rgb(0, 255, 140),
    Color::Rgb(0, 240, 120),
    Color::Rgb(0, 220, 100),
];

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_categorize_tool() {
        assert_eq!(categorize_tool("read_file"), ToolCategory::FileRead);
        assert_eq!(categorize_tool("edit_file"), ToolCategory::FileWrite);
        assert_eq!(categorize_tool("run_command"), ToolCategory::Bash);
        assert_eq!(categorize_tool("mcp__server__func"), ToolCategory::Mcp);
        assert_eq!(categorize_tool("docker_start"), ToolCategory::Docker);
        assert_eq!(categorize_tool("unknown_tool"), ToolCategory::Other);
    }

    #[test]
    fn test_tool_display_parts() {
        assert_eq!(tool_display_parts("read_file"), ("Read", "file"));
        assert_eq!(tool_display_parts("run_command"), ("Bash", "command"));
        assert_eq!(tool_display_parts("mcp__something"), ("MCP", "tool"));
    }

    #[test]
    fn test_format_tool_call_display() {
        let mut args = std::collections::HashMap::new();
        args.insert(
            "command".to_string(),
            serde_json::Value::String("ls -la".to_string()),
        );
        let display = format_tool_call_display("run_command", &args);
        assert_eq!(display, "Bash(ls -la)");
    }

    #[test]
    fn test_format_tool_call_no_args() {
        let args = std::collections::HashMap::new();
        let display = format_tool_call_display("list_todos", &args);
        assert_eq!(display, "List Todos(todos)");
    }

    #[test]
    fn test_format_mcp_tool() {
        let args = std::collections::HashMap::new();
        let display = format_tool_call_display("mcp__sqlite__query", &args);
        assert_eq!(display, "MCP(sqlite/query)");
    }
}
