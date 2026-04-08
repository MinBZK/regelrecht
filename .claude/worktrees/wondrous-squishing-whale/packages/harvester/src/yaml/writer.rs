//! YAML writer for law files.

use std::fs::{self, File};
use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::LazyLock;

use regex::Regex;
use serde::Serialize;

use super::text::{normalize_text, should_wrap_text, wrap_text_default};
use crate::config::SCHEMA_URL;
use crate::error::Result;
use crate::types::{Law, Reference};

/// Regex matching a single-quoted YAML scalar value on a key line.
/// Captures: (1) prefix including key and colon-space, (2) the unquoted value.
#[allow(clippy::expect_used)]
static QUOTED_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r"^(\s*(?:- )?[a-zA-Z_$][a-zA-Z_$0-9]*: )'([^']*)'$").expect("valid regex")
});

/// Regex matching an unquoted YAML scalar value on a key line.
/// Excludes block scalar indicators (`|`, `>`) to avoid quoting them.
/// Captures: (1) prefix including key and colon-space, (2) the plain value.
#[allow(clippy::expect_used)]
static UNQUOTED_VALUE_RE: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r##"^(\s*(?:- )?[a-zA-Z_$][a-zA-Z_$0-9]*: )([^\s'"#\[{|>][^\n]*)$"##)
        .expect("valid regex")
});

/// Preamble representation for YAML serialization.
#[derive(Debug, Serialize)]
struct YamlPreamble {
    text: String,
    url: String,
}

/// Article representation for YAML serialization.
#[derive(Debug, Serialize)]
struct YamlArticle {
    number: String,
    text: String,
    url: String,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    references: Vec<YamlReference>,
}

/// Reference representation for YAML serialization.
#[derive(Debug, Serialize)]
struct YamlReference {
    id: String,
    bwb_id: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    artikel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    lid: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    onderdeel: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    hoofdstuk: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    paragraaf: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    afdeling: Option<String>,
}

impl From<&Reference> for YamlReference {
    fn from(r: &Reference) -> Self {
        Self {
            id: r.id.clone(),
            bwb_id: r.bwb_id.clone(),
            artikel: r.artikel.clone(),
            lid: r.lid.clone(),
            onderdeel: r.onderdeel.clone(),
            hoofdstuk: r.hoofdstuk.clone(),
            paragraaf: r.paragraaf.clone(),
            afdeling: r.afdeling.clone(),
        }
    }
}

/// Full law representation for YAML serialization.
#[derive(Debug, Serialize)]
struct YamlLaw {
    #[serde(rename = "$schema")]
    schema: String,
    #[serde(rename = "$id")]
    id: String,
    regulatory_layer: String,
    publication_date: String,
    valid_from: String,
    bwb_id: String,
    url: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    preamble: Option<YamlPreamble>,
    articles: Vec<YamlArticle>,
}

/// Generate a schema-compliant YAML structure from a Law object.
fn generate_yaml_struct(law: &Law, effective_date: &str) -> YamlLaw {
    let law_id = law.metadata.to_slug();

    // Convert preamble if present
    let preamble = law.preamble.as_ref().map(|p| {
        // Normalize and wrap preamble text like articles
        let normalized = normalize_text(&p.text);
        let text = if should_wrap_text(&normalized) {
            wrap_text_default(&normalized)
        } else {
            normalized
        };
        YamlPreamble {
            text,
            url: p.url.clone(),
        }
    });

    let articles: Vec<YamlArticle> = law
        .articles
        .iter()
        .map(|article| {
            // First normalize the text to fix typographical issues from source XML
            let normalized = normalize_text(&article.text);

            // Then wrap if needed
            let text = if should_wrap_text(&normalized) {
                wrap_text_default(&normalized)
            } else {
                normalized
            };

            YamlArticle {
                number: article.number.clone(),
                text,
                url: article.url.clone(),
                references: article.references.iter().map(YamlReference::from).collect(),
            }
        })
        .collect();

    YamlLaw {
        schema: SCHEMA_URL.to_string(),
        id: law_id,
        regulatory_layer: law.metadata.regulatory_layer.as_str().to_string(),
        publication_date: law
            .metadata
            .publication_date
            .clone()
            .unwrap_or_else(|| effective_date.to_string()),
        valid_from: effective_date.to_string(),
        bwb_id: law.metadata.bwb_id.clone(),
        url: format!(
            "https://wetten.overheid.nl/{}/{}",
            law.metadata.bwb_id, effective_date
        ),
        preamble,
        articles,
    }
}

