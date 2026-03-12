//! Base trait for provider adapters.

use serde_json::Value;

/// Trait for converting between the internal Chat Completions format
/// and provider-specific API formats.
///
/// Implementations handle provider quirks like:
/// - Different message formats (Anthropic vs OpenAI)
/// - Prompt caching headers/fields
/// - Reasoning model parameters (o1/o3)
/// - Image block normalization
#[async_trait::async_trait]
pub trait ProviderAdapter: Send + Sync + std::fmt::Debug {
    /// Provider identifier (e.g., "openai", "anthropic").
    fn provider_name(&self) -> &str;

    /// Convert an internal Chat Completions payload to provider-specific format.
    ///
    /// The input is always in OpenAI Chat Completions format. The adapter
    /// transforms it as needed for its provider's API.
    fn convert_request(&self, payload: Value) -> Value;

    /// Convert a provider-specific response back to Chat Completions format.
    ///
    /// The output should be in standard OpenAI Chat Completions response format
    /// so downstream code can handle all providers uniformly.
    fn convert_response(&self, response: Value) -> Value;

    /// Get the API endpoint URL for this provider.
    fn api_url(&self) -> &str;

    /// Get required headers for this provider (e.g., api-version, anthropic-version).
    fn extra_headers(&self) -> Vec<(String, String)> {
        vec![]
    }
}
