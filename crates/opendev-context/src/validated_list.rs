//! ValidatedMessageList — write-time enforcement of message pair invariants.
//!
//! Wraps a `Vec<ApiMessage>` and enforces structural invariants on every
//! mutation. All reads work identically to Vec. Mutations are intercepted
//! and routed through validated methods.
//!
//! State machine:
//!     EXPECT_ANY --add_assistant(tc)--> EXPECT_TOOL_RESULTS{pending_ids}
//!          ^                                    |
//!          |                          add_tool_result(id) removes from pending
//!          |                                    |
//!          +------ all pending satisfied -------+

use std::collections::HashSet;
use std::sync::Mutex;

use tracing::warn;

use crate::compaction::ApiMessage;

/// Synthetic error message for incomplete tool results.
pub const SYNTHETIC_TOOL_RESULT: &str =
    "Error: Tool execution result was lost. The tool may have been interrupted or crashed.";

/// Drop-in Vec replacement that enforces message pair invariants.
///
/// All reads (iteration, indexing, len) work through `messages()`.
/// Mutations are intercepted via validated methods.
pub struct ValidatedMessageList {
    messages: Vec<ApiMessage>,
    pending_tool_ids: Mutex<HashSet<String>>,
    strict: bool,
}

impl ValidatedMessageList {
    /// Create a new validated message list.
    ///
    /// If `initial` is provided, bulk-loads without per-message validation
    /// (trusts existing data), then rebuilds pending state.
    pub fn new(initial: Option<Vec<ApiMessage>>, strict: bool) -> Self {
        let messages = initial.unwrap_or_default();
        let mut list = Self {
            messages,
            pending_tool_ids: Mutex::new(HashSet::new()),
            strict,
        };
        list.rebuild_pending_state();
        list
    }

    /// Access the underlying messages (read-only).
    pub fn messages(&self) -> &[ApiMessage] {
        &self.messages
    }

    /// Consume self and return the inner Vec.
    pub fn into_inner(self) -> Vec<ApiMessage> {
        self.messages
    }

    pub fn len(&self) -> usize {
        self.messages.len()
    }

    pub fn is_empty(&self) -> bool {
        self.messages.is_empty()
    }

    /// Tool call IDs still awaiting results.
    pub fn pending_tool_ids(&self) -> HashSet<String> {
        self.pending_tool_ids.lock().unwrap().clone()
    }

    /// True if in EXPECT_TOOL_RESULTS state.
    pub fn has_pending_tools(&self) -> bool {
        !self.pending_tool_ids.lock().unwrap().is_empty()
    }

    /// Append a user message. Auto-completes pending tool results if any.
    pub fn add_user(&mut self, content: &str) {
        self.auto_complete_pending("add_user");
        let mut msg = ApiMessage::new();
        msg.insert(
            "role".to_string(),
            serde_json::Value::String("user".to_string()),
        );
        msg.insert(
            "content".to_string(),
            serde_json::Value::String(content.to_string()),
        );
        self.messages.push(msg);
    }

