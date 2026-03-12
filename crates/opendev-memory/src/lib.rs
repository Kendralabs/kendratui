//! ACE memory system for OpenDev.
//!
//! This crate implements the Agentic Context Engine (ACE) memory system:
//! - Playbook: Structured store for accumulated strategies and insights
//! - Delta: Batch mutation operations on the playbook
//! - Embeddings: Embedding cache and cosine similarity for semantic search
//! - Selector: Intelligent bullet selection for LLM context
//! - Reflector: Post-turn reflection to extract learnable patterns
//! - Roles: ACE role data models (Reflector, Curator outputs)

pub mod delta;
pub mod embeddings;
pub mod playbook;
pub mod reflector;
pub mod roles;
pub mod selector;
pub mod summarizer;

pub use delta::{DeltaBatch, DeltaOperation, DeltaOperationType};
pub use embeddings::{EmbeddingCache, EmbeddingMetadata};
pub use playbook::{Bullet, Playbook};
pub use reflector::{ExecutionReflector, ReflectionResult};
pub use roles::{AgentResponse, BulletTag, CuratorOutput, ReflectorOutput};
pub use selector::{BulletSelector, ScoredBullet};
pub use summarizer::ConversationSummarizer;
