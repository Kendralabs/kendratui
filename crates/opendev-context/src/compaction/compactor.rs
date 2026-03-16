//! The main context compactor state machine.

use std::collections::{HashMap, HashSet};

use tracing::{debug, info, warn};

use super::artifacts::ArtifactIndex;
use super::levels::OptimizationLevel;
use super::preview::{msg_token_count, summarize_tool_output};
use super::tokens::count_tokens;
use super::{
    ApiMessage, PROTECTED_TOOL_TYPES, PRUNE_MIN_LENGTH, PRUNE_PROTECTED_TOKENS,
    SLIDING_WINDOW_RECENT, SLIDING_WINDOW_THRESHOLD, STAGE_AGGRESSIVE, STAGE_COMPACT, STAGE_MASK,
    STAGE_PRUNE, STAGE_WARNING, TOOL_OUTPUT_SUMMARIZE_THRESHOLD,
};

/// Auto-compacts conversation history when approaching context limits.
pub struct ContextCompactor {
    max_context: u64,
    last_token_count: u64,
    api_prompt_tokens: u64,
    msg_count_at_calibration: usize,
    warned_70: bool,
    warned_80: bool,
    warned_90: bool,
    session_id: Option<String>,
    pub artifact_index: ArtifactIndex,
}

impl ContextCompactor {
    pub fn new(max_context_tokens: u64) -> Self {
        info!(
            "ContextCompactor: max_context={} tokens",
            max_context_tokens
        );
        Self {
            max_context: max_context_tokens,
            last_token_count: 0,
            api_prompt_tokens: 0,
            msg_count_at_calibration: 0,
            warned_70: false,
            warned_80: false,
            warned_90: false,
            session_id: None,
            artifact_index: ArtifactIndex::new(),
        }
    }

    pub fn set_session_id(&mut self, session_id: String) {
        self.session_id = Some(session_id);
    }

    /// Save the artifact index into a session metadata map.
    ///
    /// Stores under the key `"artifact_index"` so it persists across
    /// session save/load cycles.
    pub fn save_artifact_index(
        &self,
        metadata: &mut std::collections::HashMap<String, serde_json::Value>,
    ) {
        if !self.artifact_index.is_empty() {
            metadata.insert("artifact_index".to_string(), self.artifact_index.to_json());
        }
    }

    /// Restore the artifact index from session metadata.
    ///
    /// Looks for the `"artifact_index"` key and deserializes it.
    pub fn load_artifact_index(
        &mut self,
        metadata: &std::collections::HashMap<String, serde_json::Value>,
    ) {
        if let Some(value) = metadata.get("artifact_index")
            && let Some(index) = ArtifactIndex::from_json(value)
        {
            info!(
                "Restored artifact index with {} entries from session",
                index.len()
            );
            self.artifact_index = index;
        }
    }

    /// Context usage as percentage (0-100+).
    pub fn usage_pct(&self) -> f64 {
        if self.max_context == 0 || self.last_token_count == 0 {
            return 0.0;
        }
        (self.last_token_count as f64 / self.max_context as f64) * 100.0
    }

    /// Percentage points remaining before full compaction triggers.
    pub fn pct_until_compact(&self) -> f64 {
        let threshold_pct = STAGE_COMPACT * 100.0;
        (threshold_pct - self.usage_pct()).max(0.0)
    }

    /// Check context usage and return the appropriate optimization level.
    pub fn check_usage(
        &mut self,
        messages: &[ApiMessage],
        system_prompt: &str,
    ) -> OptimizationLevel {
        self.update_token_count(messages, system_prompt);
        let pct = self.usage_pct() / 100.0;

        if pct >= STAGE_COMPACT {
            return OptimizationLevel::Compact;
        }
        if pct >= STAGE_AGGRESSIVE {
            if !self.warned_90 {
                warn!(
                    "Context at {:.1}% — aggressive optimization active",
                    pct * 100.0
                );
                self.warned_90 = true;
            }
            return OptimizationLevel::Aggressive;
        }
        if pct >= STAGE_PRUNE {
            return OptimizationLevel::Prune;
        }
        if pct >= STAGE_MASK {
            if !self.warned_80 {
                warn!(
                    "Context at {:.1}% — observation masking active",
                    pct * 100.0
                );
                self.warned_80 = true;
            }
            return OptimizationLevel::Mask;
        }
        if pct >= STAGE_WARNING {
            if !self.warned_70 {
                info!("Context at {:.1}% — approaching limits", pct * 100.0);
                self.warned_70 = true;
            }
            return OptimizationLevel::Warning;
        }
        OptimizationLevel::None
    }

