//! URI parsing for regelrecht:// URIs and file path references
//!
//! Parses and constructs references to law articles and fields.
//!
//! # Supported Formats
//!
//! 1. **regelrecht:// URI**: `regelrecht://{law_id}/{output}#{field}`
//! 2. **File path reference**: `regulation/nl/{layer}/{law_id}#{field}`
//! 3. **Internal reference**: `#{output_name}` (same-law reference)
//!
//! # Examples
//!
//! ```
//! use regelrecht_engine::uri::{RegelrechtUri, RegelrechtUriBuilder};
//!
//! // Parse a regelrecht:// URI
//! let uri = RegelrechtUri::parse("regelrecht://zvw/is_verzekerd#is_verzekerd").unwrap();
//! assert_eq!(uri.law_id(), "zvw");
//! assert_eq!(uri.output(), "is_verzekerd");
//! assert_eq!(uri.field(), Some("is_verzekerd"));
//!
//! // Build a URI
//! let uri = RegelrechtUriBuilder::new("zorgtoeslagwet", "bereken_zorgtoeslag")
//!     .with_field("heeft_recht_op_zorgtoeslag")
//!     .build();
//! assert_eq!(uri, "regelrecht://zorgtoeslagwet/bereken_zorgtoeslag#heeft_recht_op_zorgtoeslag");
//!
//! // Internal reference
//! let uri = RegelrechtUri::parse("#standaardpremie").unwrap();
//! assert!(uri.is_internal());
//! assert_eq!(uri.output(), "standaardpremie");
//! ```

use crate::error::{EngineError, Result};

/// Reference type indicating where the reference points
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReferenceType {
    /// Internal reference to same law (#output_name)
    Internal,
    /// External reference to another law (regelrecht:// or file path)
    External,
}

/// Parsed regelrecht:// URI or reference
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RegelrechtUri {
    /// Original URI string
    uri: String,
    /// Law identifier (e.g., "zorgtoeslagwet")
    law_id: String,
    /// Output name (e.g., "bereken_zorgtoeslag")
    output: String,
    /// Optional field to extract from output (e.g., "heeft_recht_op_zorgtoeslag")
    field: Option<String>,
    /// Reference type (internal or external)
    reference_type: ReferenceType,
}

impl RegelrechtUri {
    /// Parse a URI string into components.
    ///
    /// # Supported Formats
    ///
    /// - `regelrecht://law_id/output` - external reference
    /// - `regelrecht://law_id/output#field` - external reference with field
    /// - `regulation/nl/layer/law_id#field` - file path reference
    /// - `#output_name` - internal reference (same law)
    ///
    /// # Errors
    ///
    /// Returns `EngineError::InvalidUri` if the format is invalid.
    pub fn parse(uri: &str) -> Result<Self> {
        // Handle internal references (#output_name)
        if let Some(output) = uri.strip_prefix('#') {
            if output.is_empty() {
                return Err(EngineError::InvalidUri(
                    "Internal reference cannot be empty".to_string(),
                ));
            }
            return Ok(Self {
                uri: uri.to_string(),
                law_id: String::new(), // Internal references don't have law_id
                output: output.to_string(),
                field: Some(output.to_string()), // Field is same as output for internal refs
                reference_type: ReferenceType::Internal,
            });
        }

        // Split on fragment (#) first
        let (path_part, field) = if let Some(hash_pos) = uri.find('#') {
            let (path, frag) = uri.split_at(hash_pos);
            (path, Some(frag[1..].to_string())) // Skip the #
        } else {
            (uri, None)
        };

        // Check if it's a regelrecht:// URI
        if let Some(path) = path_part.strip_prefix("regelrecht://") {
            Self::parse_regelrecht_uri(uri, path, field)
        }
        // Check if it's a file path reference
        else if path_part.starts_with("regulation/nl/") {
            Self::parse_file_path(uri, path_part, field)
        } else {
            Err(EngineError::InvalidUri(format!(
                "Invalid URI format: must be regelrecht://, regulation/nl/..., or #reference, got: {}",
                uri
            )))
        }
    }

