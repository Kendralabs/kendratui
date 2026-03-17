//! Compaction and summarization logic.
//!
//! Implements full compaction (fallback and LLM-powered), sliding window
//! compaction for very long sessions, and message sanitization for
//! summarization payloads.

use tracing::info;

use super::super::ApiMessage;
use super::ContextCompactor;

impl ContextCompactor {
    /// Apply sliding window compaction for sessions with 500+ messages.
    ///
    /// Keeps the first message (system prompt) and the most recent
    /// `SLIDING_WINDOW_RECENT` messages, replacing everything in between
    /// with a compressed summary. This runs *before* the staged approach.
    pub fn sliding_window_compact(&mut self, messages: Vec<ApiMessage>) -> Vec<ApiMessage> {
        use super::super::SLIDING_WINDOW_RECENT;
        use super::super::SLIDING_WINDOW_THRESHOLD;

        if messages.len() < SLIDING_WINDOW_THRESHOLD {
            return messages;
        }

        let keep_start = 1; // preserve first message
        let keep_end = messages.len().saturating_sub(SLIDING_WINDOW_RECENT);

        if keep_end <= keep_start {
            return messages;
        }

        let head = &messages[..keep_start];
        let middle = &messages[keep_start..keep_end];
        let tail = &messages[keep_end..];

        let summary_text = Self::fallback_summary(middle);
        let artifact_summary = self.artifact_index.as_summary();
        let mut full_summary = format!(
            "[SLIDING WINDOW SUMMARY — {msg_count} messages compressed]\n{summary_text}",
            msg_count = middle.len(),
        );
        if !artifact_summary.is_empty() {
            full_summary.push_str("\n\n");
            full_summary.push_str(&artifact_summary);
        }

        let mut summary_msg = ApiMessage::new();
        summary_msg.insert("role".into(), serde_json::Value::String("user".into()));
        summary_msg.insert("content".into(), serde_json::Value::String(full_summary));

        let mut result = Vec::with_capacity(head.len() + 1 + tail.len());
        result.extend_from_slice(head);
        result.push(summary_msg);
        result.extend_from_slice(tail);

        info!(
            "Sliding window compaction: {} -> {} messages (compressed {} middle, kept {} recent)",
            messages.len(),
            result.len(),
            middle.len(),
            tail.len(),
        );

        result
    }

    /// Compact older messages into a summary, preserving recent context.
    ///
    /// Returns the compacted message list. Uses a fallback summary since
    /// LLM-powered summarization requires an HTTP client (handled at a higher layer).
    pub fn compact(&mut self, messages: Vec<ApiMessage>, _system_prompt: &str) -> Vec<ApiMessage> {
        if messages.len() <= 4 {
            return messages;
        }

        let keep_recent = (messages.len() / 3).clamp(2, 5);
        let split_point = messages.len() - keep_recent;

        let head = &messages[..1];
        let middle = &messages[1..split_point];
        let tail = &messages[split_point..];

        if middle.is_empty() {
            return messages;
        }

        let summary_text = Self::fallback_summary(middle);
        let artifact_summary = self.artifact_index.as_summary();
        let mut full_summary = format!("[CONVERSATION SUMMARY]\n{summary_text}");
        if !artifact_summary.is_empty() {
            full_summary.push_str("\n\n");
            full_summary.push_str(&artifact_summary);
        }

        let mut summary_msg = ApiMessage::new();
        summary_msg.insert("role".into(), serde_json::Value::String("user".into()));
        summary_msg.insert("content".into(), serde_json::Value::String(full_summary));

        let mut compacted = Vec::with_capacity(head.len() + 1 + tail.len());
        compacted.extend_from_slice(head);
        compacted.push(summary_msg);
        compacted.extend_from_slice(tail);

        info!(
            "Compacted {} messages -> {} (removed {}, kept {} recent)",
            messages.len(),
            compacted.len(),
            middle.len(),
            keep_recent,
        );

        // Invalidate calibration
        self.api_prompt_tokens = 0;
        self.msg_count_at_calibration = 0;
        self.warned_70 = false;
        self.warned_80 = false;
        self.warned_90 = false;

        compacted
    }

