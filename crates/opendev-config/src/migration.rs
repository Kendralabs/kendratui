//! Config version migration support.
//!
//! Adds a `config_version` field to config files and applies migration
//! functions when loading configs with older versions.

use tracing::{debug, info};

/// Current config version. Increment this when adding migrations.
pub const CURRENT_CONFIG_VERSION: u32 = 1;

/// Key used to store the config version in JSON.
pub const VERSION_KEY: &str = "config_version";

/// Migrate a JSON config value from its stored version to the current version.
///
/// Returns the migrated JSON value and whether any migrations were applied.
pub fn migrate_config(mut value: serde_json::Value) -> (serde_json::Value, bool) {
    let stored_version = value
        .as_object()
        .and_then(|obj| obj.get(VERSION_KEY))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(0);

    if stored_version >= CURRENT_CONFIG_VERSION {
        // Already at current version, just ensure version field is set
        if let Some(obj) = value.as_object_mut() {
            obj.insert(
                VERSION_KEY.to_string(),
                serde_json::Value::Number(CURRENT_CONFIG_VERSION.into()),
            );
        }
        return (value, false);
    }

    let mut version = stored_version;
    let mut migrated = false;

    // Apply migrations in order
    while version < CURRENT_CONFIG_VERSION {
        match version {
            0 => {
                // Migration from version 0 (no version field) to version 1:
                // - Add config_version field
                // - No structural changes needed for v1; this just stamps the version.
                value = migrate_v0_to_v1(value);
                version = 1;
                migrated = true;
                info!("Migrated config from v0 to v1");
            }
            _ => {
                // Unknown version — skip remaining migrations
                debug!(
                    "Unknown config version {}, skipping further migrations",
                    version
                );
                break;
            }
        }
    }

    (value, migrated)
}

/// Migrate from version 0 (unversioned) to version 1.
///
/// Version 1 simply stamps the config_version field. Future migrations
/// can add structural changes here.
fn migrate_v0_to_v1(mut value: serde_json::Value) -> serde_json::Value {
    if let Some(obj) = value.as_object_mut() {
        obj.insert(VERSION_KEY.to_string(), serde_json::Value::Number(1.into()));
    }
    value
}

/// Check whether a config value needs migration.
pub fn needs_migration(value: &serde_json::Value) -> bool {
    let stored_version = value
        .as_object()
        .and_then(|obj| obj.get(VERSION_KEY))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(0);

    stored_version < CURRENT_CONFIG_VERSION
}

/// Get the version of a config value.
pub fn config_version(value: &serde_json::Value) -> u32 {
    value
        .as_object()
        .and_then(|obj| obj.get(VERSION_KEY))
        .and_then(|v| v.as_u64())
        .map(|v| v as u32)
        .unwrap_or(0)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_current_version() {
        assert!(CURRENT_CONFIG_VERSION >= 1);
    }

    #[test]
    fn test_migrate_unversioned_config() {
        let value = serde_json::json!({
            "model_provider": "openai",
            "model": "gpt-4"
        });

        assert!(needs_migration(&value));
        assert_eq!(config_version(&value), 0);

        let (migrated, changed) = migrate_config(value);
        assert!(changed);
        assert_eq!(config_version(&migrated), CURRENT_CONFIG_VERSION);

        // Original fields preserved
        assert_eq!(migrated["model_provider"], "openai");
        assert_eq!(migrated["model"], "gpt-4");
    }

    #[test]
    fn test_migrate_already_current() {
        let value = serde_json::json!({
            "config_version": CURRENT_CONFIG_VERSION,
            "model_provider": "anthropic",
            "model": "claude-3-opus"
        });

        assert!(!needs_migration(&value));

        let (migrated, changed) = migrate_config(value);
        assert!(!changed);
        assert_eq!(config_version(&migrated), CURRENT_CONFIG_VERSION);
    }

    #[test]
    fn test_migrate_v0_to_v1() {
        let value = serde_json::json!({
            "model_provider": "fireworks",
            "temperature": 0.7,
            "verbose": true
        });

        let (migrated, changed) = migrate_config(value);
        assert!(changed);
        assert_eq!(config_version(&migrated), 1);
        assert_eq!(migrated["model_provider"], "fireworks");
        assert_eq!(migrated["temperature"], 0.7);
        assert_eq!(migrated["verbose"], true);
    }

    #[test]
    fn test_needs_migration_empty_object() {
        let value = serde_json::json!({});
        assert!(needs_migration(&value));
    }

    #[test]
    fn test_config_version_missing() {
        let value = serde_json::json!({"model": "gpt-4"});
        assert_eq!(config_version(&value), 0);
    }

    #[test]
    fn test_config_version_present() {
        let value = serde_json::json!({"config_version": 1});
        assert_eq!(config_version(&value), 1);
    }

    #[test]
    fn test_migrate_preserves_all_fields() {
        let value = serde_json::json!({
            "model_provider": "openai",
            "model": "gpt-4",
            "api_key": "sk-test",
            "max_tokens": 8192,
            "temperature": 0.5,
            "verbose": true,
            "debug_logging": false,
            "custom_field": "should_survive"
        });

        let (migrated, _) = migrate_config(value);
        assert_eq!(migrated["model_provider"], "openai");
        assert_eq!(migrated["api_key"], "sk-test");
        assert_eq!(migrated["max_tokens"], 8192);
        assert_eq!(migrated["custom_field"], "should_survive");
    }
}
