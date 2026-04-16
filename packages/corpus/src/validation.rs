use crate::models::{Scope, Source};
use crate::source_map::SourceMap;

/// A validation warning for a law outside its source's scope.
#[derive(Debug, Clone)]
pub struct ScopeWarning {
    pub law_id: String,
    pub source_id: String,
    pub source_name: String,
    pub expected_scopes: Vec<String>,
    pub actual_scope_code: Option<String>,
    pub message: String,
}

/// Validate that sources only provide laws within their declared scopes.
///
/// Returns warnings for laws that appear to be outside their source's
/// jurisdictional scope. A source with empty scopes is unrestricted.
pub fn validate_scopes(source_map: &SourceMap, sources: &[Source]) -> Vec<ScopeWarning> {
    let mut warnings = Vec::new();

    for law in source_map.laws() {
        let source = match sources.iter().find(|s| s.id == law.source_id) {
            Some(s) => s,
            None => continue,
        };

        // Unrestricted sources don't need scope validation
        if source.scopes.is_empty() {
            continue;
        }

        // Extract scope codes from the YAML content
        let gemeente_code = extract_gemeente_code(&law.yaml_content);
        let waterschap_code = extract_waterschap_code(&law.yaml_content);

        if let Some(code) = &gemeente_code {
            if !scope_matches(&source.scopes, "gemeente_code", code) {
                warnings.push(ScopeWarning {
                    law_id: law.law_id.clone(),
                    source_id: law.source_id.clone(),
                    source_name: law.source_name.clone(),
                    expected_scopes: source
                        .scopes
                        .iter()
                        .map(|s| format!("{}:{}", s.scope_type, s.value))
                        .collect(),
                    actual_scope_code: gemeente_code.clone(),
                    message: format!(
                        "Law '{}' from source '{}' has gemeente_code '{}' which is outside declared scopes {:?}",
                        law.law_id,
                        source.id,
                        code,
                        source.scopes.iter().map(|s| &s.value).collect::<Vec<_>>()
                    ),
                });
            }
        }

        if let Some(code) = &waterschap_code {
            if !scope_matches(&source.scopes, "waterschap_code", code) {
                warnings.push(ScopeWarning {
                    law_id: law.law_id.clone(),
                    source_id: law.source_id.clone(),
                    source_name: law.source_name.clone(),
                    expected_scopes: source
                        .scopes
                        .iter()
                        .map(|s| format!("{}:{}", s.scope_type, s.value))
                        .collect(),
                    actual_scope_code: waterschap_code.clone(),
                    message: format!(
                        "Law '{}' from source '{}' has waterschap_code '{}' which is outside declared scopes {:?}",
                        law.law_id,
                        source.id,
                        code,
                        source.scopes.iter().map(|s| &s.value).collect::<Vec<_>>()
                    ),
                });
            }
        }
    }

    warnings
}

/// Check if a scope code matches any of the source's scopes for the given type.
///
/// Supports `gemeente_code` and `waterschap_code` scope types.
fn scope_matches(scopes: &[Scope], scope_type: &str, code: &str) -> bool {
    let matching_scopes: Vec<_> = scopes
        .iter()
        .filter(|s| s.scope_type == scope_type)
        .collect();

    // If the source has no scopes of this type, we cannot validate —
    // treat as matching (no warning).
    if matching_scopes.is_empty() {
        return true;
    }

    matching_scopes.iter().any(|scope| scope.value == code)
}

