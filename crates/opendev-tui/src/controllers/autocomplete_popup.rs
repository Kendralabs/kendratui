//! Autocomplete popup controller for the TUI.
//!
//! Manages a popup overlay showing completion suggestions.

/// A single completion item.
#[derive(Debug, Clone)]
pub struct CompletionItem {
    /// The text to insert on selection.
    pub text: String,
    /// Display label (may differ from insertion text).
    pub label: String,
    /// Optional description shown alongside the label.
    pub description: Option<String>,
}

/// Controller for the autocomplete popup overlay.
pub struct AutocompletePopupController {
    items: Vec<CompletionItem>,
    selected: usize,
    visible: bool,
}

impl AutocompletePopupController {
    /// Create a new hidden autocomplete popup.
    pub fn new() -> Self {
        Self {
            items: Vec::new(),
            selected: 0,
            visible: false,
        }
    }

    /// Whether the popup is currently visible.
    pub fn visible(&self) -> bool {
        self.visible
    }

    /// The completion items.
    pub fn items(&self) -> &[CompletionItem] {
        &self.items
    }

    /// The currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Show the popup with the given completion items.
    pub fn show(&mut self, items: Vec<CompletionItem>) {
        self.items = items;
        self.selected = 0;
        self.visible = !self.items.is_empty();
    }

    /// Hide the popup.
    pub fn hide(&mut self) {
        self.visible = false;
        self.items.clear();
        self.selected = 0;
    }

    /// Move selection to the next item (wrapping).
    pub fn next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.items.len();
    }

    /// Move selection to the previous item (wrapping).
    pub fn prev(&mut self) {
        if self.items.is_empty() {
            return;
        }
        self.selected = (self.selected + self.items.len() - 1) % self.items.len();
    }

    /// Confirm the current selection and hide the popup.
    ///
    /// Returns `None` if items list is empty or popup is hidden.
    pub fn select(&mut self) -> Option<&CompletionItem> {
        if !self.visible || self.items.is_empty() {
            return None;
        }
        self.visible = false;
        Some(&self.items[self.selected])
    }
}

impl Default for AutocompletePopupController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_items() -> Vec<CompletionItem> {
        vec![
            CompletionItem {
                text: "/help".into(),
                label: "/help".into(),
                description: Some("Show help".into()),
            },
            CompletionItem {
                text: "/mode".into(),
                label: "/mode".into(),
                description: Some("Switch mode".into()),
            },
            CompletionItem {
                text: "/models".into(),
                label: "/models".into(),
                description: None,
            },
        ]
    }

    #[test]
    fn test_new_is_hidden() {
        let ctrl = AutocompletePopupController::new();
        assert!(!ctrl.visible());
        assert!(ctrl.items().is_empty());
    }

    #[test]
    fn test_show_and_hide() {
        let mut ctrl = AutocompletePopupController::new();
        ctrl.show(sample_items());
        assert!(ctrl.visible());
        assert_eq!(ctrl.items().len(), 3);

        ctrl.hide();
        assert!(!ctrl.visible());
        assert!(ctrl.items().is_empty());
    }

    #[test]
    fn test_show_empty_stays_hidden() {
        let mut ctrl = AutocompletePopupController::new();
        ctrl.show(vec![]);
        assert!(!ctrl.visible());
    }

    #[test]
    fn test_navigation() {
        let mut ctrl = AutocompletePopupController::new();
        ctrl.show(sample_items());
        assert_eq!(ctrl.selected_index(), 0);

        ctrl.next();
        assert_eq!(ctrl.selected_index(), 1);
        ctrl.next();
        ctrl.next();
        assert_eq!(ctrl.selected_index(), 0); // wrap

        ctrl.prev();
        assert_eq!(ctrl.selected_index(), 2); // wrap back
    }

    #[test]
    fn test_select() {
        let mut ctrl = AutocompletePopupController::new();
        ctrl.show(sample_items());
        ctrl.next(); // index 1
        let item = ctrl.select().unwrap();
        assert_eq!(item.text, "/mode");
        assert!(!ctrl.visible());
    }

    #[test]
    fn test_select_when_hidden() {
        let mut ctrl = AutocompletePopupController::new();
        assert!(ctrl.select().is_none());
    }
}
