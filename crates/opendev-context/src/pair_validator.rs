//! Message pair integrity validator for API message lists.
//!
//! Ensures structural invariants:
//! - Every assistant tool_call has a corresponding tool result message
//! - No orphaned tool results without matching tool_calls
//! - Detects consecutive same-role violations (warning only)

use std::collections::HashMap;

use crate::compaction::ApiMessage;

/// Synthetic error message for missing tool results.
pub const SYNTHETIC_TOOL_RESULT: &str =
    "Error: Tool execution result was lost. The tool may have been interrupted or crashed.";

/// Type of structural violation found.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ViolationType {
    MissingToolResult,
    OrphanedToolResult,
    ConsecutiveSameRole,
}

/// A single structural violation.
#[derive(Debug, Clone)]
pub struct Violation {
    pub violation_type: ViolationType,
    pub index: usize,
    pub detail: String,
}

/// Result of validating a message list.
#[derive(Debug, Clone, Default)]
pub struct ValidationResult {
    pub violations: Vec<Violation>,
    pub repair_actions: Vec<String>,
    pub repaired: bool,
}

impl ValidationResult {
    pub fn is_valid(&self) -> bool {
        self.violations.is_empty()
    }
}

/// Validates and repairs structural integrity of flat API message lists.
pub struct MessagePairValidator;

impl MessagePairValidator {
    /// Ensure every tool_call has an entry in results. Fill missing with synthetic errors.
    ///
    /// This is the pre-batch-add guard. Call BEFORE iterating tool_calls to add
    /// results to history.
    pub fn validate_tool_results_complete(
        tool_calls: &[serde_json::Value],
        tool_results_by_id: &mut HashMap<String, serde_json::Value>,
    ) {
        for tc in tool_calls {
            let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if !tc_id.is_empty() && !tool_results_by_id.contains_key(tc_id) {
                let tool_name = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                tracing::warn!(
                    "Missing tool result for {} (id={}), inserting synthetic error",
                    tool_name,
                    tc_id
                );
                tool_results_by_id.insert(
                    tc_id.to_string(),
                    serde_json::json!({
                        "success": false,
                        "error": format!("Tool '{}' execution was interrupted or never started.", tool_name),
                        "output": "",
                        "synthetic": true,
                    }),
                );
            }
        }
    }

    /// Validate structural integrity of an API message list.
    ///
    /// Single forward pass checking:
    /// - Every tool_call ID from assistant messages has a matching tool result
    /// - No orphaned tool results without a preceding tool_call
    /// - Consecutive same-role messages (warning only)
    pub fn validate(messages: &[ApiMessage]) -> ValidationResult {
        let mut result = ValidationResult::default();
        let mut expected_tool_results: HashMap<String, usize> = HashMap::new();
        let mut prev_role: Option<String> = None;

        for (i, msg) in messages.iter().enumerate() {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            // Check consecutive same role (warning only, skip tool role)
            if let Some(ref prev) = prev_role
                && *prev == role
                && role != "tool"
            {
                result.violations.push(Violation {
                    violation_type: ViolationType::ConsecutiveSameRole,
                    index: i,
                    detail: format!("Consecutive '{}' at index {}", role, i),
                });
            }

            if role == "assistant" {
                if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                    for tc in tool_calls {
                        let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                        if !tc_id.is_empty() {
                            expected_tool_results.insert(tc_id.to_string(), i);
                        }
                    }
                }
            } else if role == "tool" {
                let tc_id = msg
                    .get("tool_call_id")
                    .and_then(|v| v.as_str())
                    .unwrap_or("");
                if expected_tool_results.remove(tc_id).is_none() && !tc_id.is_empty() {
                    result.violations.push(Violation {
                        violation_type: ViolationType::OrphanedToolResult,
                        index: i,
                        detail: format!("Orphaned tool result for id={} at index {}", tc_id, i),
                    });
                }
            }

            prev_role = Some(role);
        }

        // Remaining expected IDs are missing
        for (tc_id, assistant_idx) in &expected_tool_results {
            result.violations.push(Violation {
                violation_type: ViolationType::MissingToolResult,
                index: *assistant_idx,
                detail: format!(
                    "Missing tool result for id={} (assistant at index {})",
                    tc_id, assistant_idx
                ),
            });
        }