    /// Check if conversation exceeds the compaction threshold.
    pub fn should_compact(&mut self, messages: &[ApiMessage], system_prompt: &str) -> bool {
        self.update_token_count(messages, system_prompt);
        self.last_token_count > (self.max_context as f64 * STAGE_COMPACT) as u64
    }

    /// Calibrate with real API token count.
    pub fn update_from_api_usage(&mut self, prompt_tokens: u64, message_count: usize) {
        if prompt_tokens > 0 {
            self.api_prompt_tokens = prompt_tokens;
            self.msg_count_at_calibration = message_count;
            self.last_token_count = prompt_tokens;
        } else {
            debug!(
                "update_from_api_usage: prompt_tokens=0, skipping calibration \
                 (max_context={}, last_token_count={})",
                self.max_context, self.last_token_count,
            );
        }
    }

    /// Build a mapping from tool_call_id to tool function name.
    pub fn build_tool_call_map(messages: &[ApiMessage]) -> HashMap<String, String> {
        let mut tc_map = HashMap::new();
        for msg in messages {
            if msg.get("role").and_then(|v| v.as_str()) != Some("assistant") {
                continue;
            }
            if let Some(tool_calls) = msg.get("tool_calls").and_then(|v| v.as_array()) {
                for tc in tool_calls {
                    let tc_id = tc.get("id").and_then(|v| v.as_str()).unwrap_or("");
                    let func_name = tc
                        .get("function")
                        .and_then(|f| f.get("name"))
                        .and_then(|n| n.as_str())
                        .unwrap_or("");
                    if !tc_id.is_empty() && !func_name.is_empty() {
                        tc_map.insert(tc_id.to_string(), func_name.to_string());
                    }
                }
            }
        }
        tc_map
    }

    /// Replace old tool result messages with compact references.
    pub fn mask_old_observations(&self, messages: &mut [ApiMessage], level: OptimizationLevel) {
        let recent_threshold = match level {
            OptimizationLevel::Mask => 6,
            OptimizationLevel::Aggressive => 3,
            _ => return,
        };

        // Find all tool result message indices
        let tool_indices: Vec<usize> = messages
            .iter()
            .enumerate()
            .filter(|(_, msg)| msg.get("role").and_then(|v| v.as_str()) == Some("tool"))
            .map(|(i, _)| i)
            .collect();

        if tool_indices.len() <= recent_threshold {
            return;
        }

        let tc_map = Self::build_tool_call_map(messages);
        let old_count = tool_indices.len() - recent_threshold;
        let old_indices: HashSet<usize> = tool_indices[..old_count].iter().copied().collect();
        let mut masked_count = 0u32;

        for &i in &old_indices {
            let content = messages[i]
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            if content.starts_with("[ref:") {
                continue;
            }
            let tool_call_id = messages[i]
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("?")
                .to_string();
            let tool_name = tc_map.get(&tool_call_id).map(|s| s.as_str()).unwrap_or("");
            if PROTECTED_TOOL_TYPES.contains(&tool_name) {
                continue;
            }
            messages[i].insert(
                "content".into(),
                serde_json::Value::String(format!(
                    "[ref: tool result {tool_call_id} — see history]"
                )),
            );
            masked_count += 1;
        }

        if masked_count > 0 {
            info!(
                "Masked {} old tool results (level={}, kept recent {})",
                masked_count,
                level.as_str(),
                recent_threshold,
            );
        }
    }

