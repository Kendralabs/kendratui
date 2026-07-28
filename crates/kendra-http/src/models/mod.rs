//! Shared types for HTTP operations.

mod retry;

pub use retry::{RetryConfig, classify_retryable_error, parse_retry_after};

/// Errors that can occur during HTTP operations.
#[derive(Debug, thiserror::Error)]
pub enum HttpError {
    #[error("HTTP request failed: {0}")]
    Request(#[from] reqwest::Error),

    #[error("Interrupted by user")]
    Interrupted,

    #[error("All retries exhausted: {message}")]
    RetriesExhausted { message: String },

    #[error("Authentication error: {0}")]
    Auth(String),

    #[error("IO error: {0}")]
    Io(#[from] std::io::Error),

    #[error("JSON error: {0}")]
    Json(#[from] serde_json::Error),

    #[error("{0}")]
    Other(String),
}

/// Result of an HTTP request attempt.
#[derive(Debug)]
pub struct HttpResult {
    /// Whether the request completed successfully.
    pub success: bool,
    /// HTTP status code, if a response was received.
    pub status: Option<u16>,
    /// Response body, if available.
    pub body: Option<serde_json::Value>,
    /// Error message, if the request failed.
    pub error: Option<String>,
    /// Whether the request was interrupted by the user.
    pub interrupted: bool,
    /// Whether the failure is transient and worth retrying.
    pub retryable: bool,
    /// Unique request identifier for end-to-end tracing.
    /// Propagated via the `X-Request-ID` header.
    pub request_id: Option<String>,
    /// Value of the `Retry-After` response header, if present.
    /// Used to honor server-requested retry delays on 429/503 responses.
    pub retry_after: Option<String>,
    /// Value of the `retry-after-ms` response header, if present.
    /// More precise than `Retry-After` (milliseconds instead of seconds).
    pub retry_after_ms: Option<String>,
}

impl HttpResult {
    /// Create a successful result.
    pub fn ok(status: u16, body: serde_json::Value) -> Self {
        Self {
            success: true,
            status: Some(status),
            body: Some(body),
            error: None,
            interrupted: false,
            retryable: false,
            request_id: None,
            retry_after: None,
            retry_after_ms: None,
        }
    }

    /// Create a failed result.
    pub fn fail(error: impl Into<String>, retryable: bool) -> Self {
        Self {
            success: false,
            status: None,
            body: None,
            error: Some(error.into()),
            interrupted: false,
            retryable,
            request_id: None,
            retry_after: None,
            retry_after_ms: None,
        }
    }

    /// Create an interrupted result.
    pub fn interrupted() -> Self {
        Self {
            success: false,
            status: None,
            body: None,
            error: Some("Interrupted by user".into()),
            interrupted: true,
            retryable: false,
            request_id: None,
            retry_after: None,
            retry_after_ms: None,
        }
    }

    /// Format an HTTP status code into a user-friendly error string.
    pub fn format_http_error(status: u16) -> String {
        match status {
            400 => "HTTP 400 (Bad Request): The LLM provider rejected the request. This often means the context is too large or parameters are invalid.".to_string(),
            401 => "HTTP 401 (Unauthorized): API key is missing or invalid. Please check your configuration.".to_string(),
            403 => "HTTP 403 (Forbidden): You do not have permission to access this model or endpoint.".to_string(),
            404 => "HTTP 404 (Not Found): The model endpoint does not exist. The provider may have changed their API.".to_string(),
            410 => "HTTP 410 (Gone): The selected model has been permanently removed or deprecated by the provider. Please select a different model.".to_string(),
            429 => "HTTP 429 (Too Many Requests): Rate limit exceeded. The provider is throttling your requests.".to_string(),
            500 => "HTTP 500 (Internal Server Error): The LLM provider experienced an internal error.".to_string(),
            502 => "HTTP 502 (Bad Gateway): The LLM provider's servers are down or unreachable.".to_string(),
            503 => "HTTP 503 (Service Unavailable): The LLM provider is overloaded or down for maintenance.".to_string(),
            504 => "HTTP 504 (Gateway Timeout): The LLM provider took too long to respond.".to_string(),
            _ => format!("HTTP {status}"),
        }
    }

    /// Create a result from an HTTP response with a retryable status.
    pub fn retryable_status(
        status: u16,
        body: Option<serde_json::Value>,
        retry_after: Option<String>,
    ) -> Self {
        Self {
            success: false,
            status: Some(status),
            body,
            error: Some(Self::format_http_error(status)),
            interrupted: false,
            retryable: true,
            request_id: None,
            retry_after,
            retry_after_ms: None,
        }
    }

    /// Attach a request ID to this result for tracing.
    pub fn with_request_id(mut self, request_id: impl Into<String>) -> Self {
        self.request_id = Some(request_id.into());
        self
    }
}

#[cfg(test)]
mod tests;
