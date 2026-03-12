//! Structured error types for OpenDev.
//!
//! Provides typed error classes with structured fields for better retry logic,
//! error-specific recovery, and comprehensive provider error classification.
//! Ported from `opendev/core/errors.py`.

use regex::Regex;
use serde::{Deserialize, Serialize};
use std::sync::LazyLock;

/// High-level error category for classification.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ErrorCategory {
    ContextOverflow,
    OutputLength,
    RateLimit,
    Auth,
    Api,
    Gateway,
    Permission,
    EditMismatch,
    FileNotFound,
    Timeout,
    Unknown,
}

/// Base structured error with metadata.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuredError {
    pub category: ErrorCategory,
    pub message: String,
    pub is_retryable: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub status_code: Option<u16>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub original_error: Option<String>,
    /// For context overflow: how many tokens were in the prompt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_count: Option<u64>,
    /// For context overflow: what the model limit is.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub token_limit: Option<u64>,
    /// For rate limit: seconds to wait before retrying.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub retry_after: Option<f64>,
}

impl StructuredError {
    /// Whether this error should trigger context compaction.
    pub fn should_compact(&self) -> bool {
        self.category == ErrorCategory::ContextOverflow
    }

    /// Whether the operation should be retried.
    pub fn should_retry(&self) -> bool {
        self.is_retryable
    }

    /// Create a generic API error.
    pub fn api(message: impl Into<String>, status_code: Option<u16>) -> Self {
        let code = status_code;
        Self {
            category: if code.is_some() {
                ErrorCategory::Api
            } else {
                ErrorCategory::Unknown
            },
            message: message.into(),
            is_retryable: matches!(code, Some(500 | 502 | 503 | 504)),
            status_code: code,
            provider: None,
            original_error: None,
            token_count: None,
            token_limit: None,
            retry_after: None,
        }
    }

    /// Create a context overflow error.
    pub fn context_overflow(
        message: impl Into<String>,
        provider: Option<String>,
        token_count: Option<u64>,
        token_limit: Option<u64>,
    ) -> Self {
        let msg = message.into();
        Self {
            category: ErrorCategory::ContextOverflow,
            message: msg.clone(),
            is_retryable: true,
            status_code: None,
            provider,
            original_error: Some(msg),
            token_count,
            token_limit,
            retry_after: None,
        }
    }

    /// Create an output length error.
    pub fn output_length(message: impl Into<String>) -> Self {
        Self {
            category: ErrorCategory::OutputLength,
            message: message.into(),
            is_retryable: true,
            status_code: None,
            provider: None,
            original_error: None,
            token_count: None,
            token_limit: None,
            retry_after: None,
        }
    }

    /// Create a rate limit error.
    pub fn rate_limit(
        message: impl Into<String>,
        provider: Option<String>,
        retry_after: Option<f64>,
    ) -> Self {
        let msg = message.into();
        Self {
            category: ErrorCategory::RateLimit,
            message: msg.clone(),
            is_retryable: true,
            status_code: None,
            provider,
            original_error: Some(msg),
            token_count: None,
            token_limit: None,
            retry_after,
        }
    }

    /// Create an authentication error.
    pub fn auth(
        message: impl Into<String>,
        status_code: Option<u16>,
        provider: Option<String>,
    ) -> Self {
        let msg = message.into();
        Self {
            category: ErrorCategory::Auth,
            message: msg.clone(),
            is_retryable: false,
            status_code,
            provider,
            original_error: Some(msg),
            token_count: None,
            token_limit: None,
            retry_after: None,
        }
    }

    /// Create a gateway error.
    pub fn gateway(
        message: impl Into<String>,
        status_code: Option<u16>,
        provider: Option<String>,
        original_error: Option<String>,
    ) -> Self {
        Self {
            category: ErrorCategory::Gateway,
            message: message.into(),
            is_retryable: true,
            status_code,
            provider,
            original_error,
            token_count: None,
            token_limit: None,
            retry_after: None,
        }
    }
}

impl std::fmt::Display for StructuredError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "[{:?}] {}", self.category, self.message)
    }
}

impl std::error::Error for StructuredError {}