/// Indent YAML sequences to comply with `indent-sequences: true`.
///
/// serde_yaml_ng places sequence items (`- `) at the same indent as their parent key.
/// This function adds 2 spaces so items are indented under their parent, e.g.:
///
/// ```yaml
/// # Before:          # After:
/// articles:          articles:
/// - number: '1'        - number: '1'
///   text: foo            text: foo
/// ```
fn indent_yaml_sequences(yaml: &str) -> String {
    let mut result: Vec<String> = Vec::new();
    // Stack of indent levels where sequences start
    let mut seq_indents: Vec<usize> = Vec::new();

    for line in yaml.lines() {
        let trimmed = line.trim_start();

        // Pass empty lines through unchanged
        if trimmed.is_empty() {
            result.push(line.to_string());
            continue;
        }

        let indent = line.len() - trimmed.len();

        // Pop sequences we've exited: either moved to a shallower indent,
        // or returned to the same indent but not as a sequence continuation.
        while let Some(&seq_indent) = seq_indents.last() {
            if indent < seq_indent || (indent == seq_indent && !trimmed.starts_with("- ")) {
                seq_indents.pop();
            } else {
                break;
            }
        }

        // Detect new or continuing sequence
        if trimmed.starts_with("- ") {
            let is_continuation = seq_indents.last().is_some_and(|&si| si == indent);
            if !is_continuation {
                seq_indents.push(indent);
            }
        }

        // Apply extra indentation
        let extra = seq_indents.len() * 2;
        if extra > 0 {
            result.push(format!("{}{}", " ".repeat(indent + extra), trimmed));
        } else {
            result.push(line.to_string());
        }
    }

    result.join("\n")
}

/// Check if a plain YAML scalar would be parsed as a non-string type.
///
/// Returns `true` if the value needs single quotes to remain a string
/// (integers, floats, dates, booleans, null, or values with special characters).
fn needs_yaml_quoting(value: &str) -> bool {
    if value.is_empty() {
        return true;
    }

    // YAML booleans and null
    match value.to_lowercase().as_str() {
        "true" | "false" | "yes" | "no" | "on" | "off" | "null" | "~" => return true,
        _ => {}
    }

    // Starts with YAML special character
    if let Some(&first) = value.as_bytes().first() {
        if b"{}[],&*#?|-<>=!%@:\"`' ".contains(&first) {
            return true;
        }
    }

    // Contains problematic sequences or trailing colon
    if value.contains(": ") || value.contains(" #") || value.ends_with(':') {
        return true;
    }

    let num_part = value.strip_prefix('-').unwrap_or(value);

    // Pure integer
    if !num_part.is_empty() && num_part.bytes().all(|b| b.is_ascii_digit()) {
        return true;
    }

    // Float: digits.digits (exactly one dot, digits on both sides)
    if let Some(dot_pos) = num_part.find('.') {
        let (before, after_with_dot) = num_part.split_at(dot_pos);
        let after = &after_with_dot[1..];
        if !before.is_empty()
            && !after.is_empty()
            && before.bytes().all(|b| b.is_ascii_digit())
            && after.bytes().all(|b| b.is_ascii_digit())
        {
            return true;
        }
    }

    // Date: YYYY-MM-DD
    let date_parts: Vec<&str> = value.split('-').collect();
    if date_parts.len() == 3
        && date_parts[0].len() == 4
        && date_parts[1].len() == 2
        && date_parts[2].len() == 2
        && date_parts
            .iter()
            .all(|p| p.bytes().all(|b| b.is_ascii_digit()))
    {
        return true;
    }

    false
}

