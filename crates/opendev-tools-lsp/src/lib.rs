//! LSP integration for OpenDev.
//!
//! This crate provides:
//! - [`wrapper`] — `LspWrapper` managing multiple language server instances
//! - [`handler`] — JSON-RPC communication with language servers
//! - [`protocol`] — Unified symbol types bridging `lsp-types` to OpenDev
//! - [`utils`] — Text/path utilities for position conversion
//! - [`cache`] — Symbol query caching
//! - [`servers`] — Language server configurations
//! - [`error`] — LSP error types

pub mod cache;
pub mod error;
pub mod handler;
pub mod protocol;
pub mod servers;
pub mod utils;
pub mod wrapper;

pub use cache::SymbolCache;
pub use error::LspError;
pub use handler::LspHandler;
pub use protocol::{
    Position, SourceLocation, SourceRange, SymbolKind, TextEdit, UnifiedSymbolInfo, WorkspaceEdit,
};
pub use servers::ServerConfig;
pub use wrapper::LspWrapper;
