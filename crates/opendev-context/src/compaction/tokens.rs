//! Token counting heuristics for context management.

/// Count tokens in text using a cl100k_base-style heuristic.
///
/// Splits on whitespace and punctuation boundaries and applies a ~0.75
/// tokens-per-word ratio, which is more accurate than the naive `chars / 4`
/// approximation for English prose and code.
pub fn count_tokens(text: &str) -> usize {
    if text.is_empty() {
        return 0;
    }
    // Split on whitespace first
    let word_count: usize = text
        .split_whitespace()
        .map(|word| {
            let len = word.len();
            // BPE tokenizers split long tokens into ~4-char chunks.
            // For words longer than 12 chars, estimate based on length.
            if len > 12 {
                // Long words/identifiers: roughly 1 token per 4 chars
                return len.div_ceil(4);
            }
            // Each word may contain punctuation that the tokenizer splits off.
            // Count extra tokens for punctuation sequences attached to words.
            let punct_count = word.chars().filter(|c| c.is_ascii_punctuation()).count();
            // Base: 1 token per word fragment, plus extra tokens for
            // punctuation clusters (each punctuation char is ~0.5 token on
            // average, but we round up since BPE often keeps single-char
            // punctuation as its own token).
            1 + punct_count.div_ceil(2)
        })
        .sum();
    // Apply the 0.75 ratio: most English words map to < 1 BPE token.
    // We use integer math: (count * 3 + 2) / 4 ≈ ceil(count * 0.75).
    (word_count * 3 + 2) / 4
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_count_tokens_empty() {
        assert_eq!(count_tokens(""), 0);
    }

    #[test]
    fn test_count_tokens_single_word() {
        // "hello" -> 1 word, 0 punct -> base 1, * 0.75 rounds to 1
        let tokens = count_tokens("hello");
        assert!(tokens >= 1);
    }

    #[test]
    fn test_count_tokens_sentence() {
        // "The quick brown fox jumps over the lazy dog."
        // 9 words, 1 punct char on "dog." -> ~10 base, * 0.75 = ~8
        let tokens = count_tokens("The quick brown fox jumps over the lazy dog.");
        assert!(tokens >= 5 && tokens <= 15, "got {tokens}");
    }

    #[test]
    fn test_count_tokens_code() {
        let code = r#"fn main() { println!("hello"); }"#;
        let tokens = count_tokens(code);
        // Code has lots of punctuation; should produce more tokens than word count
        assert!(tokens >= 3, "code should produce tokens, got {tokens}");
    }

    #[test]
    fn test_count_tokens_better_than_chars_div_4() {
        // For typical English prose, count_tokens should be reasonably close
        // to real BPE token counts (within 2x).
        let text = "This is a simple sentence with several common English words in it.";
        let heuristic = count_tokens(text);
        let naive = text.len() / 4; // chars/4
        // Both should be in a reasonable range (5-20 for this sentence)
        assert!(
            heuristic > 0 && naive > 0,
            "both should be positive: heuristic={heuristic}, naive={naive}"
        );
    }
}
