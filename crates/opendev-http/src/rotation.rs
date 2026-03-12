//! API key rotation and failover across providers.
//!
//! Supports multiple API keys per provider with automatic rotation on rate
//! limits (429) and auth failures (401/402). Keys cool down after failures
//! before being retried.

use std::collections::HashMap;
use std::time::Instant;
use tracing::{info, warn};

/// Cooldown durations in seconds by HTTP status code.
fn cooldown_seconds(status: u16) -> f64 {
    match status {
        429 => 30.0,        // Rate limit
        401 => 300.0,       // Unauthorized
        402 => 300.0,       // Payment required
        403 => 600.0,       // Forbidden
        500 => 60.0,        // Server error
        502 => 30.0,        // Bad gateway
        503 => 60.0,        // Service unavailable
        _ => 60.0,          // Default
    }
}

/// A single API key with usage tracking.
#[derive(Debug)]
struct AuthProfile {
    key: String,
    #[allow(dead_code)]
    provider: String,
    failed_at: Option<Instant>,
    failure_status: u16,
    cooldown_until: Option<Instant>,
    request_count: u64,
    failure_count: u64,
}

impl AuthProfile {
    fn new(key: String, provider: String) -> Self {
        Self {
            key,
            provider,
            failed_at: None,
            failure_status: 0,
            cooldown_until: None,
            request_count: 0,
            failure_count: 0,
        }
    }

    fn is_available(&self) -> bool {
        match self.cooldown_until {
            None => true,
            Some(until) => Instant::now() >= until,
        }
    }

    fn mark_success(&mut self) {
        self.request_count += 1;
        self.failed_at = None;
        self.failure_status = 0;
        self.cooldown_until = None;
    }

    fn mark_failure(&mut self, status_code: u16) {
        self.failure_count += 1;
        let now = Instant::now();
        self.failed_at = Some(now);
        self.failure_status = status_code;
        let cooldown = cooldown_seconds(status_code);
        self.cooldown_until = Some(now + std::time::Duration::from_secs_f64(cooldown));
        warn!(
            key_prefix = &self.key[..self.key.len().min(8)],
            status_code,
            cooldown_secs = cooldown,
            "Auth profile failed, cooling down"
        );
    }

    /// Remaining cooldown in seconds (0.0 if available).
    fn cooldown_remaining(&self) -> f64 {
        match self.cooldown_until {
            None => 0.0,
            Some(until) => {
                let now = Instant::now();
                if now >= until {
                    0.0
                } else {
                    (until - now).as_secs_f64()
                }
            }
        }
    }
}

/// Manages multiple API keys per provider with rotation and failover.
///
/// Keys rotate automatically when the active key fails. Failed keys enter
/// a cooldown period before being retried.
pub struct AuthProfileManager {
    provider: String,
    profiles: Vec<AuthProfile>,
    current_index: usize,
}

impl AuthProfileManager {
    /// Create a new manager with the given keys.
    pub fn new(provider: impl Into<String>, keys: Vec<String>) -> Self {
        let provider = provider.into();
        let profiles: Vec<_> = keys
            .into_iter()
            .filter(|k| !k.is_empty())
            .map(|k| AuthProfile::new(k, provider.clone()))
            .collect();

        if profiles.is_empty() {
            warn!("No API keys configured for provider '{}'", provider);
        }

        Self {
            provider,
            profiles,
            current_index: 0,
        }
    }

    /// Create from environment variables.
    ///
    /// Looks for `{PROVIDER}_API_KEY`, `{PROVIDER}_API_KEY_2`, etc.
    pub fn from_env(provider: &str) -> Self {
        let prefix = provider.to_uppercase().replace('-', "_");
        let mut keys = Vec::new();

        // Primary key
        if let Ok(val) = std::env::var(format!("{prefix}_API_KEY")) {
            if !val.is_empty() {
                keys.push(val);
            }
        }

        // Additional keys: _2, _3, ...
        for i in 2..10 {
            match std::env::var(format!("{prefix}_API_KEY_{i}")) {
                Ok(val) if !val.is_empty() => keys.push(val),
                _ => break,
            }
        }

        Self::new(provider, keys)
    }

    /// Create from a configuration map.
    ///
    /// Accepts `{"api_keys": [...]}` or `{"api_key": "..."}`.
    pub fn from_config(provider: &str, config: &HashMap<String, serde_json::Value>) -> Self {
        let keys = if let Some(serde_json::Value::Array(arr)) = config.get("api_keys") {
            arr.iter()
                .filter_map(|v| v.as_str().map(String::from))
                .collect()
        } else if let Some(serde_json::Value::String(single)) = config.get("api_key") {
            vec![single.clone()]
        } else {
            vec![]
        };
        Self::new(provider, keys)
    }

    /// Get the current active API key, rotating if needed.
    ///
    /// Returns `None` if all keys are in cooldown.
    pub fn get_active_key(&mut self) -> Option<&str> {
        if self.profiles.is_empty() {
            return None;
        }

        // Try current profile first
        if self.profiles[self.current_index].is_available() {
            return Some(&self.profiles[self.current_index].key);
        }

        // Rotate through other profiles
        let len = self.profiles.len();
        for i in 1..len {
            let idx = (self.current_index + i) % len;
            if self.profiles[idx].is_available() {
                self.current_index = idx;
                let profile = &self.profiles[idx];
                info!(
                    key_prefix = &profile.key[..profile.key.len().min(8)],
                    provider = %self.provider,
                    "Rotated to next API key"
                );
                return Some(&self.profiles[idx].key);
            }
        }

        // All keys in cooldown
        let soonest = self
            .profiles
            .iter()
            .map(|p| p.cooldown_remaining())
            .fold(f64::MAX, f64::min);
        warn!(
            total = self.profiles.len(),
            provider = %self.provider,
            soonest_available_secs = soonest,
            "All API keys are in cooldown"
        );
        None
    }

