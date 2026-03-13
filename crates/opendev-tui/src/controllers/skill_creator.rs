//! Skill creator controller for the TUI.
//!
//! Manages form state for creating skill files, including name,
//! description, prompt content, and invocability settings.

/// Specification for a skill, produced by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillSpec {
    pub name: String,
    pub description: String,
    pub content: String,
    pub is_user_invocable: bool,
}

/// Controller for the skill creation form.
pub struct SkillCreatorController {
    name: String,
    description: String,
    content: String,
    is_user_invocable: bool,
}

impl SkillCreatorController {
    /// Create a new skill creator controller with empty fields.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            content: String::new(),
            is_user_invocable: true,
        }
    }

    /// Set the skill name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Get the current skill name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set the skill description.
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = description.into();
    }

    /// Get the current description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Set the skill content (prompt text).
    pub fn set_content(&mut self, content: impl Into<String>) {
        self.content = content.into();
    }

    /// Get the current content.
    pub fn content(&self) -> &str {
        &self.content
    }

    /// Set whether this skill can be invoked directly by users.
    pub fn set_user_invocable(&mut self, invocable: bool) {
        self.is_user_invocable = invocable;
    }

    /// Whether this skill is user-invocable.
    pub fn is_user_invocable(&self) -> bool {
        self.is_user_invocable
    }

    /// Validate the current form state and produce a [`SkillSpec`].
    ///
    /// Returns an error string describing the first validation failure.
    pub fn validate(&self) -> Result<SkillSpec, String> {
        if self.name.trim().is_empty() {
            return Err("Skill name is required".into());
        }
        if self.description.trim().is_empty() {
            return Err("Skill description is required".into());
        }
        if self.content.trim().is_empty() {
            return Err("Skill content is required".into());
        }
        Ok(SkillSpec {
            name: self.name.trim().to_string(),
            description: self.description.trim().to_string(),
            content: self.content.clone(),
            is_user_invocable: self.is_user_invocable,
        })
    }

    /// Reset all fields to their default values.
    pub fn reset(&mut self) {
        self.name.clear();
        self.description.clear();
        self.content.clear();
        self.is_user_invocable = true;
    }
}

impl Default for SkillCreatorController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_defaults() {
        let ctrl = SkillCreatorController::new();
        assert_eq!(ctrl.name(), "");
        assert_eq!(ctrl.description(), "");
        assert_eq!(ctrl.content(), "");
        assert!(ctrl.is_user_invocable());
    }

    #[test]
    fn test_set_fields() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("review-pr");
        ctrl.set_description("Review a pull request");
        ctrl.set_content("You are a code reviewer.\nBe thorough.");
        ctrl.set_user_invocable(false);

        assert_eq!(ctrl.name(), "review-pr");
        assert_eq!(ctrl.description(), "Review a pull request");
        assert_eq!(ctrl.content(), "You are a code reviewer.\nBe thorough.");
        assert!(!ctrl.is_user_invocable());
    }

    #[test]
    fn test_validate_success() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("commit");
        ctrl.set_description("Create a git commit");
        ctrl.set_content("Analyze staged changes and create a commit.");

        let spec = ctrl.validate().unwrap();
        assert_eq!(spec.name, "commit");
        assert_eq!(spec.description, "Create a git commit");
        assert_eq!(spec.content, "Analyze staged changes and create a commit.");
        assert!(spec.is_user_invocable);
    }

    #[test]
    fn test_validate_not_invocable() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("internal");
        ctrl.set_description("Internal skill");
        ctrl.set_content("content");
        ctrl.set_user_invocable(false);

        let spec = ctrl.validate().unwrap();
        assert!(!spec.is_user_invocable);
    }

    #[test]
    fn test_validate_missing_name() {
        let ctrl = SkillCreatorController::new();
        let err = ctrl.validate().unwrap_err();
        assert!(err.contains("name"), "Error should mention name: {err}");
    }

    #[test]
    fn test_validate_missing_description() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("skill");
        let err = ctrl.validate().unwrap_err();
        assert!(
            err.contains("description"),
            "Error should mention description: {err}"
        );
    }

    #[test]
    fn test_validate_missing_content() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("skill");
        ctrl.set_description("desc");
        let err = ctrl.validate().unwrap_err();
        assert!(
            err.contains("content"),
            "Error should mention content: {err}"
        );
    }

    #[test]
    fn test_validate_trims_whitespace() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("  skill  ");
        ctrl.set_description("  desc  ");
        ctrl.set_content("content");

        let spec = ctrl.validate().unwrap();
        assert_eq!(spec.name, "skill");
        assert_eq!(spec.description, "desc");
    }

    #[test]
    fn test_validate_whitespace_only_is_invalid() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("   ");
        assert!(ctrl.validate().is_err());
    }

    #[test]
    fn test_reset() {
        let mut ctrl = SkillCreatorController::new();
        ctrl.set_name("skill");
        ctrl.set_description("desc");
        ctrl.set_content("content");
        ctrl.set_user_invocable(false);

        ctrl.reset();

        assert_eq!(ctrl.name(), "");
        assert_eq!(ctrl.description(), "");
        assert_eq!(ctrl.content(), "");
        assert!(ctrl.is_user_invocable());
    }

    #[test]
    fn test_default_trait() {
        let ctrl = SkillCreatorController::default();
        assert_eq!(ctrl.name(), "");
        assert!(ctrl.is_user_invocable());
    }
}
