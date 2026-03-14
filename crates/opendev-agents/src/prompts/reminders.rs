//! Reminder/nudge template parser and injection helpers.
//!
//! Parses `--- section_name ---` delimited sections from the embedded `reminders.md`
//! template and provides `get_reminder()` for variable substitution and `append_nudge()`
//! for injecting system nudges into the message history.

use std::collections::HashMap;
use std::sync::OnceLock;

use super::embedded;

static SECTIONS: OnceLock<HashMap<String, String>> = OnceLock::new();

/// Parse `--- section_name ---` delimited sections from the REMINDERS template.
fn parse_sections() -> HashMap<String, String> {
    let mut map = HashMap::new();
    let mut current_name: Option<String> = None;
    let mut current_lines: Vec<&str> = Vec::new();

    for line in embedded::REMINDERS.lines() {
        let trimmed = line.trim();
        if let Some(rest) = trimmed.strip_prefix("---")
            && let Some(name) = rest.strip_suffix("---")
        {
            // Flush previous section
            if let Some(prev_name) = current_name.take() {
                let content = current_lines.join("\n").trim().to_string();
                if !content.is_empty() {
                    map.insert(prev_name, content);
                }
            }
            current_name = Some(name.trim().to_string());
            current_lines.clear();
            continue;
        }
        if current_name.is_some() {
            current_lines.push(line);
        }
    }
    // Flush last section
    if let Some(name) = current_name {
        let content = current_lines.join("\n").trim().to_string();
        if !content.is_empty() {
            map.insert(name, content);
        }
    }

    map
}

/// Look up a reminder template by section name and substitute `{key}` placeholders.
///
/// Returns an empty string if the section is not found.
pub fn get_reminder(name: &str, vars: &[(&str, &str)]) -> String {
    let sections = SECTIONS.get_or_init(parse_sections);
    let template = match sections.get(name) {
        Some(t) => t.clone(),
        None => return String::new(),
    };
    let mut result = template;
    for (key, val) in vars {
        result = result.replace(&format!("{{{key}}}"), val);
    }
    result
}

/// Append a system nudge message to the conversation history.
///
/// The message is tagged with `_nudge: true` so it can be identified as
/// an injected nudge rather than a real user message.
pub fn append_nudge(messages: &mut Vec<serde_json::Value>, content: &str) {
    messages.push(serde_json::json!({
        "role": "user",
        "content": format!("[SYSTEM] {content}"),
        "_nudge": true,
    }));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_parse_sections_finds_templates() {
        let sections = SECTIONS.get_or_init(parse_sections);
        assert!(
            sections.contains_key("failed_tool_nudge"),
            "Should find failed_tool_nudge section"
        );
        assert!(
            sections.contains_key("nudge_permission_error"),
            "Should find nudge_permission_error section"
        );
        assert!(
            sections.contains_key("incomplete_todos_nudge"),
            "Should find incomplete_todos_nudge section"
        );
        assert!(
            sections.contains_key("consecutive_reads_nudge"),
            "Should find consecutive_reads_nudge section"
        );
        assert!(
            sections.contains_key("implicit_completion_nudge"),
            "Should find implicit_completion_nudge section"
        );
        assert!(
            sections.contains_key("all_todos_complete_nudge"),
            "Should find all_todos_complete_nudge section"
        );
        assert!(
            sections.contains_key("thinking_analysis_prompt"),
            "Should find thinking_analysis_prompt section"
        );
        assert!(
            sections.contains_key("thinking_analysis_prompt_with_todos"),
            "Should find thinking_analysis_prompt_with_todos section"
        );
    }

    #[test]
    fn test_get_reminder_with_vars() {
        let result = get_reminder(
            "incomplete_todos_nudge",
            &[("count", "3"), ("todo_list", "  - A\n  - B\n  - C")],
        );
        assert!(result.contains("3 incomplete todo(s)"));
        assert!(result.contains("  - A"));
    }

    #[test]
    fn test_get_reminder_missing() {
        let result = get_reminder("nonexistent_section_xyz", &[]);
        assert!(result.is_empty());
    }

    #[test]
    fn test_append_nudge() {
        let mut messages = Vec::new();
        append_nudge(&mut messages, "test nudge");
        assert_eq!(messages.len(), 1);
        assert_eq!(messages[0]["role"], "user");
        assert_eq!(messages[0]["content"], "[SYSTEM] test nudge");
        assert_eq!(messages[0]["_nudge"], true);
    }
}