    /// Mark the current key as successful.
    pub fn mark_success(&mut self) {
        if !self.profiles.is_empty() {
            self.profiles[self.current_index].mark_success();
        }
    }

    /// Mark the current key as failed with a specific HTTP status code.
    pub fn mark_failure(&mut self, status_code: u16) {
        if !self.profiles.is_empty() {
            self.profiles[self.current_index].mark_failure(status_code);
        }
    }

    /// Number of configured profiles.
    pub fn profile_count(&self) -> usize {
        self.profiles.len()
    }

    /// Number of currently available profiles.
    pub fn available_count(&self) -> usize {
        self.profiles.iter().filter(|p| p.is_available()).count()
    }

    /// Get the provider name.
    pub fn provider(&self) -> &str {
        &self.provider
    }
}

impl std::fmt::Debug for AuthProfileManager {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AuthProfileManager")
            .field("provider", &self.provider)
            .field("profile_count", &self.profiles.len())
            .field("current_index", &self.current_index)
            .finish()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_new_empty() {
        let mgr = AuthProfileManager::new("openai", vec![]);
        assert_eq!(mgr.profile_count(), 0);
        assert_eq!(mgr.available_count(), 0);
    }

    #[test]
    fn test_new_filters_empty_keys() {
        let mgr = AuthProfileManager::new(
            "openai",
            vec!["key1".into(), "".into(), "key2".into()],
        );
        assert_eq!(mgr.profile_count(), 2);
    }

    #[test]
    fn test_get_active_key() {
        let mut mgr = AuthProfileManager::new("openai", vec!["sk-test".into()]);
        assert_eq!(mgr.get_active_key(), Some("sk-test"));
    }

    #[test]
    fn test_get_active_key_empty() {
        let mut mgr = AuthProfileManager::new("openai", vec![]);
        assert_eq!(mgr.get_active_key(), None);
    }

    #[test]
    fn test_mark_success() {
        let mut mgr = AuthProfileManager::new("openai", vec!["sk-test".into()]);
        mgr.mark_success();
        assert_eq!(mgr.profiles[0].request_count, 1);
        assert_eq!(mgr.profiles[0].failure_count, 0);
    }

    #[test]
    fn test_mark_failure_and_rotate() {
        let mut mgr = AuthProfileManager::new(
            "openai",
            vec!["key-a".into(), "key-b".into()],
        );
        assert_eq!(mgr.get_active_key(), Some("key-a"));

        // Fail key-a, should rotate to key-b
        mgr.mark_failure(429);
        assert_eq!(mgr.get_active_key(), Some("key-b"));
    }

    #[test]
    fn test_all_keys_in_cooldown() {
        let mut mgr = AuthProfileManager::new(
            "openai",
            vec!["key-a".into(), "key-b".into()],
        );
        mgr.mark_failure(429);
        mgr.current_index = 1;
        mgr.mark_failure(429);

        assert_eq!(mgr.get_active_key(), None);
        assert_eq!(mgr.available_count(), 0);
    }

    #[test]
    fn test_success_resets_cooldown() {
        let mut mgr = AuthProfileManager::new("openai", vec!["sk-test".into()]);
        mgr.mark_failure(429);
        assert!(!mgr.profiles[0].is_available());

        mgr.mark_success();
        assert!(mgr.profiles[0].is_available());
        assert_eq!(mgr.available_count(), 1);
    }

    #[test]
    fn test_from_config_api_keys() {
        let mut config = HashMap::new();
        config.insert(
            "api_keys".into(),
            serde_json::json!(["key1", "key2"]),
        );
        let mgr = AuthProfileManager::from_config("openai", &config);
        assert_eq!(mgr.profile_count(), 2);
    }

    #[test]
    fn test_from_config_single_key() {
        let mut config = HashMap::new();
        config.insert("api_key".into(), serde_json::json!("single-key"));
        let mgr = AuthProfileManager::from_config("openai", &config);
        assert_eq!(mgr.profile_count(), 1);
        assert_eq!(mgr.provider(), "openai");
    }

    #[test]
    fn test_from_config_empty() {
        let config = HashMap::new();
        let mgr = AuthProfileManager::from_config("openai", &config);
        assert_eq!(mgr.profile_count(), 0);
    }

    #[test]
    fn test_cooldown_seconds() {
        assert_eq!(cooldown_seconds(429), 30.0);
        assert_eq!(cooldown_seconds(401), 300.0);
        assert_eq!(cooldown_seconds(403), 600.0);
        assert_eq!(cooldown_seconds(999), 60.0); // default
    }

    #[test]
    fn test_debug_format() {
        let mgr = AuthProfileManager::new("anthropic", vec!["key".into()]);
        let debug = format!("{:?}", mgr);
        assert!(debug.contains("anthropic"));
        assert!(debug.contains("profile_count"));
    }
}
