//! Article splitting system for Dutch law documents.
//!
//! This module implements hierarchical article splitting with dot-notation
//! numbering (e.g., "1.1.a" for artikel 1, lid 1, onderdeel a).

mod config;
mod engine;
mod registry;
mod strategy;
mod types;

pub use config::create_dutch_law_hierarchy;
pub use engine::SplitEngine;
pub use registry::HierarchyRegistry;
pub use strategy::{LeafSplitStrategy, SplitStrategy};
pub use types::{ArticleComponent, ElementSpec, SplitContext};
