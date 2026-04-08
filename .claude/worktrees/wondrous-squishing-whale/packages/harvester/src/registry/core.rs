//! Element registry for mapping tag names to handlers.

use std::collections::{HashMap, HashSet};

use roxmltree::Node;

use super::handler::ElementHandler;
use super::types::ParseContext;
use crate::xml::get_tag_name;

/// Registry mapping element names to handlers.
///
/// The registry allows registering handlers for specific tag names,
/// as well as marking tags to be skipped entirely.
pub struct ElementRegistry {
    handlers: HashMap<String, Box<dyn ElementHandler>>,
    skip_tags: HashSet<String>,
}

impl ElementRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            handlers: HashMap::new(),
            skip_tags: HashSet::new(),
        }
    }

    /// Register a handler for a specific tag name.
    pub fn register(
        &mut self,
        tag_name: impl Into<String>,
        handler: impl ElementHandler + 'static,
    ) {
        self.handlers.insert(tag_name.into(), Box::new(handler));
    }

    /// Mark tags as skip (don't process, return empty).
    pub fn skip(&mut self, tag_names: impl IntoIterator<Item = impl Into<String>>) {
        for tag in tag_names {
            self.skip_tags.insert(tag.into());
        }
    }

    /// Get the appropriate handler for an element.
    ///
    /// Returns `None` if the element should be skipped or has no handler.
    pub fn get_handler(
        &self,
        node: Node<'_, '_>,
        context: &ParseContext<'_>,
    ) -> Option<&dyn ElementHandler> {
        let tag_name = get_tag_name(node);

        if self.skip_tags.contains(tag_name) {
            return None;
        }

        self.handlers
            .get(tag_name)
            .filter(|h| h.can_handle(node, context))
            .map(|h| h.as_ref())
    }

    /// Check if a tag should be skipped.
    #[must_use]
    pub fn should_skip(&self, tag_name: &str) -> bool {
        self.skip_tags.contains(tag_name)
    }

    /// Check if a handler is registered for a tag.
    #[must_use]
    pub fn has_handler(&self, tag_name: &str) -> bool {
        self.handlers.contains_key(tag_name)
    }

    /// Return set of all registered tag names.
    #[must_use]
    pub fn registered_tags(&self) -> HashSet<&str> {
        self.handlers.keys().map(|s| s.as_str()).collect()
    }

    /// Return set of all skipped tag names.
    #[must_use]
    pub fn skipped_tags(&self) -> HashSet<&str> {
        self.skip_tags.iter().map(|s| s.as_str()).collect()
    }
}

impl Default for ElementRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::registry::{ElementType, ParseResult};
    use roxmltree::Document;

    struct DummyHandler;

    impl ElementHandler for DummyHandler {
        fn element_type(&self) -> ElementType {
            ElementType::Inline
        }

        fn handle<'a, 'input>(
            &self,
            _node: Node<'a, 'input>,
            _context: &mut ParseContext<'_>,
            _recurse: &super::super::handler::RecurseFn<'a, 'input>,
        ) -> ParseResult {
            ParseResult::new("dummy")
        }
    }

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = ElementRegistry::new();
        registry.register("test", DummyHandler);

        let xml = "<test/>";
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();
        let context = ParseContext::new("BWBR0000000", "2025-01-01");

        assert!(registry.get_handler(node, &context).is_some());
    }

    #[test]
    fn test_registry_skip() {
        let mut registry = ElementRegistry::new();
        registry.skip(["meta-data", "kop"]);

        assert!(registry.should_skip("meta-data"));
        assert!(registry.should_skip("kop"));
        assert!(!registry.should_skip("artikel"));
    }

    #[test]
    fn test_registry_has_handler() {
        let mut registry = ElementRegistry::new();
        registry.register("test", DummyHandler);

        assert!(registry.has_handler("test"));
        assert!(!registry.has_handler("missing"));
    }
}
