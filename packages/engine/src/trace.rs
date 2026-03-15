//! Execution tracing for audit trails and debugging
//!
//! This module provides structures and utilities for recording the execution
//! path through law evaluation. This is useful for:
//!
//! - **Audit trails**: Documenting exactly how a decision was reached
//! - **Debugging**: Understanding why a particular result was produced
//! - **Explainability**: Providing transparency in automated legal decisions
//!
//! # Example
//!
//! ```ignore
//! use regelrecht_engine::trace::{PathNode, PathNodeType, TraceBuilder};
//!
//! let mut builder = TraceBuilder::new();
//!
//! // Start resolving a variable
//! builder.push("inkomen", PathNodeType::Resolve);
//!
//! // Nested operation
//! builder.push("vergelijk_drempel", PathNodeType::Operation);
//! builder.set_result(Value::Bool(true));
//! builder.pop();
//!
//! // Complete the resolution
//! builder.set_result(Value::Int(50000));
//! builder.pop();
//!
//! let trace = builder.build();
//! ```

use crate::types::{PathNodeType, ResolveType, Value};
use serde::Serialize;
use std::time::Instant;

/// A node in the execution trace tree.
///
/// Each node represents a single step in the execution process, such as:
/// - Resolving a variable
/// - Executing an operation
/// - Evaluating an action
///
/// Nodes can have children, forming a tree structure that mirrors the
/// nested nature of law evaluation.
#[derive(Debug, Clone, Serialize)]
pub struct PathNode {
    /// Type of this execution step
    pub node_type: PathNodeType,

    /// Name or identifier for this step (e.g., variable name, operation type)
    pub name: String,

    /// The result value produced by this step, if any
    #[serde(skip_serializing_if = "Option::is_none")]
    pub result: Option<Value>,

    /// For resolve nodes, indicates how the value was resolved
    #[serde(skip_serializing_if = "Option::is_none")]
    pub resolve_type: Option<ResolveType>,

    /// Child nodes representing nested execution steps
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub children: Vec<PathNode>,

    /// Execution duration in microseconds
    #[serde(skip_serializing_if = "Option::is_none")]
    pub duration_us: Option<u64>,

    /// Free-form message for trace output (e.g., "Resolving from PARAMETERS: 999993653")
    #[serde(skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
}

impl PathNode {
    /// Create a new PathNode with the given type and name.
    pub fn new(node_type: PathNodeType, name: impl Into<String>) -> Self {
        Self {
            node_type,
            name: name.into(),
            result: None,
            resolve_type: None,
            children: Vec::new(),
            duration_us: None,
            message: None,
        }
    }

    /// Set the result value for this node.
    pub fn with_result(mut self, result: Value) -> Self {
        self.result = Some(result);
        self
    }

    /// Set the resolve type for this node.
    pub fn with_resolve_type(mut self, resolve_type: ResolveType) -> Self {
        self.resolve_type = Some(resolve_type);
        self
    }

    /// Add a child node.
    pub fn with_child(mut self, child: PathNode) -> Self {
        self.children.push(child);
        self
    }

    /// Set the duration in microseconds.
    pub fn with_duration(mut self, duration_us: u64) -> Self {
        self.duration_us = Some(duration_us);
        self
    }

    /// Set a free-form message for trace output.
    pub fn with_message(mut self, message: impl Into<String>) -> Self {
        self.message = Some(message.into());
        self
    }

    /// Render the trace as a human-readable tree string.
    ///
    /// Produces output like:
    /// ```text
    /// calculate (action)
    /// +-- inkomen (resolve) [parameter] = 50000
    /// +-- drempel (resolve) [definition] = 30000
    /// `-- vergelijk (operation) = true
    ///     +-- $inkomen (resolve) = 50000
    ///     `-- $drempel (resolve) = 30000
    /// ```
    ///
    /// # Arguments
    /// * `indent` - Current indentation level (start with 0)
    /// * `is_last` - Whether this is the last child in its parent (affects line prefix)
    pub fn render(&self, indent: usize, is_last: bool) -> String {
        self.render_internal(indent, is_last, true)
    }

