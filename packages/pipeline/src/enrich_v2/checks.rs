//! Deterministic checks over an enriched law file. No model, no shell, no
//! network: everything here is derivable from the file itself plus, for the
//! cross-law check, the sibling files on disk.
//!
//! These exist because the current enrichment prompt instructs a
//! `just validate` / `just bdd` loop that the runtime cannot execute (the
//! agent is spawned without `Bash`), so nothing establishes whether the
//! output is even schema-valid. Running these in-process replaces that loop
//! without a shell.
//!
//! The checks are ordered by what they cost and what they can prove:
//!
//! - [`schema_errors`] — L0. Is it valid against the declared schema version.
//! - [`coverage`] — L2. Which leden and which connectives in the statutory
//!   text have no counterpart in the model. This is the accounting
//!   obligation: it does not judge the translation, it makes the silence
//!   visible.
//! - [`enum_provenance`] — L3. Every enumerated value a model invents must
//!   appear in the statutory text of the same file.
//! - [`binding_integrity`] — L1. Every `$variable` resolves, and every
//!   cross-law `source` names a law and output that exist.

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use serde_yaml_ng::Value;

/// One finding. `article` is the article `number` it belongs to, or `None`
/// for a whole-file finding.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Finding {
    pub check: &'static str,
    pub article: Option<String>,
    pub detail: String,
}

impl Finding {
    fn new(check: &'static str, article: Option<&str>, detail: impl Into<String>) -> Self {
        Self {
            check,
            article: article.map(str::to_string),
            detail: detail.into(),
        }
    }
}

/// Connectives whose presence in statutory text means the provision has more
/// structure than a single rule. Every one of these in an article's text
/// should have a counterpart in the model or an explicit reason why not.
///
/// Kept lowercase; matching is case-insensitive on word boundaries.
pub const CONNECTIVES: &[&str] = &[
    "tenzij",
    "behoudens",
    "voor zover",
    "in afwijking van",
    "onverminderd",
    "ten hoogste",
    "ten minste",
    "dan wel",
    "noch",
    "mits",
];

/// Words that make a provision time-bound. A model whose article text
/// contains one of these and whose bindings carry no period is the
/// second-largest measured error class.
pub const TIME_WORDS: &[&str] = &[
    "peildatum",
    "berekeningsjaar",
    "kalenderjaar",
    "kalendermaand",
    "tijdvak",
    "op het tijdstip",
];

/// Schema validation against the version declared in the file. An empty
/// vector means valid. A missing or unknown `$schema` is itself an error.
pub fn schema_errors(yaml: &str) -> Vec<String> {
    let value: serde_json::Value = match serde_yaml_ng::from_str(yaml) {
        Ok(v) => v,
        Err(e) => return vec![format!("YAML parse error: {e}")],
    };
    match regelrecht_engine::schema::detect_version(&value) {
        Some(version) => match regelrecht_engine::schema::validation_errors_for(version, &value) {
            Ok(errors) => errors,
            Err(e) => vec![format!("schema validation failed: {e}")],
        },
        None => vec!["$schema: missing or unknown schema version".to_string()],
    }
}

/// Split an article's `text` into its leden. Dutch statutory text numbers
/// leden as `1. `, `2. ` at the start of a line or after a blank line. An
/// article without that numbering is a single lid.
pub fn split_leden(text: &str) -> Vec<(u32, String)> {
    let mut out: Vec<(u32, String)> = Vec::new();
    let mut current: Option<(u32, String)> = None;

    for raw in text.lines() {
        let line = raw.trim_start();
        let lid = leading_lid_number(line);
        match lid {
            Some((n, rest)) => {
                if let Some(prev) = current.take() {
                    out.push(prev);
                }
                current = Some((n, rest.to_string()));
            }
            None => {
                if let Some((_, body)) = current.as_mut() {
                    if !line.is_empty() {
                        body.push(' ');
                        body.push_str(line);
                    }
                } else if !line.is_empty() {
                    // Text before any numbering: the chapeau, or an
                    // unnumbered single-lid article.
                    current = Some((1, line.to_string()));
                }
            }
        }
    }
    if let Some(prev) = current {
        out.push(prev);
    }
    out
}

