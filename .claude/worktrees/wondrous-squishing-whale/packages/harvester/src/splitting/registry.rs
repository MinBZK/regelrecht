//! Hierarchy registry for article splitting.

use std::collections::HashMap;

use super::types::ElementSpec;

/// Registry of element specifications for the hierarchy.
pub struct HierarchyRegistry {
    specs: HashMap<String, ElementSpec>,
}

impl HierarchyRegistry {
    /// Create a new empty registry.
    #[must_use]
    pub fn new() -> Self {
        Self {
            specs: HashMap::new(),
        }
    }

    /// Register an element specification.
    pub fn register(&mut self, spec: ElementSpec) {
        self.specs.insert(spec.tag.clone(), spec);
    }

    /// Get the specification for a tag.
    #[must_use]
    pub fn get_spec(&self, tag: &str) -> Option<&ElementSpec> {
        self.specs.get(tag)
    }

    /// Check if a tag is a structural element in the hierarchy.
    #[must_use]
    pub fn is_structural(&self, tag: &str) -> bool {
        self.specs.contains_key(tag)
    }
}

impl Default for HierarchyRegistry {
    fn default() -> Self {
        Self::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_registry_register_and_get() {
        let mut registry = HierarchyRegistry::new();

        let spec = ElementSpec::new("artikel").with_split_point(true);
        registry.register(spec);

        let retrieved = registry.get_spec("artikel");
        assert!(retrieved.is_some());
        assert!(retrieved.unwrap().is_split_point);
    }

    #[test]
    fn test_registry_is_structural() {
        let mut registry = HierarchyRegistry::new();
        registry.register(ElementSpec::new("lid"));

        assert!(registry.is_structural("lid"));
        assert!(!registry.is_structural("al"));
    }
}
