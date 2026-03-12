//! Query enhancement and message preparation for the REPL.
//!
//! Mirrors `opendev/repl/query_enhancer.py`.
//!
//! Responsibilities:
//! - Strip `@` references from queries while injecting file contents
//! - Prepare the full message list for LLM API calls (system prompt,
//!   session history, multimodal content, playbook context)

use regex::Regex;
use serde_json::Value;
use std::path::PathBuf;
use tracing::warn;

use crate::file_injector::{FileContentInjector, ImageBlock};

// ---------------------------------------------------------------------------
// Constants
// ---------------------------------------------------------------------------

/// Default thinking-on instruction text (injected when thinking is visible).
const THINKING_ON_INSTRUCTION: &str =
    "Use your thinking/reasoning capabilities to work through complex problems step by step. \
     Show your reasoning process.";

/// Default thinking-off instruction text (injected when thinking is hidden).
const THINKING_OFF_INSTRUCTION: &str =
    "Proceed directly with your response without showing internal reasoning.";

// ---------------------------------------------------------------------------
// QueryEnhancer
// ---------------------------------------------------------------------------

/// Handles query enhancement (@ file injection) and message preparation.
pub struct QueryEnhancer {
    /// Working directory for resolving relative `@` paths.
    working_dir: PathBuf,
}

impl QueryEnhancer {
    /// Create a new enhancer rooted at `working_dir`.
    pub fn new(working_dir: PathBuf) -> Self {
        Self { working_dir }
    }

    /// Enhance a query by injecting `@`-referenced file contents.
    ///
    /// Returns `(enhanced_query, image_blocks)` where:
    /// - `enhanced_query` has `@` markers stripped and file content appended
    /// - `image_blocks` contains base64-encoded images for multimodal calls
    pub fn enhance_query(&self, query: &str) -> (String, Vec<ImageBlock>) {
        let injector = FileContentInjector::new(self.working_dir.clone());
        let result = injector.inject_content(query);

        // Strip @ references from the query text.
        // Pattern 1: Quoted paths @"path with spaces"
        let quoted_re = Regex::new(r#"@"([^"]+)""#).expect("valid regex");
        let enhanced = quoted_re.replace_all(query, "$1").to_string();

        // Pattern 2: Unquoted paths (but not emails like user@example.com)
        let unquoted_re =
            Regex::new(r"(?:^|\s)@([a-zA-Z0-9_./\-]+)").expect("valid regex");
        let enhanced = unquoted_re
            .replace_all(&enhanced, |caps: &regex::Captures| {
                // Preserve the leading whitespace (or start-of-string) that was matched
                let full = caps.get(0).unwrap().as_str();
                let path = &caps[1];
                if full.starts_with(char::is_whitespace) {
                    format!("{}{}", &full[..full.len() - path.len() - 1], path)
                } else {
                    path.to_string()
                }
            })
            .to_string();

        // Append injected text content if any
        let enhanced = if result.text_content.is_empty() {
            enhanced
        } else {
            format!("{}\n\n{}", enhanced, result.text_content)
        };

        (enhanced, result.image_blocks)
    }

    /// Prepare the full message list for an LLM API call.
    ///
    /// # Arguments
    ///
    /// * `query` - Original user query (before enhancement)
    /// * `enhanced_query` - Query after `@` processing
    /// * `system_prompt` - Base system prompt text
    /// * `session_messages` - Existing conversation messages (if any)
    /// * `image_blocks` - Multimodal image blocks from enhancement
    /// * `thinking_visible` - Whether thinking mode is visible to the user
    /// * `playbook_context` - Optional learned-strategies text to append
    ///
    /// # Returns
    ///
    /// A `Vec<Value>` of message objects ready for the LLM API.
    #[allow(clippy::too_many_arguments)]
    pub fn prepare_messages(
        &self,
        query: &str,
        enhanced_query: &str,
        system_prompt: &str,
        session_messages: Option<&[Value]>,
        image_blocks: &[ImageBlock],
        thinking_visible: bool,
        playbook_context: Option<&str>,
    ) -> Vec<Value> {
        // Start with session messages or empty vec
        let mut messages: Vec<Value> = match session_messages {
            Some(msgs) => msgs.to_vec(),
            None => Vec::new(),
        };

        // If the query was enhanced, replace the last user message content
        if enhanced_query != query {
            for msg in messages.iter_mut().rev() {
                if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                    msg["content"] = Value::String(enhanced_query.to_string());
                    break;
                }
            }
        }

        // Build final system content
        let mut system_content = system_prompt.to_string();

        // Replace {thinking_instruction} placeholder
        if system_content.contains("{thinking_instruction}") {
            let thinking_text = if thinking_visible {
                THINKING_ON_INSTRUCTION
            } else {
                THINKING_OFF_INSTRUCTION
            };
            system_content = system_content.replace("{thinking_instruction}", thinking_text);
        }

        // Append playbook context if present
        if let Some(playbook) = playbook_context {
            if !playbook.is_empty() {
                system_content = format!(
                    "{}\n\n## Learned Strategies\n{}",
                    system_content.trim_end(),
                    playbook
                );
            }
        }

        // Insert or update system message at position 0
        if messages.is_empty()
            || messages[0]
                .get("role")
                .and_then(|r| r.as_str())
                != Some("system")
        {
            messages.insert(
                0,
                serde_json::json!({
                    "role": "system",
                    "content": system_content,
                }),
            );
        } else {
            messages[0]["content"] = Value::String(system_content);
        }

        // Handle multimodal content (images)
        if !image_blocks.is_empty() {
            for msg in messages.iter_mut().rev() {
                if msg.get("role").and_then(|r| r.as_str()) == Some("user") {
                    let current_content = msg
                        .get("content")
                        .and_then(|c| c.as_str())
                        .unwrap_or("")
                        .to_string();

                    let mut multimodal: Vec<Value> = vec![serde_json::json!({
                        "type": "text",
                        "text": current_content,
                    })];

                    for block in image_blocks {
                        multimodal.push(serde_json::json!({
                            "type": "image",
                            "source": {
                                "type": "base64",
                                "media_type": block.media_type,
                                "data": block.data,
                            }
                        }));
                    }

                    msg["content"] = Value::Array(multimodal);
                    break;
                }
            }
        }

        // Estimate tokens and warn if large
        let total_chars: usize = messages
            .iter()
            .map(|m| {
                m.get("content")
                    .map(|c| match c {
                        Value::String(s) => s.len(),
                        other => other.to_string().len(),
                    })
                    .unwrap_or(0)
            })
            .sum();
        let estimated_tokens = total_chars / 4;
        if estimated_tokens > 100_000 {
            warn!(
                messages = messages.len(),
                estimated_tokens, "Large context detected"
            );
        }

        messages
    }

