//! Hook data models: event types, matchers, commands, and configuration.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::fmt;

/// Lifecycle events that can trigger hooks.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum HookEvent {
    /// Fired when a new session starts.
    SessionStart,
    /// Fired when the user submits a prompt.
    UserPromptSubmit,
    /// Fired before a tool is executed.
    PreToolUse,
    /// Fired after a tool executes successfully.
    PostToolUse,
    /// Fired after a tool execution fails.
    PostToolUseFailure,
    /// Fired when a subagent is spawned.
    SubagentStart,
    /// Fired when a subagent finishes.
    SubagentStop,
    /// Fired when the agent decides to stop.
    Stop,
    /// Fired before context compaction.
    PreCompact,
    /// Fired when the session ends.
    SessionEnd,
}

impl HookEvent {
    /// The string name used in config files (e.g., "PreToolUse").
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::SessionStart => "SessionStart",
            Self::UserPromptSubmit => "UserPromptSubmit",
            Self::PreToolUse => "PreToolUse",
            Self::PostToolUse => "PostToolUse",
            Self::PostToolUseFailure => "PostToolUseFailure",
            Self::SubagentStart => "SubagentStart",
            Self::SubagentStop => "SubagentStop",
            Self::Stop => "Stop",
            Self::PreCompact => "PreCompact",
            Self::SessionEnd => "SessionEnd",
        }
    }

    /// Parse from the string name used in config files.
    pub fn from_config_str(s: &str) -> Option<Self> {
        match s {
            "SessionStart" => Some(Self::SessionStart),
            "UserPromptSubmit" => Some(Self::UserPromptSubmit),
            "PreToolUse" => Some(Self::PreToolUse),
            "PostToolUse" => Some(Self::PostToolUse),
            "PostToolUseFailure" => Some(Self::PostToolUseFailure),
            "SubagentStart" => Some(Self::SubagentStart),
            "SubagentStop" => Some(Self::SubagentStop),
            "Stop" => Some(Self::Stop),
            "PreCompact" => Some(Self::PreCompact),
            "SessionEnd" => Some(Self::SessionEnd),
            _ => None,
        }
    }

    /// Whether this is a tool-related event.
    pub fn is_tool_event(&self) -> bool {
        matches!(
            self,
            Self::PreToolUse | Self::PostToolUse | Self::PostToolUseFailure
        )
    }

    /// Whether this is a subagent-related event.
    pub fn is_subagent_event(&self) -> bool {
        matches!(self, Self::SubagentStart | Self::SubagentStop)
    }

    /// All valid event variants.
    pub const ALL: &'static [HookEvent] = &[
        Self::SessionStart,
        Self::UserPromptSubmit,
        Self::PreToolUse,
        Self::PostToolUse,
        Self::PostToolUseFailure,
        Self::SubagentStart,
        Self::SubagentStop,
        Self::Stop,
        Self::PreCompact,
        Self::SessionEnd,
    ];
}

impl fmt::Display for HookEvent {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

/// A single hook command to execute as a subprocess.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookCommand {
    /// Command type — currently always "command".
    #[serde(default = "default_command_type")]
    pub r#type: String,

    /// The shell command to execute.
    pub command: String,

    /// Timeout in seconds (clamped to 1..=600).
    #[serde(default = "default_timeout")]
    pub timeout: u32,
}

fn default_command_type() -> String {
    "command".to_string()
}

fn default_timeout() -> u32 {
    60
}

impl HookCommand {
    /// Create a new hook command with defaults.
    pub fn new(command: impl Into<String>) -> Self {
        Self {
            r#type: "command".to_string(),
            command: command.into(),
            timeout: 60,
        }
    }

    /// Create a new hook command with a custom timeout.
    pub fn with_timeout(command: impl Into<String>, timeout: u32) -> Self {
        Self {
            r#type: "command".to_string(),
            command: command.into(),
            timeout: timeout.clamp(1, 600),
        }
    }

    /// The effective timeout, clamped to the valid range.
    pub fn effective_timeout(&self) -> u32 {
        self.timeout.clamp(1, 600)
    }
}

