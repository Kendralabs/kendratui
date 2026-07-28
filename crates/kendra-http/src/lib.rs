//! HTTP client, authentication, and provider adapters for KendraCLI.
//!
//! This crate provides:
//! - [`client::HttpClient`] — reqwest wrapper with retry and cancellation support
//! - [`auth::CredentialStore`] — secure credential storage (~/.kendra/auth.json)
//! - [`rotation::AuthProfileManager`] — API key rotation with cooldown
//! - [`adapters`] — provider-specific request/response adapters

pub mod adapted_client;
pub mod adapters;
pub mod auth;
pub mod circuit_breaker;
pub mod client;
pub mod models;
pub mod rotation;
pub mod streaming;
pub mod user_store;

pub use adapted_client::AdaptedClient;
pub use auth::CredentialStore;
pub use circuit_breaker::{CircuitBreaker, CircuitBreakerConfig};
pub use client::HttpClient;
pub use models::{HttpError, HttpResult, RetryConfig, classify_retryable_error, format_http_error, parse_retry_after};
pub use rotation::AuthProfileManager;
pub use user_store::UserStore;