// ---------------------------------------------------------------------------
// Provider error pattern library
// ---------------------------------------------------------------------------

/// Compiled regex patterns for each error category.
struct PatternSet {
    overflow: Vec<Regex>,
    rate_limit: Vec<Regex>,
    auth: Vec<Regex>,
    gateway: Vec<Regex>,
}

fn compile_patterns(patterns: &[&str]) -> Vec<Regex> {
    patterns
        .iter()
        .filter_map(|p| Regex::new(&format!("(?i){}", p)).ok())
        .collect()
}

static PATTERNS: LazyLock<PatternSet> = LazyLock::new(|| {
    PatternSet {
        overflow: compile_patterns(&[
            // Anthropic
            r"prompt is too long",
            r"max_tokens_exceeded",
            r"context length.*exceeded",
            r"maximum context length",
            // OpenAI
            r"maximum context length.*is \d+ tokens",
            r"This model's maximum context length is",
            r"reduce the length of the messages",
            r"context_length_exceeded",
            // Google
            r"exceeds the maximum.*tokens",
            r"RESOURCE_EXHAUSTED.*token",
            r"GenerateContentRequest.*too large",
            // Azure
            r"Tokens in prompt.*exceed.*limit",
            // Generic
            r"token limit",
            r"too many tokens",
            r"context.*too long",
            r"input.*too long",
            r"prompt.*too large",
        ]),
        rate_limit: compile_patterns(&[
            r"rate.?limit",
            r"too many requests",
            r"429",
            r"quota exceeded",
            r"capacity",
            r"overloaded",
        ]),
        auth: compile_patterns(&[
            r"invalid.*api.?key",
            r"authentication",
            r"unauthorized",
            r"invalid.*token",
            r"api key.*invalid",
        ]),
        gateway: compile_patterns(&[
            r"<!doctype html",
            r"<html",
            r"502 Bad Gateway",
            r"503 Service Unavailable",
            r"504 Gateway Timeout",
            r"CloudFlare",
            r"nginx",
        ]),
    }
});

