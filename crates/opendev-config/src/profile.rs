//! Environment-specific configuration profiles.
//!
//! Supports `OPENDEV_PROFILE` env var or `--profile` flag with values:
//! - `dev` — enables debug logging and verbose output
//! - `prod` — disables debug logging, conservative settings
//! - `fast` — reduces thinking level, lowers max tokens for speed

use opendev_models::AppConfig;
use tracing::debug;

/// Known profile names.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Profile {
    Dev,
    Prod,
    Fast,
}

impl Profile {
    /// Parse a profile name from a string (case-insensitive).
    pub fn from_str_loose(s: &str) -> Option<Self> {
        match s.to_lowercase().as_str() {
            "dev" | "development" => Some(Self::Dev),
            "prod" | "production" => Some(Self::Prod),
            "fast" | "quick" => Some(Self::Fast),
            _ => None,
        }
    }

    /// Display name.
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Dev => "dev",
            Self::Prod => "prod",
            Self::Fast => "fast",
        }
    }

    /// All known profiles.
    pub fn all() -> &'static [Profile] {
        &[Self::Dev, Self::Prod, Self::Fast]
    }
}

impl std::fmt::Display for Profile {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(self.as_str())
    }
}

/// Apply profile-specific overrides to an AppConfig.
///
/// Returns true if a valid profile was applied, false if the name was unrecognized.
pub fn apply_profile(config: &mut AppConfig, profile_name: &str) -> bool {
    let Some(profile) = Profile::from_str_loose(profile_name) else {
        tracing::warn!("Unknown profile '{}', ignoring", profile_name);
        return false;
    };

    debug!("Applying config profile: {}", profile);

    match profile {
        Profile::Dev => {
            config.verbose = true;
            config.debug_logging = true;
        }
        Profile::Prod => {
            config.verbose = false;
            config.debug_logging = false;
        }
        Profile::Fast => {
            config.verbose = false;
            config.debug_logging = false;
            // Reduce token limits for faster responses
            if config.max_tokens > 4096 {
                config.max_tokens = 4096;
            }
            // Increase temperature slightly for faster generation
            config.temperature = 0.8;
        }
    }

    true
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_profile_from_str() {
        assert_eq!(Profile::from_str_loose("dev"), Some(Profile::Dev));
        assert_eq!(Profile::from_str_loose("DEV"), Some(Profile::Dev));
        assert_eq!(Profile::from_str_loose("development"), Some(Profile::Dev));
        assert_eq!(Profile::from_str_loose("prod"), Some(Profile::Prod));
        assert_eq!(Profile::from_str_loose("production"), Some(Profile::Prod));
        assert_eq!(Profile::from_str_loose("fast"), Some(Profile::Fast));
        assert_eq!(Profile::from_str_loose("quick"), Some(Profile::Fast));
        assert_eq!(Profile::from_str_loose("unknown"), None);
    }

    #[test]
    fn test_profile_roundtrip() {
        for p in Profile::all() {
            let s = p.as_str();
            let parsed = Profile::from_str_loose(s).unwrap();
            assert_eq!(*p, parsed);
        }
    }

    #[test]
    fn test_apply_dev_profile() {
        let mut config = AppConfig::default();
        config.verbose = false;
        config.debug_logging = false;

        let applied = apply_profile(&mut config, "dev");
        assert!(applied);
        assert!(config.verbose);
        assert!(config.debug_logging);
    }

    #[test]
    fn test_apply_prod_profile() {
        let mut config = AppConfig::default();
        config.verbose = true;
        config.debug_logging = true;

        let applied = apply_profile(&mut config, "prod");
        assert!(applied);
        assert!(!config.verbose);
        assert!(!config.debug_logging);
    }

    #[test]
    fn test_apply_fast_profile() {
        let mut config = AppConfig::default();
        config.max_tokens = 16384;
        config.temperature = 0.6;

        let applied = apply_profile(&mut config, "fast");
        assert!(applied);
        assert_eq!(config.max_tokens, 4096);
        assert!((config.temperature - 0.8).abs() < f64::EPSILON);
    }

    #[test]
    fn test_apply_fast_profile_preserves_small_max_tokens() {
        let mut config = AppConfig::default();
        config.max_tokens = 2048;

        apply_profile(&mut config, "fast");
        assert_eq!(config.max_tokens, 2048); // Not increased
    }

    #[test]
    fn test_apply_unknown_profile() {
        let mut config = AppConfig::default();
        let original = config.clone();

        let applied = apply_profile(&mut config, "nonexistent");
        assert!(!applied);
        // Config should be unchanged
        assert_eq!(config.verbose, original.verbose);
        assert_eq!(config.debug_logging, original.debug_logging);
    }
}