    /// Create a basic summary without an LLM call.
    pub fn fallback_summary(messages: &[ApiMessage]) -> String {
        use std::fmt::Write;

        // Pre-allocate for ~2000 chars of content plus formatting overhead
        let mut buf = String::with_capacity(2200);
        let mut total = 0usize;
        let mut entry_count = 0usize;
        for msg in messages {
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            let role = msg.get("role").and_then(|v| v.as_str()).unwrap_or("");
            if !content.is_empty() && (role == "user" || role == "assistant") {
                let snippet: String = content.chars().take(200).collect();
                if entry_count > 0 {
                    buf.push('\n');
                }
                let _ = write!(buf, "- [{role}] {snippet}");
                total += snippet.len();
                entry_count += 1;
                if total > 2000 {
                    let remaining = messages.len().saturating_sub(entry_count);
                    let _ = write!(buf, "\n... ({remaining} more messages)");
                    break;
                }
            }
        }
        buf
    }

    /// Sanitize messages for LLM summarization.
    ///
    /// Strips tool call details and truncates content to reduce token usage.
    pub(super) fn sanitize_for_summarization(messages: &[ApiMessage]) -> String {
        let mut parts = Vec::new();
        for msg in messages {
            let role = msg
                .get("role")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown");
            let content = msg.get("content").and_then(|v| v.as_str()).unwrap_or("");
            if !content.is_empty() {
                let snippet: String = content.chars().take(500).collect();
                parts.push(format!("[{role}] {snippet}"));
            }
        }
        parts.join("\n")
    }

    /// Build the LLM API payload for compaction summarization.
    ///
    /// Returns `None` if there aren't enough messages to compact.
    /// The caller is responsible for sending this payload via `AdaptedClient`
    /// and passing the response to `apply_llm_compaction()`.
    ///
    /// # Returns
    /// `Some((payload, middle_count, keep_recent))` — the API payload and split metadata,
    /// or `None` if messages are too few to compact.
    pub fn build_compaction_payload(
        &self,
        messages: &[ApiMessage],
        system_prompt: &str,
        model: &str,
    ) -> Option<(serde_json::Value, usize, usize)> {
        if messages.len() <= 4 {
            return None;
        }

        let keep_recent = (messages.len() / 3).clamp(2, 5);
        let split_point = messages.len() - keep_recent;
        let middle = &messages[1..split_point];

        if middle.is_empty() {
            return None;
        }

        let conversation_text = Self::sanitize_for_summarization(middle);

        let payload = serde_json::json!({
            "model": model,
            "messages": [
                {"role": "system", "content": system_prompt},
                {"role": "user", "content": conversation_text},
            ],
            "max_tokens": 1024,
            "temperature": 0.2,
        });

        Some((payload, middle.len(), keep_recent))
    }

    /// Apply LLM compaction using a summary string (from LLM response or fallback).
    ///
    /// Splits messages into head/middle/tail, replaces middle with the summary,
    /// and appends the artifact index.
    pub fn apply_llm_compaction(
        &mut self,
        messages: Vec<ApiMessage>,
        summary_text: &str,
        keep_recent: usize,
    ) -> Vec<ApiMessage> {
        let split_point = messages.len().saturating_sub(keep_recent);

        let head = &messages[..1];
        let middle_len = split_point.saturating_sub(1);
        let tail = &messages[split_point..];

        let artifact_summary = self.artifact_index.as_summary();
        let mut full_summary = format!("[CONVERSATION SUMMARY]\n{summary_text}");
        if !artifact_summary.is_empty() {
            full_summary.push_str("\n\n");
            full_summary.push_str(&artifact_summary);
        }

        let mut summary_msg = ApiMessage::new();
        summary_msg.insert("role".into(), serde_json::Value::String("user".into()));
        summary_msg.insert("content".into(), serde_json::Value::String(full_summary));

        let mut compacted = Vec::with_capacity(head.len() + 1 + tail.len());
        compacted.extend_from_slice(head);
        compacted.push(summary_msg);
        compacted.extend_from_slice(tail);

        info!(
            "LLM-compacted {} messages -> {} (removed {}, kept {} recent)",
            messages.len(),
            compacted.len(),
            middle_len,
            keep_recent,
        );

        // Invalidate calibration
        self.api_prompt_tokens = 0;
        self.msg_count_at_calibration = 0;
        self.warned_70 = false;
        self.warned_80 = false;
        self.warned_90 = false;

        compacted
    }
}
