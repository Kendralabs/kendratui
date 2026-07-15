use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ObservabilityConfig {
    pub enabled: bool,
    pub sinks: Vec<SinkConfig>,
    pub governance: GovernanceConfig,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum SinkConfig {
    File {
        path: String,
    },
    Http {
        enabled: bool,
        endpoint: String,
        auth_header_env: String,
    },
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct GovernanceConfig {
    pub sanitize_pii: bool,
    pub redact_keys: Vec<String>,
}