    /// Append assistant message. If tool_calls present, enters EXPECT_TOOL_RESULTS.
    pub fn add_assistant(
        &mut self,
        content: Option<&str>,
        tool_calls: Option<Vec<serde_json::Value>>,
    ) {
        self.auto_complete_pending("add_assistant");
        let mut msg = ApiMessage::new();
        msg.insert(
            "role".to_string(),
            serde_json::Value::String("assistant".to_string()),
        );
        msg.insert(
            "content".to_string(),
            serde_json::Value::String(content.unwrap_or("").to_string()),
        );
        if let Some(tcs) = tool_calls {
            let mut pending = self.pending_tool_ids.lock().unwrap();
            for tc in &tcs {
                if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                    if !id.is_empty() {
                        pending.insert(id.to_string());
                    }
                }
            }
            msg.insert("tool_calls".to_string(), serde_json::Value::Array(tcs));
        }
        self.messages.push(msg);
    }

    /// Append tool result. Rejects orphaned IDs not in pending set (in strict mode).
    pub fn add_tool_result(&mut self, tool_call_id: &str, content: &str) -> Result<(), String> {
        let mut pending = self.pending_tool_ids.lock().unwrap();
        if !pending.contains(tool_call_id) {
            let detail = format!("Orphaned tool result for id={tool_call_id}");
            if self.strict {
                return Err(detail);
            }
            warn!(
                "ValidatedMessageList: {} (permissive mode, accepting)",
                detail
            );
        } else {
            pending.remove(tool_call_id);
        }
        drop(pending);

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
        self.messages.push(msg);
        Ok(())
    }

    /// Batch-add tool results. Fills missing with synthetic errors.
    pub fn add_tool_results_batch(
        &mut self,
        tool_calls: &[serde_json::Value],
        results_by_id: &std::collections::HashMap<String, String>,
    ) {
        let mut pending = self.pending_tool_ids.lock().unwrap();
        for tc in tool_calls {
            let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
            if tc_id.is_empty() {
                continue;
            }
            let content = if let Some(result) = results_by_id.get(tc_id) {
                result.clone()
            } else {
                let tool_name = tc
                    .get("function")
                    .and_then(|f| f.get("name"))
                    .and_then(|n| n.as_str())
                    .unwrap_or("unknown");
                warn!(
                    "ValidatedMessageList: Missing result for {} (id={}), inserting synthetic error",
                    tool_name, tc_id
                );
                SYNTHETIC_TOOL_RESULT.to_string()
            };
            pending.remove(tc_id);

            let mut msg = ApiMessage::new();
            msg.insert(
                "role".to_string(),
                serde_json::Value::String("tool".to_string()),
            );
            msg.insert(
                "tool_call_id".to_string(),
                serde_json::Value::String(tc_id.to_string()),
            );
            msg.insert(
                "content".to_string(),
                serde_json::Value::String(content),
            );
            drop(pending);
            self.messages.push(msg);
            pending = self.pending_tool_ids.lock().unwrap();
        }
    }

    /// Replace all messages (e.g., after compaction). Rebuilds pending state.
    pub fn replace_all(&mut self, messages: Vec<ApiMessage>) {
        self.messages = messages;
        self.rebuild_pending_state();
    }

    /// Scan all messages to reconstruct pending tool_call IDs.
    fn rebuild_pending_state(&mut self) {
        let mut expected: HashSet<String> = HashSet::new();
        for msg in &self.messages {
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if role == "assistant" {
                if let Some(tcs) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                    for tc in tcs {
                        if let Some(id) = tc.get("id").and_then(|v| v.as_str()) {
                            if !id.is_empty() {
                                expected.insert(id.to_string());
                            }
                        }
                    }
                }
            } else if role == "tool" {
                if let Some(id) = msg.get("tool_call_id").and_then(|v| v.as_str()) {
                    expected.remove(id);
                }
            }
        }
        *self.pending_tool_ids.lock().unwrap() = expected;
    }

    /// Insert synthetic error results for any pending tool calls.
    fn auto_complete_pending(&mut self, source: &str) {
        let mut pending = self.pending_tool_ids.lock().unwrap();
        if pending.is_empty() {
            return;
        }
        warn!(
            "ValidatedMessageList: Auto-completing {} pending tool results before {}: {:?}",
            pending.len(),
            source,
            pending,
        );
        let ids: Vec<String> = pending.drain().collect();
        drop(pending);

        for tc_id in ids {
            let mut msg = ApiMessage::new();
            msg.insert(
                "role".to_string(),
                serde_json::Value::String("tool".to_string()),
            );
            msg.insert(
                "tool_call_id".to_string(),
                serde_json::Value::String(tc_id),
            );
            msg.insert(
                "content".to_string(),
                serde_json::Value::String(SYNTHETIC_TOOL_RESULT.to_string()),
            );
            self.messages.push(msg);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tc(id: &str, name: &str) -> serde_json::Value {
        serde_json::json!({
            "id": id,
            "function": { "name": name, "arguments": "{}" }
        })
    }

    #[test]
    fn test_basic_user_assistant_flow() {
        let mut list = ValidatedMessageList::new(None, false);
        list.add_user("hello");
        list.add_assistant(Some("hi there"), None);
        assert_eq!(list.len(), 2);
        assert!(!list.has_pending_tools());
    }

    #[test]
    fn test_tool_call_flow() {
        let mut list = ValidatedMessageList::new(None, false);
        list.add_user("run ls");
        list.add_assistant(None, Some(vec![tc("tc-1", "bash")]));
        assert!(list.has_pending_tools());
        assert!(list.pending_tool_ids().contains("tc-1"));

        list.add_tool_result("tc-1", "file1.txt\nfile2.txt")
            .unwrap();
        assert!(!list.has_pending_tools());
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_auto_complete_pending_on_new_user() {
        let mut list = ValidatedMessageList::new(None, false);
        list.add_assistant(None, Some(vec![tc("tc-1", "bash")]));
        assert!(list.has_pending_tools());

        // Adding a user message auto-completes the pending tool result
        list.add_user("next question");
        assert!(!list.has_pending_tools());
        // Should have: assistant + synthetic tool result + user = 3
        assert_eq!(list.len(), 3);

        // Check synthetic result was inserted
        let tool_msg = &list.messages()[1];
        assert_eq!(
            tool_msg.get("role").and_then(|v| v.as_str()),
            Some("tool")
        );
        assert_eq!(
            tool_msg.get("content").and_then(|v| v.as_str()),
            Some(SYNTHETIC_TOOL_RESULT)
        );
    }

    #[test]
    fn test_strict_mode_rejects_orphan() {
        let mut list = ValidatedMessageList::new(None, true);
        list.add_user("hello");

        // No pending tool calls, so adding a tool result should fail in strict mode
        let result = list.add_tool_result("nonexistent", "data");
        assert!(result.is_err());
    }

    #[test]
    fn test_permissive_mode_accepts_orphan() {
        let mut list = ValidatedMessageList::new(None, false);
        list.add_user("hello");

        // Permissive mode: warns but accepts
        let result = list.add_tool_result("nonexistent", "data");
        assert!(result.is_ok());
        assert_eq!(list.len(), 2);
    }

    #[test]
    fn test_batch_tool_results() {
        let mut list = ValidatedMessageList::new(None, false);
        let tcs = vec![tc("tc-1", "bash"), tc("tc-2", "read_file")];
        list.add_assistant(None, Some(tcs.clone()));

        let mut results = std::collections::HashMap::new();
        results.insert("tc-1".to_string(), "output1".to_string());
        // tc-2 is missing — should get synthetic error

        list.add_tool_results_batch(&tcs, &results);
        assert!(!list.has_pending_tools());
        // assistant + 2 tool results = 3
        assert_eq!(list.len(), 3);
    }

    #[test]
    fn test_replace_all_rebuilds_state() {
        let mut list = ValidatedMessageList::new(None, false);
        list.add_user("hello");
        list.add_assistant(None, Some(vec![tc("tc-1", "bash")]));

        // Replace with a clean conversation
        let mut new_msgs = vec![];
        let mut msg = ApiMessage::new();
        msg.insert(
            "role".to_string(),
            serde_json::Value::String("user".to_string()),
        );
        msg.insert(
            "content".to_string(),
            serde_json::Value::String("fresh start".to_string()),
        );
        new_msgs.push(msg);

        list.replace_all(new_msgs);
        assert_eq!(list.len(), 1);
        assert!(!list.has_pending_tools());
    }

    #[test]
    fn test_initial_data_rebuild() {
        // Simulate loading from persisted data with a pending tool call
        let assistant_msg = {
            let mut msg = ApiMessage::new();
            msg.insert(
                "role".to_string(),
                serde_json::Value::String("assistant".to_string()),
            );
            msg.insert(
                "content".to_string(),
                serde_json::Value::String(String::new()),
            );
            msg.insert(
                "tool_calls".to_string(),
                serde_json::json!([{"id": "tc-1", "function": {"name": "bash", "arguments": "{}"}}]),
            );
            msg
        };

        let list = ValidatedMessageList::new(Some(vec![assistant_msg]), false);
        assert!(list.has_pending_tools());
        assert!(list.pending_tool_ids().contains("tc-1"));
    }
}
