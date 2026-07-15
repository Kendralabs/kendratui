//! Semantic context aware compaction for kendra.
//!
//! Provides structural awareness to the compactor to prioritize retaining
//! core project knowledge during aggressive compaction stages.

use serde::{Deserialize, Serialize};

/// Structural information about the codebase.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct StructuralContext {
    /// High-level overview of the project structure and key components.
    pub overview: String,
    /// Dependencies list.
    pub dependencies: String,
    /// List of critical files (e.g., config, entry points).
    pub key_files: Vec<String>,
}
