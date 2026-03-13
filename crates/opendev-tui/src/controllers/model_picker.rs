//! Model picker controller for selecting LLM models in the TUI.
//!
//! Mirrors Python's `ModelPickerController` from
//! `opendev/ui_textual/controllers/model_picker_controller.py`.

/// A model option displayed in the picker.
#[derive(Debug, Clone)]
pub struct ModelOption {
    /// Unique model identifier (e.g. "claude-sonnet-4").
    pub id: String,
    /// Human-readable name.
    pub name: String,
    /// Provider name (e.g. "anthropic").
    pub provider: String,
    /// Context window length in tokens.
    pub context_length: u64,
}

/// Controller for navigating and selecting a model from a list.
pub struct ModelPickerController {
    models: Vec<ModelOption>,
    selected_index: usize,
    active: bool,
}

impl ModelPickerController {
    /// Create a new picker with the given model options.
    pub fn new(models: Vec<ModelOption>) -> Self {
        Self {
            models,
            selected_index: 0,
            active: true,
        }
    }

    /// Whether the picker is currently active.
    pub fn active(&self) -> bool {
        self.active
    }

    /// The available model options.
    pub fn models(&self) -> &[ModelOption] {
        &self.models
    }

    /// The currently selected index.
    pub fn selected_index(&self) -> usize {
        self.selected_index
    }

    /// Move selection to the next item (wrapping).
    pub fn next(&mut self) {
        if self.models.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + 1) % self.models.len();
    }

    /// Move selection to the previous item (wrapping).
    pub fn prev(&mut self) {
        if self.models.is_empty() {
            return;
        }
        self.selected_index = (self.selected_index + self.models.len() - 1) % self.models.len();
    }

    /// Confirm the current selection and deactivate the picker.
    ///
    /// Returns `None` if the model list is empty.
    pub fn select(&mut self) -> Option<&ModelOption> {
        if self.models.is_empty() {
            return None;
        }
        self.active = false;
        Some(&self.models[self.selected_index])
    }

    /// Cancel the picker without selecting.
    pub fn cancel(&mut self) {
        self.active = false;
    }

    /// Format the context length for display (e.g. "128k context").
    pub fn format_context(ctx: u64) -> String {
        if ctx >= 1000 {
            format!("{}k context", ctx / 1000)
        } else {
            format!("{} context", ctx)
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample_models() -> Vec<ModelOption> {
        vec![
            ModelOption {
                id: "claude-sonnet-4".into(),
                name: "Claude Sonnet 4".into(),
                provider: "anthropic".into(),
                context_length: 200_000,
            },
            ModelOption {
                id: "gpt-4o".into(),
                name: "GPT-4o".into(),
                provider: "openai".into(),
                context_length: 128_000,
            },
            ModelOption {
                id: "gemini-2.5-pro".into(),
                name: "Gemini 2.5 Pro".into(),
                provider: "google".into(),
                context_length: 1_000_000,
            },
        ]
    }

    #[test]
    fn test_new_picker() {
        let picker = ModelPickerController::new(sample_models());
        assert!(picker.active());
        assert_eq!(picker.selected_index(), 0);
        assert_eq!(picker.models().len(), 3);
    }

    #[test]
    fn test_next_wraps() {
        let mut picker = ModelPickerController::new(sample_models());
        picker.next();
        assert_eq!(picker.selected_index(), 1);
        picker.next();
        assert_eq!(picker.selected_index(), 2);
        picker.next();
        assert_eq!(picker.selected_index(), 0); // wrap
    }

    #[test]
    fn test_prev_wraps() {
        let mut picker = ModelPickerController::new(sample_models());
        picker.prev();
        assert_eq!(picker.selected_index(), 2); // wrap back
        picker.prev();
        assert_eq!(picker.selected_index(), 1);
    }

    #[test]
    fn test_select() {
        let mut picker = ModelPickerController::new(sample_models());
        picker.next(); // select index 1
        let selected = picker.select().unwrap();
        assert_eq!(selected.id, "gpt-4o");
        assert!(!picker.active());
    }

    #[test]
    fn test_select_empty() {
        let mut picker = ModelPickerController::new(vec![]);
        assert!(picker.select().is_none());
    }

    #[test]
    fn test_cancel() {
        let mut picker = ModelPickerController::new(sample_models());
        picker.cancel();
        assert!(!picker.active());
    }

    #[test]
    fn test_next_on_empty_is_noop() {
        let mut picker = ModelPickerController::new(vec![]);
        picker.next(); // should not panic
        assert_eq!(picker.selected_index(), 0);
    }

    #[test]
    fn test_format_context() {
        assert_eq!(
            ModelPickerController::format_context(128_000),
            "128k context"
        );
        assert_eq!(ModelPickerController::format_context(500), "500 context");
    }
}