/// Extract top-level gemeente_code from YAML content using line-based parsing.
///
/// Only matches `gemeente_code:` at the start of a line (no leading whitespace)
/// to avoid matching nested fields, consistent with `extract_law_id`.
fn extract_gemeente_code(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("gemeente_code:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

/// Extract top-level waterschap_code from YAML content using line-based parsing.
///
/// Only matches `waterschap_code:` at the start of a line (no leading whitespace)
/// to avoid matching nested fields, consistent with `extract_gemeente_code`.
fn extract_waterschap_code(yaml: &str) -> Option<String> {
    for line in yaml.lines() {
        if let Some(rest) = line.strip_prefix("waterschap_code:") {
            let value = rest.trim().trim_matches('"').trim_matches('\'');
            if !value.is_empty() {
                return Some(value.to_string());
            }
        }
    }
    None
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;
    use crate::models::{LocalSource, SourceType};
    use tempfile::TempDir;

    fn make_scoped_source(
        id: &str,
        path: &std::path::Path,
        scopes: Vec<Scope>,
        priority: u32,
    ) -> Source {
        Source {
            id: id.to_string(),
            name: format!("Source {}", id),
            source_type: SourceType::Local {
                local: LocalSource {
                    path: path.to_path_buf(),
                },
            },
            scopes,
            priority,
            auth_ref: None,
        }
    }

    fn write_law(dir: &std::path::Path, name: &str, id: &str, gemeente_code: Option<&str>) {
        let path = dir.join(format!("{}.yaml", name));
        let gc = gemeente_code
            .map(|c| format!("\ngemeente_code: '{}'", c))
            .unwrap_or_default();
        std::fs::write(
            &path,
            format!("$id: {id}\nregulatory_layer: GEMEENTELIJKE_VERORDENING\npublication_date: '2025-01-01'{gc}\narticles: []\n"),
        ).unwrap();
    }

    #[test]
    fn test_extract_gemeente_code() {
        assert_eq!(
            extract_gemeente_code("gemeente_code: GM0363\nfoo: bar"),
            Some("GM0363".to_string())
        );
        assert_eq!(
            extract_gemeente_code("gemeente_code: 'GM0518'\nfoo: bar"),
            Some("GM0518".to_string())
        );
        assert_eq!(extract_gemeente_code("foo: bar\nbaz: qux"), None);
    }

    #[test]
    fn test_extract_waterschap_code() {
        assert_eq!(
            extract_waterschap_code("waterschap_code: WS0653\nfoo: bar"),
            Some("WS0653".to_string())
        );
        assert_eq!(
            extract_waterschap_code("waterschap_code: 'WS0653'\nfoo: bar"),
            Some("WS0653".to_string())
        );
        assert_eq!(extract_waterschap_code("foo: bar\nbaz: qux"), None);
    }

    #[test]
    fn test_scope_valid_no_warnings() {
        let dir = TempDir::new().unwrap();
        write_law(dir.path(), "verordening", "test_v", Some("GM0363"));

        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_scope_violation_warning() {
        let dir = TempDir::new().unwrap();
        // Source declares GM0363, but law has GM0518
        write_law(dir.path(), "verordening", "wrong_v", Some("GM0518"));

        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].law_id, "wrong_v");
        assert_eq!(warnings[0].actual_scope_code, Some("GM0518".to_string()));
    }

    #[test]
    fn test_unrestricted_scope_no_warnings() {
        let dir = TempDir::new().unwrap();
        write_law(dir.path(), "verordening", "any_v", Some("GM0518"));

        // Empty scopes = unrestricted
        let source = make_scoped_source("central", dir.path(), vec![], 1);

        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_law_without_gemeente_code_no_warning() {
        let dir = TempDir::new().unwrap();
        write_law(dir.path(), "wet", "national_wet", None);

        let source = make_scoped_source(
            "amsterdam",
            dir.path(),
            vec![Scope {
                scope_type: "gemeente_code".to_string(),
                value: "GM0363".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    #[test]
    fn test_waterschap_scope_valid_no_warnings() {
        let dir = TempDir::new().unwrap();
        write_waterschap_law(dir.path(), "keur", "keur_valid", "WS0653");

        let source = make_scoped_source(
            "waterschap_hhnk",
            dir.path(),
            vec![Scope {
                scope_type: "waterschap_code".to_string(),
                value: "WS0653".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert!(warnings.is_empty());
    }

    fn write_waterschap_law(dir: &std::path::Path, name: &str, id: &str, code: &str) {
        let path = dir.join(format!("{}.yaml", name));
        std::fs::write(
            &path,
            format!("$id: {id}\nregulatory_layer: WATERSCHAPS_VERORDENING\npublication_date: '2025-01-01'\nwaterschap_code: '{code}'\narticles: []\n"),
        ).unwrap();
    }

    #[test]
    fn test_waterschap_scope_violation_warning() {
        let dir = TempDir::new().unwrap();
        // Source declares WS0653, but law has WS0999
        write_waterschap_law(dir.path(), "keur", "keur_test", "WS0999");

        let source = make_scoped_source(
            "waterschap_test",
            dir.path(),
            vec![Scope {
                scope_type: "waterschap_code".to_string(),
                value: "WS0653".to_string(),
            }],
            10,
        );

        let mut map = SourceMap::new();
        map.load_source(&source).unwrap();

        let warnings = validate_scopes(&map, &[source]);
        assert_eq!(warnings.len(), 1);
        assert_eq!(warnings[0].law_id, "keur_test");
        assert_eq!(warnings[0].actual_scope_code, Some("WS0999".to_string()));
    }
}