    /// Internal render implementation.
    fn render_internal(&self, indent: usize, is_last: bool, is_top_level: bool) -> String {
        let mut lines = Vec::new();

        // Build the prefix for this node
        let prefix = if is_top_level {
            String::new()
        } else if is_last {
            "`-- ".to_string()
        } else {
            "+-- ".to_string()
        };

        // Build indentation for child nodes
        let child_indent = if is_top_level {
            String::new()
        } else if is_last {
            "    ".to_string()
        } else {
            "|   ".to_string()
        };

        // Format the node type
        let type_str = match self.node_type {
            PathNodeType::Resolve => "resolve",
            PathNodeType::Operation => "operation",
            PathNodeType::Action => "action",
            PathNodeType::Requirement => "requirement",
            PathNodeType::UriCall => "uri_call",
            PathNodeType::Article => "article",
            PathNodeType::Cached => "cached",
            PathNodeType::OpenTermResolution => "open_term",
        };

        // Build the main line
        let mut line = format!("{}{} ({})", prefix, self.name, type_str);

        // Add resolve type if present
        if let Some(ref rt) = self.resolve_type {
            let rt_str = match rt {
                ResolveType::Uri => "uri",
                ResolveType::Parameter => "parameter",
                ResolveType::Definition => "definition",
                ResolveType::Output => "output",
                ResolveType::Input => "input",
                ResolveType::Local => "local",
                ResolveType::Context => "context",
                ResolveType::ResolvedInput => "resolved_input",
                ResolveType::DataSource => "data_source",
                ResolveType::OpenTerm => "open_term",
            };
            line.push_str(&format!(" [{}]", rt_str));
        }

        // Add result if present
        if let Some(ref result) = self.result {
            line.push_str(&format!(" = {}", format_value_compact(result)));
        }

        // Add duration if present (and significant)
        if let Some(duration) = self.duration_us {
            if duration >= 100 {
                // Only show if >= 0.1ms
                line.push_str(&format!(" ({}μs)", duration));
            }
        }

        lines.push(line);

        // Render children
        let child_count = self.children.len();
        for (i, child) in self.children.iter().enumerate() {
            let is_last_child = i == child_count - 1;
            let child_str = child.render_internal(0, is_last_child, false);

            // Add proper indentation to each line of the child's output
            for (j, child_line) in child_str.lines().enumerate() {
                if j == 0 {
                    // First line gets the current indentation
                    lines.push(format!(
                        "{}{}",
                        " ".repeat(indent * 4) + &child_indent,
                        child_line
                    ));
                } else {
                    // Subsequent lines need continued indentation
                    lines.push(format!(
                        "{}{}",
                        " ".repeat(indent * 4) + &child_indent,
                        child_line
                    ));
                }
            }
        }

        lines.join("\n")
    }

    /// Render the trace using box-drawing characters for human-readable output.
    ///
    /// Produces output like:
    /// ```text
    /// SVB: zorgtoeslagwet (2025-01-01 {BSN: 999993653} hoogte_zorgtoeslag)
    /// ║──Evaluating rules for SVB zorgtoeslagwet (2025-01-01 hoogte_zorgtoeslag)
    /// ║──Requirements {'all': [...]}
    /// ║   ├──Resolving $LEEFTIJD
    /// ║   │   ├──Resolving from DATA_SOURCE: 20
    /// ║   ├──Compute GREATER_OR_EQUAL(20, 18) = True
    /// ║   ├──Requirement met
    /// ║──Computing hoogte_zorgtoeslag
    /// ║   ├──Resolving $TOETSINGSINKOMEN
    /// ║   │   ├──Resolving from DATA_SOURCE: 79547
    /// ║──Result of hoogte_zorgtoeslag: 209692
    /// ```
    pub fn render_box_drawing(&self) -> String {
        let mut lines = Vec::new();
        self.render_box_node(&mut lines, "");
        lines.join("\n")
    }

