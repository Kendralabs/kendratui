//! Completion scoring and ranking strategies.
//!
//! Mirrors the Python `CompletionStrategy` — supports prefix matching, fuzzy
//! matching, and frecency-weighted ranking.

use std::collections::HashMap;
use std::time::Instant;

use super::CompletionItem;

// ── Frecency tracker ───────────────────────────────────────────────

/// Tracks access frequency and recency for a set of keys.
///
/// The score formula is `frequency * recency_weight` where `recency_weight`
/// decays over time.
#[derive(Debug)]
struct FrecencyEntry {
    /// Total number of accesses.
    count: u32,
    /// Timestamp of the last access.
    last_access: Instant,
}

/// Manages frecency data for completion items.
#[derive(Debug)]
pub struct FrecencyTracker {
    entries: HashMap<String, FrecencyEntry>,
}

impl FrecencyTracker {
    /// Create an empty tracker.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Record an access for `key`.
    pub fn record(&mut self, key: &str) {
        let entry = self
            .entries
            .entry(key.to_string())
            .or_insert(FrecencyEntry {
                count: 0,
                last_access: Instant::now(),
            });
        entry.count += 1;
        entry.last_access = Instant::now();
    }

    /// Compute a frecency score for `key`. Returns 0.0 if the key has never
    /// been accessed.
    pub fn score(&self, key: &str) -> f64 {
        match self.entries.get(key) {
            None => 0.0,
            Some(entry) => {
                let elapsed_secs = entry.last_access.elapsed().as_secs_f64();
                // Recency weight: 1.0 right after access, decaying with a
                // half-life of ~5 minutes.
                let recency = (-elapsed_secs / 300.0).exp();
                entry.count as f64 * recency
            }
        }
    }
}

impl Default for FrecencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

// ── Fuzzy matching ─────────────────────────────────────────────────

/// Simple fuzzy-match scoring.
///
/// Returns a score in `[0.0, 1.0]` where 1.0 is a perfect prefix match.
/// Returns 0.0 if the characters of `pattern` do not appear in order in
/// `text`.
pub fn fuzzy_score(pattern: &str, text: &str) -> f64 {
    if pattern.is_empty() {
        return 1.0;
    }
    let pattern_lower: Vec<char> = pattern.to_lowercase().chars().collect();
    let text_lower: Vec<char> = text.to_lowercase().chars().collect();

    let mut pi = 0; // index into pattern
    let mut consecutive = 0u32;
    let mut total_bonus = 0.0f64;
    let mut matched = false;

    for (ti, &tc) in text_lower.iter().enumerate() {
        if pi < pattern_lower.len() && tc == pattern_lower[pi] {
            // Bonus for matching at the start of the string or after a separator
            if ti == 0
                || matches!(
                    text_lower.get(ti.wrapping_sub(1)),
                    Some(&'/' | &'_' | &'-' | &'.')
                )
            {
                total_bonus += 0.15;
            }
            consecutive += 1;
            total_bonus += consecutive as f64 * 0.05;
            pi += 1;
        } else {
            consecutive = 0;
        }
    }

    if pi == pattern_lower.len() {
        matched = true;
    }

    if !matched {
        return 0.0;
    }

    // Base score: ratio of matched chars to text length
    let base = pattern_lower.len() as f64 / text_lower.len().max(1) as f64;
    (base + total_bonus).min(1.0)
}

// ── CompletionStrategy ─────────────────────────────────────────────

/// The matching mode used by [`CompletionStrategy`].
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MatchMode {
    /// Only prefix matches.
    Prefix,
    /// Fuzzy substring matching with scoring.
    Fuzzy,
}

/// Configurable strategy for scoring and sorting completion items.
pub struct CompletionStrategy {
    mode: MatchMode,
    frecency: FrecencyTracker,
    /// Weight applied to the frecency component (0.0 to disable).
    frecency_weight: f64,
}

impl CompletionStrategy {
    /// Create a strategy with the given mode.
    pub fn new(mode: MatchMode) -> Self {
        Self {
            mode,
            frecency: FrecencyTracker::new(),
            frecency_weight: 5.0,
        }
    }

    /// Record a frecency access.
    pub fn record_access(&mut self, key: &str) {
        self.frecency.record(key);
    }