/// Fix YAML scalar quoting to match yamllint's `quoted-strings: {required: only-when-needed}`.
///
/// - Strips quotes from values that YAML would parse as strings anyway.
/// - Adds quotes to unquoted values that would be misinterpreted (dates, booleans, numbers).
fn fix_yaml_quoting(yaml: &str) -> String {
    yaml.lines()
        .map(|line| {
            // Try to strip redundant quotes from quoted values
            if let Some(caps) = QUOTED_VALUE_RE.captures(line) {
                let (Some(prefix), Some(value)) = (caps.get(1), caps.get(2)) else {
                    return line.to_string();
                };
                let value = value.as_str();
                if needs_yaml_quoting(value) {
                    line.to_string()
                } else {
                    format!("{}{value}", prefix.as_str())
                }
            }
            // Try to add missing quotes to unquoted values that need them
            else if let Some(caps) = UNQUOTED_VALUE_RE.captures(line) {
                let (Some(prefix), Some(value)) = (caps.get(1), caps.get(2)) else {
                    return line.to_string();
                };
                let value = value.as_str();
                if needs_yaml_quoting(value) {
                    format!("{}'{value}'", prefix.as_str())
                } else {
                    line.to_string()
                }
            } else {
                line.to_string()
            }
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Generate YAML string from a Law object.
pub fn generate_yaml(law: &Law, effective_date: &str) -> Result<String> {
    let yaml_struct = generate_yaml_struct(law, effective_date);
    let yaml_string = serde_yaml_ng::to_string(&yaml_struct)?;

    // Post-process for yamllint compliance
    let yaml_string = fix_yaml_quoting(&yaml_string);
    let yaml_string = indent_yaml_sequences(&yaml_string);

    // Add document start marker and clean up trailing whitespace
    let lines: Vec<&str> = yaml_string.lines().map(|l| l.trim_end()).collect();
    let content = format!("---\n{}\n", lines.join("\n"));

    Ok(content)
}

/// Save a Law object as a YAML file.
///
/// Uses atomic write pattern: writes to temp file, syncs to disk, then renames.
/// This ensures partial writes don't corrupt existing files on crash.
///
/// # Arguments
/// * `law` - The Law object to save
/// * `effective_date` - The effective date in YYYY-MM-DD format
/// * `output_base` - Base directory for output (default: "regulation/nl/")
///
/// # Returns
/// Path to the saved file
pub fn save_yaml(law: &Law, effective_date: &str, output_base: Option<&Path>) -> Result<PathBuf> {
    let output_base = output_base.unwrap_or(Path::new("regulation/nl"));

    // Determine directory structure
    let layer_dir = law.metadata.regulatory_layer.as_dir_name();
    let law_id = law.metadata.to_slug();
    let output_dir = output_base.join(layer_dir).join(&law_id);
    fs::create_dir_all(&output_dir)?;

    let output_file = output_dir.join(format!("{effective_date}.yaml"));
    let temp_file = output_dir.join(format!(".{effective_date}.yaml.tmp"));

    // Generate YAML content
    let content = generate_yaml(law, effective_date)?;

    // Write to temp file first, then sync and rename for atomicity
    {
        let mut file = File::create(&temp_file)?;
        file.write_all(content.as_bytes())?;
        file.sync_all()?; // Ensure data is flushed to disk
    }

    // On Windows, rename fails if the destination already exists
    #[cfg(target_os = "windows")]
    if output_file.exists() {
        fs::remove_file(&output_file)?;
    }

    // Atomic rename (on most filesystems)
    fs::rename(&temp_file, &output_file)?;

    Ok(output_file)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::types::{Article, LawMetadata, RegulatoryLayer};
    use tempfile::tempdir;

    fn create_test_law() -> Law {
        let metadata = LawMetadata {
            bwb_id: "BWBR0018451".to_string(),
            title: "Wet op de zorgtoeslag".to_string(),
            regulatory_layer: RegulatoryLayer::Wet,
            publication_date: Some("2005-12-29".to_string()),
            effective_date: None,
        };

        let mut law = Law::new(metadata);
        law.add_article(Article::new(
            "1",
            "In deze wet wordt verstaan onder toeslagpartner: partner.",
            "https://wetten.overheid.nl/BWBR0018451/2025-01-01#Artikel1",
        ));
        law
    }

    #[test]
    fn test_generate_yaml() {
        let law = create_test_law();
        let yaml = generate_yaml(&law, "2025-01-01").unwrap();

        assert!(yaml.starts_with("---\n"));
        assert!(yaml.contains("$schema:"));
        assert!(yaml.contains("$id: wet_op_de_zorgtoeslag"));
        assert!(yaml.contains("regulatory_layer: WET"));
        assert!(yaml.contains("bwb_id: BWBR0018451"));
    }

    #[test]
    fn test_save_yaml() {
        let law = create_test_law();
        let temp_dir = tempdir().unwrap();
        let output_path = save_yaml(&law, "2025-01-01", Some(temp_dir.path())).unwrap();

        assert!(output_path.exists());
        // Check path components (works on both Windows and Unix)
        let path_str = output_path.to_string_lossy();
        assert!(path_str.contains("wet"));
        assert!(path_str.contains("wet_op_de_zorgtoeslag"));
        assert!(path_str.contains("2025-01-01.yaml"));

        let content = fs::read_to_string(output_path).unwrap();
        assert!(content.starts_with("---\n"));
    }

    #[test]
    fn test_generate_yaml_indented_sequences() {
        let law = create_test_law();
        let yaml = generate_yaml(&law, "2025-01-01").unwrap();

        // Articles should be indented under their key
        assert!(
            yaml.contains("articles:\n  - number:"),
            "Sequence items should be indented under articles key"
        );
    }

    #[test]
    fn test_generate_yaml_no_redundant_quotes() {
        let mut law = create_test_law();
        law.add_article(Article::new(
            "1.1.a",
            "Sub-article text",
            "https://example.com",
        ));
        let yaml = generate_yaml(&law, "2025-01-01").unwrap();

        // 1.1.a should NOT be quoted (contains letters, not a number)
        assert!(
            yaml.contains("number: 1.1.a"),
            "1.1.a should not be quoted, got: {}",
            yaml
        );
        // But dates should remain quoted
        assert!(
            yaml.contains("publication_date: '2005-12-29'"),
            "Dates should remain quoted"
        );
    }

    #[test]
    fn test_needs_yaml_quoting() {
        // Values that need quoting
        assert!(needs_yaml_quoting("1.1")); // float
        assert!(needs_yaml_quoting("42")); // integer
        assert!(needs_yaml_quoting("2024-10-16")); // date
        assert!(needs_yaml_quoting("true")); // boolean
        assert!(needs_yaml_quoting("null")); // null
        assert!(needs_yaml_quoting("")); // empty
        assert!(needs_yaml_quoting("foo: bar")); // contains ": "
        assert!(needs_yaml_quoting("end:")); // ends with ":"

        // Values that don't need quoting
        assert!(!needs_yaml_quoting("1.1.a"));
        assert!(!needs_yaml_quoting("68b"));
        assert!(!needs_yaml_quoting("18d"));
        assert!(!needs_yaml_quoting("3.3.1"));
        assert!(!needs_yaml_quoting("4a.1"));
        assert!(!needs_yaml_quoting("ref1"));
        assert!(!needs_yaml_quoting("BWBR0018451"));
        assert!(!needs_yaml_quoting("hello"));
    }

    #[test]
    fn test_indent_yaml_sequences() {
        let input =
            "top: val\nitems:\n- name: a\n  val: 1\n- name: b\n  nested:\n  - id: x\n    v: 1";
        let result = indent_yaml_sequences(input);
        assert_eq!(
            result,
            "top: val\nitems:\n  - name: a\n    val: 1\n  - name: b\n    nested:\n      - id: x\n        v: 1"
        );
    }

    #[test]
    fn test_fix_yaml_quoting() {
        let input = "number: '1.1.a'\ndate: '2024-10-16'\nartikel: '68b'\ncount: '1'";
        let result = fix_yaml_quoting(input);
        assert_eq!(
            result,
            "number: 1.1.a\ndate: '2024-10-16'\nartikel: 68b\ncount: '1'"
        );
    }

    #[test]
    fn test_yaml_reference_serialization() {
        let reference = Reference {
            id: "ref1".to_string(),
            bwb_id: "BWBR0018451".to_string(),
            artikel: Some("4".to_string()),
            lid: None,
            onderdeel: None,
            hoofdstuk: None,
            paragraaf: None,
            afdeling: None,
        };

        let yaml_ref = YamlReference::from(&reference);
        assert_eq!(yaml_ref.id, "ref1");
        assert_eq!(yaml_ref.artikel, Some("4".to_string()));
        assert!(yaml_ref.lid.is_none());
    }
}