    /// Internal recursive renderer for box-drawing format.
    ///
    /// `prefix` is the leading string for continuation lines (e.g., "║   ").
    fn render_box_node(&self, lines: &mut Vec<String>, prefix: &str) {
        match self.node_type {
            PathNodeType::Article => {
                // Header line: "AGENCY: law_id (date {params} output_name)"
                if let Some(ref msg) = self.message {
                    lines.push(msg.clone());
                } else {
                    lines.push(self.name.clone());
                }

                // Evaluating rules line
                lines.push(format!("{}║──Evaluating rules for {}", prefix, self.name));

                let child_prefix = format!("{}║   ", prefix);
                for child in &self.children {
                    child.render_box_node(lines, &child_prefix);
                }

                // Result line
                if let Some(ref result) = self.result {
                    // Extract the output name from "law_id (output_name)" format
                    let output_name = self
                        .name
                        .split_once(' ')
                        .map(|(_, rest)| rest.trim_matches(|c| c == '(' || c == ')'))
                        .unwrap_or(&self.name);
                    lines.push(format!(
                        "{}║──Result of {}: {}",
                        prefix,
                        output_name,
                        format_value_display(result)
                    ));
                }
            }

            PathNodeType::Requirement => {
                // Requirements line
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}├──{}", prefix, msg));
                } else {
                    lines.push(format!("{}├──Requirements", prefix));
                }

                let child_prefix = format!("{}│   ", prefix);
                for child in &self.children {
                    child.render_box_node(lines, &child_prefix);
                }

                // Requirement met/not met
                if let Some(ref result) = self.result {
                    if result.to_bool() {
                        lines.push(format!("{}├──Requirement met", prefix));
                    } else {
                        lines.push(format!("{}├──Requirement NOT met", prefix));
                    }
                }
            }

            PathNodeType::Resolve => {
                // Resolving $VAR
                lines.push(format!(
                    "{}├──Resolving ${}",
                    prefix,
                    self.name.to_uppercase()
                ));

                let child_prefix = format!("{}│   ", prefix);

                // Show resolution source
                if let Some(ref rt) = self.resolve_type {
                    let rt_name = resolve_type_name(rt);
                    let val_str = self
                        .result
                        .as_ref()
                        .map(format_value_display)
                        .unwrap_or_else(|| "?".to_string());
                    lines.push(format!(
                        "{}├──Resolving from {}: {}",
                        child_prefix, rt_name, val_str
                    ));
                } else if let Some(ref msg) = self.message {
                    lines.push(format!("{}├──{}", child_prefix, msg));
                }

                // Render children (sub-resolutions)
                for child in &self.children {
                    child.render_box_node(lines, &child_prefix);
                }
            }

            PathNodeType::Operation => {
                // Compute OP(args) = result
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}├──{}", prefix, msg));
                } else {
                    let result_str = self
                        .result
                        .as_ref()
                        .map(format_value_display)
                        .unwrap_or_else(|| "?".to_string());
                    lines.push(format!(
                        "{}├──Compute {} = {}",
                        prefix, self.name, result_str
                    ));
                }

                let child_prefix = format!("{}│   ", prefix);
                for child in &self.children {
                    child.render_box_node(lines, &child_prefix);
                }
            }

            PathNodeType::Action => {
                // Computing output_name
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}├──{}", prefix, msg));
                } else {
                    lines.push(format!("{}├──Computing {}", prefix, self.name));
                }

                let child_prefix = format!("{}│   ", prefix);
                for child in &self.children {
                    child.render_box_node(lines, &child_prefix);
                }

                // Result of output_name: value
                if let Some(ref result) = self.result {
                    // Use the top-level article prefix (║──) for results instead of branch prefix
                    // Pop the last 4 chars ("│   ") from prefix and replace with "║──"
                    let result_prefix = if let Some(stripped) = prefix.strip_suffix("│   ") {
                        format!("{}║──", stripped)
                    } else {
                        format!("{}├──", prefix)
                    };
                    lines.push(format!(
                        "{}Result of {}: {}",
                        result_prefix,
                        self.name,
                        format_value_display(result)
                    ));
                }
            }

            PathNodeType::UriCall => {
                // Nested sub-law call: render with its own ║ scope
                if let Some(ref msg) = self.message {
                    lines.push(format!("{}├──{}", prefix, msg));
                } else {
                    lines.push(format!("{}├──URI call: {}", prefix, self.name));
                }

                let child_prefix = format!("{}║   ", prefix);
                for child in &self.children {
                    child.render_box_node(lines, &child_prefix);
                }
            }

            PathNodeType::Cached => {
                let result_str = self
                    .result
                    .as_ref()
                    .map(|v| format!(": {}", format_value_display(v)))
                    .unwrap_or_default();
                lines.push(format!("{}├──CACHED: {}{}", prefix, self.name, result_str));
            }

            PathNodeType::OpenTermResolution => {
                let result_str = self
                    .result
                    .as_ref()
                    .map(|v| format!(": {}", format_value_display(v)))
                    .unwrap_or_default();
                let msg = self.message.as_deref().unwrap_or(&self.name);
                lines.push(format!("{}├──OPEN_TERM: {}{}", prefix, msg, result_str));

                let child_prefix = format!("{}│   ", prefix);
                for child in &self.children {
                    child.render_box_node(lines, &child_prefix);
                }
            }
        }
    }

    /// Render the trace as a compact single-line summary.
    pub fn render_compact(&self) -> String {
        let type_str = match self.node_type {
            PathNodeType::Resolve => "res",
            PathNodeType::Operation => "op",
            PathNodeType::Action => "act",
            PathNodeType::Requirement => "req",
            PathNodeType::UriCall => "uri",
            PathNodeType::Article => "art",
            PathNodeType::Cached => "cache",
            PathNodeType::OpenTermResolution => "ot",
        };

        let result_str = self
            .result
            .as_ref()
            .map(|v| format!("={}", format_value_compact(v)))
            .unwrap_or_default();

        format!("{}:{}{}", type_str, self.name, result_str)
    }
}