/// `"3. De percentages…"` → `Some((3, "De percentages…"))`. Only accepts a
/// small number followed by `. ` so an amount like `2.18` never matches.
fn leading_lid_number(line: &str) -> Option<(u32, &str)> {
    let digits: String = line.chars().take_while(char::is_ascii_digit).collect();
    if digits.is_empty() || digits.len() > 2 {
        return None;
    }
    let rest = &line[digits.len()..];
    let rest = rest.strip_prefix('.')?;
    // Require whitespace after the dot: "2. De normpremie" is a lid,
    // "2.18 van de Wet IB" is a reference.
    if !rest.starts_with(char::is_whitespace) {
        return None;
    }
    digits.parse().ok().map(|n| (n, rest.trim_start()))
}

/// Case-insensitive whole-phrase search.
fn contains_phrase(haystack_lower: &str, needle: &str) -> bool {
    haystack_lower
        .match_indices(needle)
        .any(|(i, _)| is_boundary(haystack_lower, i, needle.len()))
}

fn is_boundary(s: &str, start: usize, len: usize) -> bool {
    let before_ok = start == 0
        || !s[..start]
            .chars()
            .next_back()
            .is_some_and(|c| c.is_alphanumeric());
    let end = start + len;
    let after_ok = end >= s.len() || !s[end..].chars().next().is_some_and(char::is_alphanumeric);
    before_ok && after_ok
}

/// Per-article accounting of what the statutory text signals against what
/// the model contains. Reports rather than judges: a connective without a
/// counterpart is a question for a reader, not proof of an error.
pub fn coverage(doc: &Value) -> Vec<Finding> {
    let mut findings = Vec::new();
    for article in articles(doc).iter() {
        let number = article_number(article).unwrap_or_default();
        let text = article
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let lower = text.to_lowercase();
        let mr = article.get("machine_readable");

        let leden = split_leden(text);
        let modelled = mr.is_some();

        if !modelled {
            // Nothing is claimed, so nothing is silently dropped. Only worth
            // reporting when the text carries structure that suggests logic.
            let signals: Vec<&str> = CONNECTIVES
                .iter()
                .copied()
                .filter(|c| contains_phrase(&lower, c))
                .collect();
            if leden.len() > 1 || !signals.is_empty() {
                findings.push(Finding::new(
                    "coverage",
                    Some(&number),
                    format!(
                        "no machine_readable; {} lid(en), signals: {}",
                        leden.len(),
                        if signals.is_empty() {
                            "none".to_string()
                        } else {
                            signals.join(", ")
                        }
                    ),
                ));
            }
            continue;
        }

        // Modelled: every lid and every connective needs a counterpart.
        let model_text = render(mr.unwrap_or(&Value::Null)).to_lowercase();
        for (n, body) in &leden {
            let body_lower = body.to_lowercase();
            let hits: Vec<&str> = CONNECTIVES
                .iter()
                .copied()
                .filter(|c| contains_phrase(&body_lower, c))
                .collect();
            for hit in hits {
                findings.push(Finding::new(
                    "coverage",
                    Some(&number),
                    format!("lid {n} contains \"{hit}\"; confirm the model branches on it"),
                ));
            }
            let time_hits: Vec<&str> = TIME_WORDS
                .iter()
                .copied()
                .filter(|w| contains_phrase(&body_lower, w))
                .collect();
            for hit in time_hits {
                if !model_text.contains(hit) {
                    findings.push(Finding::new(
                        "coverage",
                        Some(&number),
                        format!("lid {n} is bound to \"{hit}\" but no binding carries a period"),
                    ));
                }
            }
        }
        if leden.len() > 1 {
            findings.push(Finding::new(
                "coverage",
                Some(&number),
                format!(
                    "{} leden; account for each one in the model or state why not",
                    leden.len()
                ),
            ));
        }
    }
    findings
}

/// Every enumerated value in the model must be traceable to the statutory
/// text of the same file. An enum the model invents is the clearest form of
/// content that no reader authorised.
pub fn enum_provenance(doc: &Value) -> Vec<Finding> {
    let corpus_text = all_article_text(doc).to_lowercase();
    let mut findings = Vec::new();
    for article in articles(doc).iter() {
        let number = article_number(article).unwrap_or_default();
        let Some(mr) = article.get("machine_readable") else {
            continue;
        };
        for value in collect_enum_values(mr) {
            if value
                .chars()
                .all(|c| c.is_ascii_digit() || c == '.' || c == '-')
            {
                continue; // numeric constants are not enum members
            }
            let spoken = value.replace('_', " ").to_lowercase();
            if !corpus_text.contains(&spoken) && !corpus_text.contains(&value.to_lowercase()) {
                findings.push(Finding::new(
                    "enum-provenance",
                    Some(&number),
                    format!("enumerated value \"{value}\" appears nowhere in this law's text"),
                ));
            }
        }
    }
    findings
}