    /// Parse a regelrecht:// URI
    fn parse_regelrecht_uri(original: &str, path: &str, field: Option<String>) -> Result<Self> {
        // Split path on first /
        let slash_pos = path.find('/').ok_or_else(|| {
            EngineError::InvalidUri(format!(
                "Invalid regelrecht URI: must contain law_id/output, got: {}",
                original
            ))
        })?;

        let (law_id, output) = path.split_at(slash_pos);
        let output = &output[1..]; // Skip the /

        if law_id.is_empty() {
            return Err(EngineError::InvalidUri(format!(
                "Invalid regelrecht URI: law_id cannot be empty, got: {}",
                original
            )));
        }
        if output.is_empty() {
            return Err(EngineError::InvalidUri(format!(
                "Invalid regelrecht URI: output cannot be empty, got: {}",
                original
            )));
        }

        Ok(Self {
            uri: original.to_string(),
            law_id: law_id.to_string(),
            output: output.to_string(),
            field,
            reference_type: ReferenceType::External,
        })
    }

    /// Parse a file path reference (regulation/nl/layer/law_id#field)
    fn parse_file_path(original: &str, path: &str, field: Option<String>) -> Result<Self> {
        // Parse path parts: regulation/nl/layer/law_id
        let parts: Vec<&str> = path.split('/').collect();
        if parts.len() < 4 {
            return Err(EngineError::InvalidUri(format!(
                "Invalid file path reference: expected regulation/nl/layer/law_id, got: {}",
                original
            )));
        }

        // Extract law_id (last part of path)
        let law_id = parts[parts.len() - 1].to_string();

        // For file path references, the output is the field name
        // (we look up the article that produces this output)
        let output = field.clone().unwrap_or_else(|| law_id.clone());

        Ok(Self {
            uri: original.to_string(),
            law_id,
            output,
            field,
            reference_type: ReferenceType::External,
        })
    }

    /// Get the original URI string
    pub fn uri(&self) -> &str {
        &self.uri
    }

    /// Get the law identifier
    ///
    /// For internal references, this returns an empty string.
    pub fn law_id(&self) -> &str {
        &self.law_id
    }

    /// Get the output name
    pub fn output(&self) -> &str {
        &self.output
    }

    /// Get the field name (if specified)
    pub fn field(&self) -> Option<&str> {
        self.field.as_deref()
    }

    /// Get the reference type
    pub fn reference_type(&self) -> ReferenceType {
        self.reference_type
    }

    /// Check if this is an internal reference (#output_name)
    pub fn is_internal(&self) -> bool {
        self.reference_type == ReferenceType::Internal
    }

    /// Check if this is an external reference (regelrecht:// or file path)
    pub fn is_external(&self) -> bool {
        self.reference_type == ReferenceType::External
    }
}

impl std::fmt::Display for RegelrechtUri {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.uri)
    }
}

/// Builder for constructing regelrecht:// URIs in a type-safe way
#[derive(Debug, Clone)]
pub struct RegelrechtUriBuilder {
    law_id: String,
    output: String,
    field: Option<String>,
}

impl RegelrechtUriBuilder {
    /// Create a new URI builder
    ///
    /// # Arguments
    /// * `law_id` - Law identifier (e.g., "zorgtoeslagwet")
    /// * `output` - Output name (e.g., "bereken_zorgtoeslag")
    ///
    /// # Panics
    /// Panics if `law_id` or `output` is empty. Use `try_new()` for fallible construction.
    pub fn new(law_id: impl Into<String>, output: impl Into<String>) -> Self {
        let law_id = law_id.into();
        let output = output.into();
        assert!(!law_id.is_empty(), "law_id cannot be empty");
        assert!(!output.is_empty(), "output cannot be empty");
        Self {
            law_id,
            output,
            field: None,
        }
    }

    /// Create a new URI builder with validation
    ///
    /// Returns an error if `law_id` or `output` is empty.
    pub fn try_new(law_id: impl Into<String>, output: impl Into<String>) -> Result<Self> {
        let law_id = law_id.into();
        let output = output.into();
        if law_id.is_empty() {
            return Err(EngineError::InvalidUri(
                "Cannot build URI: law_id is empty".to_string(),
            ));
        }
        if output.is_empty() {
            return Err(EngineError::InvalidUri(
                "Cannot build URI: output is empty".to_string(),
            ));
        }
        Ok(Self {
            law_id,
            output,
            field: None,
        })
    }

    /// Add a field to extract from the output
    ///
    /// # Panics
    /// Panics if `field` is empty. Use `try_with_field()` for fallible construction.
    pub fn with_field(mut self, field: impl Into<String>) -> Self {
        let field = field.into();
        assert!(!field.is_empty(), "field cannot be empty");
        self.field = Some(field);
        self
    }