    /// Format a debug summary of a message list.
    pub fn format_messages_summary(messages: &[Value], max_preview: usize) -> String {
        if messages.is_empty() {
            return "0 messages".to_string();
        }

        let mut summary_parts = Vec::new();
        for msg in messages {
            let role = msg
                .get("role")
                .and_then(|r| r.as_str())
                .unwrap_or("unknown");
            let content = msg.get("content");

            let preview = match content {
                Some(Value::String(s)) => {
                    if s.len() > max_preview {
                        format!("{}...", &s[..max_preview])
                    } else {
                        s.clone()
                    }
                }
                Some(Value::Array(arr)) => {
                    format!("[{} blocks]", arr.len())
                }
                Some(other) => {
                    let s = other.to_string();
                    if s.len() > max_preview {
                        format!("{}...", &s[..max_preview])
                    } else {
                        s
                    }
                }
                None => String::new(),
            };

            summary_parts.push(format!("{}: {}", role, preview));
        }

        format!(
            "{} messages: {}",
            messages.len(),
            summary_parts.join(" | ")
        )
    }
}

// ---------------------------------------------------------------------------
// Tests
// ---------------------------------------------------------------------------

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    use std::fs;
    use tempfile::TempDir;

    fn tmp_enhancer() -> (TempDir, QueryEnhancer) {
        let dir = TempDir::new().unwrap();
        let enh = QueryEnhancer::new(dir.path().to_path_buf());
        (dir, enh)
    }

    // -- enhance_query ------------------------------------------------------

    #[test]
    fn test_enhance_query_no_refs() {
        let (_dir, enh) = tmp_enhancer();
        let (enhanced, images) = enh.enhance_query("just a plain query");
        assert_eq!(enhanced, "just a plain query");
        assert!(images.is_empty());
    }

    #[test]
    fn test_enhance_query_strips_at_unquoted() {
        let (_dir, enh) = tmp_enhancer();
        // File doesn't exist so no content appended, but @ should still be stripped
        let (enhanced, _) = enh.enhance_query("look at @main.py please");
        assert!(!enhanced.contains("@main.py"));
        assert!(enhanced.contains("main.py"));
    }

    #[test]
    fn test_enhance_query_strips_at_quoted() {
        let (_dir, enh) = tmp_enhancer();
        let (enhanced, _) = enh.enhance_query(r#"check @"my file.py" now"#);
        assert!(!enhanced.contains("@\""));
        assert!(enhanced.contains("my file.py"));
    }

    #[test]
    fn test_enhance_query_preserves_email() {
        let (_dir, enh) = tmp_enhancer();
        let (enhanced, _) = enh.enhance_query("send to user@example.com");
        assert!(enhanced.contains("user@example.com"));
    }

    #[test]
    fn test_enhance_query_injects_file_content() {
        let (dir, enh) = tmp_enhancer();
        let p = dir.path().join("hello.rs");
        fs::write(&p, "fn main() {}").unwrap();

        let (enhanced, images) = enh.enhance_query("explain @hello.rs");
        assert!(enhanced.contains("<file_content"));
        assert!(enhanced.contains("fn main() {}"));
        assert!(images.is_empty());
    }

    #[test]
    fn test_enhance_query_image_blocks() {
        let (dir, enh) = tmp_enhancer();
        let p = dir.path().join("pic.png");
        fs::write(&p, &[0x89, 0x50, 0x4E, 0x47]).unwrap();

        let (enhanced, images) = enh.enhance_query("analyze @pic.png");
        assert!(enhanced.contains("<image"));
        assert_eq!(images.len(), 1);
        assert_eq!(images[0].media_type, "image/png");
    }

    // -- prepare_messages ---------------------------------------------------

    #[test]
    fn test_prepare_messages_basic() {
        let (_dir, enh) = tmp_enhancer();
        let msgs = enh.prepare_messages(
            "hello",
            "hello",
            "You are helpful.",
            None,
            &[],
            false,
            None,
        );
        assert_eq!(msgs.len(), 1); // just system message
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[0]["content"], "You are helpful.");
    }

    #[test]
    fn test_prepare_messages_with_session() {
        let (_dir, enh) = tmp_enhancer();
        let session = vec![
            json!({"role": "user", "content": "hi"}),
            json!({"role": "assistant", "content": "hello"}),
        ];
        let msgs = enh.prepare_messages(
            "hi",
            "hi",
            "system prompt",
            Some(&session),
            &[],
            false,
            None,
        );
        // system + user + assistant
        assert_eq!(msgs.len(), 3);
        assert_eq!(msgs[0]["role"], "system");
        assert_eq!(msgs[1]["role"], "user");
        assert_eq!(msgs[2]["role"], "assistant");
    }

    #[test]
    fn test_prepare_messages_replaces_enhanced_content() {
        let (_dir, enh) = tmp_enhancer();
        let session = vec![
            json!({"role": "user", "content": "look at @foo.py"}),
        ];
        let msgs = enh.prepare_messages(
            "look at @foo.py",
            "look at foo.py\n\n<file_content>...</file_content>",
            "sys",
            Some(&session),
            &[],
            false,
            None,
        );
        // Last user message content should be the enhanced version
        let user_msg = &msgs[1];
        assert_eq!(user_msg["role"], "user");
        assert!(user_msg["content"].as_str().unwrap().contains("<file_content>"));
    }

    #[test]
    fn test_prepare_messages_thinking_placeholder() {
        let (_dir, enh) = tmp_enhancer();

        // thinking visible
        let msgs = enh.prepare_messages(
            "q",
            "q",
            "Do this: {thinking_instruction}",
            None,
            &[],
            true,
            None,
        );
        let content = msgs[0]["content"].as_str().unwrap();
        assert!(content.contains("reasoning"));
        assert!(!content.contains("{thinking_instruction}"));

        // thinking hidden
        let msgs = enh.prepare_messages(
            "q",
            "q",
            "Do this: {thinking_instruction}",
            None,
            &[],
            false,
            None,
        );
        let content = msgs[0]["content"].as_str().unwrap();
        assert!(content.contains("directly"));
        assert!(!content.contains("{thinking_instruction}"));
    }

    #[test]
    fn test_prepare_messages_playbook_context() {
        let (_dir, enh) = tmp_enhancer();
        let msgs = enh.prepare_messages(
            "q",
            "q",
            "base prompt",
            None,
            &[],
            false,
            Some("- Always run tests before committing"),
        );
        let content = msgs[0]["content"].as_str().unwrap();
        assert!(content.contains("## Learned Strategies"));
        assert!(content.contains("Always run tests before committing"));
    }

    #[test]
    fn test_prepare_messages_multimodal_images() {
        let (_dir, enh) = tmp_enhancer();
        let session = vec![
            json!({"role": "user", "content": "analyze this image"}),
        ];
        let images = vec![ImageBlock {
            media_type: "image/png".to_string(),
            data: "base64data".to_string(),
        }];
        let msgs = enh.prepare_messages(
            "analyze this image",
            "analyze this image",
            "sys",
            Some(&session),
            &images,
            false,
            None,
        );
        // Last user message should be multimodal (array of content blocks)
        let user_content = &msgs[1]["content"];
        assert!(user_content.is_array());
        let blocks = user_content.as_array().unwrap();
        assert_eq!(blocks.len(), 2); // text + image
        assert_eq!(blocks[0]["type"], "text");
        assert_eq!(blocks[1]["type"], "image");
    }

    // -- format_messages_summary --------------------------------------------

    #[test]
    fn test_format_messages_summary_empty() {
        let summary = QueryEnhancer::format_messages_summary(&[], 60);
        assert_eq!(summary, "0 messages");
    }

    #[test]
    fn test_format_messages_summary_basic() {
        let msgs = vec![
            json!({"role": "system", "content": "You are helpful."}),
            json!({"role": "user", "content": "Hello world"}),
        ];
        let summary = QueryEnhancer::format_messages_summary(&msgs, 60);
        assert!(summary.starts_with("2 messages:"));
        assert!(summary.contains("system:"));
        assert!(summary.contains("user:"));
    }

    #[test]
    fn test_format_messages_summary_truncates() {
        let msgs = vec![
            json!({"role": "user", "content": "a]".repeat(100)}),
        ];
        let summary = QueryEnhancer::format_messages_summary(&msgs, 10);
        assert!(summary.contains("..."));
    }
}
