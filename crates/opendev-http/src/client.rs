//! HTTP client with retry logic and cancellation support.

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderValue};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::warn;

use crate::models::{HttpError, HttpResult, RetryConfig};

/// Timeout configuration for HTTP requests.
#[derive(Debug, Clone)]
pub struct TimeoutConfig {
    pub connect: Duration,
    pub read: Duration,
    pub write: Duration,
}

impl Default for TimeoutConfig {
    fn default() -> Self {
        Self {
            connect: Duration::from_secs(10),
            read: Duration::from_secs(300),
            write: Duration::from_secs(10),
        }
    }
}

/// Async HTTP client with retry and cancellation support.
///
/// Wraps reqwest with:
/// - Exponential backoff retries on 429/503
/// - Respect for `Retry-After` headers
/// - Cancellation via `CancellationToken` (checked between retries and via `tokio::select!`)
pub struct HttpClient {
    client: reqwest::Client,
    api_url: String,
    retry_config: RetryConfig,
}

impl HttpClient {
    /// Create a new HTTP client.
    pub fn new(
        api_url: impl Into<String>,
        headers: HeaderMap,
        timeout: Option<TimeoutConfig>,
    ) -> Result<Self, HttpError> {
        let timeout = timeout.unwrap_or_default();
        let client = reqwest::Client::builder()
            .default_headers(headers)
            .connect_timeout(timeout.connect)
            .timeout(timeout.read)
            .build()?;

        Ok(Self {
            client,
            api_url: api_url.into(),
            retry_config: RetryConfig::default(),
        })
    }

    /// Create a client with custom retry configuration.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// POST JSON with retry logic and optional cancellation.
    ///
    /// On 429/503 responses, retries with exponential backoff. Respects
    /// `Retry-After` headers. Checks the cancellation token between attempts
    /// and races it against each request via `tokio::select!`.
    pub async fn post_json(
        &self,
        payload: &serde_json::Value,
        cancel: Option<&CancellationToken>,
    ) -> Result<HttpResult, HttpError> {
        let mut last_result: Option<HttpResult> = None;

        for attempt in 0..=self.retry_config.max_retries {
            // Check cancellation before each attempt
            if let Some(token) = cancel
                && token.is_cancelled()
            {
                return Ok(HttpResult::interrupted());
            }

            let result = self.execute_request(payload, cancel).await;

            match result {
                Ok(hr) if hr.success => {
                    // Check if status is retryable (429/503 with a body)
                    if let Some(status) = hr.status
                        && self.retry_config.is_retryable_status(status)
                    {
                        let delay = self.get_retry_delay(None, attempt);
                        last_result = Some(hr);
                        if attempt < self.retry_config.max_retries {
                            warn!(
                                status,
                                attempt = attempt + 1,
                                max = self.retry_config.max_retries,
                                "Retryable HTTP status, backing off for {:.1}s",
                                delay.as_secs_f64()
                            );
                            self.interruptible_sleep(delay, cancel).await?;
                            continue;
                        }
                        warn!(
                            status,
                            "Exhausted {} retries", self.retry_config.max_retries
                        );
                        return Ok(last_result.unwrap());
                    }
                    return Ok(hr);
                }
                Ok(hr) if hr.retryable => {
                    last_result = Some(hr);
                    if attempt < self.retry_config.max_retries {
                        let delay = self.retry_config.delay_for_attempt(attempt);
                        warn!(
                            error = last_result.as_ref().and_then(|r| r.error.as_deref()),
                            attempt = attempt + 1,
                            max = self.retry_config.max_retries,
                            "Retryable error, backing off for {:.1}s",
                            delay.as_secs_f64()
                        );
                        self.interruptible_sleep(delay, cancel).await?;
                        continue;
                    }
                    warn!("Exhausted {} retries", self.retry_config.max_retries);
                    return Ok(last_result.unwrap());
                }
                Ok(hr) => return Ok(hr),
                Err(e) => return Err(e),
            }
        }

        Ok(last_result.unwrap_or_else(|| HttpResult::fail("Unexpected retry exhaustion", false)))
    }