    /// Add a field with validation
    ///
    /// Returns an error if `field` is empty.
    pub fn try_with_field(mut self, field: impl Into<String>) -> Result<Self> {
        let field = field.into();
        if field.is_empty() {
            return Err(EngineError::InvalidUri(
                "Cannot build URI: field is empty".to_string(),
            ));
        }
        self.field = Some(field);
        Ok(self)
    }

    /// Build the URI string
    pub fn build(&self) -> String {
        let mut uri = format!("regelrecht://{}/{}", self.law_id, self.output);
        if let Some(field) = &self.field {
            uri.push('#');
            uri.push_str(field);
        }
        uri
    }

    /// Build and parse into a RegelrechtUri
    ///
    /// This method is guaranteed to succeed because the builder validates inputs.
    #[allow(clippy::unwrap_used)]
    pub fn build_parsed(&self) -> RegelrechtUri {
        // Safe to unwrap because:
        // 1. law_id and output are validated as non-empty in new()/try_new()
        // 2. field is validated as non-empty in with_field()/try_with_field()
        // 3. The format regelrecht://{law_id}/{output}#{field} is always valid
        RegelrechtUri::parse(&self.build()).unwrap()
    }
}

/// Build an internal reference URI
pub fn internal_reference(output: impl Into<String>) -> String {
    format!("#{}", output.into())
}

#[cfg(test)]
mod tests {
    use super::*;

    // -------------------------------------------------------------------------
    // RegelrechtUri Parsing Tests
    // -------------------------------------------------------------------------

    mod parsing {
        use super::*;

        #[test]
        fn test_parse_regelrecht_uri_basic() {
            let uri = RegelrechtUri::parse("regelrecht://zvw/is_verzekerd").unwrap();
            assert_eq!(uri.law_id(), "zvw");
            assert_eq!(uri.output(), "is_verzekerd");
            assert_eq!(uri.field(), None);
            assert!(uri.is_external());
        }

        #[test]
        fn test_parse_regelrecht_uri_with_field() {
            let uri = RegelrechtUri::parse("regelrecht://zvw/is_verzekerd#verzekerd").unwrap();
            assert_eq!(uri.law_id(), "zvw");
            assert_eq!(uri.output(), "is_verzekerd");
            assert_eq!(uri.field(), Some("verzekerd"));
            assert!(uri.is_external());
        }

        #[test]
        fn test_parse_regelrecht_uri_long_ids() {
            let uri = RegelrechtUri::parse(
                "regelrecht://zorgtoeslagwet/bereken_zorgtoeslag#heeft_recht_op_zorgtoeslag",
            )
            .unwrap();
            assert_eq!(uri.law_id(), "zorgtoeslagwet");
            assert_eq!(uri.output(), "bereken_zorgtoeslag");
            assert_eq!(uri.field(), Some("heeft_recht_op_zorgtoeslag"));
        }

        #[test]
        fn test_parse_file_path_with_field() {
            let uri = RegelrechtUri::parse(
                "regulation/nl/ministeriele_regeling/regeling_standaardpremie#standaardpremie",
            )
            .unwrap();
            assert_eq!(uri.law_id(), "regeling_standaardpremie");
            assert_eq!(uri.output(), "standaardpremie");
            assert_eq!(uri.field(), Some("standaardpremie"));
            assert!(uri.is_external());
        }

        #[test]
        fn test_parse_file_path_without_field() {
            let uri = RegelrechtUri::parse("regulation/nl/wet/zorgtoeslagwet").unwrap();
            assert_eq!(uri.law_id(), "zorgtoeslagwet");
            // Output defaults to law_id when no field
            assert_eq!(uri.output(), "zorgtoeslagwet");
            assert_eq!(uri.field(), None);
        }

        #[test]
        fn test_parse_internal_reference() {
            let uri = RegelrechtUri::parse("#standaardpremie").unwrap();
            assert!(uri.is_internal());
            assert_eq!(uri.output(), "standaardpremie");
            assert_eq!(uri.field(), Some("standaardpremie"));
            assert!(uri.law_id().is_empty());
        }

        #[test]
        fn test_parse_invalid_regelrecht_no_output() {
            let result = RegelrechtUri::parse("regelrecht://zvw");
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(matches!(err, EngineError::InvalidUri(_)));
        }

