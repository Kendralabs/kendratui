//! Agent creator controller for the TUI.
//!
//! Manages form state for creating custom agent definitions, including
//! name, description, model, tools, and instructions.

/// Specification for a custom agent, produced by validation.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AgentSpec {
    pub name: String,
    pub description: String,
    pub model: Option<String>,
    pub tools: Vec<String>,
    pub instructions: String,
}

/// Controller for the agent creation form.
pub struct AgentCreatorController {
    name: String,
    description: String,
    model: Option<String>,
    tools: Vec<String>,
    instructions: String,
}

impl AgentCreatorController {
    /// Create a new agent creator controller with empty fields.
    pub fn new() -> Self {
        Self {
            name: String::new(),
            description: String::new(),
            model: None,
            tools: Vec::new(),
            instructions: String::new(),
        }
    }

    /// Set the agent name.
    pub fn set_name(&mut self, name: impl Into<String>) {
        self.name = name.into();
    }

    /// Get the current agent name.
    pub fn name(&self) -> &str {
        &self.name
    }

    /// Set the agent description.
    pub fn set_description(&mut self, description: impl Into<String>) {
        self.description = description.into();
    }

    /// Get the current description.
    pub fn description(&self) -> &str {
        &self.description
    }

    /// Set the model (or `None` to use the default).
    pub fn set_model(&mut self, model: Option<String>) {
        self.model = model;
    }

    /// Get the current model selection.
    pub fn model(&self) -> Option<&str> {
        self.model.as_deref()
    }

    /// Add a tool to the agent's tool list.
    ///
    /// Duplicate tool names are silently ignored.
    pub fn add_tool(&mut self, tool: impl Into<String>) {
        let tool = tool.into();
        if !self.tools.contains(&tool) {
            self.tools.push(tool);
        }
    }

    /// Remove a tool by name. Returns `true` if the tool was present.
    pub fn remove_tool(&mut self, tool: &str) -> bool {
        if let Some(pos) = self.tools.iter().position(|t| t == tool) {
            self.tools.remove(pos);
            true
        } else {
            false
        }
    }

    /// Get the current tool list.
    pub fn tools(&self) -> &[String] {
        &self.tools
    }

    /// Set the agent instructions (multi-line prompt text).
    pub fn set_instructions(&mut self, instructions: impl Into<String>) {
        self.instructions = instructions.into();
    }

    /// Get the current instructions.
    pub fn instructions(&self) -> &str {
        &self.instructions
    }

    /// Validate the current form state and produce an [`AgentSpec`].
    ///
    /// Returns an error string describing the first validation failure.
    pub fn validate(&self) -> Result<AgentSpec, String> {
        if self.name.trim().is_empty() {
            return Err("Agent name is required".into());
        }
        if self.description.trim().is_empty() {
            return Err("Agent description is required".into());
        }
        if self.instructions.trim().is_empty() {
            return Err("Agent instructions are required".into());
        }
        Ok(AgentSpec {
            name: self.name.trim().to_string(),
            description: self.description.trim().to_string(),
            model: self.model.clone(),
            tools: self.tools.clone(),
            instructions: self.instructions.clone(),
        })
    }

    /// Reset all fields to their default (empty) values.
    pub fn reset(&mut self) {
        self.name.clear();
        self.description.clear();
        self.model = None;
        self.tools.clear();
        self.instructions.clear();
    }
}

impl Default for AgentCreatorController {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_is_empty() {
        let ctrl = AgentCreatorController::new();
        assert_eq!(ctrl.name(), "");
        assert_eq!(ctrl.description(), "");
        assert_eq!(ctrl.model(), None);
        assert!(ctrl.tools().is_empty());
        assert_eq!(ctrl.instructions(), "");
    }

    #[test]
    fn test_set_fields() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("my-agent");
        ctrl.set_description("A helpful agent");
        ctrl.set_model(Some("gpt-4".into()));
        ctrl.set_instructions("You are a coding assistant.\nBe concise.");

        assert_eq!(ctrl.name(), "my-agent");
        assert_eq!(ctrl.description(), "A helpful agent");
        assert_eq!(ctrl.model(), Some("gpt-4"));
        assert_eq!(ctrl.instructions(), "You are a coding assistant.\nBe concise.");
    }

    #[test]
    fn test_add_and_remove_tools() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.add_tool("bash");
        ctrl.add_tool("file_read");
        ctrl.add_tool("bash"); // duplicate
        assert_eq!(ctrl.tools(), &["bash", "file_read"]);

        assert!(ctrl.remove_tool("bash"));
        assert_eq!(ctrl.tools(), &["file_read"]);

        assert!(!ctrl.remove_tool("nonexistent"));
    }

    #[test]
    fn test_validate_success() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("test-agent");
        ctrl.set_description("desc");
        ctrl.set_instructions("do stuff");
        ctrl.add_tool("bash");

        let spec = ctrl.validate().unwrap();
        assert_eq!(spec.name, "test-agent");
        assert_eq!(spec.description, "desc");
        assert_eq!(spec.model, None);
        assert_eq!(spec.tools, vec!["bash".to_string()]);
        assert_eq!(spec.instructions, "do stuff");
    }

    #[test]
    fn test_validate_with_model() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("agent");
        ctrl.set_description("desc");
        ctrl.set_model(Some("claude-3".into()));
        ctrl.set_instructions("instructions");

        let spec = ctrl.validate().unwrap();
        assert_eq!(spec.model, Some("claude-3".into()));
    }

    #[test]
    fn test_validate_missing_name() {
        let ctrl = AgentCreatorController::new();
        let err = ctrl.validate().unwrap_err();
        assert!(err.contains("name"), "Error should mention name: {err}");
    }

    #[test]
    fn test_validate_missing_description() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("agent");
        let err = ctrl.validate().unwrap_err();
        assert!(err.contains("description"), "Error should mention description: {err}");
    }

    #[test]
    fn test_validate_missing_instructions() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("agent");
        ctrl.set_description("desc");
        let err = ctrl.validate().unwrap_err();
        assert!(err.contains("instructions"), "Error should mention instructions: {err}");
    }

    #[test]
    fn test_validate_trims_whitespace() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("  agent  ");
        ctrl.set_description("  desc  ");
        ctrl.set_instructions("instructions");

        let spec = ctrl.validate().unwrap();
        assert_eq!(spec.name, "agent");
        assert_eq!(spec.description, "desc");
    }

    #[test]
    fn test_validate_whitespace_only_is_invalid() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("   ");
        assert!(ctrl.validate().is_err());
    }

    #[test]
    fn test_reset() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("agent");
        ctrl.set_description("desc");
        ctrl.set_model(Some("model".into()));
        ctrl.add_tool("bash");
        ctrl.set_instructions("instr");

        ctrl.reset();

        assert_eq!(ctrl.name(), "");
        assert_eq!(ctrl.description(), "");
        assert_eq!(ctrl.model(), None);
        assert!(ctrl.tools().is_empty());
        assert_eq!(ctrl.instructions(), "");
    }

    #[test]
    fn test_validate_empty_tools_is_ok() {
        let mut ctrl = AgentCreatorController::new();
        ctrl.set_name("agent");
        ctrl.set_description("desc");
        ctrl.set_instructions("instr");

        let spec = ctrl.validate().unwrap();
        assert!(spec.tools.is_empty());
    }

    #[test]
    fn test_default_trait() {
        let ctrl = AgentCreatorController::default();
        assert_eq!(ctrl.name(), "");
    }
}
