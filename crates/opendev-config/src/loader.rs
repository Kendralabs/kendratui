//! Hierarchical config loading.
//!
//! Priority: project settings > user settings > env vars > defaults.

use opendev_models::AppConfig;
use std::path::Path;
use thiserror::Error;
use tracing::{debug, warn};

#[derive(Debug, Error)]
pub enum ConfigError {
    #[error("failed to read config file {path}: {source}")]
    ReadError {
        path: String,
        source: std::io::Error,
    },
    #[error("failed to parse config file {path}: {source}")]
    ParseError {
        path: String,
        source: serde_json::Error,
    },
}

/// Loads and merges configuration from multiple sources.
pub struct ConfigLoader;

impl ConfigLoader {
    /// Load configuration with hierarchical merge.
    ///
    /// Priority: project settings > user settings > env vars > defaults.
    pub fn load(global_settings: &Path, project_settings: &Path) -> Result<AppConfig, ConfigError> {
        let mut config = AppConfig::default();

        // Load user-level settings
        if global_settings.exists() {
            match Self::load_file(global_settings) {
                Ok(user_config) => {
                    config = Self::merge(config, user_config);
                    debug!("Loaded global settings from {:?}", global_settings);
                }
                Err(e) => {
                    warn!("Failed to load global settings: {}", e);
                }
            }
        }

        // Load project-level settings (higher priority)
        if project_settings.exists() {
            match Self::load_file(project_settings) {
                Ok(project_config) => {
                    config = Self::merge(config, project_config);
                    debug!("Loaded project settings from {:?}", project_settings);
                }
                Err(e) => {
                    warn!("Failed to load project settings: {}", e);
                }
            }
        }

        // Apply environment variable overrides
        Self::apply_env_overrides(&mut config);

        Ok(config)
    }

    /// Load a config file as a partial JSON value.
    fn load_file(path: &Path) -> Result<serde_json::Value, ConfigError> {
        let content = std::fs::read_to_string(path).map_err(|e| ConfigError::ReadError {
            path: path.display().to_string(),
            source: e,
        })?;
        serde_json::from_str(&content).map_err(|e| ConfigError::ParseError {
            path: path.display().to_string(),
            source: e,
        })
    }

    /// Merge a partial JSON config onto an existing AppConfig.
    ///
    /// Only fields present in the override are applied.
    fn merge(base: AppConfig, overrides: serde_json::Value) -> AppConfig {
        let mut base_value = serde_json::to_value(&base).unwrap_or(serde_json::Value::Null);
        if let (Some(base_obj), Some(override_obj)) =
            (base_value.as_object_mut(), overrides.as_object())
        {
            for (key, value) in override_obj {
                base_obj.insert(key.clone(), value.clone());
            }
        }
        serde_json::from_value(base_value).unwrap_or(base)
    }