    /// Strip old tool outputs while protecting the most recent ones.
    pub fn prune_old_tool_outputs(&self, messages: &mut [ApiMessage]) {
        // Collect tool result indices in reverse order
        let mut tool_indices: Vec<usize> = Vec::new();
        for i in (0..messages.len()).rev() {
            if messages[i].get("role").and_then(|v| v.as_str()) == Some("tool") {
                tool_indices.push(i);
            }
        }

        if tool_indices.is_empty() {
            return;
        }

        let tc_map = Self::build_tool_call_map(messages);
        let mut protected_tokens: u64 = 0;
        let mut protected_indices: HashSet<usize> = HashSet::new();

        for &idx in &tool_indices {
            let content = messages[idx]
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if content.starts_with("[ref:")
                || content == "[pruned]"
                || content.starts_with("[summary:")
            {
                continue;
            }
            let tool_call_id = messages[idx]
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            let tool_name = tc_map.get(tool_call_id).map(|s| s.as_str()).unwrap_or("");
            if PROTECTED_TOOL_TYPES.contains(&tool_name) {
                protected_indices.insert(idx);
                continue;
            }
            // Small outputs aren't worth pruning — keep them
            if content.len() < PRUNE_MIN_LENGTH {
                protected_indices.insert(idx);
                continue;
            }
            let token_estimate = count_tokens(content) as u64;
            if protected_tokens + token_estimate <= PRUNE_PROTECTED_TOKENS {
                protected_tokens += token_estimate;
                protected_indices.insert(idx);
            }
        }

        let mut pruned_count = 0u32;
        for &idx in &tool_indices {
            if protected_indices.contains(&idx) {
                continue;
            }
            let content = messages[idx]
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("");
            if content.starts_with("[ref:")
                || content == "[pruned]"
                || content.starts_with("[summary:")
            {
                continue;
            }
            // Small outputs survive even without budget
            if content.len() < PRUNE_MIN_LENGTH {
                continue;
            }
            messages[idx].insert(
                "content".into(),
                serde_json::Value::String("[pruned]".into()),
            );
            pruned_count += 1;
        }

        if pruned_count > 0 {
            info!(
                "Pruned {} old tool outputs (protected {}, ~{}K tokens kept)",
                pruned_count,
                protected_indices.len(),
                protected_tokens / 1000,
            );
        }
    }

