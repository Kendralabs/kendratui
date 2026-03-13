//! Approval prompt controller for inline command approval within the TUI.
//!
//! Mirrors Python's `ApprovalPromptController` from
//! `opendev/ui_textual/controllers/approval_prompt_controller.py`.
//!
//! The controller manages a state machine:
//! 1. A tool requests approval → `start()` activates the prompt
//! 2. User navigates options with Up/Down, confirms with Enter, cancels with Esc
//! 3. Result is sent back via a oneshot channel

use tokio::sync::oneshot;

/// User's decision from the approval prompt.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ApprovalDecision {
    /// Whether the action was approved.
    pub approved: bool,
    /// Which option was selected ("1", "2", or "3").
    pub choice: String,
    /// The (potentially edited) command text.
    pub command: String,
}

/// A single option in the approval prompt.
#[derive(Debug, Clone)]
pub struct ApprovalOption {
    /// Display choice key (e.g. "1", "2", "3").
    pub choice: String,
    /// Short label (e.g. "Yes", "No").
    pub label: String,
    /// Longer description.
    pub description: String,
    /// Whether selecting this option means approval.
    pub approved: bool,
}

/// Manages the inline approval prompt state machine.
pub struct ApprovalController {
    active: bool,
    options: Vec<ApprovalOption>,
    selected_index: usize,
    command: String,
    working_dir: String,
    response_tx: Option<oneshot::Sender<ApprovalDecision>>,
}

impl ApprovalController {
    /// Create a new inactive approval controller.
    pub fn new() -> Self {
        Self {
            active: false,
            options: Vec::new(),
            selected_index: 0,
            command: String::new(),
            working_dir: String::from("."),
            response_tx: None,
        }
    }

    /// Whether the approval prompt is currently active.
    pub fn active(&self) -> bool {
        self.active
    }

    /// The command being approved.
    pub fn command(&self) -> &str {
        &self.command
    }

    /// The working directory for the command.
    pub fn working_dir(&self) -> &str {
        &self.working_dir
    }

    /// The available options.
    pub fn options(&self) -> &[ApprovalOption] {
        &self.options
    }

    /// The currently selected option index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Start the approval prompt for a command.
    ///
    /// Returns a receiver that will yield the user's decision.
    pub fn start(
        &mut self,
        command: String,
        working_dir: String,
    ) -> oneshot::Receiver<ApprovalDecision> {
        let base_prefix = command.split_whitespace().next().unwrap_or("").to_string();

        let auto_desc = if !base_prefix.is_empty() {
            format!(
                "Automatically approve commands starting with '{}' in {}.",
                base_prefix, working_dir
            )
        } else {
            format!("Automatically approve future commands in {}.", working_dir)
        };

        self.options = vec![
            ApprovalOption {
                choice: "1".into(),
                label: "Yes".into(),
                description: "Run this command now.".into(),
                approved: true,
            },
            ApprovalOption {
                choice: "2".into(),
                label: "Yes, and don't ask again".into(),
                description: auto_desc,
                approved: true,
            },
            ApprovalOption {
                choice: "3".into(),
                label: "No".into(),
                description: "Cancel and adjust your request.".into(),
                approved: false,
            },
        ];

        self.command = command;
        self.working_dir = working_dir;
        self.selected_index = 0;
        self.active = true;

        let (tx, rx) = oneshot::channel();
        self.response_tx = Some(tx);
        rx
    }

    /// Move the selection by `delta` positions (wrapping).
    pub fn move_selection(&mut self, delta: i32) {
        if !self.active || self.options.is_empty() {
            return;
        }
        let len = self.options.len() as i32;
        let new_idx = ((self.selected_index as i32) + delta).rem_euclid(len);
        self.selected_index = new_idx as usize;
    }

