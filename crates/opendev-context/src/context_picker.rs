//! Dynamic context selection for LLM calls.
//!
//! Provides the data models and types for context picking. The actual
//! picking logic (file injection, playbook strategies, etc.) depends on
//! higher-level crates and is wired up at the application layer.
//!
//! All decisions are logged as `ContextReason` objects for full traceability.

use std::collections::HashMap;
use std::fmt;
use std::path::Path;

use serde::{Deserialize, Serialize};

/// Category of context piece for organization and filtering.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ContextCategory {
    SystemPrompt,
    FileReference,
    DirectoryListing,
    ConversationHistory,
    PlaybookStrategy,
    ImageContent,
    PdfContent,
    ToolResult,
    UserQuery,
}

impl ContextCategory {
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SystemPrompt => "system_prompt",
            Self::FileReference => "file_reference",
            Self::DirectoryListing => "directory_listing",
            Self::ConversationHistory => "conversation_history",
            Self::PlaybookStrategy => "playbook_strategy",
            Self::ImageContent => "image_content",
            Self::PdfContent => "pdf_content",
            Self::ToolResult => "tool_result",
            Self::UserQuery => "user_query",
        }
    }
}

impl fmt::Display for ContextCategory {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Documents why a context piece was included.
///
/// This is the key to traceability -- every piece of context should have
/// a clear reason for inclusion that can be logged and debugged.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextReason {
    pub source: String,
    pub reason: String,
    #[serde(default = "default_relevance")]
    pub relevance_score: f64,
    #[serde(default)]
    pub tokens_estimate: usize,
    #[serde(default)]
    pub metadata: HashMap<String, serde_json::Value>,
}

fn default_relevance() -> f64 {
    1.0
}

impl ContextReason {
    pub fn new(source: &str, reason: &str) -> Self {
        Self {
            source: source.to_string(),
            reason: reason.to_string(),
            relevance_score: 1.0,
            tokens_estimate: 0,
            metadata: HashMap::new(),
        }
    }

    pub fn with_tokens(mut self, tokens: usize) -> Self {
        self.tokens_estimate = tokens;
        self
    }

    pub fn with_score(mut self, score: f64) -> Self {
        self.relevance_score = score;
        self
    }
}

impl fmt::Display for ContextReason {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let score_str = if self.relevance_score < 1.0 {
            format!(" (score={:.2})", self.relevance_score)
        } else {
            String::new()
        };
        let tokens_str = if self.tokens_estimate > 0 {
            format!(" [{} tokens]", self.tokens_estimate)
        } else {
            String::new()
        };
        write!(f, "[{}]{}{}: {}", self.source, score_str, tokens_str, self.reason)
    }
}

/// A single piece of context to include in the LLM call.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ContextPiece {
    pub content: String,
    pub reason: ContextReason,
    pub category: ContextCategory,
    /// Ordering hint (lower = earlier in context).
    #[serde(default = "default_order")]
    pub order: i32,
}

fn default_order() -> i32 {
    100
}

impl ContextPiece {
    pub fn new(content: String, reason: ContextReason, category: ContextCategory) -> Self {
        Self {
            content,
            reason,
            category,
            order: 100,
        }
    }

    pub fn with_order(mut self, order: i32) -> Self {
        self.order = order;
        self
    }

    /// Estimated token count (from reason or calculated).
    pub fn tokens_estimate(&self) -> usize {
        if self.reason.tokens_estimate > 0 {
            return self.reason.tokens_estimate;
        }
        self.content.len() / 4
    }
}

impl fmt::Display for ContextPiece {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let preview: String = self.content.chars().take(50).collect();
        let preview = preview.replace('\n', "\\n");
        let ellipsis = if self.content.len() > 50 { "..." } else { "" };
        write!(f, "{}: {}{}", self.category, preview, ellipsis)
    }
}

/// Final assembled context ready for LLM call.
///
/// Contains everything needed for an LLM call plus traceability information.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct AssembledContext {
    pub system_prompt: String,
    pub messages: Vec<serde_json::Map<String, serde_json::Value>>,
    #[serde(default)]
    pub pieces: Vec<ContextPiece>,
    #[serde(default)]
    pub image_blocks: Vec<serde_json::Value>,
    #[serde(default)]
    pub total_tokens_estimate: usize,
}

