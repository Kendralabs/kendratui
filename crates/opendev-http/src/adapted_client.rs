//! Adapted HTTP client that wraps HttpClient + ProviderAdapter.
//!
//! Transparently converts requests/responses through the provider adapter
//! so the rest of the codebase can use a uniform Chat Completions format.

use crate::adapters::base::ProviderAdapter;
use crate::client::HttpClient;
use crate::models::{HttpError, HttpResult};
use tokio_util::sync::CancellationToken;

/// HTTP client with provider-specific request/response adaptation.
///
/// Wraps `HttpClient` and an optional `ProviderAdapter`. When an adapter
/// is present, `post_json` will:
/// 1. Convert the payload via `adapter.convert_request()`
/// 2. Send via `HttpClient::post_json()`
/// 3. Convert the response body via `adapter.convert_response()`
pub struct AdaptedClient {
    client: HttpClient,
    adapter: Option<Box<dyn ProviderAdapter>>,
}

impl AdaptedClient {
    /// Create an adapted client without any adapter (passthrough).
    pub fn new(client: HttpClient) -> Self {
        Self {
            client,
            adapter: None,
        }
    }

    /// Create an adapted client with a provider adapter.
    pub fn with_adapter(client: HttpClient, adapter: Box<dyn ProviderAdapter>) -> Self {
        Self {
            client,
            adapter: Some(adapter),
        }
    }

    /// POST JSON with optional request/response conversion.
    pub async fn post_json(
        &self,
        payload: &serde_json::Value,
        cancel: Option<&CancellationToken>,
    ) -> Result<HttpResult, HttpError> {
        let converted_payload = match &self.adapter {
            Some(adapter) => adapter.convert_request(payload.clone()),
            None => payload.clone(),
        };

        let mut result = self.client.post_json(&converted_payload, cancel).await?;

        // Convert response body back to Chat Completions format
        if let (Some(adapter), Some(body)) = (&self.adapter, &result.body)
            && result.success
        {
            result.body = Some(adapter.convert_response(body.clone()));
        }

        Ok(result)
    }

    /// Get the configured API URL.
    pub fn api_url(&self) -> &str {
        self.client.api_url()
    }
}

impl std::fmt::Debug for AdaptedClient {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AdaptedClient")
            .field("api_url", &self.client.api_url())
            .field(
                "adapter",
                &self
                    .adapter
                    .as_ref()
                    .map(|a| a.provider_name())
                    .unwrap_or("none"),
            )
            .finish()
    }
}