    /// Save configuration to a settings file.
    ///
    /// Writes the config as pretty-printed JSON. Uses atomic write
    /// (write to temp file then rename) to prevent corruption.
    pub fn save(config: &AppConfig, path: &Path) -> Result<(), ConfigError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| ConfigError::ReadError {
                path: parent.display().to_string(),
                source: e,
            })?;
        }

        let json = serde_json::to_string_pretty(config).map_err(|e| ConfigError::ParseError {
            path: path.display().to_string(),
            source: e,
        })?;

        // Atomic write: write to .tmp then rename
        let tmp_path = path.with_extension("json.tmp");
        std::fs::write(&tmp_path, &json).map_err(|e| ConfigError::ReadError {
            path: tmp_path.display().to_string(),
            source: e,
        })?;
        std::fs::rename(&tmp_path, path).map_err(|e| ConfigError::ReadError {
            path: path.display().to_string(),
            source: e,
        })?;

        debug!("Saved config to {:?}", path);
        Ok(())
    }

    /// Apply environment variable overrides.
    fn apply_env_overrides(config: &mut AppConfig) {
        if let Ok(provider) = std::env::var("OPENDEV_MODEL_PROVIDER") {
            config.model_provider = provider;
        }
        if let Ok(model) = std::env::var("OPENDEV_MODEL") {
            config.model = model;
        }
        if let Ok(val) = std::env::var("OPENDEV_MAX_TOKENS")
            && let Ok(max_tokens) = val.parse()
        {
            config.max_tokens = max_tokens;
        }
        if let Ok(val) = std::env::var("OPENDEV_TEMPERATURE")
            && let Ok(temp) = val.parse()
        {
            config.temperature = temp;
        }
        if let Ok(val) = std::env::var("OPENDEV_VERBOSE") {
            config.verbose = val == "1" || val.eq_ignore_ascii_case("true");
        }
        if let Ok(val) = std::env::var("OPENDEV_DEBUG") {
            config.debug_logging = val == "1" || val.eq_ignore_ascii_case("true");
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use tempfile::TempDir;

    #[test]
    fn test_load_defaults() {
        let tmp = TempDir::new().unwrap();
        let global = tmp.path().join("global.json");
        let project = tmp.path().join("project.json");

        let config = ConfigLoader::load(&global, &project).unwrap();
        assert_eq!(config.model_provider, "fireworks");
        assert_eq!(config.temperature, 0.6);
    }

    #[test]
    fn test_load_with_global_settings() {
        let tmp = TempDir::new().unwrap();
        let global = tmp.path().join("global.json");
        let project = tmp.path().join("project.json");

        std::fs::write(&global, r#"{"model_provider": "openai", "model": "gpt-4"}"#).unwrap();

        let config = ConfigLoader::load(&global, &project).unwrap();
        assert_eq!(config.model_provider, "openai");
        assert_eq!(config.model, "gpt-4");
        // Defaults preserved for unset fields
        assert_eq!(config.temperature, 0.6);
    }

    #[test]
    fn test_project_overrides_global() {
        let tmp = TempDir::new().unwrap();
        let global = tmp.path().join("global.json");
        let project = tmp.path().join("project.json");

        std::fs::write(&global, r#"{"model_provider": "openai", "model": "gpt-4"}"#).unwrap();
        std::fs::write(&project, r#"{"model": "gpt-4-turbo"}"#).unwrap();

        let config = ConfigLoader::load(&global, &project).unwrap();
        assert_eq!(config.model_provider, "openai"); // from global
        assert_eq!(config.model, "gpt-4-turbo"); // overridden by project
    }

    #[test]
    fn test_merge_preserves_defaults() {
        let base = AppConfig::default();
        let overrides = serde_json::json!({"verbose": true});
        let merged = ConfigLoader::merge(base, overrides);
        assert!(merged.verbose);
        assert_eq!(merged.temperature, 0.6); // default preserved
    }

    #[test]
    fn test_save_and_reload() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");

        let mut config = AppConfig::default();
        config.model_provider = "anthropic".to_string();
        config.model = "claude-3-opus".to_string();
        config.verbose = true;

        ConfigLoader::save(&config, &path).unwrap();

        // Reload
        let loaded = ConfigLoader::load(&path, &tmp.path().join("nonexistent.json")).unwrap();
        assert_eq!(loaded.model_provider, "anthropic");
        assert_eq!(loaded.model, "claude-3-opus");
        assert!(loaded.verbose);
        assert_eq!(loaded.temperature, 0.6); // default preserved
    }

    #[test]
    fn test_save_creates_parent_dirs() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("nested").join("dir").join("settings.json");

        let config = AppConfig::default();
        ConfigLoader::save(&config, &path).unwrap();

        assert!(path.exists());
    }

    #[test]
    fn test_save_atomic_no_corruption() {
        let tmp = TempDir::new().unwrap();
        let path = tmp.path().join("settings.json");

        // Save twice; second write should not leave tmp file
        let config = AppConfig::default();
        ConfigLoader::save(&config, &path).unwrap();
        ConfigLoader::save(&config, &path).unwrap();

        assert!(path.exists());
        assert!(!tmp.path().join("settings.json.tmp").exists());
    }
}
