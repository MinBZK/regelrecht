//! YAML output generation for law files.

mod text;
mod writer;

pub use text::{normalize_text, should_wrap_text, wrap_text, wrap_text_default};
pub use writer::{generate_yaml, save_yaml};