/// Format a Value compactly for trace output.
fn format_value_compact(value: &Value) -> String {
    match value {
        Value::Null => "null".to_string(),
        Value::Bool(b) => b.to_string(),
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            // Limit decimal places
            if f.fract() == 0.0 {
                format!("{:.1}", f)
            } else {
                format!("{:.2}", f)
            }
        }
        Value::String(s) => {
            // Truncate long strings (use chars() for UTF-8 safety)
            if s.chars().count() > 20 {
                let truncated: String = s.chars().take(17).collect();
                format!("\"{}...\"", truncated)
            } else {
                format!("\"{}\"", s)
            }
        }
        Value::Array(arr) => {
            if arr.len() <= 3 {
                let items: Vec<String> = arr.iter().map(format_value_compact).collect();
                format!("[{}]", items.join(", "))
            } else {
                format!("[{} items]", arr.len())
            }
        }
        Value::Object(obj) => {
            format!("{{...{} keys}}", obj.len())
        }
    }
}

/// Format a Value for box-drawing trace output.
///
/// Uses display formatting: True/False for bools, quoted strings, etc.
fn format_value_display(value: &Value) -> String {
    match value {
        Value::Null => "None".to_string(),
        Value::Bool(b) => {
            if *b {
                "True".to_string()
            } else {
                "False".to_string()
            }
        }
        Value::Int(i) => i.to_string(),
        Value::Float(f) => {
            if f.fract() == 0.0 {
                format!("{:.1}", f)
            } else {
                format!("{}", f)
            }
        }
        Value::String(s) => format!("'{}'", s),
        Value::Array(arr) => {
            let items: Vec<String> = arr.iter().map(format_value_display).collect();
            format!("[{}]", items.join(", "))
        }
        Value::Object(obj) => {
            let mut keys: Vec<&String> = obj.keys().collect();
            keys.sort();
            let items: Vec<String> = keys
                .iter()
                .map(|k| format!("'{}': {}", k, format_value_display(&obj[k.as_str()])))
                .collect();
            format!("{{{}}}", items.join(", "))
        }
    }
}

/// Get a human-readable name for a ResolveType.
fn resolve_type_name(rt: &ResolveType) -> &'static str {
    match rt {
        ResolveType::Uri => "URI",
        ResolveType::Parameter => "PARAMETERS",
        ResolveType::Definition => "DEFINITION",
        ResolveType::Output => "OUTPUT",
        ResolveType::Input => "INPUT",
        ResolveType::Local => "LOCAL",
        ResolveType::Context => "CONTEXT",
        ResolveType::ResolvedInput => "RESOLVED_INPUT",
        ResolveType::DataSource => "DATA_SOURCE",
        ResolveType::OpenTerm => "OPEN_TERM",
    }
}

/// A node being built, with timing information.
#[derive(Debug)]
struct BuildingNode {
    node: PathNode,
    start_time: Instant,
}

/// Builder for constructing execution traces using a stack-based approach.
///
/// The builder maintains a stack of nodes being constructed. As execution
/// proceeds, nodes are pushed when entering a new scope and popped when
/// leaving. This naturally produces the tree structure of nested execution.
#[derive(Debug)]
pub struct TraceBuilder {
    /// Stack of nodes being built (last is current)
    stack: Vec<BuildingNode>,