    /// Execute a single POST request, racing against cancellation.
    async fn execute_request(
        &self,
        payload: &serde_json::Value,
        cancel: Option<&CancellationToken>,
    ) -> Result<HttpResult, HttpError> {
        let request = self
            .client
            .post(&self.api_url)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .json(payload)
            .send();

        let response = match cancel {
            Some(token) => {
                tokio::select! {
                    resp = request => resp,
                    _ = token.cancelled() => {
                        return Ok(HttpResult::interrupted());
                    }
                }
            }
            None => request.await,
        };

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                if self.retry_config.is_retryable_status(status) {
                    // Parse Retry-After for the caller's retry logic
                    let body = resp.json::<serde_json::Value>().await.ok();
                    return Ok(HttpResult::retryable_status(status, body));
                }
                let body = resp.json::<serde_json::Value>().await?;
                if status >= 400 {
                    let error_msg = body
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format!("HTTP {status}"));
                    return Ok(HttpResult {
                        success: false,
                        status: Some(status),
                        body: Some(body),
                        error: Some(error_msg),
                        interrupted: false,
                        retryable: false,
                    });
                }
                Ok(HttpResult::ok(status, body))
            }
            Err(e) if is_retryable_error(&e) => Ok(HttpResult::fail(e.to_string(), true)),
            Err(e) => Ok(HttpResult::fail(e.to_string(), false)),
        }
    }

    /// Determine retry delay from Retry-After header value or default backoff.
    fn get_retry_delay(&self, retry_after: Option<&str>, attempt: u32) -> Duration {
        if let Some(val) = retry_after
            && let Ok(secs) = val.parse::<f64>()
            && secs > 0.0
        {
            return Duration::from_secs_f64(secs);
        }
        self.retry_config.delay_for_attempt(attempt)
    }

    /// Sleep that can be interrupted by cancellation.
    async fn interruptible_sleep(
        &self,
        duration: Duration,
        cancel: Option<&CancellationToken>,
    ) -> Result<(), HttpError> {
        match cancel {
            Some(token) => {
                tokio::select! {
                    _ = tokio::time::sleep(duration) => Ok(()),
                    _ = token.cancelled() => Err(HttpError::Interrupted),
                }
            }
            None => {
                tokio::time::sleep(duration).await;
                Ok(())
            }
        }
    }

    /// Get the configured API URL.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("HttpClient")
            .field("api_url", &self.api_url)
            .field("retry_config", &self.retry_config)
            .finish()
    }
}

/// Check if a reqwest error is transient and worth retrying.
fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_timeout_config_default() {
        let tc = TimeoutConfig::default();
        assert_eq!(tc.connect, Duration::from_secs(10));
        assert_eq!(tc.read, Duration::from_secs(300));
        assert_eq!(tc.write, Duration::from_secs(10));
    }

    #[test]
    fn test_http_client_debug() {
        let client =
            HttpClient::new("https://api.example.com/v1/chat", HeaderMap::new(), None).unwrap();
        let debug = format!("{:?}", client);
        assert!(debug.contains("api.example.com"));
    }

    #[test]
    fn test_get_retry_delay_with_header() {
        let client = HttpClient::new("https://example.com", HeaderMap::new(), None).unwrap();
        let delay = client.get_retry_delay(Some("5.0"), 0);
        assert_eq!(delay, Duration::from_secs(5));
    }

    #[test]
    fn test_get_retry_delay_fallback() {
        let client = HttpClient::new("https://example.com", HeaderMap::new(), None).unwrap();
        let delay = client.get_retry_delay(None, 0);
        assert_eq!(delay, Duration::from_secs(1));
        let delay = client.get_retry_delay(Some("invalid"), 1);
        assert_eq!(delay, Duration::from_secs(2));
    }

    #[tokio::test]
    async fn test_cancellation_before_request() {
        let client = HttpClient::new("https://example.com", HeaderMap::new(), None).unwrap();
        let token = CancellationToken::new();
        token.cancel();

        let result = client
            .post_json(&serde_json::json!({}), Some(&token))
            .await
            .unwrap();
        assert!(result.interrupted);
        assert!(!result.success);
    }

    #[tokio::test]
    async fn test_interruptible_sleep_cancel() {
        let client = HttpClient::new("https://example.com", HeaderMap::new(), None).unwrap();
        let token = CancellationToken::new();
        token.cancel();

        let err = client
            .interruptible_sleep(Duration::from_secs(60), Some(&token))
            .await;
        assert!(matches!(err, Err(HttpError::Interrupted)));
    }

    #[tokio::test]
    async fn test_interruptible_sleep_completes() {
        let client = HttpClient::new("https://example.com", HeaderMap::new(), None).unwrap();
        let result = client
            .interruptible_sleep(Duration::from_millis(10), None)
            .await;
        assert!(result.is_ok());
    }
}
