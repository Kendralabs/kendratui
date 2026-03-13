//! Shared constants for the approval system.
//!
//! Provides canonical definitions for safe commands, autonomy levels, and thinking
//! levels used by both TUI and Web UI approval managers.
//!
//! Ported from `opendev/core/runtime/approval/constants.py`.

use serde::{Deserialize, Serialize};
use std::fmt;

/// Autonomy levels for command approval.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum AutonomyLevel {
    /// Every command requires manual approval.
    #[serde(rename = "Manual")]
    Manual,
    /// Safe commands auto-approved; others require approval.
    #[serde(rename = "Semi-Auto")]
    #[default]
    SemiAuto,
    /// All commands auto-approved (dangerous still flagged).
    #[serde(rename = "Auto")]
    Auto,
}

impl fmt::Display for AutonomyLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            AutonomyLevel::Manual => write!(f, "Manual"),
            AutonomyLevel::SemiAuto => write!(f, "Semi-Auto"),
            AutonomyLevel::Auto => write!(f, "Auto"),
        }
    }
}

impl AutonomyLevel {
    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "manual" => Some(Self::Manual),
            "semi-auto" | "semiauto" | "semi" => Some(Self::SemiAuto),
            "auto" | "full" => Some(Self::Auto),
            _ => None,
        }
    }
}

/// Thinking depth levels.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, Default)]
pub enum ThinkingLevel {
    #[serde(rename = "Off")]
    Off,
    #[serde(rename = "Low")]
    Low,
    #[serde(rename = "Medium")]
    #[default]
    Medium,
    #[serde(rename = "High")]
    High,
}

impl fmt::Display for ThinkingLevel {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            ThinkingLevel::Off => write!(f, "Off"),
            ThinkingLevel::Low => write!(f, "Low"),
            ThinkingLevel::Medium => write!(f, "Medium"),
            ThinkingLevel::High => write!(f, "High"),
        }
    }
}

impl ThinkingLevel {
    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "off" | "none" | "disabled" => Some(Self::Off),
            "low" | "lite" => Some(Self::Low),
            "medium" | "med" | "default" => Some(Self::Medium),
            "high" | "full" | "max" => Some(Self::High),
            _ => None,
        }
    }

    /// Whether thinking is enabled at all (any level above Off).
    pub fn is_enabled(&self) -> bool {
        !matches!(self, ThinkingLevel::Off)
    }

    /// Whether critique/refinement is active (High level only).
    pub fn use_critique(&self) -> bool {
        matches!(self, ThinkingLevel::High)
    }
}

/// Safe commands that can be auto-approved in Semi-Auto mode.
///
/// Shared between TUI and Web approval managers.
pub const SAFE_COMMANDS: &[&str] = &[
    "ls",
    "cat",
    "head",
    "tail",
    "grep",
    "find",
    "wc",
    "pwd",
    "echo",
    "which",
    "type",
    "file",
    "stat",
    "du",
    "df",
    "tree",
    "git status",
    "git log",
    "git diff",
    "git branch",
    "git show",
    "git remote",
    "git tag",
    "git stash list",
    "python --version",
    "python3 --version",
    "node --version",
    "npm --version",
    "cargo --version",
    "go version",
];

/// Check if a command is considered safe for auto-approval.
///
/// Uses strict matching: the command must either equal a safe command exactly
/// or start with it followed by a space (preventing e.g. `cat` from matching
/// `catastrophe`).
pub fn is_safe_command(command: &str) -> bool {
    if command.is_empty() {
        return false;
    }
    let cmd_lower = command.trim().to_lowercase();
    SAFE_COMMANDS.iter().any(|safe| {
        let safe_lower = safe.to_lowercase();
        cmd_lower == safe_lower || cmd_lower.starts_with(&format!("{safe_lower} "))
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_is_safe_command() {
        assert!(is_safe_command("ls"));
        assert!(is_safe_command("ls -la"));
        assert!(is_safe_command("git status"));
        assert!(is_safe_command("git diff --staged"));
        assert!(is_safe_command("cat foo.txt"));
        assert!(!is_safe_command("rm -rf /"));
        assert!(!is_safe_command("catastrophe")); // must not match "cat"
        assert!(!is_safe_command(""));
    }

    #[test]
    fn test_safe_command_case_insensitive() {
        assert!(is_safe_command("LS -la"));
        assert!(is_safe_command("Git Status"));
    }

    #[test]
    fn test_autonomy_level_display() {
        assert_eq!(AutonomyLevel::Manual.to_string(), "Manual");
        assert_eq!(AutonomyLevel::SemiAuto.to_string(), "Semi-Auto");
        assert_eq!(AutonomyLevel::Auto.to_string(), "Auto");
    }

    #[test]
    fn test_autonomy_level_parse() {
        assert_eq!(
            AutonomyLevel::from_str_loose("manual"),
            Some(AutonomyLevel::Manual)
        );
        assert_eq!(
            AutonomyLevel::from_str_loose("Semi-Auto"),
            Some(AutonomyLevel::SemiAuto)
        );
        assert_eq!(
            AutonomyLevel::from_str_loose("auto"),
            Some(AutonomyLevel::Auto)
        );
        assert_eq!(AutonomyLevel::from_str_loose("garbage"), None);
    }

    #[test]
    fn test_thinking_level_display() {
        assert_eq!(ThinkingLevel::Off.to_string(), "Off");
        assert_eq!(ThinkingLevel::Low.to_string(), "Low");
        assert_eq!(ThinkingLevel::Medium.to_string(), "Medium");
        assert_eq!(ThinkingLevel::High.to_string(), "High");
    }

    #[test]
    fn test_thinking_level_parse() {
        assert_eq!(
            ThinkingLevel::from_str_loose("off"),
            Some(ThinkingLevel::Off)
        );
        assert_eq!(
            ThinkingLevel::from_str_loose("Medium"),
            Some(ThinkingLevel::Medium)
        );
        assert_eq!(
            ThinkingLevel::from_str_loose("high"),
            Some(ThinkingLevel::High)
        );
        assert_eq!(ThinkingLevel::from_str_loose("garbage"), None);
    }

    #[test]
    fn test_thinking_level_flags() {
        assert!(!ThinkingLevel::Off.is_enabled());
        assert!(ThinkingLevel::Low.is_enabled());
        assert!(ThinkingLevel::Medium.is_enabled());
        assert!(ThinkingLevel::High.is_enabled());
        assert!(!ThinkingLevel::Low.use_critique());
        assert!(ThinkingLevel::High.use_critique());
    }

    #[test]
    fn test_autonomy_level_serde_roundtrip() {
        let level = AutonomyLevel::SemiAuto;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"Semi-Auto\"");
        let deserialized: AutonomyLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, level);
    }

    #[test]
    fn test_thinking_level_serde_roundtrip() {
        let level = ThinkingLevel::High;
        let json = serde_json::to_string(&level).unwrap();
        assert_eq!(json, "\"High\"");
        let deserialized: ThinkingLevel = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized, level);
    }
}