/// `$variable` resolution within the file, and cross-law `source` targets.
/// `corpus_root` is the directory that holds `<country>/<layer>/<law>/…`;
/// when it is `None` the cross-law half is skipped.
pub fn binding_integrity(doc: &Value, corpus_root: Option<&Path>) -> Vec<Finding> {
    let mut findings = Vec::new();
    let defined = defined_names(doc);
    let own_outputs = declared_outputs(doc);

    for article in articles(doc).iter() {
        let number = article_number(article).unwrap_or_default();
        let Some(mr) = article.get("machine_readable") else {
            continue;
        };
        for name in referenced_variables(mr) {
            if !defined.contains(&name) {
                findings.push(Finding::new(
                    "binding",
                    Some(&number),
                    format!("${name} is referenced but never defined in this law"),
                ));
            }
        }
        for (regulation, output) in cross_law_sources(mr) {
            // A source without `regulation` resolves inside this law: it
            // names an output another article here produces. That is
            // legitimate, so check it against this file instead of the
            // corpus.
            if regulation.is_empty() {
                if output.is_empty() {
                    findings.push(Finding::new(
                        "binding",
                        Some(&number),
                        "source names neither a regulation nor an output",
                    ));
                } else if !own_outputs.contains(&output) {
                    findings.push(Finding::new(
                        "binding",
                        Some(&number),
                        format!("source resolves to \"{output}\" within this law, which produces no such output"),
                    ));
                }
                continue;
            }
            let Some(root) = corpus_root else { continue };
            match find_law_file(root, &regulation) {
                None => findings.push(Finding::new(
                    "binding",
                    Some(&number),
                    format!("source regulation \"{regulation}\" not found in the corpus"),
                )),
                Some(path) => {
                    if !output.is_empty() && !law_defines_output(&path, &output) {
                        findings.push(Finding::new(
                            "binding",
                            Some(&number),
                            format!("\"{regulation}\" does not produce output \"{output}\""),
                        ));
                    }
                }
            }
        }
    }
    findings
}

// --- traversal helpers -------------------------------------------------

fn articles(doc: &Value) -> &[Value] {
    doc.get("articles")
        .and_then(Value::as_sequence)
        .map(|s| s.as_slice())
        .unwrap_or(&[])
}

fn article_number(article: &Value) -> Option<String> {
    article.get("number").map(|n| match n {
        Value::String(s) => s.clone(),
        other => render(other),
    })
}

fn all_article_text(doc: &Value) -> String {
    articles(doc)
        .iter()
        .filter_map(|a| a.get("text").and_then(Value::as_str))
        .collect::<Vec<_>>()
        .join("\n")
}

/// Flatten a YAML subtree to a string, for substring checks.
fn render(v: &Value) -> String {
    serde_yaml_ng::to_string(v).unwrap_or_default()
}

/// Values under any `value:` key that holds a sequence of scalars. That is
/// the shape an enumerated domain takes in the corpus.
fn collect_enum_values(v: &Value) -> Vec<String> {
    let mut out = Vec::new();
    walk(v, &mut |key, node| {
        if key == Some("value") {
            if let Some(seq) = node.as_sequence() {
                for item in seq {
                    if let Some(s) = item.as_str() {
                        out.push(s.to_string());
                    }
                }
            }
        }
    });
    out
}

/// Names defined anywhere in the law: `definitions` keys, `parameters`,
/// `inputs` and `outputs` entries with a `name`.
fn defined_names(doc: &Value) -> BTreeSet<String> {
    let mut names = BTreeSet::new();
    walk(doc, &mut |key, node| {
        if key == Some("definitions") {
            if let Some(map) = node.as_mapping() {
                for k in map.keys() {
                    if let Some(s) = k.as_str() {
                        names.insert(s.to_string());
                    }
                }
            }
        }
        // `execution` uses the singular `input`/`output`; `parameters` is
        // plural. An `action` carries its produced name on `output` as a
        // scalar. All of them define a name this law may then reference.
        if matches!(key, Some("parameters") | Some("input") | Some("output")) {
            match node {
                Value::Sequence(seq) => {
                    for item in seq {
                        if let Some(n) = item.get("name").and_then(Value::as_str) {
                            names.insert(n.to_string());
                        }
                    }
                }
                Value::String(s) => {
                    names.insert(s.clone());
                }
                _ => {}
            }
        }
    });
    names
}