/// A matcher that filters when hooks fire, with associated commands.
///
/// If `matcher` is `None`, the hooks fire for every occurrence of the event.
/// If `matcher` is `Some(pattern)`, it is compiled as a regex and tested
/// against the match value (e.g., tool name).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct HookMatcher {
    /// Optional regex pattern to filter events (e.g., tool name pattern).
    pub matcher: Option<String>,

    /// Commands to execute when the matcher matches.
    pub hooks: Vec<HookCommand>,

    /// Compiled regex (not serialized).
    #[serde(skip)]
    compiled_regex: Option<CompiledRegex>,
}

/// Wrapper to hold a compiled regex (Regex doesn't implement Debug well for our needs).
#[derive(Clone)]
struct CompiledRegex(Regex);

impl fmt::Debug for CompiledRegex {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "Regex({})", self.0.as_str())
    }
}

impl HookMatcher {
    /// Create a matcher that matches everything.
    pub fn catch_all(hooks: Vec<HookCommand>) -> Self {
        Self {
            matcher: None,
            hooks,
            compiled_regex: None,
        }
    }

    /// Create a matcher with a regex pattern.
    pub fn with_pattern(pattern: impl Into<String>, hooks: Vec<HookCommand>) -> Self {
        let pattern = pattern.into();
        let compiled = Regex::new(&pattern).ok().map(CompiledRegex);
        Self {
            matcher: Some(pattern),
            hooks,
            compiled_regex: compiled,
        }
    }

    /// Compile (or recompile) the regex from the matcher pattern.
    ///
    /// Call this after deserialization to populate the compiled regex.
    pub fn compile(&mut self) {
        if let Some(ref pattern) = self.matcher {
            self.compiled_regex = Regex::new(pattern).ok().map(CompiledRegex);
        }
    }

    /// Check if this matcher matches the given value.
    ///
    /// - If `matcher` is `None`, matches everything.
    /// - If `value` is `None`, matches everything.
    /// - Otherwise, tests the compiled regex (or falls back to exact string match).
    pub fn matches(&self, value: Option<&str>) -> bool {
        let pattern = match &self.matcher {
            None => return true,
            Some(p) => p,
        };

        let value = match value {
            None => return true,
            Some(v) => v,
        };

        match &self.compiled_regex {
            Some(compiled) => compiled.0.is_match(value),
            None => pattern == value,
        }
    }
}

/// Top-level hooks configuration, typically loaded from settings.json.
///
/// The keys in the `hooks` map are event names (e.g., "PreToolUse").
/// Unknown event names are silently dropped for forward compatibility.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct HookConfig {
    /// Map of event name to list of matchers.
    #[serde(default)]
    pub hooks: HashMap<String, Vec<HookMatcher>>,
}

impl HookConfig {
    /// Create an empty hook configuration.
    pub fn empty() -> Self {
        Self {
            hooks: HashMap::new(),
        }
    }

    /// Compile all regex patterns in all matchers.
    ///
    /// Call this after deserialization.
    pub fn compile_all(&mut self) {
        for matchers in self.hooks.values_mut() {
            for matcher in matchers.iter_mut() {
                matcher.compile();
            }
        }
    }

    /// Get matchers for a given event. Returns an empty slice if none.
    pub fn get_matchers(&self, event: HookEvent) -> &[HookMatcher] {
        self.hooks
            .get(event.as_str())
            .map(|v| v.as_slice())
            .unwrap_or(&[])
    }

    /// Fast check: are there any matchers registered for this event?
    pub fn has_hooks_for(&self, event: HookEvent) -> bool {
        self.hooks
            .get(event.as_str())
            .map(|v| !v.is_empty())
            .unwrap_or(false)
    }

    /// Remove any event keys that are not recognized.
    /// This provides forward compatibility — unknown events are silently dropped.
    pub fn strip_unknown_events(&mut self) {
        let valid: std::collections::HashSet<&str> =
            HookEvent::ALL.iter().map(|e| e.as_str()).collect();
        self.hooks.retain(|key, _| valid.contains(key.as_str()));
    }

