//! HTTP client with retry logic and cancellation support.

use reqwest::header::{CONTENT_TYPE, HeaderMap, HeaderName, HeaderValue};
use std::time::Duration;
use tokio_util::sync::CancellationToken;
use tracing::{debug, warn};
use uuid::Uuid;

use crate::models::{HttpError, HttpResult, RetryConfig};

fn format_http_error(status: u16) -> String {
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
    circuit_breaker: Option<std::sync::Arc<crate::circuit_breaker::CircuitBreaker>>,
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
            circuit_breaker: None,
        })
    }

    /// Create a client with custom retry configuration.
    pub fn with_retry_config(mut self, config: RetryConfig) -> Self {
        self.retry_config = config;
        self
    }

    /// Attach a circuit breaker to this client.
    ///
    /// When set, every request is gated by the circuit breaker. Successful
    /// responses close the circuit; failures (transport-level or 5xx) open it.
    pub fn with_circuit_breaker(
        mut self,
        cb: std::sync::Arc<crate::circuit_breaker::CircuitBreaker>,
    ) -> Self {
        self.circuit_breaker = Some(cb);
        self
    }

    /// POST JSON with retry logic and optional cancellation.
    ///
    /// On 429/503 responses, retries with exponential backoff. Respects
    /// `Retry-After` headers. Checks the cancellation token between attempts
    /// and races it against each request via `tokio::select!`.
    ///
    /// When a circuit breaker is attached, requests are rejected immediately
    /// if the circuit is open.
    pub async fn post_json(
        &self,
        payload: &serde_json::Value,
        cancel: Option<&CancellationToken>,
    ) -> Result<HttpResult, HttpError> {
        // Check circuit breaker before attempting any request.
        if let Some(cb) = &self.circuit_breaker {
            cb.check()?;
        }

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
                        let delay = self.get_retry_delay(
                            hr.retry_after.as_deref(),
                            hr.retry_after_ms.as_deref(),
                            attempt,
                        );
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
                        self.cb_record_failure();
                        return Ok(last_result.unwrap_or_else(|| {
                            HttpResult::fail("Unexpected retry exhaustion", false)
                        }));
                    }
                    self.cb_record_success();
                    return Ok(hr);
                }
                Ok(hr) if hr.retryable => {
                    let retry_after = hr.retry_after.clone();
                    let retry_after_ms = hr.retry_after_ms.clone();
                    last_result = Some(hr);
                    if attempt < self.retry_config.max_retries {
                        let delay = self.get_retry_delay(
                            retry_after.as_deref(),
                            retry_after_ms.as_deref(),
                            attempt,
                        );
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
                    self.cb_record_failure();
                    return Ok(last_result.unwrap_or_else(|| {
                        HttpResult::fail("Unexpected retry exhaustion", false)
                    }));
                }
                Ok(hr) => {
                    if hr.success {
                        self.cb_record_success();
                    } else {
                        self.cb_record_failure();
                    }
                    return Ok(hr);
                }
                Err(e) => {
                    self.cb_record_failure();
                    return Err(e);
                }
            }
        }

        self.cb_record_failure();
        Ok(last_result.unwrap_or_else(|| HttpResult::fail("Unexpected retry exhaustion", false)))
    }

    /// Record a success on the circuit breaker (if attached).
    fn cb_record_success(&self) {
        if let Some(cb) = &self.circuit_breaker {
            cb.record_success();
        }
    }

    /// Record a failure on the circuit breaker (if attached).
    fn cb_record_failure(&self) {
        if let Some(cb) = &self.circuit_breaker {
            cb.record_failure();
        }
    }

    /// Execute a single POST request, racing against cancellation.
    ///
    /// Each request is tagged with a unique `X-Request-Id` header and
    /// logged via a tracing span for end-to-end observability.
    async fn execute_request(
        &self,
        payload: &serde_json::Value,
        cancel: Option<&CancellationToken>,
    ) -> Result<HttpResult, HttpError> {
        let request_id = Uuid::new_v4().to_string();
        debug!(request_id = %request_id, api_url = %self.api_url, "Sending LLM request");

        let request = self
            .client
            .post(&self.api_url)
            .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
            .header(
                HeaderName::from_static("x-request-id"),
                HeaderValue::from_str(&request_id)
                    .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
            )
            .json(payload)
            .send();

        let response = match cancel {
            Some(token) => {
                tokio::select! {
                    resp = request => resp,
                    _ = token.cancelled() => {
                        return Ok(HttpResult::interrupted()
                            .with_request_id(request_id));
                    }
                }
            }
            None => request.await,
        };

        match response {
            Ok(resp) => {
                let status = resp.status().as_u16();
                debug!(request_id = %request_id, status, "LLM response received");
                if self.retry_config.is_retryable_status(status) {
                    // Extract Retry-After and retry-after-ms headers
                    let retry_after = resp
                        .headers()
                        .get("retry-after")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);
                    let retry_after_ms = resp
                        .headers()
                        .get("retry-after-ms")
                        .and_then(|v| v.to_str().ok())
                        .map(String::from);
                    let body = resp.json::<serde_json::Value>().await.ok();
                    let mut result = HttpResult::retryable_status(status, body, retry_after)
                        .with_request_id(request_id);
                    result.retry_after_ms = retry_after_ms;
                    return Ok(result);
                }
                let body = resp.json::<serde_json::Value>().await?;
                if status >= 400 {
                    let error_msg = body
                        .get("error")
                        .and_then(|e| e.get("message"))
                        .and_then(|m| m.as_str())
                        .map(|s| s.to_string())
                        .unwrap_or_else(|| format_http_error(status));
                    warn!(request_id = %request_id, status, error = %error_msg, "LLM request failed");
                    return Ok(HttpResult {
                        success: false,
                        status: Some(status),
                        body: Some(body),
                        error: Some(format!("[request_id={}] {}", request_id, error_msg)),
                        interrupted: false,
                        retryable: false,
                        request_id: Some(request_id),
                        retry_after: None,
                        retry_after_ms: None,
                    });
                }
                Ok(HttpResult::ok(status, body).with_request_id(request_id))
            }
            Err(e) if is_retryable_error(&e) => {
                warn!(request_id = %request_id, error = %e, "LLM request retryable error");
                Ok(
                    HttpResult::fail(format!("[request_id={}] {}", request_id, e), true)
                        .with_request_id(request_id),
                )
            }
            Err(e) => {
                warn!(request_id = %request_id, error = %e, "LLM request error");
                Ok(
                    HttpResult::fail(format!("[request_id={}] {}", request_id, e), false)
                        .with_request_id(request_id),
                )
            }
        }
    }

    /// Determine retry delay from Retry-After/retry-after-ms headers or default backoff.
    fn get_retry_delay(
        &self,
        retry_after: Option<&str>,
        retry_after_ms: Option<&str>,
        attempt: u32,
    ) -> Duration {
        if let Some(parsed) = crate::models::parse_retry_after(retry_after, retry_after_ms) {
            // Cap server-requested delay at max_delay_ms
            let max = Duration::from_millis(self.retry_config.max_delay_ms);
            return parsed.min(max);
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

    /// Send a POST request and return the raw response for streaming.
    ///
    /// Unlike `post_json`, this does NOT read the response body. The caller
    /// is responsible for consuming the response (e.g., reading SSE lines).
    ///
    /// Retries on transport errors and retryable HTTP status codes (429/503)
    /// before any response body has been consumed. Once a successful response
    /// is returned to the caller, no further retries are attempted.
    pub async fn send_streaming_request(
        &self,
        url: &str,
        payload: &serde_json::Value,
        cancel: Option<&CancellationToken>,
    ) -> Result<reqwest::Response, HttpError> {
        // Check circuit breaker
        if let Some(cb) = &self.circuit_breaker {
            cb.check()?;
        }

        let mut last_error: Option<HttpError> = None;

        for attempt in 0..=self.retry_config.max_retries {
            // Check cancellation before each attempt
            if let Some(token) = cancel
                && token.is_cancelled()
            {
                return Err(HttpError::Interrupted);
            }

            let request_id = Uuid::new_v4().to_string();
            debug!(request_id = %request_id, api_url = %url, attempt, "Sending streaming LLM request");

            let request = self
                .client
                .post(url)
                .header(CONTENT_TYPE, HeaderValue::from_static("application/json"))
                .header(
                    HeaderName::from_static("x-request-id"),
                    HeaderValue::from_str(&request_id)
                        .unwrap_or_else(|_| HeaderValue::from_static("unknown")),
                )
                .json(payload)
                .send();

            let response = match cancel {
                Some(token) => {
                    tokio::select! {
                        resp = request => resp,
                        _ = token.cancelled() => {
                            return Err(HttpError::Interrupted);
                        }
                    }
                }
                None => request.await,
            };

            match response {
                Ok(resp) => {
                    let status = resp.status().as_u16();

                    if self.retry_config.is_retryable_status(status) {
                        // Extract Retry-After headers before consuming the body
                        let retry_after = resp
                            .headers()
                            .get("retry-after")
                            .and_then(|v| v.to_str().ok())
                            .map(String::from);
                        let retry_after_ms = resp
                            .headers()
                            .get("retry-after-ms")
                            .and_then(|v| v.to_str().ok())
                            .map(String::from);
                        let body = resp.text().await.unwrap_or_default();
                        let error_msg = serde_json::from_str::<serde_json::Value>(&body)
                            .ok()
                            .and_then(|v| {
                                v.get("error")
                                    .and_then(|e| e.get("message"))
                                    .and_then(|m| m.as_str())
                                    .map(String::from)
                            })
                            .unwrap_or_else(|| format_http_error(status));

                        last_error = Some(HttpError::Other(format!(
                            "[request_id={request_id}] {error_msg}"
                        )));

                        if attempt < self.retry_config.max_retries {
                            let delay = self.get_retry_delay(
                                retry_after.as_deref(),
                                retry_after_ms.as_deref(),
                                attempt,
                            );
                            warn!(
                                request_id = %request_id,
                                status,
                                attempt = attempt + 1,
                                max = self.retry_config.max_retries,
                                "Streaming request retryable status {status}, backing off for {:.1}s",
                                delay.as_secs_f64()
                            );
                            self.interruptible_sleep(delay, cancel).await?;
                            continue;
                        }
                        warn!(
                            request_id = %request_id,
                            status,
                            "Streaming request exhausted {} retries",
                            self.retry_config.max_retries
                        );
                    } else if status >= 400 {
                        // Non-retryable error — fail immediately
                        let body = resp.text().await.unwrap_or_default();
                        let error_msg = serde_json::from_str::<serde_json::Value>(&body)
                            .ok()
                            .and_then(|v| {
                                v.get("error")
                                    .and_then(|e| e.get("message"))
                                    .and_then(|m| m.as_str())
                                    .map(String::from)
                            })
                            .unwrap_or_else(|| format_http_error(status));
                        warn!(request_id = %request_id, status, error = %error_msg, "Streaming request failed");
                        self.cb_record_failure();
                        return Err(HttpError::Other(format!(
                            "[request_id={request_id}] {error_msg}"
                        )));
                    } else {
                        self.cb_record_success();
                        return Ok(resp);
                    }
                }
                Err(e) if is_retryable_error(&e) => {
                    warn!(error = %e, attempt = attempt + 1, max = self.retry_config.max_retries, "Streaming request transport error");
                    last_error = Some(HttpError::Request(e));
                    if attempt < self.retry_config.max_retries {
                        let delay = self.get_retry_delay(None, None, attempt);
                        warn!(
                            "Streaming request backing off for {:.1}s",
                            delay.as_secs_f64()
                        );
                        self.interruptible_sleep(delay, cancel).await?;
                        continue;
                    }
                }
                Err(e) => {
                    // Non-retryable transport error — fail immediately
                    self.cb_record_failure();
                    return Err(e.into());
                }
            }
        }

        self.cb_record_failure();
        Err(last_error.unwrap_or_else(|| HttpError::Other("Streaming retries exhausted".into())))
    }

    /// Get the configured API URL.
    pub fn api_url(&self) -> &str {
        &self.api_url
    }
}

impl std::fmt::Debug for HttpClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let mut s = f.debug_struct("HttpClient");
        s.field("api_url", &self.api_url)
            .field("retry_config", &self.retry_config);
        if let Some(cb) = &self.circuit_breaker {
            s.field("circuit_breaker", cb);
        }
        s.finish()
    }
}

/// Check if a reqwest error is transient and worth retrying.
fn is_retryable_error(err: &reqwest::Error) -> bool {
    err.is_connect() || err.is_timeout() || err.is_request()
}

#[cfg(test)]
#[path = "client_tests.rs"]
mod tests;