/// The outputs this law itself declares, from every `execution.output`
/// list and every action's `output` name.
fn declared_outputs(doc: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    walk(doc, &mut |key, node| {
        if key != Some("output") {
            return;
        }
        match node {
            Value::Sequence(seq) => {
                for item in seq {
                    if let Some(n) = item.get("name").and_then(Value::as_str) {
                        out.insert(n.to_string());
                    }
                }
            }
            Value::String(s) => {
                out.insert(s.clone());
            }
            _ => {}
        }
    });
    out
}

/// Every `$name` occurring as a scalar value in the subtree.
fn referenced_variables(v: &Value) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    walk(v, &mut |_, node| {
        if let Some(s) = node.as_str() {
            if let Some(name) = s.strip_prefix('$') {
                if !name.is_empty() && name.chars().all(|c| c.is_alphanumeric() || c == '_') {
                    out.insert(name.to_string());
                }
            }
        }
    });
    out
}

/// `(regulation, output)` for every `source` mapping in the subtree.
fn cross_law_sources(v: &Value) -> Vec<(String, String)> {
    let mut out = Vec::new();
    walk(v, &mut |key, node| {
        if key == Some("source") {
            if let Some(map) = node.as_mapping() {
                let reg = map
                    .get(Value::from("regulation"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                let output = map
                    .get(Value::from("output"))
                    .and_then(Value::as_str)
                    .unwrap_or_default()
                    .to_string();
                out.push((reg, output));
            }
        }
    });
    out
}

/// Depth-first walk calling `f(key_of_this_node, node)`.
fn walk<'a>(v: &'a Value, f: &mut impl FnMut(Option<&'a str>, &'a Value)) {
    walk_inner(None, v, f);
}

fn walk_inner<'a>(
    key: Option<&'a str>,
    v: &'a Value,
    f: &mut impl FnMut(Option<&'a str>, &'a Value),
) {
    f(key, v);
    match v {
        Value::Mapping(map) => {
            for (k, val) in map {
                walk_inner(k.as_str(), val, f);
            }
        }
        Value::Sequence(seq) => {
            for item in seq {
                walk_inner(key, item, f);
            }
        }
        _ => {}
    }
}

/// Locate a law directory by `$id` under `corpus_root`, returning the most
/// recent version file. The corpus lays out `<country>/<layer>/<id>/<date>.yaml`.
fn find_law_file(corpus_root: &Path, law_id: &str) -> Option<std::path::PathBuf> {
    let mut newest: Option<std::path::PathBuf> = None;
    let mut stack = vec![corpus_root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                if path.file_name().and_then(|n| n.to_str()) == Some(law_id) {
                    if let Ok(files) = std::fs::read_dir(&path) {
                        for f in files.flatten() {
                            let p = f.path();
                            if p.extension().and_then(|e| e.to_str()) == Some("yaml")
                                && newest.as_ref().is_none_or(|cur| p > *cur)
                            {
                                newest = Some(p);
                            }
                        }
                    }
                } else {
                    stack.push(path);
                }
            }
        }
    }
    newest
}

fn law_defines_output(path: &Path, output: &str) -> bool {
    let Ok(raw) = std::fs::read_to_string(path) else {
        return false;
    };
    let Ok(doc) = serde_yaml_ng::from_str::<Value>(&raw) else {
        return false;
    };
    let mut found = false;
    walk(&doc, &mut |key, node| {
        if key != Some("output") {
            return;
        }
        match node {
            // `execution.output` is a list of declared outputs.
            Value::Sequence(seq) => {
                if seq
                    .iter()
                    .any(|item| item.get("name").and_then(Value::as_str) == Some(output))
                {
                    found = true;
                }
            }
            // An action names what it produces as a scalar.
            Value::String(s) if s == output => found = true,
            _ => {}
        }
    });
    found
}

/// Everything the deterministic layer can say about one file.
#[derive(Debug, Default)]
pub struct Report {
    pub schema: Vec<String>,
    pub findings: Vec<Finding>,
}