        result
    }

    /// Validate and repair an API message list.
    ///
    /// - MISSING_TOOL_RESULT: Insert synthetic tool result after the assistant's tool results.
    /// - ORPHANED_TOOL_RESULT: Remove the orphaned message.
    pub fn repair(messages: &[ApiMessage]) -> (Vec<ApiMessage>, ValidationResult) {
        let mut vr = Self::validate(messages);
        if vr.is_valid() {
            return (messages.to_vec(), vr);
        }

        // Collect orphan indices
        let mut orphan_indices: std::collections::HashSet<usize> = std::collections::HashSet::new();
        for v in &vr.violations {
            if v.violation_type == ViolationType::OrphanedToolResult {
                orphan_indices.insert(v.index);
                vr.repair_actions
                    .push(format!("Removed orphaned tool result at index {}", v.index));
            }
        }

        // Collect missing tool result IDs grouped by assistant message index
        let mut missing_by_assistant: HashMap<usize, Vec<String>> = HashMap::new();
        for v in &vr.violations {
            if v.violation_type == ViolationType::MissingToolResult {
                // Extract tc_id from detail string
                if let Some(pos) = v.detail.find("id=") {
                    let rest = &v.detail[pos + 3..];
                    let tc_id: String = rest.chars().take_while(|c| *c != ' ').collect();
                    if !tc_id.is_empty() {
                        missing_by_assistant.entry(v.index).or_default().push(tc_id);
                    }
                }
            }
        }

        // Build repaired list
        let mut repaired: Vec<ApiMessage> = Vec::new();
        let mut i = 0;
        while i < messages.len() {
            if orphan_indices.contains(&i) {
                i += 1;
                continue;
            }

            repaired.push(messages[i].clone());

            if missing_by_assistant.contains_key(&i) {
                // Skip past existing tool results for this assistant
                let mut j = i + 1;
                while j < messages.len()
                    && messages[j].get("role").and_then(|v| v.as_str()) == Some("tool")
                {
                    if !orphan_indices.contains(&j) {
                        repaired.push(messages[j].clone());
                    }
                    j += 1;
                }

                // Insert synthetic results for missing IDs
                for tc_id in &missing_by_assistant[&i] {
                    let mut msg = ApiMessage::new();
                    msg.insert(
                        "role".to_string(),
                        serde_json::Value::String("tool".to_string()),
                    );
                    msg.insert(
                        "tool_call_id".to_string(),
                        serde_json::Value::String(tc_id.clone()),
                    );
                    msg.insert(
                        "content".to_string(),
                        serde_json::Value::String(SYNTHETIC_TOOL_RESULT.to_string()),
                    );
                    repaired.push(msg);
                    vr.repair_actions
                        .push(format!("Inserted synthetic tool result for id={}", tc_id));
                }

                i = j;
                continue;
            }

            i += 1;
        }

        vr.repaired = !vr.repair_actions.is_empty();
        (repaired, vr)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn make_msg(role: &str, content: &str) -> ApiMessage {
        let mut msg = ApiMessage::new();
        msg.insert(
            "role".to_string(),
            serde_json::Value::String(role.to_string()),
        );
        msg.insert(
            "content".to_string(),
            serde_json::Value::String(content.to_string()),
        );
        msg
    }

    fn make_assistant_with_tc(tc_ids: &[&str]) -> ApiMessage {
        let mut msg = ApiMessage::new();
        msg.insert(
            "role".to_string(),
            serde_json::Value::String("assistant".to_string()),
        );
        msg.insert(
            "content".to_string(),
            serde_json::Value::String(String::new()),
        );
        let tcs: Vec<serde_json::Value> = tc_ids
            .iter()
            .map(|id| {
                serde_json::json!({
                    "id": id,
                    "function": { "name": "bash", "arguments": "{}" }
                })
            })
            .collect();
        msg.insert("tool_calls".to_string(), serde_json::Value::Array(tcs));
        msg
    }

    fn make_tool_result(tool_call_id: &str, content: &str) -> ApiMessage {
        let mut msg = ApiMessage::new();
        msg.insert(
            "role".to_string(),
            serde_json::Value::String("tool".to_string()),
        );
        msg.insert(
            "tool_call_id".to_string(),
            serde_json::Value::String(tool_call_id.to_string()),
        );
        msg.insert(
            "content".to_string(),
            serde_json::Value::String(content.to_string()),
        );
        msg
    }

    #[test]
    fn test_valid_messages() {
        let messages = vec![
            make_msg("user", "hello"),
            make_assistant_with_tc(&["tc-1"]),
            make_tool_result("tc-1", "result"),
            make_msg("assistant", "done"),
        ];
        let result = MessagePairValidator::validate(&messages);
        assert!(result.is_valid());
    }

    #[test]
    fn test_missing_tool_result() {
        let messages = vec![
            make_msg("user", "hello"),
            make_assistant_with_tc(&["tc-1", "tc-2"]),
            make_tool_result("tc-1", "result"),
            // tc-2 result is missing
        ];
        let result = MessagePairValidator::validate(&messages);
        assert!(!result.is_valid());
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.violation_type == ViolationType::MissingToolResult)
        );
    }

    #[test]
    fn test_orphaned_tool_result() {
        let messages = vec![
            make_msg("user", "hello"),
            make_msg("assistant", "thinking..."),
            make_tool_result("tc-999", "orphan result"),
        ];
        let result = MessagePairValidator::validate(&messages);
        assert!(!result.is_valid());
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.violation_type == ViolationType::OrphanedToolResult)
        );
    }

    #[test]
    fn test_consecutive_same_role() {
        let messages = vec![make_msg("user", "hello"), make_msg("user", "hello again")];
        let result = MessagePairValidator::validate(&messages);
        assert!(
            result
                .violations
                .iter()
                .any(|v| v.violation_type == ViolationType::ConsecutiveSameRole)
        );
    }

    #[test]
    fn test_consecutive_tool_messages_ok() {
        // Multiple tool results in a row are valid (parallel tool execution)
        let messages = vec![
            make_msg("user", "run two things"),
            make_assistant_with_tc(&["tc-1", "tc-2"]),
            make_tool_result("tc-1", "result1"),
            make_tool_result("tc-2", "result2"),
        ];
        let result = MessagePairValidator::validate(&messages);
        // Should not have ConsecutiveSameRole for tool messages
        assert!(
            !result
                .violations
                .iter()
                .any(|v| v.violation_type == ViolationType::ConsecutiveSameRole)
        );
    }

    #[test]
    fn test_repair_missing_tool_result() {
        let messages = vec![
            make_msg("user", "hello"),
            make_assistant_with_tc(&["tc-1"]),
            // Missing tool result for tc-1
            make_msg("user", "next"),
        ];
        let (repaired, vr) = MessagePairValidator::repair(&messages);
        assert!(vr.repaired);
        // Should have inserted synthetic result
        let has_synthetic = repaired
            .iter()
            .any(|m| m.get("content").and_then(|v| v.as_str()) == Some(SYNTHETIC_TOOL_RESULT));
        assert!(has_synthetic);
    }

    #[test]
    fn test_repair_orphaned_tool_result() {
        let messages = vec![
            make_msg("user", "hello"),
            make_msg("assistant", "thinking"),
            make_tool_result("tc-orphan", "orphan"),
        ];
        let (repaired, vr) = MessagePairValidator::repair(&messages);
        assert!(vr.repaired);
        // Orphan should be removed
        assert_eq!(repaired.len(), 2);
    }

    #[test]
    fn test_validate_tool_results_complete() {
        let tool_calls = vec![
            serde_json::json!({"id": "tc-1", "function": {"name": "bash", "arguments": "{}"}}),
            serde_json::json!({"id": "tc-2", "function": {"name": "read_file", "arguments": "{}"}}),
        ];
        let mut results: HashMap<String, serde_json::Value> = HashMap::new();
        results.insert("tc-1".to_string(), serde_json::json!({"output": "ok"}));
        // tc-2 is missing

        MessagePairValidator::validate_tool_results_complete(&tool_calls, &mut results);
        assert!(results.contains_key("tc-2"));
        assert_eq!(results["tc-2"]["synthetic"].as_bool(), Some(true));
    }
}
