//! Frecency-based scoring for suggestion ranking.
//!
//! Mirrors Python's `FrecencyManager` from
//! `opendev/ui_textual/managers/frecency_manager.py`.
//!
//! Score formula: `frequency * (1.0 / (1.0 + hours_since_last_use))`

use std::collections::HashMap;
use std::time::Instant;

/// Entry tracking usage frequency and recency.
#[derive(Debug, Clone)]
pub struct FrecencyEntry {
    /// Number of times this item has been used.
    pub frequency: u64,
    /// When the item was last used.
    pub last_used: Instant,
}

/// Tracks and scores items by frequency and recency.
pub struct FrecencyTracker {
    entries: HashMap<String, FrecencyEntry>,
}

impl FrecencyTracker {
    /// Create a new empty tracker.
    pub fn new() -> Self {
        Self {
            entries: HashMap::new(),
        }
    }

    /// Record a usage of the given key.
    pub fn record(&mut self, key: &str) {
        let now = Instant::now();
        self.entries
            .entry(key.to_string())
            .and_modify(|e| {
                e.frequency += 1;
                e.last_used = now;
            })
            .or_insert(FrecencyEntry {
                frequency: 1,
                last_used: now,
            });
    }

    /// Calculate the frecency score for a key.
    ///
    /// Returns 0.0 if the key has never been recorded.
    /// Score = frequency * (1.0 / (1.0 + hours_since_last_use))
    pub fn score(&self, key: &str) -> f64 {
        match self.entries.get(key) {
            Some(entry) => {
                let hours = entry.last_used.elapsed().as_secs_f64() / 3600.0;
                entry.frequency as f64 * (1.0 / (1.0 + hours))
            }
            None => 0.0,
        }
    }

    /// Get the top N items sorted by frecency score (highest first).
    pub fn top_n(&self, n: usize) -> Vec<(&str, f64)> {
        let mut scored: Vec<(&str, f64)> = self
            .entries
            .keys()
            .map(|k| (k.as_str(), self.score(k)))
            .collect();
        scored.sort_by(|a, b| b.1.partial_cmp(&a.1).unwrap_or(std::cmp::Ordering::Equal));
        scored.truncate(n);
        scored
    }

    /// Get the entry for a key, if it exists.
    pub fn get(&self, key: &str) -> Option<&FrecencyEntry> {
        self.entries.get(key)
    }

    /// Number of tracked items.
    pub fn len(&self) -> usize {
        self.entries.len()
    }

    /// Whether the tracker is empty.
    pub fn is_empty(&self) -> bool {
        self.entries.is_empty()
    }

    /// Clear all entries.
    pub fn clear(&mut self) {
        self.entries.clear();
    }
}

impl Default for FrecencyTracker {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let tracker = FrecencyTracker::new();
        assert!(tracker.is_empty());
        assert_eq!(tracker.score("anything"), 0.0);
    }

    #[test]
    fn test_record_and_score() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("hello");
        assert_eq!(tracker.len(), 1);

        // Just recorded, so hours_since ~= 0, score ~= frequency (1.0)
        let s = tracker.score("hello");
        assert!(s > 0.9 && s <= 1.0, "score was {}", s);
    }

    #[test]
    fn test_multiple_records_increase_frequency() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("cmd");
        tracker.record("cmd");
        tracker.record("cmd");

        // Frequency = 3, recency ~= 1.0, so score ~= 3.0
        let s = tracker.score("cmd");
        assert!(s > 2.9 && s <= 3.0, "score was {}", s);
    }

    #[test]
    fn test_top_n() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("rare");
        tracker.record("common");
        tracker.record("common");
        tracker.record("common");
        tracker.record("mid");
        tracker.record("mid");

        let top = tracker.top_n(2);
        assert_eq!(top.len(), 2);
        assert_eq!(top[0].0, "common");
        assert_eq!(top[1].0, "mid");
    }

    #[test]
    fn test_top_n_more_than_entries() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("only");
        let top = tracker.top_n(10);
        assert_eq!(top.len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("a");
        tracker.record("b");
        tracker.clear();
        assert!(tracker.is_empty());
    }

    #[test]
    fn test_get_entry() {
        let mut tracker = FrecencyTracker::new();
        tracker.record("x");
        tracker.record("x");
        let entry = tracker.get("x").unwrap();
        assert_eq!(entry.frequency, 2);
    }

    #[test]
    fn test_unrecorded_score_zero() {
        let tracker = FrecencyTracker::new();
        assert_eq!(tracker.score("nonexistent"), 0.0);
    }
}