/// Classify an API error into a structured error type.
///
/// Checks the raw error message against known patterns for context overflow,
/// rate limiting, authentication failures, and gateway/proxy issues across
/// all supported providers (Anthropic, OpenAI, Google, Azure).
pub fn classify_api_error(
    error_message: &str,
    status_code: Option<u16>,
    provider: Option<&str>,
) -> StructuredError {
    let patterns = &*PATTERNS;
    let provider_owned = provider.map(|s| s.to_string());

    // Check gateway patterns first (HTML responses)
    for re in &patterns.gateway {
        if re.is_match(error_message) {
            let friendly_msg = match status_code {
                Some(401) => {
                    "Authentication failed at gateway. Check your API key and proxy settings."
                        .to_string()
                }
                Some(403) => {
                    "Access denied at gateway. Check your permissions and proxy settings."
                        .to_string()
                }
                _ => "API returned an HTML error page. Check your proxy/VPN settings or try again."
                    .to_string(),
            };
            let truncated = if error_message.len() > 500 {
                &error_message[..500]
            } else {
                error_message
            };
            return StructuredError::gateway(
                friendly_msg,
                status_code,
                provider_owned,
                Some(truncated.to_string()),
            );
        }
    }

    // Context overflow
    for re in &patterns.overflow {
        if re.is_match(error_message) {
            return StructuredError::context_overflow(error_message, provider_owned, None, None);
        }
    }

    // Rate limiting
    for re in &patterns.rate_limit {
        if re.is_match(error_message) {
            let retry_after = Regex::new(r"(?i)retry.?after[:\s]+(\d+\.?\d*)")
                .ok()
                .and_then(|ra_re| ra_re.captures(error_message))
                .and_then(|caps| caps.get(1))
                .and_then(|m| m.as_str().parse::<f64>().ok());
            return StructuredError::rate_limit(error_message, provider_owned, retry_after);
        }
    }

    // Auth errors — check status code first, then patterns
    if matches!(status_code, Some(401 | 403)) {
        return StructuredError::auth(error_message, status_code, provider_owned);
    }
    for re in &patterns.auth {
        if re.is_match(error_message) {
            return StructuredError::auth(error_message, status_code, provider_owned);
        }
    }

    // Generic API error
    StructuredError {
        category: if status_code.is_some() {
            ErrorCategory::Api
        } else {
            ErrorCategory::Unknown
        },
        message: error_message.to_string(),
        is_retryable: matches!(status_code, Some(500 | 502 | 503 | 504)),
        status_code,
        provider: provider_owned,
        original_error: Some(error_message.to_string()),
        token_count: None,
        token_limit: None,
        retry_after: None,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_classify_context_overflow_anthropic() {
        let err = classify_api_error("prompt is too long: 250000 tokens", None, Some("anthropic"));
        assert_eq!(err.category, ErrorCategory::ContextOverflow);
        assert!(err.is_retryable);
        assert!(err.should_compact());
    }

    #[test]
    fn test_classify_context_overflow_openai() {
        let err = classify_api_error(
            "This model's maximum context length is 128000 tokens",
            None,
            Some("openai"),
        );
        assert_eq!(err.category, ErrorCategory::ContextOverflow);
        assert!(err.is_retryable);
    }

    #[test]
    fn test_classify_context_overflow_google() {
        let err = classify_api_error(
            "GenerateContentRequest is too large",
            None,
            Some("google"),
        );
        assert_eq!(err.category, ErrorCategory::ContextOverflow);
    }

    #[test]
    fn test_classify_rate_limit() {
        let err = classify_api_error("Rate limit exceeded", Some(429), Some("openai"));
        assert_eq!(err.category, ErrorCategory::RateLimit);
        assert!(err.is_retryable);
    }

    #[test]
    fn test_classify_rate_limit_with_retry_after() {
        let err = classify_api_error(
            "Too many requests. Retry-After: 30",
            Some(429),
            Some("anthropic"),
        );
        assert_eq!(err.category, ErrorCategory::RateLimit);
        assert_eq!(err.retry_after, Some(30.0));
    }

    #[test]
    fn test_classify_auth_by_status_code() {
        let err = classify_api_error("Forbidden", Some(401), None);
        assert_eq!(err.category, ErrorCategory::Auth);
        assert!(!err.is_retryable);
    }

    #[test]
    fn test_classify_auth_by_pattern() {
        let err = classify_api_error("Invalid API key provided", Some(400), Some("openai"));
        assert_eq!(err.category, ErrorCategory::Auth);
        assert!(!err.is_retryable);
    }

    #[test]
    fn test_classify_gateway_html() {
        let err = classify_api_error(
            "<!doctype html><html>502 Bad Gateway</html>",
            Some(502),
            None,
        );
        assert_eq!(err.category, ErrorCategory::Gateway);
        assert!(err.is_retryable);
        assert!(err.original_error.is_some());
    }

    #[test]
    fn test_classify_gateway_401_html() {
        let err = classify_api_error("<html>Unauthorized</html>", Some(401), None);
        assert_eq!(err.category, ErrorCategory::Gateway);
        assert!(err
            .message
            .contains("Authentication failed at gateway"));
    }

    #[test]
    fn test_classify_generic_500() {
        let err = classify_api_error("Internal server error", Some(500), None);
        assert_eq!(err.category, ErrorCategory::Api);
        assert!(err.is_retryable);
    }

    #[test]
    fn test_classify_unknown() {
        let err = classify_api_error("Something went wrong", None, None);
        assert_eq!(err.category, ErrorCategory::Unknown);
        assert!(!err.is_retryable);
    }

    #[test]
    fn test_structured_error_display() {
        let err = StructuredError::api("test error", Some(500));
        let display = format!("{}", err);
        assert!(display.contains("Api"));
        assert!(display.contains("test error"));
    }

    #[test]
    fn test_structured_error_serialization() {
        let err = StructuredError::context_overflow(
            "too many tokens",
            Some("anthropic".to_string()),
            Some(200000),
            Some(128000),
        );
        let json = serde_json::to_string(&err).unwrap();
        let deserialized: StructuredError = serde_json::from_str(&json).unwrap();
        assert_eq!(deserialized.category, ErrorCategory::ContextOverflow);
        assert_eq!(deserialized.token_count, Some(200000));
        assert_eq!(deserialized.token_limit, Some(128000));
    }
}
