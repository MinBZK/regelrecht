//! Configuration for Dutch law hierarchy.

use super::registry::HierarchyRegistry;
use super::types::ElementSpec;

/// Create hierarchy registry for Dutch law structure.
///
/// The hierarchy represents the structural nesting of Dutch legal documents:
///
/// ```text
/// artikel
/// ├── lid (numbered paragraph, e.g. "1.")
/// │   ├── al (text paragraph)
/// │   └── lijst (list)
/// │       └── li (list item, e.g. "a.")
/// │           ├── al
/// │           └── lijst (nested)
/// │               └── li (e.g. "1°")
/// │                   └── al
/// ```
#[must_use]
pub fn create_dutch_law_hierarchy() -> HierarchyRegistry {
    let mut registry = HierarchyRegistry::new();

    // Artikel: top-level article element
    registry.register(
        ElementSpec::new("artikel")
            .with_children(["lid", "lijst"])
            .with_number_source("kop/nr")
            .with_content_tags(["al"])
            .with_split_point(true)
            .with_skip_for_number(["kop", "meta-data"]),
    );

    // Lid: numbered paragraph within artikel
    registry.register(
        ElementSpec::new("lid")
            .with_children(["lijst"])
            .with_number_source("lidnr")
            .with_content_tags(["al"])
            .with_split_point(true)
            .with_skip_for_number(["lidnr", "meta-data"]),
    );

    // Lijst: list container (not a split point itself)
    registry.register(
        ElementSpec::new("lijst")
            .with_children(["li"])
            .with_split_point(false),
    );

    // Li: list item
    registry.register(
        ElementSpec::new("li")
            .with_children(["lijst"])
            .with_number_source("li.nr")
            .with_content_tags(["al"])
            .with_split_point(true)
            .with_skip_for_number(["li.nr"]),
    );

    registry
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_create_dutch_law_hierarchy() {
        let registry = create_dutch_law_hierarchy();

        // Check artikel spec
        let artikel_spec = registry.get_spec("artikel").unwrap();
        assert!(artikel_spec.is_split_point);
        assert_eq!(artikel_spec.children, vec!["lid", "lijst"]);
        assert_eq!(artikel_spec.number_source, Some("kop/nr".to_string()));

        // Check lid spec
        let lid_spec = registry.get_spec("lid").unwrap();
        assert!(lid_spec.is_split_point);
        assert_eq!(lid_spec.children, vec!["lijst"]);

        // Check lijst spec
        let lijst_spec = registry.get_spec("lijst").unwrap();
        assert!(!lijst_spec.is_split_point);

        // Check li spec
        let li_spec = registry.get_spec("li").unwrap();
        assert!(li_spec.is_split_point);
    }
}
