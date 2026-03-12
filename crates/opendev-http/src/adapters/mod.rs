//! Provider-specific request/response adapters.
//!
//! Each LLM provider has slightly different API conventions. Adapters
//! normalize requests to the provider's format and responses back to
//! a common Chat Completions format.

pub mod anthropic;
pub mod base;
pub mod openai;

pub use base::ProviderAdapter;
