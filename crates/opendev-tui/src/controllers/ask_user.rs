//! Ask-user prompt controller for the TUI.
//!
//! Mirrors Python's `AskUserPromptController` from
//! `opendev/ui_textual/controllers/ask_user_prompt_controller.py`.
//!
//! Displays a question with numbered options and collects the user's selection.

/// Controller for displaying questions with selectable options.
pub struct AskUserController {
    question: String,
    options: Vec<String>,
    selected: usize,
    active: bool,
}

impl AskUserController {
    /// Create a new ask-user controller.
    pub fn new(question: String, options: Vec<String>) -> Self {
        Self {
            question,
            options,
            selected: 0,
            active: true,
        }
    }

    /// Whether the prompt is currently active.
    pub fn active(&self) -> bool {
        self.active
    }

    /// The question being asked.
    pub fn question(&self) -> &str {
        &self.question
    }

    /// The available options.
    pub fn options(&self) -> &[String] {
        &self.options
    }

    /// The currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected
    }

    /// Move selection to the next option (wrapping).
    pub fn next(&mut self) {
        if self.options.is_empty() {
            return;
        }
        self.selected = (self.selected + 1) % self.options.len();
    }

    /// Move selection to the previous option (wrapping).
    pub fn prev(&mut self) {
        if self.options.is_empty() {
            return;
        }
        self.selected = (self.selected + self.options.len() - 1) % self.options.len();
    }

    /// Confirm the current selection and deactivate.
    ///
    /// Returns `None` if options list is empty.
    pub fn select(&mut self) -> Option<&str> {
        if self.options.is_empty() {
            return None;
        }
        self.active = false;
        Some(&self.options[self.selected])
    }

    /// Cancel the prompt without selecting.
    pub fn cancel(&mut self) {
        self.active = false;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_options() -> Vec<String> {
        vec!["Yes".into(), "No".into(), "Maybe".into()]
    }

    #[test]
    fn test_new_controller() {
        let ctrl = AskUserController::new("Continue?".into(), sample_options());
        assert!(ctrl.active());
        assert_eq!(ctrl.question(), "Continue?");
        assert_eq!(ctrl.options().len(), 3);
        assert_eq!(ctrl.selected_index(), 0);
    }

    #[test]
    fn test_next_wraps() {
        let mut ctrl = AskUserController::new("Q?".into(), sample_options());
        ctrl.next();
        assert_eq!(ctrl.selected_index(), 1);
        ctrl.next();
        ctrl.next();
        assert_eq!(ctrl.selected_index(), 0); // wrap
    }

    #[test]
    fn test_prev_wraps() {
        let mut ctrl = AskUserController::new("Q?".into(), sample_options());
        ctrl.prev();
        assert_eq!(ctrl.selected_index(), 2); // wrap back
    }

    #[test]
    fn test_select() {
        let mut ctrl = AskUserController::new("Q?".into(), sample_options());
        ctrl.next(); // index 1
        let answer = ctrl.select().unwrap();
        assert_eq!(answer, "No");
        assert!(!ctrl.active());
    }

    #[test]
    fn test_select_empty() {
        let mut ctrl = AskUserController::new("Q?".into(), vec![]);
        assert!(ctrl.select().is_none());
    }

    #[test]
    fn test_cancel() {
        let mut ctrl = AskUserController::new("Q?".into(), sample_options());
        ctrl.cancel();
        assert!(!ctrl.active());
    }
}