    /// Register hooks for an event programmatically.
    pub fn add_matcher(&mut self, event: HookEvent, matcher: HookMatcher) {
        self.hooks
            .entry(event.as_str().to_string())
            .or_default()
            .push(matcher);
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_hook_event_roundtrip() {
        for event in HookEvent::ALL {
            let s = event.as_str();
            let parsed = HookEvent::from_config_str(s);
            assert_eq!(parsed, Some(*event), "roundtrip failed for {s}");
        }
    }

    #[test]
    fn test_hook_event_from_invalid() {
        assert_eq!(HookEvent::from_config_str("NotAnEvent"), None);
        assert_eq!(HookEvent::from_config_str(""), None);
    }

    #[test]
    fn test_hook_command_timeout_clamping() {
        let cmd = HookCommand::with_timeout("echo hi", 0);
        assert_eq!(cmd.effective_timeout(), 1);

        let cmd = HookCommand::with_timeout("echo hi", 9999);
        assert_eq!(cmd.effective_timeout(), 600);

        let cmd = HookCommand::with_timeout("echo hi", 30);
        assert_eq!(cmd.effective_timeout(), 30);
    }

    #[test]
    fn test_matcher_catch_all() {
        let m = HookMatcher::catch_all(vec![HookCommand::new("echo test")]);
        assert!(m.matches(None));
        assert!(m.matches(Some("anything")));
        assert!(m.matches(Some("bash")));
    }

    #[test]
    fn test_matcher_with_regex() {
        let m = HookMatcher::with_pattern(
            r"^(bash|edit)$",
            vec![HookCommand::new("echo test")],
        );
        assert!(m.matches(Some("bash")));
        assert!(m.matches(Some("edit")));
        assert!(!m.matches(Some("read")));
        // None value always matches
        assert!(m.matches(None));
    }

    #[test]
    fn test_matcher_invalid_regex_falls_back_to_exact() {
        let m = HookMatcher::with_pattern(
            r"[invalid",
            vec![HookCommand::new("echo test")],
        );
        // Invalid regex can't compile, falls back to exact match
        assert!(!m.matches(Some("anything")));
        assert!(m.matches(Some("[invalid")));
    }

    #[test]
    fn test_hook_config_deserialize() {
        let json = r#"{
            "hooks": {
                "PreToolUse": [
                    {
                        "matcher": "bash",
                        "hooks": [
                            { "command": "echo pre-bash", "timeout": 10 }
                        ]
                    }
                ],
                "UnknownEvent": [
                    {
                        "hooks": [
                            { "command": "echo unknown" }
                        ]
                    }
                ]
            }
        }"#;
        let mut config: HookConfig = serde_json::from_str(json).unwrap();
        config.compile_all();
        config.strip_unknown_events();

        assert!(config.has_hooks_for(HookEvent::PreToolUse));
        assert!(!config.has_hooks_for(HookEvent::PostToolUse));

        let matchers = config.get_matchers(HookEvent::PreToolUse);
        assert_eq!(matchers.len(), 1);
        assert!(matchers[0].matches(Some("bash")));
        assert!(!matchers[0].matches(Some("edit")));
    }

    #[test]
    fn test_hook_config_add_matcher() {
        let mut config = HookConfig::empty();
        config.add_matcher(
            HookEvent::PostToolUse,
            HookMatcher::catch_all(vec![HookCommand::new("echo done")]),
        );
        assert!(config.has_hooks_for(HookEvent::PostToolUse));
        assert!(!config.has_hooks_for(HookEvent::PreToolUse));
    }

    #[test]
    fn test_hook_event_classification() {
        assert!(HookEvent::PreToolUse.is_tool_event());
        assert!(HookEvent::PostToolUse.is_tool_event());
        assert!(HookEvent::PostToolUseFailure.is_tool_event());
        assert!(!HookEvent::SessionStart.is_tool_event());

        assert!(HookEvent::SubagentStart.is_subagent_event());
        assert!(HookEvent::SubagentStop.is_subagent_event());
        assert!(!HookEvent::PreToolUse.is_subagent_event());
    }
}
