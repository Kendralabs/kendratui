//! Operation mode and autonomy level enums.

/// Operation mode — mirrors `OperationMode` from the Python side.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OperationMode {
    Normal,
    Plan,
}

impl std::fmt::Display for OperationMode {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Normal => write!(f, "Normal"),
            Self::Plan => write!(f, "Plan"),
        }
    }
}

impl OperationMode {
    /// Parse from string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "normal" => Some(Self::Normal),
            "plan" => Some(Self::Plan),
            _ => None,
        }
    }
}

/// Autonomy level — mirrors Python `StatusBar.autonomy`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum AutonomyLevel {
    Manual,
    SemiAuto,
    Auto,
}

impl std::fmt::Display for AutonomyLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Manual => write!(f, "Manual"),
            Self::SemiAuto => write!(f, "Semi-Auto"),
            Self::Auto => write!(f, "Auto"),
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

/// Reasoning effort level for native provider thinking.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReasoningLevel {
    Off,
    Low,
    Medium,
    High,
}

impl std::fmt::Display for ReasoningLevel {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Off => write!(f, "Off"),
            Self::Low => write!(f, "Low"),
            Self::Medium => write!(f, "Medium"),
            Self::High => write!(f, "High"),
        }
    }
}

impl ReasoningLevel {
    /// Parse from config string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Self {
        match s.to_lowercase().as_str() {
            "off" | "none" => Self::Off,
            "low" => Self::Low,
            "high" => Self::High,
            _ => Self::Medium,
        }
    }

    /// Convert to the config string used by LlmCallConfig.
    pub fn to_config_string(&self) -> Option<String> {
        match self {
            Self::Off => None,
            Self::Low => Some("low".to_string()),
            Self::Medium => Some("medium".to_string()),
            Self::High => Some("high".to_string()),
        }
    }

    /// Cycle to the next level: Off → Low → Medium → High → Off.
    pub fn next(self) -> Self {
        match self {
            Self::Off => Self::Low,
            Self::Low => Self::Medium,
            Self::Medium => Self::High,
            Self::High => Self::Off,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_operation_mode_display() {
        assert_eq!(OperationMode::Normal.to_string(), "Normal");
        assert_eq!(OperationMode::Plan.to_string(), "Plan");
    }

    #[test]
    fn test_operation_mode_from_str_loose() {
        assert_eq!(
            OperationMode::from_str_loose("plan"),
            Some(OperationMode::Plan)
        );
        assert_eq!(
            OperationMode::from_str_loose("Normal"),
            Some(OperationMode::Normal)
        );
        assert_eq!(OperationMode::from_str_loose("bogus"), None);
    }

    #[test]
    fn test_autonomy_level_from_str_loose() {
        assert_eq!(
            AutonomyLevel::from_str_loose("auto"),
            Some(AutonomyLevel::Auto)
        );
        assert_eq!(
            AutonomyLevel::from_str_loose("Semi-Auto"),
            Some(AutonomyLevel::SemiAuto)
        );
        assert_eq!(
            AutonomyLevel::from_str_loose("manual"),
            Some(AutonomyLevel::Manual)
        );
        assert_eq!(AutonomyLevel::from_str_loose("bogus"), None);
    }

    #[test]
    fn test_reasoning_level_cycle() {
        assert_eq!(ReasoningLevel::Off.next(), ReasoningLevel::Low);
        assert_eq!(ReasoningLevel::Low.next(), ReasoningLevel::Medium);
        assert_eq!(ReasoningLevel::Medium.next(), ReasoningLevel::High);
        assert_eq!(ReasoningLevel::High.next(), ReasoningLevel::Off);
    }

    #[test]
    fn test_reasoning_level_from_str() {
        assert_eq!(ReasoningLevel::from_str_loose("none"), ReasoningLevel::Off);
        assert_eq!(ReasoningLevel::from_str_loose("low"), ReasoningLevel::Low);
        assert_eq!(
            ReasoningLevel::from_str_loose("medium"),
            ReasoningLevel::Medium
        );
        assert_eq!(ReasoningLevel::from_str_loose("high"), ReasoningLevel::High);
    }

    #[test]
    fn test_reasoning_level_to_config() {
        assert_eq!(ReasoningLevel::Off.to_config_string(), None);
        assert_eq!(
            ReasoningLevel::Low.to_config_string(),
            Some("low".to_string())
        );
        assert_eq!(
            ReasoningLevel::Medium.to_config_string(),
            Some("medium".to_string())
        );
    }
}
