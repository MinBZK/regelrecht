//! Structural element handlers for container elements.
//!
//! These handlers process structural elements that contain other elements,
//! such as lid (paragraph), lijst (list), and li (list item).

use roxmltree::Node;

use crate::registry::handler::{extract_text_with_tail, ElementHandler, RecurseFn};
use crate::registry::types::{ElementType, ParseContext, ParseResult};
use crate::xml::get_tag_name;

/// Handler for `<lidnr>` (paragraph number) elements.
///
/// Returns the paragraph number text.
pub struct LidnrHandler;

impl ElementHandler for LidnrHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        _context: &mut ParseContext<'_>,
        _recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        let text = node.text().map(|s| s.trim()).unwrap_or_default();
        ParseResult::new(text)
    }
}

/// Handler for `<li.nr>` (list item number) elements.
///
/// Returns the list item marker (a., b., 1., etc.) without trailing period.
pub struct LiNrHandler;

impl ElementHandler for LiNrHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        _context: &mut ParseContext<'_>,
        _recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        let mut nr = node
            .text()
            .map(|s| s.trim())
            .unwrap_or_default()
            .to_string();
        if nr.ends_with('.') {
            nr.pop();
        }
        ParseResult::new(nr)
    }
}

/// Handler for `<lid>` (paragraph/subdivision) elements.
///
/// Extracts text from a lid, processing all child `<al>` elements.
/// Skips lidnr and meta-data elements.
pub struct LidHandler;

impl ElementHandler for LidHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        let mut parts: Vec<String> = Vec::new();

        for child in node.children() {
            if !child.is_element() {
                continue;
            }

            let child_tag = get_tag_name(child);

            // Skip lidnr (handled separately for numbering)
            if child_tag == "lidnr" || child_tag == "meta-data" {
                continue;
            }

            let result = recurse(child, context);
            if !result.text.is_empty() {
                parts.push(result.text);
            }
        }

        ParseResult::new(parts.join(" ").trim().to_string())
    }
}

/// Handler for `<lijst>` (list) elements.
///
/// Processes each `<li>` child element. Handles both marked and
/// unmarked (ongemarkeerd) list types.
pub struct LijstHandler;

impl ElementHandler for LijstHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        let mut items: Vec<String> = Vec::new();

        for child in node.children() {
            if child.is_element() && get_tag_name(child) == "li" {
                let result = recurse(child, context);
                if !result.text.is_empty() {
                    items.push(result.text);
                }
            }
        }

        ParseResult::new(items.join("\n"))
    }
}

/// Handler for `<li>` (list item) elements.
///
/// Extracts text from list items, processing child `<al>` elements.
/// Skips li.nr (handled separately for numbering).
pub struct LiHandler;

impl ElementHandler for LiHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Structural
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        let mut parts: Vec<String> = Vec::new();

        for child in node.children() {
            if !child.is_element() {
                continue;
            }

            let child_tag = get_tag_name(child);

            // Skip li.nr (handled separately for numbering)
            if child_tag == "li.nr" {
                continue;
            }

            let result = recurse(child, context);
            if !result.text.is_empty() {
                parts.push(result.text);
            }
        }

        ParseResult::new(parts.join(" ").trim().to_string())
    }
}

/// Handler that skips elements (returns empty text).
///
/// Used for elements that should not contribute to text output.
pub struct SkipHandler;

impl ElementHandler for SkipHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Skip
    }

    fn handle<'a, 'input>(
        &self,
        _node: Node<'a, 'input>,
        _context: &mut ParseContext<'_>,
        _recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        ParseResult::empty()
    }
}

/// Handler that extracts text from element and all children.
///
/// Used for container elements that should contribute their text content.
pub struct PassthroughHandler;

impl ElementHandler for PassthroughHandler {
    fn element_type(&self) -> ElementType {
        ElementType::Inline
    }

    fn handle<'a, 'input>(
        &self,
        node: Node<'a, 'input>,
        context: &mut ParseContext<'_>,
        recurse: &RecurseFn<'a, 'input>,
    ) -> ParseResult {
        ParseResult::new(extract_text_with_tail(node, context, recurse))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use roxmltree::Document;

    fn parse_and_handle<H: ElementHandler>(handler: &H, xml: &str) -> ParseResult {
        let doc = Document::parse(xml).unwrap();
        let node = doc.root_element();
        let mut context = ParseContext::new("BWBR0000000", "2025-01-01");
        let recurse = |_: Node<'_, '_>, _: &mut ParseContext<'_>| ParseResult::empty();
        handler.handle(node, &mut context, &recurse)
    }

    #[test]
    fn test_lidnr_handler() {
        let result = parse_and_handle(&LidnrHandler, "<lidnr>1.</lidnr>");
        assert_eq!(result.text, "1.");
    }

    #[test]
    fn test_linr_handler() {
        let result = parse_and_handle(&LiNrHandler, "<li.nr>a.</li.nr>");
        assert_eq!(result.text, "a");
    }

    #[test]
    fn test_linr_handler_no_period() {
        let result = parse_and_handle(&LiNrHandler, "<li.nr>1°</li.nr>");
        assert_eq!(result.text, "1°");
    }

    #[test]
    fn test_skip_handler() {
        let result = parse_and_handle(&SkipHandler, "<meta-data>ignored</meta-data>");
        assert_eq!(result.text, "");
    }
}
