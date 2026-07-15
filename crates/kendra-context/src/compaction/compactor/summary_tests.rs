use super::*;
use crate::compaction::StructuralContext;
use serde_json::json;

fn make_msg(role: &str, content: &str) -> ApiMessage {
    let mut msg = ApiMessage::new();
    msg.insert("role".into(), json!(role));
    msg.insert("content".into(), json!(content));
    msg
}

fn make_tool_msg(name: &str, content: &str) -> ApiMessage {
    let mut msg = ApiMessage::new();
    msg.insert("role".into(), json!("tool"));
    msg.insert("name".into(), json!(name));
    msg.insert("content".into(), json!(content));
    msg
}

fn make_array_content_msg(role: &str, text: &str) -> ApiMessage {
    let mut msg = ApiMessage::new();
    msg.insert("role".into(), json!(role));
    msg.insert("content".into(), json!([{"type": "text", "text": text}]));
    msg
}

#[test]
fn test_fallback_summary_basic_structure() {
    let mut compactor = ContextCompactor::new(1000);
    let messages = vec![
        make_msg("user", "Fix the login bug in auth.rs"),
        make_tool_msg("read_file", "fn login() { /* broken */ }"),
        make_msg("assistant", "I found the issue in the login function"),
    ];
    let summary = compactor.fallback_summary(&messages);
    assert!(summary.contains("## Goal"));
    assert!(summary.contains("Fix the login bug"));
    assert!(summary.contains("## Key Actions"));
    assert!(summary.contains("read_file:"));
    assert!(summary.contains("## Current State"));
    assert!(summary.contains("I found the issue"));
}

#[test]
fn test_fallback_summary_with_array_content() {
    let mut compactor = ContextCompactor::new(1000);
    let messages = vec![
        make_array_content_msg("user", "Refactor the parser"),
        make_msg("assistant", "Working on it"),
    ];
    let summary = compactor.fallback_summary(&messages);
    assert!(summary.contains("Refactor the parser"));
}

#[test]
fn test_fallback_summary_tool_results_included() {
    let mut compactor = ContextCompactor::new(1000);
    let messages = vec![
        make_msg("user", "Read the config"),
        make_tool_msg("read_file", "key = value"),
        make_tool_msg("search", "found 3 matches"),
        make_msg("assistant", "Done analyzing"),
    ];
    let summary = compactor.fallback_summary(&messages);
    assert!(summary.contains("read_file: key = value"));
    assert!(summary.contains("search: found 3 matches"));
}

#[test]
fn test_fallback_summary_truncation_at_4000_chars() {
    let mut compactor = ContextCompactor::new(1000);
    let long_content = "x".repeat(200);
    let mut messages = Vec::new();
    messages.push(make_msg("user", "Do something"));
    for i in 0..50 {
        messages.push(make_tool_msg(&format!("tool_{i}"), &long_content));
    }
    let summary = compactor.fallback_summary(&messages);
    // Should stop before including all 50 tool results
    let action_count = summary.matches("- tool_").count();
    assert!(action_count < 50);
    assert!(action_count > 0);
}

#[test]
fn test_fallback_summary_empty_messages() {
    let mut compactor = ContextCompactor::new(1000);
    let summary = compactor.fallback_summary(&[]);
    assert!(summary.contains("Unknown"));
    assert!(summary.contains("None recorded"));
    assert!(summary.contains("No assistant response recorded"));
}

#[test]
fn test_fallback_summary_skips_system_messages_for_goal() {
    let mut compactor = ContextCompactor::new(1000);
    let messages = vec![
        make_msg("user", "[SYSTEM] You are an AI assistant"),
        make_msg("user", "Help me with X"),
        make_msg("assistant", "Sure"),
    ];
    let summary = compactor.fallback_summary(&messages);
    assert!(summary.contains("Help me with X"));
    assert!(!summary.contains("[SYSTEM]"));
}

#[test]
fn test_compact_includes_structural_context() {
    let mut compactor = ContextCompactor::new(1000);
    compactor.set_structural_context(StructuralContext {
        overview: "My Project Overview".to_string(),
        dependencies: "dep1, dep2".to_string(),
        key_files: vec!["main.rs".to_string()],
    });

    let messages = vec![
        make_msg("system", "System prompt"),
        make_msg("user", "Message 1"),
        make_msg("assistant", "Response 1"),
        make_msg("user", "Message 2"),
        make_msg("assistant", "Response 2"),
        make_msg("user", "Message 3"),
    ];

    let compacted = compactor.compact(messages, "System prompt");

    // compacted[0] is head (system prompt)
    // compacted[1] is summary message
    let summary_content = compacted[1]
        .get("content")
        .and_then(|v| v.as_str())
        .unwrap();

    assert!(summary_content.contains("## Project Context"));
    assert!(summary_content.contains("My Project Overview"));
}

#[test]
fn test_extract_content_string() {
    let msg = make_msg("user", "hello");
    assert_eq!(ContextCompactor::extract_content(&msg), "hello");
}

#[test]
fn test_extract_content_array() {
    let msg = make_array_content_msg("user", "multi-part content");
    assert_eq!(
        ContextCompactor::extract_content(&msg),
        "multi-part content"
    );
}

#[test]
fn test_extract_content_missing() {
    let msg = ApiMessage::new();
    assert_eq!(ContextCompactor::extract_content(&msg), "");
}

#[test]
fn test_sanitize_for_summarization_handles_array_content() {
    let messages = vec![
        make_array_content_msg("user", "array content message"),
        make_msg("assistant", "string content message"),
    ];
    let result = ContextCompactor::sanitize_for_summarization(&messages);
    assert!(result.contains("array content message"));
    assert!(result.contains("string content message"));
}