    /// Apply sliding window compaction for sessions with 500+ messages.
    ///
    /// Keeps the first message (system prompt) and the most recent
    /// `SLIDING_WINDOW_RECENT` messages, replacing everything in between
    /// with a compressed summary. This runs *before* the staged approach.
    pub fn sliding_window_compact(&mut self, messages: Vec<ApiMessage>) -> Vec<ApiMessage> {
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

    /// Summarize verbose tool outputs (>500 chars) with 2-3 line summaries.
    ///
    /// Replaces long tool outputs with a compact summary preserving the tool
    /// name, success/failure status, and first/last lines. Protected tool
    /// types and already-processed outputs are skipped.
    pub fn summarize_verbose_tool_outputs(&self, messages: &mut [ApiMessage]) {
        let tc_map = Self::build_tool_call_map(messages);
        let mut summarized_count = 0u32;

        for msg in messages.iter_mut() {
            if msg.get("role").and_then(|v| v.as_str()) != Some("tool") {
                continue;
            }
            let content = msg
                .get("content")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();

            if content.len() <= TOOL_OUTPUT_SUMMARIZE_THRESHOLD {
                continue;
            }
            if content.starts_with("[ref:")
                || content == "[pruned]"
                || content.starts_with("[summary:")
            {
                continue;
            }

            let tool_call_id = msg
                .get("tool_call_id")
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            let tool_name = tc_map
                .get(&tool_call_id)
                .map(|s| s.as_str())
                .unwrap_or("tool");

            if PROTECTED_TOOL_TYPES.contains(&tool_name) {
                continue;
            }

            let summary = summarize_tool_output(tool_name, &content);
            msg.insert("content".into(), serde_json::Value::String(summary));
            summarized_count += 1;
        }

        if summarized_count > 0 {
            info!(
                "Summarized {} verbose tool outputs (>{} chars)",
                summarized_count, TOOL_OUTPUT_SUMMARIZE_THRESHOLD,
            );
        }
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
    fn sanitize_for_summarization(messages: &[ApiMessage]) -> String {
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

    /// Estimate total tokens across messages and system prompt.
    ///
    /// Uses the improved `count_tokens` heuristic (cl100k_base approximation)
    /// instead of the naive `chars / 4`.
    pub(super) fn count_message_tokens(messages: &[ApiMessage], system_prompt: &str) -> u64 {
        let mut total = count_tokens(system_prompt) as u64;
        for msg in messages {
            total += msg_token_count(msg) as u64;
        }
        total
    }

    fn update_token_count(&mut self, messages: &[ApiMessage], system_prompt: &str) {
        if self.api_prompt_tokens > 0 {
            let new_msg_count = messages.len().saturating_sub(self.msg_count_at_calibration);
            if new_msg_count > 0 {
                let start = messages.len() - new_msg_count;
                let delta = Self::count_message_tokens(&messages[start..], "");
                self.last_token_count = self.api_prompt_tokens + delta;
            } else {
                self.last_token_count = self.api_prompt_tokens;
            }
        } else {
            self.last_token_count = Self::count_message_tokens(messages, system_prompt);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::tests::{make_assistant_with_tc, make_msg, make_tool_msg};
    use super::*;

    #[test]
    fn test_optimization_levels() {
        let mut compactor = ContextCompactor::new(1000);

        // At 0% usage
        let messages = vec![make_msg("user", "hi")];
        assert_eq!(
            compactor.check_usage(&messages, ""),
            OptimizationLevel::None
        );

        // Force usage to 75% via API calibration
        compactor.update_from_api_usage(750, 1);
        assert_eq!(
            compactor.check_usage(&messages, ""),
            OptimizationLevel::Warning
        );

        // 85%
        compactor.update_from_api_usage(850, 1);
        assert_eq!(
            compactor.check_usage(&messages, ""),
            OptimizationLevel::Prune
        );

        // 95%
        compactor.update_from_api_usage(950, 1);
        assert_eq!(
            compactor.check_usage(&messages, ""),
            OptimizationLevel::Aggressive
        );

        // 99.5%
        compactor.update_from_api_usage(995, 1);
        assert_eq!(
            compactor.check_usage(&messages, ""),
            OptimizationLevel::Compact
        );
    }

    #[test]
    fn test_should_compact() {
        let mut compactor = ContextCompactor::new(1000);
        let messages = vec![make_msg("user", "hi")];
        assert!(!compactor.should_compact(&messages, ""));

        compactor.update_from_api_usage(995, 1);
        assert!(compactor.should_compact(&messages, ""));
    }

    #[test]
    fn test_mask_old_observations() {
        let compactor = ContextCompactor::new(100_000);

        // Create messages: assistant with tool calls, then 8 tool results
        let mut messages = vec![make_msg("system", "system prompt")];
        let tc_ids: Vec<String> = (0..8).map(|i| format!("tc-{i}")).collect();
        let tc_pairs: Vec<(&str, &str)> = tc_ids.iter().map(|id| (id.as_str(), "bash")).collect();
        messages.push(make_assistant_with_tc(tc_pairs));
        for id in &tc_ids {
            messages.push(make_tool_msg(id, &"x".repeat(100)));
        }

        // Mask level: keep recent 6, mask 2
        compactor.mask_old_observations(&mut messages, OptimizationLevel::Mask);

        let masked: Vec<_> = messages
            .iter()
            .filter(|m| {
                m.get("content")
                    .and_then(|v| v.as_str())
                    .map(|s| s.starts_with("[ref:"))
                    .unwrap_or(false)
            })
            .collect();
        assert_eq!(masked.len(), 2);
    }

    #[test]
    fn test_protected_tools_not_masked() {
        let compactor = ContextCompactor::new(100_000);

        let mut messages = vec![make_msg("system", "sys")];
        let tc_ids: Vec<String> = (0..10).map(|i| format!("tc-{i}")).collect();
        let mut names = vec!["read_file"];
        for _ in 1..10 {
            names.push("bash");
        }
        let pairs: Vec<(&str, &str)> = tc_ids
            .iter()
            .zip(names.iter())
            .map(|(id, name)| (id.as_str(), *name))
            .collect();
        messages.push(make_assistant_with_tc(pairs));
        for id in &tc_ids {
            messages.push(make_tool_msg(id, &"x".repeat(100)));
        }

        compactor.mask_old_observations(&mut messages, OptimizationLevel::Aggressive);

        // tc-0 is read_file and should NOT be masked
        let tc0_msg = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-0"))
            .unwrap();
        let content = tc0_msg.get("content").and_then(|v| v.as_str()).unwrap();
        assert!(!content.starts_with("[ref:"));
    }

    #[test]
    fn test_compact_small_conversation() {
        let mut compactor = ContextCompactor::new(100_000);
        let messages = vec![
            make_msg("system", "sys"),
            make_msg("user", "hello"),
            make_msg("assistant", "hi"),
        ];
        // Should not compact if <= 4 messages
        let result = compactor.compact(messages.clone(), "sys");
        assert_eq!(result.len(), 3);
    }

    #[test]
    fn test_compact_large_conversation() {
        let mut compactor = ContextCompactor::new(100_000);
        let mut messages = vec![make_msg("system", "sys")];
        for i in 0..20 {
            messages.push(make_msg("user", &format!("question {i}")));
            messages.push(make_msg("assistant", &format!("answer {i}")));
        }
        let original_len = messages.len();
        let result = compactor.compact(messages, "sys");
        assert!(result.len() < original_len);
        // First message preserved
        assert_eq!(
            result[0].get("role").and_then(|v| v.as_str()),
            Some("system")
        );
        // Summary message present
        let has_summary = result.iter().any(|m| {
            m.get("content")
                .and_then(|v| v.as_str())
                .map(|s| s.contains("[CONVERSATION SUMMARY]"))
                .unwrap_or(false)
        });
        assert!(has_summary);
    }

    #[test]
    fn test_compactor_save_load_artifact_index() {
        let mut compactor = ContextCompactor::new(100_000);
        compactor
            .artifact_index
            .record("src/app.rs", "created", "new file");
        compactor
            .artifact_index
            .record("src/app.rs", "modified", "added fn");

        // Save to metadata
        let mut metadata = std::collections::HashMap::new();
        compactor.save_artifact_index(&mut metadata);
        assert!(metadata.contains_key("artifact_index"));

        // Load into a fresh compactor
        let mut compactor2 = ContextCompactor::new(100_000);
        assert!(compactor2.artifact_index.is_empty());
        compactor2.load_artifact_index(&metadata);
        assert_eq!(compactor2.artifact_index.len(), 1);
        let entry = compactor2.artifact_index.entries.get("src/app.rs").unwrap();
        assert_eq!(entry.operation_count, 2);
    }

    #[test]
    fn test_prune_old_tool_outputs() {
        let compactor = ContextCompactor::new(100_000);

        let mut messages = vec![make_msg("system", "sys")];
        // Many tool calls with large outputs
        let tc_ids: Vec<String> = (0..20).map(|i| format!("tc-{i}")).collect();
        let pairs: Vec<(&str, &str)> = tc_ids.iter().map(|id| (id.as_str(), "bash")).collect();
        messages.push(make_assistant_with_tc(pairs));
        for id in &tc_ids {
            // Each tool output is large enough to exceed budget
            messages.push(make_tool_msg(id, &"x".repeat(20_000)));
        }

        compactor.prune_old_tool_outputs(&mut messages);

        let pruned_count = messages
            .iter()
            .filter(|m| m.get("content").and_then(|v| v.as_str()) == Some("[pruned]"))
            .count();
        assert!(pruned_count > 0, "Some messages should have been pruned");
    }

    #[test]
    fn test_fallback_summary() {
        let messages = vec![
            make_msg("user", "What is Rust?"),
            make_msg("assistant", "Rust is a systems programming language."),
            make_msg("user", "Tell me more."),
        ];
        let summary = ContextCompactor::fallback_summary(&messages);
        assert!(summary.contains("[user] What is Rust?"));
        assert!(summary.contains("[assistant] Rust is a systems programming language."));
    }

    #[test]
    fn test_sliding_window_below_threshold() {
        let mut compactor = ContextCompactor::new(1_000_000);
        let mut messages = vec![make_msg("system", "sys")];
        for i in 0..100 {
            messages.push(make_msg("user", &format!("q{i}")));
            messages.push(make_msg("assistant", &format!("a{i}")));
        }
        // 201 messages, below SLIDING_WINDOW_THRESHOLD (500)
        let result = compactor.sliding_window_compact(messages.clone());
        assert_eq!(result.len(), messages.len());
    }

    #[test]
    fn test_sliding_window_above_threshold() {
        let mut compactor = ContextCompactor::new(1_000_000);
        let mut messages = vec![make_msg("system", "sys")];
        for i in 0..300 {
            messages.push(make_msg("user", &format!("q{i}")));
            messages.push(make_msg("assistant", &format!("a{i}")));
        }
        // 601 messages, above threshold
        assert!(messages.len() >= SLIDING_WINDOW_THRESHOLD);

        let result = compactor.sliding_window_compact(messages.clone());
        // Should keep: 1 (system) + 1 (summary) + SLIDING_WINDOW_RECENT
        assert_eq!(result.len(), 1 + 1 + SLIDING_WINDOW_RECENT);

        // First message is system
        assert_eq!(
            result[0].get("role").and_then(|v| v.as_str()),
            Some("system")
        );
        // Second message is the sliding window summary
        let summary_content = result[1].get("content").and_then(|v| v.as_str()).unwrap();
        assert!(summary_content.contains("[SLIDING WINDOW SUMMARY"));
    }

    #[test]
    fn test_summarize_verbose_tool_outputs() {
        let compactor = ContextCompactor::new(100_000);

        let mut messages = vec![make_msg("system", "sys")];
        let tc_ids: Vec<String> = (0..5).map(|i| format!("tc-{i}")).collect();
        let pairs: Vec<(&str, &str)> = tc_ids.iter().map(|id| (id.as_str(), "bash")).collect();
        messages.push(make_assistant_with_tc(pairs));

        // Mix of short and long outputs
        messages.push(make_tool_msg("tc-0", "ok")); // short, skip
        messages.push(make_tool_msg("tc-1", &"long output line\n".repeat(50))); // > 500
        messages.push(make_tool_msg("tc-2", &"x".repeat(600))); // > 500
        messages.push(make_tool_msg("tc-3", "[pruned]")); // already pruned
        messages.push(make_tool_msg("tc-4", &"data ".repeat(200))); // > 500

        compactor.summarize_verbose_tool_outputs(&mut messages);

        // tc-0 should be unchanged (short)
        let tc0 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-0"))
            .unwrap();
        assert_eq!(tc0.get("content").and_then(|v| v.as_str()).unwrap(), "ok");

        // tc-1 should be summarized
        let tc1 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-1"))
            .unwrap();
        assert!(
            tc1.get("content")
                .and_then(|v| v.as_str())
                .unwrap()
                .starts_with("[summary:")
        );

        // tc-3 should remain [pruned]
        let tc3 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-3"))
            .unwrap();
        assert_eq!(
            tc3.get("content").and_then(|v| v.as_str()).unwrap(),
            "[pruned]"
        );
    }

    #[test]
    fn test_summarize_skips_protected_tools() {
        let compactor = ContextCompactor::new(100_000);

        let mut messages = vec![make_msg("system", "sys")];
        let pairs = vec![("tc-0", "read_file"), ("tc-1", "bash")];
        messages.push(make_assistant_with_tc(pairs));
        messages.push(make_tool_msg("tc-0", &"file content ".repeat(100))); // protected
        messages.push(make_tool_msg("tc-1", &"bash output ".repeat(100))); // not protected

        compactor.summarize_verbose_tool_outputs(&mut messages);

        // read_file output should NOT be summarized
        let tc0 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-0"))
            .unwrap();
        assert!(
            !tc0.get("content")
                .and_then(|v| v.as_str())
                .unwrap()
                .starts_with("[summary:")
        );

        // bash output SHOULD be summarized
        let tc1 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-1"))
            .unwrap();
        assert!(
            tc1.get("content")
                .and_then(|v| v.as_str())
                .unwrap()
                .starts_with("[summary:")
        );
    }

    #[test]
    fn test_count_message_tokens_integration() {
        let messages = vec![
            make_msg("system", "You are a helpful assistant."),
            make_msg("user", "Hello world"),
            make_msg("assistant", "Hi there! How can I help?"),
        ];
        let total = ContextCompactor::count_message_tokens(&messages, "system prompt");
        assert!(total > 0);
    }

    #[test]
    fn test_prune_skips_summarized_outputs() {
        let compactor = ContextCompactor::new(100_000);

        let mut messages = vec![make_msg("system", "sys")];
        let tc_ids: Vec<String> = (0..5).map(|i| format!("tc-{i}")).collect();
        let pairs: Vec<(&str, &str)> = tc_ids.iter().map(|id| (id.as_str(), "bash")).collect();
        messages.push(make_assistant_with_tc(pairs));

        // Some already summarized, some not
        messages.push(make_tool_msg(
            "tc-0",
            "[summary: bash succeeded, 10 lines]\nfirst line",
        ));
        messages.push(make_tool_msg("tc-1", &"x".repeat(20_000)));
        messages.push(make_tool_msg("tc-2", &"y".repeat(20_000)));
        messages.push(make_tool_msg(
            "tc-3",
            "[summary: bash failed, 5 lines]\nerror",
        ));
        messages.push(make_tool_msg("tc-4", &"z".repeat(20_000)));

        compactor.prune_old_tool_outputs(&mut messages);

        // Summarized messages should NOT be changed to [pruned]
        let tc0 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-0"))
            .unwrap();
        assert!(
            tc0.get("content")
                .and_then(|v| v.as_str())
                .unwrap()
                .starts_with("[summary:")
        );
    }

    #[test]
    fn test_sanitize_for_summarization() {
        let messages = vec![
            make_msg("user", "Fix the login bug"),
            make_msg("assistant", "I'll look into that"),
            make_msg("tool", ""), // empty content, should be skipped
        ];
        let sanitized = ContextCompactor::sanitize_for_summarization(&messages);
        assert!(sanitized.contains("[user]"));
        assert!(sanitized.contains("[assistant]"));
        assert!(!sanitized.contains("[tool]"));
    }

    #[test]
    fn test_sanitize_truncates_long_content() {
        let long_content = "x".repeat(1000);
        let messages = vec![make_msg("user", &long_content)];
        let sanitized = ContextCompactor::sanitize_for_summarization(&messages);
        // [user] prefix + space + 500 chars of content
        assert!(sanitized.len() < 520);
    }

    #[test]
    fn test_build_compaction_payload() {
        let compactor = ContextCompactor::new(100_000);
        let messages = vec![
            make_msg("system", "You are helpful."),
            make_msg("user", "Step 1"),
            make_msg("assistant", "Done step 1"),
            make_msg("user", "Step 2"),
            make_msg("assistant", "Done step 2"),
            make_msg("user", "Step 3"),
            make_msg("assistant", "Done step 3"),
        ];

        let result = compactor.build_compaction_payload(&messages, "Summarize.", "gpt-4o-mini");
        assert!(result.is_some());

        let (payload, middle_count, keep_recent) = result.unwrap();
        assert!(middle_count > 0);
        assert!(keep_recent >= 2);
        assert_eq!(
            payload.pointer("/messages/0/role").and_then(|v| v.as_str()),
            Some("system")
        );
        assert_eq!(
            payload.get("model").and_then(|v| v.as_str()),
            Some("gpt-4o-mini")
        );
    }

    #[test]
    fn test_build_compaction_payload_too_few() {
        let compactor = ContextCompactor::new(100_000);
        let messages = vec![make_msg("system", "sys"), make_msg("user", "hi")];
        assert!(
            compactor
                .build_compaction_payload(&messages, "sys", "model")
                .is_none()
        );
    }

    #[test]
    fn test_apply_llm_compaction() {
        let mut compactor = ContextCompactor::new(100_000);
        let messages = vec![
            make_msg("system", "You are helpful."),
            make_msg("user", "Step 1"),
            make_msg("assistant", "Done step 1"),
            make_msg("user", "Step 2"),
            make_msg("assistant", "Done step 2"),
            make_msg("user", "Step 3"),
            make_msg("assistant", "Done step 3"),
        ];

        let keep_recent = 2;
        let result = compactor.apply_llm_compaction(
            messages,
            "This is the LLM summary of the conversation.",
            keep_recent,
        );

        // head(1) + summary(1) + tail(keep_recent)
        assert_eq!(result.len(), 1 + 1 + keep_recent);
        assert_eq!(
            result[0].get("role").and_then(|v| v.as_str()),
            Some("system")
        );
        let summary = result[1].get("content").and_then(|v| v.as_str()).unwrap();
        assert!(summary.contains("[CONVERSATION SUMMARY]"));
        assert!(summary.contains("LLM summary"));
    }

    #[test]
    fn test_apply_llm_compaction_resets_calibration() {
        let mut compactor = ContextCompactor::new(100_000);
        compactor.api_prompt_tokens = 50_000;
        compactor.warned_70 = true;
        compactor.warned_80 = true;

        let messages = vec![
            make_msg("system", "sys"),
            make_msg("user", "a"),
            make_msg("assistant", "b"),
            make_msg("user", "c"),
            make_msg("assistant", "d"),
            make_msg("user", "e"),
        ];

        compactor.apply_llm_compaction(messages, "summary", 2);

        assert_eq!(compactor.api_prompt_tokens, 0);
        assert!(!compactor.warned_70);
        assert!(!compactor.warned_80);
    }

    #[test]
    fn test_prune_skips_small_outputs() {
        let compactor = ContextCompactor::new(100_000);

        let mut messages = vec![make_msg("system", "sys")];
        let tc_ids: Vec<String> = (0..5).map(|i| format!("tc-{i}")).collect();
        let pairs: Vec<(&str, &str)> = tc_ids.iter().map(|id| (id.as_str(), "bash")).collect();
        messages.push(make_assistant_with_tc(pairs));

        // Small output (< PRUNE_MIN_LENGTH)
        messages.push(make_tool_msg("tc-0", "ok"));
        messages.push(make_tool_msg("tc-1", "short result"));
        // Large outputs that should be prunable
        messages.push(make_tool_msg("tc-2", &"x".repeat(20_000)));
        messages.push(make_tool_msg("tc-3", &"y".repeat(20_000)));
        messages.push(make_tool_msg("tc-4", &"z".repeat(20_000)));

        compactor.prune_old_tool_outputs(&mut messages);

        // Small outputs should NOT be pruned
        let tc0 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-0"))
            .unwrap();
        assert_eq!(tc0.get("content").and_then(|v| v.as_str()).unwrap(), "ok");

        let tc1 = messages
            .iter()
            .find(|m| m.get("tool_call_id").and_then(|v| v.as_str()) == Some("tc-1"))
            .unwrap();
        assert_eq!(
            tc1.get("content").and_then(|v| v.as_str()).unwrap(),
            "short result"
        );
    }

    #[test]
    fn test_protected_tool_types_includes_web_screenshot() {
        assert!(PROTECTED_TOOL_TYPES.contains(&"web_screenshot"));
        assert!(PROTECTED_TOOL_TYPES.contains(&"vlm"));
    }
}
