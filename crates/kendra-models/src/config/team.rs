use serde::{Deserialize, Serialize};

/// Persona/Team configuration schema (`team.json`).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TeamConfig {
    /// The specialized system prompt for this persona.
    pub system_prompt: String,

    /// List of enabled tool identifiers (e.g., ["bash", "git", "file_system"]).
    #[serde(default)]
    pub enabled_tools: Vec<String>,

    /// Context scopes to restrict the agent (e.g., ["src/", "tests/"]).
    #[serde(default)]
    pub context_scopes: Vec<String>,
}

impl Default for TeamConfig {
    fn default() -> Self {
        Self {
            system_prompt: "You are a helpful coding assistant.".to_string(),
            enabled_tools: vec!["bash".to_string(), "file_system".to_string()],
            context_scopes: Vec::new(),
        }
    }
}