impl Report {
    /// True when the file is schema-valid and no finding was raised.
    pub fn is_clean(&self) -> bool {
        self.schema.is_empty() && self.findings.is_empty()
    }

    /// Count of findings per check name, for a one-line summary.
    pub fn by_check(&self) -> BTreeMap<&'static str, usize> {
        let mut counts = BTreeMap::new();
        for f in &self.findings {
            *counts.entry(f.check).or_insert(0) += 1;
        }
        counts
    }
}

/// Run every deterministic check over one law file.
pub fn run(yaml: &str, corpus_root: Option<&Path>) -> Report {
    let schema = schema_errors(yaml);
    let Ok(doc) = serde_yaml_ng::from_str::<Value>(yaml) else {
        return Report {
            schema,
            findings: Vec::new(),
        };
    };
    let mut findings = coverage(&doc);
    findings.extend(enum_provenance(&doc));
    findings.extend(binding_integrity(&doc, corpus_root));
    Report { schema, findings }
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn leden_split_on_numbering_and_not_on_article_references() {
        let text = "1. Indien de normpremie minder bedraagt dan de standaardpremie.\n\n\
                    2. De normpremie bedraagt een percentage, bedoeld in artikel 2.18 van de Wet IB.\n\n\
                    4. In afwijking van het eerste lid bedraagt de aanspraak vijftig procent.";
        let leden = split_leden(text);
        assert_eq!(leden.len(), 3);
        assert_eq!(leden[0].0, 1);
        assert_eq!(leden[2].0, 4);
        // "artikel 2.18" must not start a new lid.
        assert!(leden[1].1.contains("2.18"));
    }

    #[test]
    fn unnumbered_article_is_one_lid() {
        let leden = split_leden("In deze wet wordt verstaan onder verzekerde: degene die …");
        assert_eq!(leden.len(), 1);
    }

    #[test]
    fn coverage_reports_a_connective_in_a_modelled_article() {
        let yaml = r#"
articles:
  - number: '2'
    text: |-
      1. De verzekerde heeft aanspraak op een zorgtoeslag.

      4. In afwijking van het eerste lid bedraagt de aanspraak vijftig procent.
    machine_readable:
      outputs:
        - name: hoogte_zorgtoeslag
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = coverage(&doc);
        assert!(
            findings
                .iter()
                .any(|f| f.detail.contains("in afwijking van")),
            "expected the derogation to be reported: {findings:?}"
        );
    }

    #[test]
    fn coverage_flags_a_time_bound_lid_without_a_period_in_the_model() {
        let yaml = r#"
articles:
  - number: '2'
    text: |-
      5. De aanspraak wordt voor iedere kalendermaand afzonderlijk bepaald.
    machine_readable:
      outputs:
        - name: hoogte_zorgtoeslag
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = coverage(&doc);
        assert!(findings.iter().any(|f| f.detail.contains("kalendermaand")));
    }

    #[test]
    fn enum_provenance_flags_values_absent_from_the_text() {
        let yaml = r#"
articles:
  - number: '1'
    text: De rechter kan een maatregel opleggen.
    machine_readable:
      definitions:
        geldige_titels:
          value:
            - TBS
            - ZORGMACHTIGING
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = enum_provenance(&doc);
        assert_eq!(findings.len(), 2, "{findings:?}");
        assert!(findings[0].detail.contains("TBS"));
    }

    #[test]
    fn enum_provenance_accepts_a_value_that_is_in_the_text() {
        let yaml = r#"
articles:
  - number: '1'
    text: Een zorgmachtiging kan worden verleend.
    machine_readable:
      definitions:
        titels:
          value:
            - ZORGMACHTIGING
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(enum_provenance(&doc).is_empty());
    }

    #[test]
    fn binding_integrity_flags_an_undefined_variable() {
        let yaml = r#"
articles:
  - number: '2'
    text: tekst
    machine_readable:
      definitions:
        drempel:
          value: 100
      execution:
        outputs:
          - name: bedrag
            value: $onbekend
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = binding_integrity(&doc, None);
        assert!(findings.iter().any(|f| f.detail.contains("$onbekend")));
        assert!(!findings.iter().any(|f| f.detail.contains("$drempel")));
    }

    #[test]
    fn schema_errors_rejects_a_file_without_a_schema_declaration() {
        let errors = schema_errors("articles: []\n");
        assert!(!errors.is_empty());
        assert!(errors[0].contains("$schema"));
    }
}
