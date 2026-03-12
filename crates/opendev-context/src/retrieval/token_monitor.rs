//! Token counting utilities for context summaries.
//!
//! Uses a simple heuristic (text length / 4) as an approximation
//! of token count, avoiding the need for a full tokenizer dependency.

/// Stateless token counter using a character-based heuristic.
#[derive(Debug, Clone, Default)]
pub struct ContextTokenMonitor;

impl ContextTokenMonitor {
    /// Create a new token monitor.
    pub fn new() -> Self {
        Self
    }

    /// Estimate the number of tokens in the given text.
    ///
    /// Uses a simple heuristic: `text.len() / 4`, which approximates
    /// the average token length for English text with code.
    pub fn count_tokens(&self, text: &str) -> usize {
        text.len() / 4
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_empty() {
        let monitor = ContextTokenMonitor::new();
        assert_eq!(monitor.count_tokens(""), 0);
    }

    #[test]
    fn test_count_tokens_short() {
        let monitor = ContextTokenMonitor::new();
        // 11 chars / 4 = 2
        assert_eq!(monitor.count_tokens("hello world"), 2);
    }

    #[test]
    fn test_count_tokens_longer() {
        let monitor = ContextTokenMonitor::new();
        let text = "a".repeat(100);
        assert_eq!(monitor.count_tokens(&text), 25);
    }

    #[test]
    fn test_default_trait() {
        let monitor = ContextTokenMonitor::default();
        assert_eq!(monitor.count_tokens("test"), 1);
    }
}
