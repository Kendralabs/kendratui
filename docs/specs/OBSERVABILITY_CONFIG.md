# Specification: Universal CLI Observability & Governance

This document defines a universal configuration schema and governance protocol for CLI observability, designed to be adopted by KendraCLI, Gemini CLI, Claude Code, and other CLI tools.

## 1. CLI-Agnostic Event Headers

All telemetry events exported MUST include the following metadata headers to enable cross-CLI analysis:

```json
{
  "header": {
    "cli_name": "kendra-cli",
    "cli_version": "0.1.0",
    "session_id": "uuid-v4",
    "user_id": "hashed-identifier",
    "timestamp": "ISO-8601"
  },
  "payload": { ... }
}
```

## 2. Configuration Schema (`observability.json`)

Tools should load this from a standard location (e.g., `~/.config/<cli-name>/observability.json`).

```json
{
  "enabled": true,
  "sinks": [
    {
      "type": "file",
      "path": "~/.config/<cli-name>/telemetry.ndjson"
    },
    {
      "type": "http",
      "enabled": false,
      "endpoint": "https://api.example.com/ingest",
      "auth_header_env": "TELEMETRY_API_KEY"
    }
  ],
  "governance": {
    "sanitize_pii": true,
    "redact_keys": ["api_key", "password", "token"]
  }
}
```

## 3. Universal Adoption Template (for other CLIs)

To adopt this observability standard, implement the following:

1.  **Event Bus:** Implement an internal event bus to capture all activities (LLM turns, tool calls, user input).
2.  **Telemetry Collector:** Subscribe to all events and wrap them with the mandatory `header`.
3.  **Governance Layer:** Apply sanitization logic before dispatching to sinks.
4.  **Exporter Interface:** Use the `TelemetryExporter` trait:

```rust
#[async_trait]
pub trait TelemetryExporter: Send + Sync {
    async fn export(&self, event: &RuntimeEvent) -> Result<(), Box<dyn std::error::Error + Send + Sync>>;
}
```
*(Language-agnostic version requires similar interface/callback structure.)*
