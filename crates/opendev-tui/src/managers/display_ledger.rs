//! Display ledger for deduplicating rendered messages.
//!
//! Mirrors Python's `DisplayLedger` from
//! `opendev/ui_textual/managers/display_ledger.py`.
//!
//! Tracks which message IDs have been rendered to prevent duplicate display
//! across multiple code paths (streaming, history hydration, etc.).

use std::collections::HashSet;

/// Tracks which messages have been rendered to prevent duplicates.
pub struct DisplayLedger {
    rendered: HashSet<String>,
}

impl DisplayLedger {
    /// Create a new empty ledger.
    pub fn new() -> Self {
        Self {
            rendered: HashSet::new(),
        }
    }

    /// Mark a message ID as rendered.
    ///
    /// Returns `true` if this is the first time the ID was marked (i.e. not a duplicate).
    pub fn mark_rendered(&mut self, id: &str) -> bool {
        self.rendered.insert(id.to_string())
    }

    /// Check whether a message ID has been rendered.
    pub fn is_rendered(&self, id: &str) -> bool {
        self.rendered.contains(id)
    }

    /// Clear all rendered tracking state.
    pub fn clear(&mut self) {
        self.rendered.clear();
    }

    /// Number of tracked rendered messages.
    pub fn len(&self) -> usize {
        self.rendered.len()
    }

    /// Whether the ledger is empty.
    pub fn is_empty(&self) -> bool {
        self.rendered.is_empty()
    }
}

impl Default for DisplayLedger {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let ledger = DisplayLedger::new();
        assert!(ledger.is_empty());
        assert_eq!(ledger.len(), 0);
    }

    #[test]
    fn test_mark_and_check() {
        let mut ledger = DisplayLedger::new();
        assert!(!ledger.is_rendered("msg-1"));

        assert!(ledger.mark_rendered("msg-1")); // first time -> true
        assert!(ledger.is_rendered("msg-1"));
        assert_eq!(ledger.len(), 1);

        assert!(!ledger.mark_rendered("msg-1")); // duplicate -> false
        assert_eq!(ledger.len(), 1);
    }

    #[test]
    fn test_clear() {
        let mut ledger = DisplayLedger::new();
        ledger.mark_rendered("a");
        ledger.mark_rendered("b");
        assert_eq!(ledger.len(), 2);

        ledger.clear();
        assert!(ledger.is_empty());
        assert!(!ledger.is_rendered("a"));
    }
}
