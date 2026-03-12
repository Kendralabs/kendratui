//! Prompt composition and loading system.
//!
//! - [`composer`] — Priority-ordered section composition with conditional loading
//! - [`embedded`] — All `.md` templates embedded at compile time via `include_str!`
//! - [`loader`] — Template file loading with frontmatter stripping

pub mod composer;
pub mod embedded;
pub mod loader;

pub use composer::{
    create_composer, create_default_composer, create_thinking_composer, strip_frontmatter,
    substitute_variables, ConditionFn, PromptComposer, PromptContext, PromptSection,
};
pub use embedded::{get_embedded, TEMPLATES, TEMPLATE_COUNT};
pub use loader::{PromptLoadError, PromptLoader};