        #[test]
        fn test_parse_invalid_regelrecht_empty_law_id() {
            let result = RegelrechtUri::parse("regelrecht:///output");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_invalid_regelrecht_empty_output() {
            let result = RegelrechtUri::parse("regelrecht://law/");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_invalid_internal_empty() {
            let result = RegelrechtUri::parse("#");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_invalid_format() {
            let result = RegelrechtUri::parse("https://example.com/law");
            assert!(result.is_err());
        }

        #[test]
        fn test_parse_invalid_file_path_too_short() {
            let result = RegelrechtUri::parse("regulation/nl/wet");
            assert!(result.is_err());
        }
    }

    // -------------------------------------------------------------------------
    // RegelrechtUriBuilder Tests
    // -------------------------------------------------------------------------

    mod builder {
        use super::*;

        #[test]
        fn test_build_basic() {
            let uri = RegelrechtUriBuilder::new("zorgtoeslagwet", "bereken_zorgtoeslag").build();
            assert_eq!(uri, "regelrecht://zorgtoeslagwet/bereken_zorgtoeslag");
        }

        #[test]
        fn test_build_with_field() {
            let uri = RegelrechtUriBuilder::new("zvw", "is_verzekerd")
                .with_field("is_verzekerd")
                .build();
            assert_eq!(uri, "regelrecht://zvw/is_verzekerd#is_verzekerd");
        }

        #[test]
        fn test_build_parsed() {
            let uri = RegelrechtUriBuilder::new("law", "output")
                .with_field("field")
                .build_parsed();
            assert_eq!(uri.law_id(), "law");
            assert_eq!(uri.output(), "output");
            assert_eq!(uri.field(), Some("field"));
        }

        #[test]
        fn test_try_new_success() {
            let builder = RegelrechtUriBuilder::try_new("law", "output").unwrap();
            assert_eq!(builder.build(), "regelrecht://law/output");
        }

        #[test]
        fn test_try_new_empty_law_id() {
            let result = RegelrechtUriBuilder::try_new("", "output");
            assert!(result.is_err());
            if let Err(EngineError::InvalidUri(msg)) = result {
                assert!(msg.contains("law_id"));
            }
        }

        #[test]
        fn test_try_new_empty_output() {
            let result = RegelrechtUriBuilder::try_new("law", "");
            assert!(result.is_err());
            if let Err(EngineError::InvalidUri(msg)) = result {
                assert!(msg.contains("output"));
            }
        }

        #[test]
        fn test_try_with_field_success() {
            let builder = RegelrechtUriBuilder::try_new("law", "output")
                .unwrap()
                .try_with_field("field")
                .unwrap();
            assert_eq!(builder.build(), "regelrecht://law/output#field");
        }

        #[test]
        fn test_try_with_field_empty() {
            let result = RegelrechtUriBuilder::try_new("law", "output")
                .unwrap()
                .try_with_field("");
            assert!(result.is_err());
            if let Err(EngineError::InvalidUri(msg)) = result {
                assert!(msg.contains("field"));
            }
        }

        #[test]
        #[should_panic(expected = "law_id cannot be empty")]
        fn test_new_panics_on_empty_law_id() {
            let _ = RegelrechtUriBuilder::new("", "output");
        }

        #[test]
        #[should_panic(expected = "output cannot be empty")]
        fn test_new_panics_on_empty_output() {
            let _ = RegelrechtUriBuilder::new("law", "");
        }

        #[test]
        #[should_panic(expected = "field cannot be empty")]
        fn test_with_field_panics_on_empty() {
            let _ = RegelrechtUriBuilder::new("law", "output").with_field("");
        }
    }

    // -------------------------------------------------------------------------
    // Helper Function Tests
    // -------------------------------------------------------------------------

    mod helpers {
        use super::*;

        #[test]
        fn test_internal_reference() {
            let uri = internal_reference("standaardpremie");
            assert_eq!(uri, "#standaardpremie");
        }

        #[test]
        fn test_internal_reference_roundtrip() {
            let uri_str = internal_reference("output_name");
            let parsed = RegelrechtUri::parse(&uri_str).unwrap();
            assert!(parsed.is_internal());
            assert_eq!(parsed.output(), "output_name");
        }
    }

    // -------------------------------------------------------------------------
    // Display Trait Tests
    // -------------------------------------------------------------------------

    mod display {
        use super::*;

        #[test]
        fn test_display() {
            let uri = RegelrechtUri::parse("regelrecht://law/output#field").unwrap();
            assert_eq!(uri.to_string(), "regelrecht://law/output#field");
        }

        #[test]
        fn test_display_internal() {
            let uri = RegelrechtUri::parse("#output").unwrap();
            assert_eq!(uri.to_string(), "#output");
        }
    }
}
