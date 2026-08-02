//! Article-based law loading (engine side).
//!
//! The law-YAML **document model** lives in the dependency-light
//! [`regelrecht_law_model`] crate and is re-exported below so the historical
//! `regelrecht_engine::article::*` paths keep working unchanged. This module
//! adds the engine-only loading layer on top of that model: the [`LawLoad`]
//! trait, which deserializes a law from YAML and enforces the security limits
//! ([`crate::config`]) — YAML size, array sizes — and the RFC-013 schema-version
//! check and content hash.
//!
//! # Security
//!
//! - **YAML size limits**: prevents YAML-bomb attacks (see [`config::MAX_YAML_SIZE`])
//! - **Array size limits**: prevents DoS via huge arrays (see [`config::MAX_ARRAY_SIZE`])

use crate::config;
use crate::error::{EngineError, Result};
use std::fs;
use std::path::Path;

/// Re-export the canonical document model at the historical `article` path.
pub use regelrecht_law_model::{
    Action, ActionOperation, ActionValue, Article, ArticleBasedLaw, ArticleReference, Case,
    CompetentAuthority, Declaration, DeclaredProperty, Definition, Execution, HookDeclaration,
    HookFilter, HookPoint, ImplementsDeclaration, Input, LegalBasis, MachineReadable, Marking,
    MarkingResolution, OpenTerm, OpenTermDefault, Output, OverrideDeclaration, Parameter,
    Placement, PlacementContainer, ProcedureAppliesTo, ProcedureDefinition, Produces, Source,
    Stage, StageRequirement, TypeSpec, UntranslatableEntry,
};

/// Engine-side loading of an [`ArticleBasedLaw`] from YAML, with the security
/// limits and RFC-013 schema-version / provenance checks applied.
///
/// Loading lives here (not in `regelrecht-law-model`) because it depends on the
/// engine's configurable limits and error type. Bring this trait into scope to
/// call `ArticleBasedLaw::from_yaml_str` / `from_yaml_file`.
pub trait LawLoad: Sized {
    /// Load a law from a YAML file (enforces the YAML size limit before reading).
    ///
    /// # Errors
    ///
    /// Returns [`EngineError::LoadError`] if the file cannot be read or exceeds
    /// the maximum size, and any error from [`LawLoad::from_yaml_str`].
    fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self>;

    /// Parse a law from a YAML string.
    ///
    /// # Errors
    ///
    /// Returns an error if the content exceeds the size limit, the YAML is
    /// invalid, an array exceeds the maximum size, or the schema version is not
    /// supported (RFC-013).
    fn from_yaml_str(content: &str) -> Result<Self>;
}

impl LawLoad for ArticleBasedLaw {
    fn from_yaml_file<P: AsRef<Path>>(path: P) -> Result<Self> {
        let path_ref = path.as_ref();

        // Log the load attempt (without exposing full path in errors)
        tracing::debug!(path = %path_ref.display(), "Loading law from YAML file");

        // Note on path traversal protection:
        // We don't implement strict path traversal checking here because:
        // 1. Legitimate use cases (like tests) often need relative paths with ".."
        // 2. The engine is typically used in controlled server environments
        // 3. File permissions and sandboxing should be handled at the OS/container level
        //
        // For production deployments, consider:
        // - Running in a container with limited filesystem access
        // - Using a whitelist of allowed directories
        // - Canonicalizing paths against a known base directory

        // Read file with size check
        let metadata = fs::metadata(path_ref).map_err(|_| {
            // Sanitized error message - don't expose path details
            EngineError::LoadError("Failed to access law file".to_string())
        })?;

        let file_size = metadata.len() as usize;
        if file_size > config::MAX_YAML_SIZE {
            tracing::warn!(
                size = file_size,
                max = config::MAX_YAML_SIZE,
                "YAML file exceeds size limit"
            );
            return Err(EngineError::LoadError(format!(
                "File exceeds maximum size limit ({} bytes)",
                config::MAX_YAML_SIZE
            )));
        }

        let content = fs::read_to_string(path_ref).map_err(|_| {
            // Sanitized error message
            EngineError::LoadError("Failed to read law file".to_string())
        })?;

        Self::from_yaml_str(&content)
    }

    fn from_yaml_str(content: &str) -> Result<Self> {
        // Check content size before parsing
        if content.len() > config::MAX_YAML_SIZE {
            tracing::warn!(
                size = content.len(),
                max = config::MAX_YAML_SIZE,
                "YAML content exceeds size limit"
            );
            return Err(EngineError::LoadError(format!(
                "YAML content exceeds maximum size limit ({} bytes)",
                config::MAX_YAML_SIZE
            )));
        }

        let mut law: Self = serde_yaml_ng::from_str(content).map_err(EngineError::YamlError)?;

        // Validate array sizes after parsing
        validate_array_sizes(&law)?;

        // Validate schema version is supported (RFC-013)
        if let Some(version) = law.schema_version() {
            if !config::SUPPORTED_SCHEMAS.contains(&version) {
                return Err(EngineError::LoadError(format!(
                    "Unsupported schema version '{}' in law '{}'. Supported: {:?}",
                    version,
                    law.id,
                    config::SUPPORTED_SCHEMAS
                )));
            }
        }

        // Compute SHA-256 content hash for provenance (RFC-013)
        use sha2::Digest;
        let hash = sha2::Sha256::digest(content.as_bytes());
        law.content_hash = Some(format!("sha256:{}", hex::encode(hash)));

        tracing::debug!(law_id = %law.id, articles = law.articles.len(), "Parsed law successfully");

        Ok(law)
    }
}

