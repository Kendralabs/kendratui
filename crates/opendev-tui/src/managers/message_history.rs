//! Input message history with up/down navigation.
//!
//! Mirrors Python's `MessageHistory` from
//! `opendev/ui_textual/managers/message_history.py`.

/// Manages a bounded history of sent messages with cursor-based navigation.
pub struct MessageHistory {
    history: Vec<String>,
    cursor: usize,
    capacity: usize,
    /// Tracks whether the cursor is active (user has navigated).
    navigating: bool,
}

impl MessageHistory {
    /// Create a new message history with the given capacity.
    pub fn new(capacity: usize) -> Self {
        Self {
            history: Vec::new(),
            cursor: 0,
            capacity,
            navigating: false,
        }
    }

    /// Push a new message onto the history.
    ///
    /// If capacity is exceeded, the oldest message is dropped.
    pub fn push(&mut self, msg: String) {
        if msg.is_empty() {
            return;
        }
        // Avoid consecutive duplicates
        if self.history.last().map(|s| s.as_str()) == Some(&msg) {
            self.reset_cursor();
            return;
        }
        self.history.push(msg);
        if self.history.len() > self.capacity {
            self.history.remove(0);
        }
        self.reset_cursor();
    }

    /// Navigate up (to older messages).
    ///
    /// Returns the message at the new cursor position, or `None` if
    /// there is no history.
    pub fn up(&mut self) -> Option<&str> {
        if self.history.is_empty() {
            return None;
        }
        if !self.navigating {
            self.navigating = true;
            self.cursor = self.history.len() - 1;
        } else if self.cursor > 0 {
            self.cursor -= 1;
        }
        Some(&self.history[self.cursor])
    }

    /// Navigate down (to newer messages).
    ///
    /// Returns the message at the new cursor position, or `None` if
    /// already past the newest entry.
    pub fn down(&mut self) -> Option<&str> {
        if !self.navigating || self.history.is_empty() {
            return None;
        }
        if self.cursor < self.history.len() - 1 {
            self.cursor += 1;
            Some(&self.history[self.cursor])
        } else {
            self.navigating = false;
            None
        }
    }

    /// Reset the navigation cursor.
    pub fn reset_cursor(&mut self) {
        self.cursor = 0;
        self.navigating = false;
    }

    /// The number of messages in history.
    pub fn len(&self) -> usize {
        self.history.len()
    }

    /// Whether the history is empty.
    pub fn is_empty(&self) -> bool {
        self.history.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let hist = MessageHistory::new(100);
        assert!(hist.is_empty());
        assert_eq!(hist.len(), 0);
    }

    #[test]
    fn test_push_and_navigate() {
        let mut hist = MessageHistory::new(100);
        hist.push("first".into());
        hist.push("second".into());
        hist.push("third".into());
        assert_eq!(hist.len(), 3);

        // up() returns most recent first
        assert_eq!(hist.up(), Some("third"));
        assert_eq!(hist.up(), Some("second"));
        assert_eq!(hist.up(), Some("first"));
        assert_eq!(hist.up(), Some("first")); // stays at oldest

        // down() navigates back
        assert_eq!(hist.down(), Some("second"));
        assert_eq!(hist.down(), Some("third"));
        assert_eq!(hist.down(), None); // past newest
    }

    #[test]
    fn test_capacity_eviction() {
        let mut hist = MessageHistory::new(3);
        hist.push("a".into());
        hist.push("b".into());
        hist.push("c".into());
        hist.push("d".into()); // evicts "a"
        assert_eq!(hist.len(), 3);

        assert_eq!(hist.up(), Some("d"));
        assert_eq!(hist.up(), Some("c"));
        assert_eq!(hist.up(), Some("b"));
        assert_eq!(hist.up(), Some("b")); // "a" is gone
    }

    #[test]
    fn test_empty_push_ignored() {
        let mut hist = MessageHistory::new(100);
        hist.push("".into());
        assert!(hist.is_empty());
    }

    #[test]
    fn test_consecutive_duplicate_ignored() {
        let mut hist = MessageHistory::new(100);
        hist.push("same".into());
        hist.push("same".into());
        assert_eq!(hist.len(), 1);
    }

    #[test]
    fn test_up_empty() {
        let mut hist = MessageHistory::new(100);
        assert_eq!(hist.up(), None);
    }

    #[test]
    fn test_down_without_navigating() {
        let mut hist = MessageHistory::new(100);
        hist.push("msg".into());
        assert_eq!(hist.down(), None);
    }

    #[test]
    fn test_reset_cursor() {
        let mut hist = MessageHistory::new(100);
        hist.push("a".into());
        hist.push("b".into());
        hist.up();
        hist.reset_cursor();
        // After reset, up() should start from newest again
        assert_eq!(hist.up(), Some("b"));
    }
}