    /// Confirm the current selection and send the decision.
    pub fn confirm(&mut self) {
        if !self.active {
            return;
        }

        let option = &self.options[self.selected_index];
        let decision = ApprovalDecision {
            approved: option.approved,
            choice: option.choice.clone(),
            command: self.command.clone(),
        };

        if let Some(tx) = self.response_tx.take() {
            let _ = tx.send(decision);
        }

        self.cleanup();
    }

    /// Cancel the approval (selects "No" and confirms).
    pub fn cancel(&mut self) {
        if !self.active || self.options.is_empty() {
            return;
        }
        // Select the last option ("No")
        self.selected_index = self.options.len() - 1;
        self.confirm();
    }

    /// Reset the controller to inactive state.
    fn cleanup(&mut self) {
        self.active = false;
        self.options.clear();
        self.selected_index = 0;
        self.command.clear();
        self.response_tx = None;
    }
}

impl Default for ApprovalController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_controller_is_inactive() {
        let ctrl = ApprovalController::new();
        assert!(!ctrl.active());
        assert!(ctrl.options().is_empty());
    }

    #[tokio::test]
    async fn test_start_activates() {
        let mut ctrl = ApprovalController::new();
        let _rx = ctrl.start("rm -rf /tmp/test".into(), "/home/user".into());
        assert!(ctrl.active());
        assert_eq!(ctrl.options().len(), 3);
        assert_eq!(ctrl.command(), "rm -rf /tmp/test");
        assert_eq!(ctrl.selected_index(), 0);
    }

    #[tokio::test]
    async fn test_move_selection_wraps() {
        let mut ctrl = ApprovalController::new();
        let _rx = ctrl.start("cmd".into(), ".".into());

        ctrl.move_selection(1); // 0 -> 1
        assert_eq!(ctrl.selected_index(), 1);

        ctrl.move_selection(1); // 1 -> 2
        assert_eq!(ctrl.selected_index(), 2);

        ctrl.move_selection(1); // 2 -> 0 (wrap)
        assert_eq!(ctrl.selected_index(), 0);

        ctrl.move_selection(-1); // 0 -> 2 (wrap back)
        assert_eq!(ctrl.selected_index(), 2);
    }

    #[tokio::test]
    async fn test_confirm_sends_decision() {
        let mut ctrl = ApprovalController::new();
        let rx = ctrl.start("git push".into(), ".".into());

        // Default selection is "Yes" (index 0)
        ctrl.confirm();
        assert!(!ctrl.active());

        let decision = rx.await.unwrap();
        assert!(decision.approved);
        assert_eq!(decision.choice, "1");
        assert_eq!(decision.command, "git push");
    }

    #[tokio::test]
    async fn test_cancel_sends_no() {
        let mut ctrl = ApprovalController::new();
        let rx = ctrl.start("dangerous cmd".into(), ".".into());

        ctrl.cancel();
        assert!(!ctrl.active());

        let decision = rx.await.unwrap();
        assert!(!decision.approved);
        assert_eq!(decision.choice, "3");
    }

    #[tokio::test]
    async fn test_confirm_second_option() {
        let mut ctrl = ApprovalController::new();
        let rx = ctrl.start("npm install".into(), ".".into());

        ctrl.move_selection(1); // Select "Yes, and don't ask again"
        ctrl.confirm();

        let decision = rx.await.unwrap();
        assert!(decision.approved);
        assert_eq!(decision.choice, "2");
    }

    #[test]
    fn test_move_on_inactive_is_noop() {
        let mut ctrl = ApprovalController::new();
        ctrl.move_selection(1); // Should not panic
        assert!(!ctrl.active());
    }

    #[test]
    fn test_confirm_on_inactive_is_noop() {
        let mut ctrl = ApprovalController::new();
        ctrl.confirm(); // Should not panic
    }

    #[test]
    fn test_auto_desc_with_prefix() {
        let mut ctrl = ApprovalController::new();
        let _rx = ctrl.start("git push --force".into(), "/project".into());
        let opt2 = &ctrl.options()[1];
        assert!(opt2.description.contains("git"));
        assert!(opt2.description.contains("/project"));
    }
}