    /// Whether tracing is enabled
    enabled: bool,
}

impl Default for TraceBuilder {
    fn default() -> Self {
        Self::new()
    }
}

impl TraceBuilder {
    /// Create a new TraceBuilder with tracing enabled.
    pub fn new() -> Self {
        Self {
            stack: Vec::new(),
            enabled: true,
        }
    }

    /// Create a new TraceBuilder with tracing disabled (no-op).
    pub fn disabled() -> Self {
        Self {
            stack: Vec::new(),
            enabled: false,
        }
    }

    /// Check if tracing is enabled.
    pub fn is_enabled(&self) -> bool {
        self.enabled
    }

    /// Push a new node onto the stack.
    ///
    /// Call this when entering a new execution scope (resolving a variable,
    /// starting an operation, etc.).
    pub fn push(&mut self, name: impl Into<String>, node_type: PathNodeType) {
        if !self.enabled {
            return;
        }

        let node = PathNode::new(node_type, name);
        self.stack.push(BuildingNode {
            node,
            start_time: Instant::now(),
        });
    }

    /// Set the result value for the current node.
    pub fn set_result(&mut self, result: Value) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.result = Some(result);
        }
    }

    /// Set a free-form message on the current node.
    pub fn set_message(&mut self, msg: impl Into<String>) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.message = Some(msg.into());
        }
    }

    /// Set the resolve type for the current node.
    pub fn set_resolve_type(&mut self, resolve_type: ResolveType) {
        if !self.enabled {
            return;
        }

        if let Some(current) = self.stack.last_mut() {
            current.node.resolve_type = Some(resolve_type);
        }
    }

    /// Pop the current node from the stack, making it a child of the parent.
    ///
    /// Returns the popped node. If this was the last node on the stack,
    /// returns the completed root node.
    pub fn pop(&mut self) -> Option<PathNode> {
        if !self.enabled {
            return None;
        }

        let building = self.stack.pop()?;
        let duration = building.start_time.elapsed().as_micros() as u64;

        let mut completed = building.node;
        completed.duration_us = Some(duration);

        // If there's a parent, add this as a child (move, not clone)
        if let Some(parent) = self.stack.last_mut() {
            parent.node.children.push(completed);
            return None;
        }

        Some(completed)
    }

    /// Build the final trace, consuming the builder.
    ///
    /// Pops all remaining nodes from the stack, returning the root node.
    /// Returns None if the stack is empty or tracing was disabled.
    pub fn build(mut self) -> Option<PathNode> {
        if !self.enabled {
            return None;
        }

        // Pop all nodes to build complete tree
        let mut result = None;
        while !self.stack.is_empty() {
            result = self.pop();
        }
        result
    }

    /// Get the current depth of the trace stack.
    pub fn depth(&self) -> usize {
        self.stack.len()
    }

    /// Check if the stack is empty.
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_path_node_creation() {
        let node = PathNode::new(PathNodeType::Resolve, "test_var");
        assert_eq!(node.name, "test_var");
        assert!(matches!(node.node_type, PathNodeType::Resolve));
        assert!(node.result.is_none());
        assert!(node.children.is_empty());
    }

    #[test]
    fn test_path_node_builder_pattern() {
        let node = PathNode::new(PathNodeType::Operation, "ADD")
            .with_result(Value::Int(42))
            .with_child(PathNode::new(PathNodeType::Resolve, "a"))
            .with_child(PathNode::new(PathNodeType::Resolve, "b"));

        assert_eq!(node.result, Some(Value::Int(42)));
        assert_eq!(node.children.len(), 2);
    }

    #[test]
    fn test_trace_builder_simple() {
        let mut builder = TraceBuilder::new();
        assert!(builder.is_enabled());
        assert!(builder.is_empty());

        builder.push("root", PathNodeType::Resolve);
        assert_eq!(builder.depth(), 1);

        builder.set_result(Value::Int(100));
        let node = builder.pop().unwrap();

        assert_eq!(node.name, "root");
        assert_eq!(node.result, Some(Value::Int(100)));
        assert!(node.duration_us.is_some());
    }

    #[test]
    fn test_trace_builder_nested() {
        let mut builder = TraceBuilder::new();

        // Root node
        builder.push("calculate", PathNodeType::Action);

        // Nested operation
        builder.push("add_values", PathNodeType::Operation);
        builder.set_result(Value::Int(30));
        builder.pop();

        // Complete root
        builder.set_result(Value::Int(30));
        let root = builder.build().unwrap();

        assert_eq!(root.name, "calculate");
        assert_eq!(root.children.len(), 1);
        assert_eq!(root.children[0].name, "add_values");
        assert_eq!(root.children[0].result, Some(Value::Int(30)));
    }

    #[test]
    fn test_trace_builder_disabled() {
        let mut builder = TraceBuilder::disabled();
        assert!(!builder.is_enabled());

        builder.push("should_be_ignored", PathNodeType::Resolve);
        builder.set_result(Value::Int(42));

        assert!(builder.is_empty()); // Nothing was actually pushed
        assert!(builder.pop().is_none());
        assert!(builder.build().is_none());
    }

    #[test]
    fn test_trace_builder_resolve_type() {
        let mut builder = TraceBuilder::new();

        builder.push("param", PathNodeType::Resolve);
        builder.set_resolve_type(ResolveType::Parameter);
        builder.set_result(Value::String("test".to_string()));

        let node = builder.pop().unwrap();
        assert_eq!(node.resolve_type, Some(ResolveType::Parameter));
    }

    #[test]
    fn test_path_node_serialization() {
        let node = PathNode::new(PathNodeType::Resolve, "test")
            .with_result(Value::Int(42))
            .with_resolve_type(ResolveType::Definition);

        let json = serde_json::to_string(&node).unwrap();
        assert!(json.contains("\"name\":\"test\""));
        assert!(json.contains("\"result\":42"));
    }

    #[test]
    fn test_deeply_nested_trace() {
        let mut builder = TraceBuilder::new();

        // Build a 3-level deep trace
        builder.push("level1", PathNodeType::Action);
        builder.push("level2", PathNodeType::Operation);
        builder.push("level3", PathNodeType::Resolve);
        builder.set_result(Value::Int(1));
        builder.pop(); // level3

        builder.set_result(Value::Int(2));
        builder.pop(); // level2

        builder.set_result(Value::Int(3));
        let root = builder.build().unwrap();

        assert_eq!(root.name, "level1");
        assert_eq!(root.result, Some(Value::Int(3)));
        assert_eq!(root.children.len(), 1);

        let level2 = &root.children[0];
        assert_eq!(level2.name, "level2");
        assert_eq!(level2.result, Some(Value::Int(2)));
        assert_eq!(level2.children.len(), 1);

        let level3 = &level2.children[0];
        assert_eq!(level3.name, "level3");
        assert_eq!(level3.result, Some(Value::Int(1)));
        assert!(level3.children.is_empty());
    }

    // -------------------------------------------------------------------------
    // Render Tests
    // -------------------------------------------------------------------------

    #[test]
    fn test_render_simple_node() {
        let node = PathNode::new(PathNodeType::Resolve, "inkomen")
            .with_result(Value::Int(50000))
            .with_resolve_type(ResolveType::Parameter);

        let rendered = node.render(0, false);
        assert!(rendered.contains("inkomen"));
        assert!(rendered.contains("resolve"));
        assert!(rendered.contains("parameter"));
        assert!(rendered.contains("50000"));
    }

    #[test]
    fn test_render_nested_trace() {
        let child1 = PathNode::new(PathNodeType::Resolve, "a")
            .with_result(Value::Int(10))
            .with_resolve_type(ResolveType::Parameter);

        let child2 = PathNode::new(PathNodeType::Resolve, "b")
            .with_result(Value::Int(20))
            .with_resolve_type(ResolveType::Definition);

        let root = PathNode::new(PathNodeType::Operation, "ADD")
            .with_result(Value::Int(30))
            .with_child(child1)
            .with_child(child2);

        let rendered = root.render(0, false);

        // Check structure
        assert!(rendered.contains("ADD (operation)"));
        assert!(rendered.contains("a (resolve)"));
        assert!(rendered.contains("b (resolve)"));

        // Check tree characters
        assert!(rendered.contains("+--") || rendered.contains("`--"));
    }

    #[test]
    fn test_render_complex_tree() {
        // Build a more complex tree
        let resolve_a = PathNode::new(PathNodeType::Resolve, "var_a").with_result(Value::Int(100));

        let resolve_b = PathNode::new(PathNodeType::Resolve, "var_b").with_result(Value::Int(200));

        let add_op = PathNode::new(PathNodeType::Operation, "ADD")
            .with_result(Value::Int(300))
            .with_child(resolve_a)
            .with_child(resolve_b);

        let article = PathNode::new(PathNodeType::Article, "artikel_1").with_child(add_op);

        let rendered = article.render(0, false);

        // Verify the structure is readable
        let lines: Vec<&str> = rendered.lines().collect();
        assert!(lines[0].contains("artikel_1"));
        assert!(lines.iter().any(|l| l.contains("ADD")));
        assert!(lines.iter().any(|l| l.contains("var_a")));
        assert!(lines.iter().any(|l| l.contains("var_b")));
    }

    #[test]
    fn test_render_compact() {
        let node = PathNode::new(PathNodeType::Operation, "MULTIPLY").with_result(Value::Int(42));

        let compact = node.render_compact();
        assert_eq!(compact, "op:MULTIPLY=42");
    }

    #[test]
    fn test_render_compact_resolve() {
        let node = PathNode::new(PathNodeType::Resolve, "param")
            .with_result(Value::String("test".to_string()));

        let compact = node.render_compact();
        assert!(compact.starts_with("res:param="));
    }

    #[test]
    fn test_format_value_compact_truncates_long_strings() {
        let long_string = "this is a very long string that should be truncated";
        let formatted = format_value_compact(&Value::String(long_string.to_string()));
        assert!(formatted.len() < long_string.len());
        assert!(formatted.contains("..."));
    }

    #[test]
    fn test_format_value_compact_utf8_safety() {
        // Dutch legal text with diacritics - would panic with byte slicing at position 17
        // if using &s[..17] instead of chars().take(17)
        let dutch_text = "éénentwintigste regeling met diakritische tekens";
        assert!(
            dutch_text.chars().count() > 20,
            "Test string must be > 20 chars"
        );
        let formatted = format_value_compact(&Value::String(dutch_text.to_string()));
        assert!(
            formatted.contains("..."),
            "Expected truncation for: {}",
            formatted
        );
        // Verify it produces valid output (doesn't panic on UTF-8 boundary)
        assert!(!formatted.is_empty());

        // String with multi-byte chars throughout - old code would panic
        // Each letter with accent is 2 bytes, slicing at byte 17 would cut mid-character
        let accented = "àáâãäåæçèéêëìíîïðñòóôõö";
        assert!(
            accented.chars().count() > 20,
            "Test string must be > 20 chars"
        );
        let formatted = format_value_compact(&Value::String(accented.to_string()));
        assert!(
            formatted.contains("..."),
            "Expected truncation for accented: {}",
            formatted
        );
    }

    #[test]
    fn test_format_value_compact_array() {
        let small_array = Value::Array(vec![Value::Int(1), Value::Int(2)]);
        let formatted = format_value_compact(&small_array);
        assert_eq!(formatted, "[1, 2]");

        let large_array = Value::Array(vec![
            Value::Int(1),
            Value::Int(2),
            Value::Int(3),
            Value::Int(4),
            Value::Int(5),
        ]);
        let formatted = format_value_compact(&large_array);
        assert!(formatted.contains("5 items"));
    }

    #[test]
    fn test_render_with_all_resolve_types() {
        let types = vec![
            (ResolveType::Uri, "uri"),
            (ResolveType::Parameter, "parameter"),
            (ResolveType::Definition, "definition"),
            (ResolveType::Output, "output"),
            (ResolveType::Input, "input"),
            (ResolveType::Local, "local"),
            (ResolveType::Context, "context"),
            (ResolveType::ResolvedInput, "resolved_input"),
            (ResolveType::DataSource, "data_source"),
        ];

        for (rt, expected_str) in types {
            let node = PathNode::new(PathNodeType::Resolve, "test").with_resolve_type(rt);
            let rendered = node.render(0, false);
            assert!(
                rendered.contains(&format!("[{}]", expected_str)),
                "Expected [{}] in: {}",
                expected_str,
                rendered
            );
        }
    }
}