impl AssembledContext {
    /// Return concise summary of context for display.
    pub fn summary(&self) -> String {
        let mut by_category: HashMap<ContextCategory, Vec<&ContextPiece>> = HashMap::new();
        for piece in &self.pieces {
            by_category.entry(piece.category).or_default().push(piece);
        }

        let mut parts = Vec::new();
        for (category, pieces) in &by_category {
            let total_tokens: usize = pieces.iter().map(|p| p.tokens_estimate()).sum();
            parts.push(format!("{}: ~{} tokens", category, total_tokens));
        }

        let mut summary = format!("Context: {} tokens", self.total_tokens_estimate);
        if !parts.is_empty() {
            summary = format!("{} ({})", summary, parts.join(", "));
        }
        summary
    }
}

/// Simple tracer for context selection decisions.
pub struct ContextTracer;

impl ContextTracer {
    pub fn new() -> Self {
        Self
    }

    pub fn trace(&self, context: &AssembledContext) {
        tracing::debug!("[ContextPicker] {}", context.summary());
    }

    /// Export trace to a JSON file for debugging.
    pub fn export_trace(&self, context: &AssembledContext, path: &Path) -> std::io::Result<()> {
        let trace_data = serde_json::json!({
            "timestamp": chrono::Utc::now().to_rfc3339(),
            "total_tokens_estimate": context.total_tokens_estimate,
            "message_count": context.messages.len(),
            "piece_count": context.pieces.len(),
            "image_count": context.image_blocks.len(),
            "pieces": context.pieces.iter().map(|p| {
                serde_json::json!({
                    "category": p.category.as_str(),
                    "source": p.reason.source,
                    "tokens_estimate": p.tokens_estimate(),
                })
            }).collect::<Vec<_>>(),
        });

        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = std::fs::File::create(path)?;
        serde_json::to_writer_pretty(file, &trace_data)?;
        tracing::debug!("Context trace exported to {}", path.display());
        Ok(())
    }
}

impl Default for ContextTracer {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_context_category_display() {
        assert_eq!(ContextCategory::SystemPrompt.as_str(), "system_prompt");
        assert_eq!(ContextCategory::FileReference.as_str(), "file_reference");
        assert_eq!(ContextCategory::UserQuery.as_str(), "user_query");
    }

    #[test]
    fn test_context_reason_display() {
        let reason = ContextReason::new("file_reference", "User referenced @main.rs")
            .with_tokens(500)
            .with_score(0.85);
        let display = format!("{}", reason);
        assert!(display.contains("file_reference"));
        assert!(display.contains("0.85"));
        assert!(display.contains("500 tokens"));
    }

    #[test]
    fn test_context_piece_tokens() {
        let reason = ContextReason::new("test", "test").with_tokens(100);
        let piece = ContextPiece::new(
            "hello world".to_string(),
            reason,
            ContextCategory::UserQuery,
        );
        assert_eq!(piece.tokens_estimate(), 100);

        // Without explicit tokens, estimate from content
        let reason2 = ContextReason::new("test", "test");
        let piece2 = ContextPiece::new(
            "x".repeat(400),
            reason2,
            ContextCategory::UserQuery,
        );
        assert_eq!(piece2.tokens_estimate(), 100);
    }

    #[test]
    fn test_assembled_context_summary() {
        let ctx = AssembledContext {
            system_prompt: "test".to_string(),
            messages: vec![],
            pieces: vec![
                ContextPiece::new(
                    "prompt content".to_string(),
                    ContextReason::new("system_prompt", "Agent system prompt").with_tokens(200),
                    ContextCategory::SystemPrompt,
                ),
                ContextPiece::new(
                    "file content".to_string(),
                    ContextReason::new("file_reference", "Referenced file").with_tokens(300),
                    ContextCategory::FileReference,
                ),
            ],
            image_blocks: vec![],
            total_tokens_estimate: 500,
        };
        let summary = ctx.summary();
        assert!(summary.contains("500 tokens"));
    }

    #[test]
    fn test_context_piece_ordering() {
        let p1 = ContextPiece::new(
            "system".to_string(),
            ContextReason::new("sys", "sys"),
            ContextCategory::SystemPrompt,
        )
        .with_order(0);

        let p2 = ContextPiece::new(
            "query".to_string(),
            ContextReason::new("query", "query"),
            ContextCategory::UserQuery,
        )
        .with_order(100);

        assert!(p1.order < p2.order);
    }

    #[test]
    fn test_context_reason_serialization() {
        let reason = ContextReason::new("test_source", "test reason")
            .with_tokens(42)
            .with_score(0.9);
        let json = serde_json::to_string(&reason).unwrap();
        let deserialized: ContextReason = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.source, "test_source");
        assert_eq!(deserialized.tokens_estimate, 42);
    }
}