/// Validate that all arrays in the law are within size limits.
///
/// This prevents DoS attacks via YAML documents with extremely large arrays.
fn validate_array_sizes(law: &ArticleBasedLaw) -> Result<()> {
    // Check articles array
    if law.articles.len() > config::MAX_ARRAY_SIZE {
        return Err(EngineError::LoadError(format!(
            "Too many articles ({}, max {})",
            law.articles.len(),
            config::MAX_ARRAY_SIZE
        )));
    }

    // Check each article's nested arrays
    for article in &law.articles {
        if let Some(mr) = &article.machine_readable {
            // Check open_terms array
            if let Some(open_terms) = &mr.open_terms {
                if open_terms.len() > config::MAX_ARRAY_SIZE {
                    return Err(EngineError::LoadError(format!(
                        "Too many open_terms in article {} ({}, max {})",
                        article.number,
                        open_terms.len(),
                        config::MAX_ARRAY_SIZE
                    )));
                }
            }

            // Check implements array
            if let Some(implements) = &mr.implements {
                if implements.len() > config::MAX_ARRAY_SIZE {
                    return Err(EngineError::LoadError(format!(
                        "Too many implements in article {} ({}, max {})",
                        article.number,
                        implements.len(),
                        config::MAX_ARRAY_SIZE
                    )));
                }
            }

            if let Some(exec) = &mr.execution {
                // Check parameters
                if let Some(params) = &exec.parameters {
                    if params.len() > config::MAX_ARRAY_SIZE {
                        return Err(EngineError::LoadError(format!(
                            "Too many parameters in article {} ({}, max {})",
                            article.number,
                            params.len(),
                            config::MAX_ARRAY_SIZE
                        )));
                    }
                }

                // Check inputs
                if let Some(inputs) = &exec.input {
                    if inputs.len() > config::MAX_ARRAY_SIZE {
                        return Err(EngineError::LoadError(format!(
                            "Too many inputs in article {} ({}, max {})",
                            article.number,
                            inputs.len(),
                            config::MAX_ARRAY_SIZE
                        )));
                    }
                }

                // Check outputs
                if let Some(outputs) = &exec.output {
                    if outputs.len() > config::MAX_ARRAY_SIZE {
                        return Err(EngineError::LoadError(format!(
                            "Too many outputs in article {} ({}, max {})",
                            article.number,
                            outputs.len(),
                            config::MAX_ARRAY_SIZE
                        )));
                    }
                }

                // Check actions
                if let Some(actions) = &exec.actions {
                    if actions.len() > config::MAX_ARRAY_SIZE {
                        return Err(EngineError::LoadError(format!(
                            "Too many actions in article {} ({}, max {})",
                            article.number,
                            actions.len(),
                            config::MAX_ARRAY_SIZE
                        )));
                    }

                    // Check nested arrays in actions (values, conditions, cases)
                    for action in actions {
                        validate_action_arrays(action, &article.number)?;
                    }
                }
            }
        }
    }

    Ok(())
}

/// Validate arrays within an action.
fn validate_action_arrays(action: &Action, article_number: &str) -> Result<()> {
    if let Some(values) = &action.values {
        if values.len() > config::MAX_ARRAY_SIZE {
            return Err(EngineError::LoadError(format!(
                "Too many values in action in article {} ({}, max {})",
                article_number,
                values.len(),
                config::MAX_ARRAY_SIZE
            )));
        }
    }

    if let Some(conditions) = &action.conditions {
        if conditions.len() > config::MAX_ARRAY_SIZE {
            return Err(EngineError::LoadError(format!(
                "Too many conditions in action in article {} ({}, max {})",
                article_number,
                conditions.len(),
                config::MAX_ARRAY_SIZE
            )));
        }
    }

    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Operation, ParameterType, RegulatoryLayer, Value};

    const MINIMAL_LAW_YAML: &str = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article text
"#;

    const LAW_WITH_OUTPUTS_YAML: &str = r#"
$id: law_with_outputs
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: First article
    machine_readable:
      definitions:
        CONSTANT_VALUE:
          value: 100
      execution:
        output:
          - name: test_output
            type: boolean
        actions:
          - output: test_output
            value: true
  - number: '2'
    text: Second article
    machine_readable:
      execution:
        output:
          - name: another_output
            type: number
        actions:
          - output: another_output
            value: 42