    /// Sort `items` in-place, assigning scores and ordering by descending
    /// score.
    pub fn sort(&self, items: &mut [CompletionItem]) {
        for item in items.iter_mut() {
            let frecency = self.frecency.score(&item.insert_text) * self.frecency_weight;
            // For items already produced by a completer the base score is 0.
            // We can overlay a fuzzy score if in fuzzy mode.
            let match_score = match self.mode {
                MatchMode::Prefix => {
                    // Items are already prefix-filtered by the completer; give
                    // a small bonus for shorter labels (more relevant).
                    1.0 / (item.label.len() as f64 + 1.0)
                }
                MatchMode::Fuzzy => {
                    // Re-score using the label vs some implicit query. Since
                    // the completer already filtered, we just reward short
                    // labels.
                    1.0 / (item.label.len() as f64 + 1.0)
                }
            };
            item.score = match_score + frecency;
        }
        items.sort_by(|a, b| {
            b.score
                .partial_cmp(&a.score)
                .unwrap_or(std::cmp::Ordering::Equal)
        });
    }

    /// Return the current match mode.
    pub fn mode(&self) -> MatchMode {
        self.mode
    }

    /// Set the match mode.
    pub fn set_mode(&mut self, mode: MatchMode) {
        self.mode = mode;
    }
}

impl Default for CompletionStrategy {
    fn default() -> Self {
        Self::new(MatchMode::Prefix)
    }
}

// ── Tests ──────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;
    use crate::autocomplete::CompletionKind;

    #[test]
    fn test_fuzzy_score_exact() {
        let score = fuzzy_score("help", "help");
        assert!(score > 0.5, "exact match should score high: {}", score);
    }

    #[test]
    fn test_fuzzy_score_prefix() {
        let score = fuzzy_score("hel", "help");
        assert!(score > 0.3, "prefix should score well: {}", score);
    }

    #[test]
    fn test_fuzzy_score_no_match() {
        let score = fuzzy_score("xyz", "help");
        assert_eq!(score, 0.0);
    }

    #[test]
    fn test_fuzzy_score_empty_pattern() {
        let score = fuzzy_score("", "anything");
        assert_eq!(score, 1.0);
    }

    #[test]
    fn test_fuzzy_score_subsequence() {
        let score = fuzzy_score("hp", "help");
        assert!(score > 0.0, "subsequence should match: {}", score);
    }

    #[test]
    fn test_frecency_new_key() {
        let tracker = FrecencyTracker::new();
        assert_eq!(tracker.score("unknown"), 0.0);
    }

    #[test]
    fn test_frecency_after_access() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("foo");
        let score = tracker.score("foo");
        assert!(score > 0.0);
    }

    #[test]
    fn test_frecency_multiple_accesses() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("foo");
        tracker.record("foo");
        tracker.record("foo");
        let s3 = tracker.score("foo");
        // Three accesses should score higher than one
        let mut tracker2 = FrecencyTracker::new();
        tracker2.record("foo");
        let s1 = tracker2.score("foo");
        assert!(s3 > s1);
    }

    #[test]
    fn test_strategy_sort_by_label_length() {
        let strategy = CompletionStrategy::default();
        let mut items = vec![
            CompletionItem {
                insert_text: "/session-models".into(),
                label: "/session-models".into(),
                description: String::new(),
                kind: CompletionKind::Command,
                score: 0.0,
            },
            CompletionItem {
                insert_text: "/help".into(),
                label: "/help".into(),
                description: String::new(),
                kind: CompletionKind::Command,
                score: 0.0,
            },
        ];
        strategy.sort(&mut items);
        // Shorter label ("/help") should rank first
        assert_eq!(items[0].label, "/help");
    }

    #[test]
    fn test_strategy_frecency_boost() {
        let mut strategy = CompletionStrategy::default();
        strategy.record_access("/exit");

        let mut items = vec![
            CompletionItem {
                insert_text: "/help".into(),
                label: "/help".into(),
                description: String::new(),
                kind: CompletionKind::Command,
                score: 0.0,
            },
            CompletionItem {
                insert_text: "/exit".into(),
                label: "/exit".into(),
                description: String::new(),
                kind: CompletionKind::Command,
                score: 0.0,
            },
        ];
        strategy.sort(&mut items);
        // "/exit" has frecency boost and same length as "/help", should rank first
        assert_eq!(items[0].label, "/exit");
    }
}