"#;

    #[test]
    fn test_parse_minimal_law() {
        let law = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        assert_eq!(law.id, "test_law");
        assert_eq!(law.regulatory_layer, RegulatoryLayer::Wet);
        assert_eq!(law.publication_date, "2025-01-01");
        assert_eq!(law.articles.len(), 1);
        assert_eq!(law.articles[0].number, "1");
        assert_eq!(law.articles[0].text, "Test article text");
    }

    #[test]
    fn test_find_article_by_output() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();

        let article = law.find_article_by_output("test_output");
        assert!(article.is_some());
        assert_eq!(article.unwrap().number, "1");

        let article2 = law.find_article_by_output("another_output");
        assert!(article2.is_some());
        assert_eq!(article2.unwrap().number, "2");

        let not_found = law.find_article_by_output("nonexistent");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_find_article_by_number() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();

        let article = law.find_article_by_number("1");
        assert!(article.is_some());
        assert_eq!(article.unwrap().text, "First article");

        let article2 = law.find_article_by_number("2");
        assert!(article2.is_some());

        let not_found = law.find_article_by_number("99");
        assert!(not_found.is_none());
    }

    #[test]
    fn test_get_all_outputs() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();
        let outputs = law.get_all_outputs();

        assert_eq!(outputs.len(), 2);
        assert!(outputs.contains_key("test_output"));
        assert!(outputs.contains_key("another_output"));
    }

    #[test]
    fn test_get_public_articles() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();
        let public = law.get_public_articles();
        assert_eq!(public.len(), 2);
    }

    #[test]
    fn test_article_get_output_names() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();
        let names = law.articles[0].get_output_names();
        assert_eq!(names, vec!["test_output"]);
    }

    #[test]
    fn test_article_has_output() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();

        // Article 1 has "test_output"
        assert!(law.articles[0].has_output("test_output"));
        assert!(!law.articles[0].has_output("another_output"));
        assert!(!law.articles[0].has_output("nonexistent"));

        // Article 2 has "another_output"
        assert!(law.articles[1].has_output("another_output"));
        assert!(!law.articles[1].has_output("test_output"));

        // Minimal law articles have no outputs
        let minimal = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        assert!(!minimal.articles[0].has_output("anything"));
    }

    #[test]
    fn test_article_is_public() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();
        assert!(law.articles[0].is_public());

        let minimal = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        assert!(!minimal.articles[0].is_public());
    }

    #[test]
    fn test_article_get_definitions() {
        let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();
        let defs = law.articles[0]
            .get_definitions()
            .expect("should have definitions");
        assert_eq!(defs.len(), 1);
        assert!(defs.contains_key("CONSTANT_VALUE"));

        // Article without definitions should return None
        let minimal = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
        assert!(minimal.articles[0].get_definitions().is_none());
    }

    #[test]
    fn test_parse_gemeentelijke_verordening() {
        let yaml = r#"
$id: apv_amsterdam
uuid: a0a0a0a0-0000-0000-0000-000000000363
regulatory_layer: GEMEENTELIJKE_VERORDENING
publication_date: '2024-01-01'
gemeente_code: GM0363
officiele_titel: APV Amsterdam
articles:
  - number: '1'
    text: Test
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        assert_eq!(law.id, "apv_amsterdam");
        assert_eq!(
            law.regulatory_layer,
            RegulatoryLayer::GemeentelijkeVerordening
        );
        assert_eq!(law.gemeente_code, Some("GM0363".to_string()));
        assert_eq!(
            law.uuid,
            Some("a0a0a0a0-0000-0000-0000-000000000363".to_string())
        );
    }

    #[test]
    fn test_parse_waterschaps_verordening() {
        let yaml = r#"
$id: keur_waterschap_test
uuid: b1b1b1b1-0000-0000-0000-000000000653
regulatory_layer: WATERSCHAPS_VERORDENING
publication_date: '2024-01-01'
waterschap_code: WS0653
officiele_titel: Keur Waterschap Test
articles:
  - number: '1'
    text: Test
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        assert_eq!(law.id, "keur_waterschap_test");
        assert_eq!(
            law.regulatory_layer,
            RegulatoryLayer::WaterschapsVerordening
        );
        assert_eq!(law.waterschap_code, Some("WS0653".to_string()));
        assert_eq!(
            law.uuid,
            Some("b1b1b1b1-0000-0000-0000-000000000653".to_string())
        );
    }

    #[test]
    fn test_parse_ministeriele_regeling() {
        let yaml = r#"
$id: regeling_test
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-01-01'
bwb_id: BWBR0050536
url: https://wetten.overheid.nl/test
legal_basis:
  - law_id: test_law
    article: '1'
    description: Test basis
articles:
  - number: '1'
    text: Test
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        assert_eq!(law.regulatory_layer, RegulatoryLayer::MinisterieleRegeling);
        assert_eq!(law.bwb_id, Some("BWBR0050536".to_string()));
        assert!(law.legal_basis.is_some());
        let basis = law.legal_basis.as_ref().unwrap();
        assert_eq!(basis.len(), 1);
        assert_eq!(basis[0].law_id, "test_law");
    }

    #[test]
    fn test_parse_competent_authority_string() {
        let yaml = r#"
$id: test
regulatory_layer: WET
publication_date: '2024-01-01'
competent_authority: '#bevoegd_gezag'
articles: []
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        match law.competent_authority {
            Some(CompetentAuthority::String(s)) => assert_eq!(s, "#bevoegd_gezag"),
            _ => panic!("Expected string authority"),
        }
    }

    #[test]
    fn test_parse_competent_authority_structured() {
        let yaml = r#"
$id: test
regulatory_layer: WET
publication_date: '2024-01-01'
competent_authority:
  name: Minister van Test
articles: []
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        match law.competent_authority {
            Some(CompetentAuthority::Structured { name }) => {
                assert_eq!(name, "Minister van Test")
            }
            _ => panic!("Expected structured authority"),
        }
    }

    #[test]
    fn test_parse_action_with_nested_operations() {
        let yaml = r#"
$id: test
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Test
    machine_readable:
      execution:
        output:
          - name: result
            type: number
        actions:
          - output: result
            operation: MAX
            values:
              - 0
              - operation: SUBTRACT
                values:
                  - 100
                  - 50
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = &law.articles[0];
        let exec = article.get_execution_spec().unwrap();
        let actions = exec.actions.as_ref().unwrap();
        assert_eq!(actions.len(), 1);
        assert_eq!(actions[0].operation, Some(Operation::Max));
    }

    #[test]
    fn test_parse_action_with_if_operation() {
        let yaml = r#"
$id: test
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Test
    machine_readable:
      execution:
        output:
          - name: result
            type: number
        actions:
          - output: result
            value:
              operation: IF
              cases:
                - when:
                    operation: EQUALS
                    subject: $has_partner
                    value: true
                  then: 100
              default: 50
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let article = &law.articles[0];
        let exec = article.get_execution_spec().unwrap();
        let actions = exec.actions.as_ref().unwrap();
        assert_eq!(actions.len(), 1);

        match &actions[0].value {
            Some(ActionValue::Operation(op)) => {
                assert!(
                    matches!(
                        op.as_ref(),
                        ActionOperation::If {
                            cases: _,
                            default: Some(_)
                        }
                    ),
                    "Expected IF operation with cases and default"
                );
            }
            _ => panic!("Expected operation value"),
        }
    }

    #[test]
    fn test_parse_input_with_source() {
        let yaml = r#"
$id: test
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Test
    machine_readable:
      execution:
        input:
          - name: external_value
            type: number
            source:
              regulation: other_law
              output: some_output
              parameters:
                BSN: $BSN
        output:
          - name: result
            type: number
        actions:
          - output: result
            value: $external_value
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let exec = law.articles[0].get_execution_spec().unwrap();
        let inputs = exec.input.as_ref().unwrap();
        assert_eq!(inputs.len(), 1);

        let source = inputs[0].source.as_ref().unwrap();
        assert_eq!(source.regulation, Some("other_law".to_string()));
        assert_eq!(source.output, Some("some_output".to_string()));
        assert!(source.parameters.is_some());
    }

    #[test]
    fn test_action_value_literal_fallback() {
        // Verify that objects without 'operation' field correctly fall through to Literal
        // This tests the safety of the #[serde(untagged)] enum ordering
        let yaml = r#"
$id: test
regulatory_layer: WET
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: Test
    machine_readable:
      execution:
        output:
          - name: result
            type: string
        actions:
          - output: result
            value: "simple string"
          - output: result2
            value: 42
          - output: result3
            value: true
"#;
        let law = ArticleBasedLaw::from_yaml_str(yaml).unwrap();
        let exec = law.articles[0].get_execution_spec().unwrap();
        let actions = exec.actions.as_ref().unwrap();
        assert_eq!(actions.len(), 3);

        // All values should be Literal since they don't have 'operation' field
        match &actions[0].value {
            Some(ActionValue::Literal(Value::String(s))) => assert_eq!(s, "simple string"),
            other => panic!("Expected Literal(String), got {:?}", other),
        }
        match &actions[1].value {
            Some(ActionValue::Literal(Value::Int(n))) => assert_eq!(*n, 42),
            other => panic!("Expected Literal(Int), got {:?}", other),
        }
        match &actions[2].value {
            Some(ActionValue::Literal(Value::Bool(b))) => assert!(*b),
            other => panic!("Expected Literal(Bool), got {:?}", other),
        }
    }

    // Integration tests that load real regulation files
    mod integration {
        use super::*;
        use std::path::PathBuf;

        fn get_regulation_path() -> PathBuf {
            std::env::var("REGULATION_PATH")
                .map(PathBuf::from)
                .unwrap_or_else(|_| {
                    PathBuf::from(env!("CARGO_MANIFEST_DIR"))
                        .join("..")
                        .join("..")
                        .join("corpus")
                        .join("regulation")
                })
        }

        #[test]
        fn test_load_wet_op_de_zorgtoeslag() {
            let path = get_regulation_path().join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load wet_op_de_zorgtoeslag: {}", e));

            assert_eq!(law.id, "wet_op_de_zorgtoeslag");
            assert_eq!(law.regulatory_layer, RegulatoryLayer::Wet);
            assert!(!law.articles.is_empty());

            // Verify key output can be found
            let article = law.find_article_by_output("heeft_recht_op_zorgtoeslag");
            assert!(
                article.is_some(),
                "Should find article with heeft_recht_op_zorgtoeslag output"
            );
        }

        #[test]
        fn test_load_zorgverzekeringswet() {
            let path = get_regulation_path().join("nl/wet/zorgverzekeringswet/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load zorgverzekeringswet: {}", e));

            assert_eq!(law.id, "zorgverzekeringswet");
            assert_eq!(law.regulatory_layer, RegulatoryLayer::Wet);
        }

        #[test]
        fn test_load_awir() {
            let path = get_regulation_path()
                .join("nl/wet/algemene_wet_inkomensafhankelijke_regelingen/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load AWIR: {}", e));

            assert_eq!(law.id, "algemene_wet_inkomensafhankelijke_regelingen");
        }

        #[test]
        fn test_load_kieswet() {
            let path = get_regulation_path().join("nl/wet/kieswet/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load kieswet: {}", e));

            assert_eq!(law.id, "kieswet");
        }

        #[test]
        fn test_load_wet_langdurige_zorg() {
            let path = get_regulation_path().join("nl/wet/wet_langdurige_zorg/2025-07-05.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load wet langdurige zorg: {}", e));

            assert_eq!(law.id, "wet_langdurige_zorg");
        }

        #[test]
        fn test_load_burgerlijk_wetboek_boek_5() {
            let path =
                get_regulation_path().join("nl/wet/burgerlijk_wetboek_boek_5/2024-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load BW5: {}", e));

            assert_eq!(law.id, "burgerlijk_wetboek_boek_5");
        }

        #[test]
        fn test_load_participatiewet() {
            let path = get_regulation_path().join("nl/wet/participatiewet/2022-03-15.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load participatiewet: {}", e));

            assert_eq!(law.id, "participatiewet");
        }

        #[test]
        fn test_load_wet_brp() {
            let path =
                get_regulation_path().join("nl/wet/wet_basisregistratie_personen/2025-02-12.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load wet BRP: {}", e));

            assert_eq!(law.id, "wet_basisregistratie_personen");
        }

        #[test]
        fn test_load_wet_ib_2001() {
            let path =
                get_regulation_path().join("nl/wet/wet_inkomstenbelasting_2001/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load wet IB 2001: {}", e));

            assert_eq!(law.id, "wet_inkomstenbelasting_2001");
        }

        #[test]
        fn test_load_regeling_standaardpremie() {
            let path = get_regulation_path()
                .join("nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load regeling standaardpremie: {}", e));

            assert_eq!(law.id, "regeling_standaardpremie");
            assert_eq!(law.regulatory_layer, RegulatoryLayer::MinisterieleRegeling);
        }

        #[test]
        fn test_load_apv_erfgrens_amsterdam() {
            let path = get_regulation_path()
                .join("nl/gemeentelijke_verordening/amsterdam/apv_erfgrens/2024-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load APV erfgrens Amsterdam: {}", e));

            assert_eq!(law.id, "apv_erfgrens_amsterdam");
            assert_eq!(
                law.regulatory_layer,
                RegulatoryLayer::GemeentelijkeVerordening
            );
            assert_eq!(law.gemeente_code, Some("GM0363".to_string()));
        }

        #[test]
        fn test_load_afstemmingsverordening_diemen() {
            let path = get_regulation_path()
                .join("nl/gemeentelijke_verordening/diemen/afstemmingsverordening_participatiewet/2015-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("Failed to load afstemmingsverordening Diemen: {}", e));

            assert_eq!(
                law.regulatory_layer,
                RegulatoryLayer::GemeentelijkeVerordening
            );
        }

        #[test]
        fn test_all_12_regulations_load_successfully() {
            let regulation_files = vec![
                "nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml",
                "nl/wet/zorgverzekeringswet/2025-01-01.yaml",
                "nl/wet/algemene_wet_inkomensafhankelijke_regelingen/2025-01-01.yaml",
                "nl/wet/kieswet/2025-01-01.yaml",
                "nl/wet/wet_langdurige_zorg/2025-07-05.yaml",
                "nl/wet/burgerlijk_wetboek_boek_5/2024-01-01.yaml",
                "nl/wet/participatiewet/2022-03-15.yaml",
                "nl/wet/wet_basisregistratie_personen/2025-02-12.yaml",
                "nl/wet/wet_inkomstenbelasting_2001/2025-01-01.yaml",
                "nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml",
                "nl/gemeentelijke_verordening/amsterdam/apv_erfgrens/2024-01-01.yaml",
                "nl/gemeentelijke_verordening/diemen/afstemmingsverordening_participatiewet/2015-01-01.yaml",
            ];

            let base_path = get_regulation_path();
            let mut loaded_count = 0;

            for file in &regulation_files {
                let path = base_path.join(file);
                match ArticleBasedLaw::from_yaml_file(&path) {
                    Ok(law) => {
                        assert!(!law.id.is_empty(), "Law {} should have non-empty id", file);
                        loaded_count += 1;
                    }
                    Err(e) => {
                        panic!("Failed to load {}: {}", file, e);
                    }
                }
            }

            assert_eq!(
                loaded_count, 12,
                "Should have loaded all 12 regulation files"
            );
        }

        #[test]
        fn test_wet_op_de_zorgtoeslag_find_article_by_output_works() {
            let path = get_regulation_path().join("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml");
            let law = ArticleBasedLaw::from_yaml_file(&path).unwrap();

            // Test find_article_by_output for key outputs
            assert!(law
                .find_article_by_output("heeft_recht_op_zorgtoeslag")
                .is_some());
            assert!(law.find_article_by_output("hoogte_zorgtoeslag").is_some());
            assert!(law.find_article_by_output("vermogen_onder_grens").is_some());

            // Test that nonexistent outputs return None
            assert!(law.find_article_by_output("nonexistent_output").is_none());
        }
    }

    // IoC: open_terms and implements parsing tests
    mod ioc {
        use super::*;

        const LAW_WITH_OPEN_TERMS: &str = r#"
$id: test_wet
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '4'
    text: "De minister stelt de standaardpremie vast."
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          required: true
          delegated_to: minister
          delegation_type: MINISTERIELE_REGELING
          legal_basis: "artikel 4 Wet op de zorgtoeslag"
      execution:
        output:
          - name: standaardpremie
            type: amount
        actions:
          - output: standaardpremie
            value: 0
"#;

        const LAW_WITH_OPEN_TERMS_AND_DEFAULT: &str = r#"
$id: test_beleidsregel
regulatory_layer: BELEIDSREGEL
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: "Redelijke kosten bedragen 6%."
    machine_readable:
      open_terms:
        - id: redelijke_kosten
          type: amount
          required: false
          description: "Percentage redelijke kosten"
          default:
            actions:
              - output: redelijke_kosten
                value: 600
      execution:
        output:
          - name: redelijke_kosten
            type: amount
        actions:
          - output: redelijke_kosten
            value: 600
"#;

        const REGELING_WITH_IMPLEMENTS: &str = r#"
$id: regeling_test
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2025-01-01'
bwb_id: BWBR0050536
legal_basis:
  - law_id: test_wet
    article: '4'
articles:
  - number: '1'
    text: "De standaardpremie bedraagt 2112 euro."
    machine_readable:
      implements:
        - law: test_wet
          article: '4'
          open_term: standaardpremie
          gelet_op: "Gelet op artikel 4 van de test wet"
      execution:
        output:
          - name: standaardpremie
            type: amount
        actions:
          - output: standaardpremie
            value: 211200
"#;

        #[test]
        fn test_parse_open_terms() {
            let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OPEN_TERMS).unwrap();
            let article = &law.articles[0];
            let open_terms = article.get_open_terms().unwrap();

            assert_eq!(open_terms.len(), 1);
            assert_eq!(open_terms[0].id, "standaardpremie");
            assert_eq!(open_terms[0].term_type, ParameterType::Amount);
            assert!(open_terms[0].required);
            assert_eq!(open_terms[0].delegated_to.as_deref(), Some("minister"));
            assert_eq!(
                open_terms[0].delegation_type.as_deref(),
                Some("MINISTERIELE_REGELING")
            );
            assert!(open_terms[0].default.is_none());
        }

        #[test]
        fn test_parse_open_terms_with_default() {
            let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OPEN_TERMS_AND_DEFAULT).unwrap();
            let article = &law.articles[0];
            let open_terms = article.get_open_terms().unwrap();

            assert_eq!(open_terms.len(), 1);
            assert_eq!(open_terms[0].id, "redelijke_kosten");
            assert!(!open_terms[0].required);

            let default = open_terms[0].default.as_ref().unwrap();
            let actions = default.actions.as_ref().unwrap();
            assert_eq!(actions.len(), 1);
            assert_eq!(actions[0].output.as_deref(), Some("redelijke_kosten"));
        }

        #[test]
        fn test_parse_implements() {
            let law = ArticleBasedLaw::from_yaml_str(REGELING_WITH_IMPLEMENTS).unwrap();
            let article = &law.articles[0];
            let implements = article.get_implements().unwrap();

            assert_eq!(implements.len(), 1);
            assert_eq!(implements[0].law, "test_wet");
            assert_eq!(implements[0].article, "4");
            assert_eq!(implements[0].open_term, "standaardpremie");
            assert_eq!(
                implements[0].gelet_op.as_deref(),
                Some("Gelet op artikel 4 van de test wet")
            );
        }

        #[test]
        fn test_backward_compat_no_open_terms() {
            let law = ArticleBasedLaw::from_yaml_str(MINIMAL_LAW_YAML).unwrap();
            assert!(law.articles[0].get_open_terms().is_none());
            assert!(law.articles[0].get_implements().is_none());
        }

        #[test]
        fn test_backward_compat_existing_law_with_outputs() {
            let law = ArticleBasedLaw::from_yaml_str(LAW_WITH_OUTPUTS_YAML).unwrap();
            assert!(law.articles[0].get_open_terms().is_none());
            assert!(law.articles[0].get_implements().is_none());
            // Existing functionality still works
            assert!(law.articles[0].has_output("test_output"));
        }
    }

    // Schema v0.6.0: markings, declares and placement.
    //
    // The engine reads these and does not act on them. What execution should do
    // with a marked article is a separate decision; until it is taken, parsing
    // is what keeps a v0.6.0 law from silently losing what it declares.
    mod v0_6_0 {
        use super::*;

        const LAW_V0_6_0: &str = r#"
$schema: https://example.org/schema/v0.6.0/schema.json
$id: test_markings
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000000
articles:
  - number: '1'
    text: "Deze wet wordt aangehaald als: Testwet."
    placement:
      hoofdstuk:
        number: '1'
        heading: Algemene bepalingen
      afdeling:
        number: '1.2'
    machine_readable:
      endpoint: begripsbepalingen
      declares:
        - property: name
          value: Testwet
        - property: valid_from
          value: '2025-01-01'
          applies_from: '2026-01-01'
      markings:
        - about: het kalenderjaar waarop de tegemoetkoming betrekking heeft
          resolution: engine
          resolved_by: "Een YEAR-bewerking die het jaardeel van een datum oplevert"
          target: [berekeningsjaar]
          legal_text_excerpt: "het berekeningsjaar waarop de tegemoetkoming betrekking heeft"
        - about: kwantificatie over de leden van het huishouden
          resolution: model
          target: []
          legal_text_excerpt: "de leden van het huishouden"
          accepted: true
"#;

        #[test]
        fn test_parse_markings() {
            let law = ArticleBasedLaw::from_yaml_str(LAW_V0_6_0).unwrap();
            let markings = law.articles[0].get_markings().unwrap();

            assert_eq!(markings.len(), 2);
            assert_eq!(markings[0].resolution, MarkingResolution::Engine);
            assert_eq!(markings[0].target, vec!["berekeningsjaar".to_string()]);
            assert!(markings[0].resolved_by.is_some());
            assert!(!markings[0].accepted);

            // An empty target is a statement, not an omission: the article stays
            // executable and only its explanation is incomplete.
            assert_eq!(markings[1].resolution, MarkingResolution::Model);
            assert!(markings[1].target.is_empty());
            assert!(markings[1].resolved_by.is_none());
            assert!(markings[1].accepted);
        }

        #[test]
        fn test_parse_declares() {
            let law = ArticleBasedLaw::from_yaml_str(LAW_V0_6_0).unwrap();
            let declares = law.articles[0].get_declares().unwrap();

            assert_eq!(declares.len(), 2);
            assert_eq!(declares[0].property, DeclaredProperty::Name);
            assert_eq!(declares[0].value, Value::String("Testwet".to_string()));
            assert!(declares[0].applies_from.is_none());
            assert_eq!(declares[1].property, DeclaredProperty::ValidFrom);
            assert_eq!(declares[1].applies_from.as_deref(), Some("2026-01-01"));
        }

        #[test]
        fn test_parse_placement_and_endpoint() {
            let law = ArticleBasedLaw::from_yaml_str(LAW_V0_6_0).unwrap();
            let placement = law.articles[0].placement.as_ref().unwrap();

            let hoofdstuk = placement.hoofdstuk.as_ref().unwrap();
            assert_eq!(hoofdstuk.number, "1");
            assert_eq!(hoofdstuk.heading.as_deref(), Some("Algemene bepalingen"));

            // A container may carry a number without an opschrift.
            let afdeling = placement.afdeling.as_ref().unwrap();
            assert_eq!(afdeling.number, "1.2");
            assert!(afdeling.heading.is_none());
            assert!(placement.boek.is_none());

            let mr = law.articles[0].machine_readable.as_ref().unwrap();
            assert_eq!(mr.endpoint.as_deref(), Some("begripsbepalingen"));
        }

        /// A v0.5 law carries no markings, and the RFC-012 untranslatables it
        /// does carry keep parsing — the model serves every supported schema
        /// version at once.
        #[test]
        fn test_untranslatables_still_parse_beside_markings() {
            let law = ArticleBasedLaw::from_yaml_str(
                r#"
$id: test_legacy
regulatory_layer: WET
publication_date: '2025-01-01'
bwb_id: BWBR0000000
articles:
  - number: '1'
    text: "Afronding op hele euro's."
    machine_readable:
      untranslatables:
        - construct: "afronden op hele euro's"
          reason: "geen ROUND-bewerking op eurocenten"
          accepted: true
"#,
            )
            .unwrap();
            let mr = law.articles[0].machine_readable.as_ref().unwrap();
            assert_eq!(mr.untranslatables.as_ref().unwrap().len(), 1);
            assert!(mr.markings.is_none());
            assert!(law.articles[0].placement.is_none());
        }
    }

    // Security tests
    mod security {
        use super::*;

        #[test]
        fn test_yaml_size_limit() {
            // Create a YAML string larger than MAX_YAML_SIZE
            let large_content = format!(
                "$id: test\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles: []\n# {}",
                "x".repeat(config::MAX_YAML_SIZE + 1)
            );

            let result = ArticleBasedLaw::from_yaml_str(&large_content);
            assert!(result.is_err());
            let err = result.unwrap_err();
            assert!(
                err.to_string().contains("size limit"),
                "Error should mention size limit: {}",
                err
            );
        }

        #[test]
        fn test_error_sanitization() {
            // Test that file not found errors don't expose full paths
            let result = ArticleBasedLaw::from_yaml_file("/nonexistent/path/to/secret/file.yaml");
            assert!(result.is_err());
            let err = result.unwrap_err();
            let err_str = err.to_string();

            // Should NOT contain the actual path
            assert!(
                !err_str.contains("/nonexistent/path"),
                "Error should not expose path: {}",
                err_str
            );
            assert!(
                !err_str.contains("secret"),
                "Error should not expose path: {}",
                err_str
            );
        }

        #[test]
        fn test_valid_yaml_within_limits() {
            // A normal-sized YAML should work fine
            let yaml = r#"
$id: test_law
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Test article text
"#;
            let result = ArticleBasedLaw::from_yaml_str(yaml);
            assert!(result.is_ok());
        }

        #[test]
        fn test_file_size_limit_check() {
            // Verify that the file size is checked before reading
            // We can't easily test with a real large file, but we can verify
            // the size limit constant is reasonable
            assert!(
                config::MAX_YAML_SIZE >= 100_000,
                "MAX_YAML_SIZE should allow at least 100KB"
            );
            assert!(
                config::MAX_YAML_SIZE <= 10_000_000,
                "MAX_YAML_SIZE should not exceed 10MB"
            );
        }
    }

    /// The size limits are inclusive: a document that is exactly at the limit
    /// still loads, one element or byte over it is refused. Every test here
    /// pins both sides of that boundary, because a limit that silently rejects
    /// a legitimate law is as much a defect as one that lets a YAML bomb past.
    mod limits {
        use super::*;
        use std::io::Write;

        const HEADER: &str =
            "$id: limits_test\nregulatory_layer: WET\npublication_date: '2025-01-01'\n";

        /// A law with `count` articles and nothing else.
        fn law_with_articles(count: usize) -> String {
            let mut yaml = String::from(HEADER);
            yaml.push_str("articles:\n");
            for i in 0..count {
                yaml.push_str(&format!("  - number: '{i}'\n    text: Artikel\n"));
            }
            yaml
        }

        /// A law with a single article whose `machine_readable` section is
        /// `body` (indented six spaces, i.e. one level under `machine_readable:`).
        fn law_with_machine_readable(body: &str) -> String {
            format!(
                "{HEADER}articles:\n  - number: '1'\n    text: Artikel\n    machine_readable:\n{body}"
            )
        }

        fn law_with_open_terms(count: usize) -> String {
            let mut body = String::from("      open_terms:\n");
            for i in 0..count {
                body.push_str(&format!("        - id: term_{i}\n          type: amount\n"));
            }
            law_with_machine_readable(&body)
        }

        fn law_with_implements(count: usize) -> String {
            let mut body = String::from("      implements:\n");
            for i in 0..count {
                body.push_str(&format!(
                    "        - law: andere_wet\n          article: '1'\n          open_term: term_{i}\n"
                ));
            }
            law_with_machine_readable(&body)
        }

        fn law_with_parameters(count: usize) -> String {
            let mut body = String::from("      execution:\n        parameters:\n");
            for i in 0..count {
                body.push_str(&format!(
                    "          - name: parameter_{i}\n            type: number\n"
                ));
            }
            law_with_machine_readable(&body)
        }

        fn law_with_inputs(count: usize) -> String {
            let mut body = String::from("      execution:\n        input:\n");
            for i in 0..count {
                body.push_str(&format!(
                    "          - name: invoer_{i}\n            type: number\n"
                ));
            }
            law_with_machine_readable(&body)
        }

        fn law_with_outputs(count: usize) -> String {
            let mut body = String::from("      execution:\n        output:\n");
            for i in 0..count {
                body.push_str(&format!(
                    "          - name: uitvoer_{i}\n            type: number\n"
                ));
            }
            law_with_machine_readable(&body)
        }

        fn law_with_actions(count: usize) -> String {
            let mut body = String::from("      execution:\n        actions:\n");
            for i in 0..count {
                body.push_str(&format!(
                    "          - output: uitvoer_{i}\n            value: 0\n"
                ));
            }
            law_with_machine_readable(&body)
        }

        fn law_with_action_values(count: usize) -> String {
            let mut body = String::from(
                "      execution:\n        actions:\n          - output: resultaat\n            operation: ADD\n            values:\n",
            );
            for _ in 0..count {
                body.push_str("              - 0\n");
            }
            law_with_machine_readable(&body)
        }

        fn law_with_action_conditions(count: usize) -> String {
            let mut body = String::from(
                "      execution:\n        actions:\n          - output: resultaat\n            operation: AND\n            conditions:\n",
            );
            for _ in 0..count {
                body.push_str("              - true\n");
            }
            law_with_machine_readable(&body)
        }

        /// Assert that `yaml` is refused with a message mentioning `fragment`.
        fn assert_rejected(yaml: &str, fragment: &str) {
            let err = ArticleBasedLaw::from_yaml_str(yaml)
                .expect_err("law over the array limit should be refused");
            let message = err.to_string();
            assert!(
                message.contains(fragment),
                "error should mention '{fragment}', got: {message}"
            );
        }

        /// Assert that `yaml` loads.
        fn assert_accepted(yaml: &str) {
            ArticleBasedLaw::from_yaml_str(yaml)
                .unwrap_or_else(|e| panic!("law exactly at the array limit should load: {e}"));
        }

        #[test]
        fn test_articles_at_and_over_limit() {
            assert_accepted(&law_with_articles(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_articles(config::MAX_ARRAY_SIZE + 1),
                "Too many articles",
            );
        }

        #[test]
        fn test_open_terms_at_and_over_limit() {
            assert_accepted(&law_with_open_terms(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_open_terms(config::MAX_ARRAY_SIZE + 1),
                "Too many open_terms",
            );
        }

        #[test]
        fn test_implements_at_and_over_limit() {
            assert_accepted(&law_with_implements(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_implements(config::MAX_ARRAY_SIZE + 1),
                "Too many implements",
            );
        }

        #[test]
        fn test_parameters_at_and_over_limit() {
            assert_accepted(&law_with_parameters(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_parameters(config::MAX_ARRAY_SIZE + 1),
                "Too many parameters",
            );
        }

        #[test]
        fn test_inputs_at_and_over_limit() {
            assert_accepted(&law_with_inputs(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_inputs(config::MAX_ARRAY_SIZE + 1),
                "Too many inputs",
            );
        }

        #[test]
        fn test_outputs_at_and_over_limit() {
            assert_accepted(&law_with_outputs(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_outputs(config::MAX_ARRAY_SIZE + 1),
                "Too many outputs",
            );
        }

        #[test]
        fn test_actions_at_and_over_limit() {
            assert_accepted(&law_with_actions(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_actions(config::MAX_ARRAY_SIZE + 1),
                "Too many actions",
            );
        }

        #[test]
        fn test_action_values_at_and_over_limit() {
            assert_accepted(&law_with_action_values(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_action_values(config::MAX_ARRAY_SIZE + 1),
                "Too many values in action",
            );
        }

        #[test]
        fn test_action_conditions_at_and_over_limit() {
            assert_accepted(&law_with_action_conditions(config::MAX_ARRAY_SIZE));
            assert_rejected(
                &law_with_action_conditions(config::MAX_ARRAY_SIZE + 1),
                "Too many conditions in action",
            );
        }

        /// A valid law padded with a trailing YAML comment to exactly `size` bytes.
        fn law_of_exact_size(size: usize) -> String {
            let base = format!("{HEADER}articles: []\n# ");
            assert!(size > base.len(), "padding target must exceed the header");
            let mut yaml = String::with_capacity(size);
            yaml.push_str(&base);
            yaml.push_str(&"x".repeat(size - base.len()));
            debug_assert_eq!(yaml.len(), size);
            yaml
        }

        #[test]
        fn test_yaml_string_exactly_at_size_limit_is_accepted() {
            let yaml = law_of_exact_size(config::MAX_YAML_SIZE);
            assert_eq!(yaml.len(), config::MAX_YAML_SIZE);
            let law = ArticleBasedLaw::from_yaml_str(&yaml)
                .unwrap_or_else(|e| panic!("YAML exactly at MAX_YAML_SIZE should load: {e}"));
            assert_eq!(law.id, "limits_test");
        }

        /// Write `content` to a uniquely named file under `OUT_DIR` (writable,
        /// inside the build directory) and return the path.
        fn write_temp_law(name: &str, content: &str) -> std::path::PathBuf {
            let path = std::path::PathBuf::from(env!("OUT_DIR")).join(name);
            let mut file = std::fs::File::create(&path).expect("should create temp law file");
            file.write_all(content.as_bytes())
                .expect("should write temp law file");
            file.flush().expect("should flush temp law file");
            path
        }

        #[test]
        fn test_file_exactly_at_size_limit_is_accepted() {
            let yaml = law_of_exact_size(config::MAX_YAML_SIZE);
            let path = write_temp_law("law_at_size_limit.yaml", &yaml);

            let law = ArticleBasedLaw::from_yaml_file(&path)
                .unwrap_or_else(|e| panic!("file exactly at MAX_YAML_SIZE should load: {e}"));
            assert_eq!(law.id, "limits_test");

            let _ = std::fs::remove_file(&path);
        }

        #[test]
        fn test_file_one_byte_over_size_limit_is_refused() {
            let yaml = law_of_exact_size(config::MAX_YAML_SIZE + 1);
            let path = write_temp_law("law_over_size_limit.yaml", &yaml);

            let err = ArticleBasedLaw::from_yaml_file(&path)
                .expect_err("file over MAX_YAML_SIZE should be refused");
            let message = err.to_string();

            let _ = std::fs::remove_file(&path);

            // The file-size check must fire before the file is read, so the
            // message is the file one, not the parsed-content one.
            assert!(
                message.contains("File exceeds maximum size limit"),
                "size check should reject before reading, got: {message}"
            );
        }
    }
}
