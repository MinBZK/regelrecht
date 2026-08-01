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
/// leden as `1. `, `2. `. An article without that numbering is a single lid.
///
/// Scans the whole string rather than line starts. A folded YAML scalar
/// (`>-` without blank lines) puts an entire article on one line, and a
/// line-based split would then count every article as a single lid. That
/// silently disabled the per-lid accounting on every law whose harvest
/// happened to fold, while it worked on laws that kept blank lines.
pub fn split_leden(text: &str) -> Vec<(u32, String)> {
    let marks = lid_marks(text);
    if marks.is_empty() {
        let body = text.trim();
        return if body.is_empty() {
            Vec::new()
        } else {
            vec![(1, body.to_string())]
        };
    }

    let mut out = Vec::new();
    // Anything before the first marker is the chapeau; it belongs to the
    // article, not to a lid, so it is not counted as one.
    for (i, (num, _, body_start)) in marks.iter().enumerate() {
        let end = marks
            .get(i + 1)
            .map_or(text.len(), |(_, next_marker, _)| *next_marker);
        let body = text[*body_start..end.max(*body_start)].trim();
        out.push((*num, normalize_ws(body)));
    }
    out
}

/// `(lid number, byte offset of the marker, byte offset of its body)`. A
/// marker only counts at the start of the text or after whitespace, so
/// `artikel 2.18` never matches, and the number is capped at two digits.
fn lid_marks(text: &str) -> Vec<(u32, usize, usize)> {
    let bytes = text.as_bytes();
    let mut marks = Vec::new();
    let mut i = 0usize;
    while i < bytes.len() {
        if !bytes[i].is_ascii_digit() {
            i += 1;
            continue;
        }
        let at_boundary = i == 0 || bytes[i - 1].is_ascii_whitespace();
        if !at_boundary {
            // Skip the rest of this number so `2.18` cannot re-enter at `18`.
            while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
                i += 1;
            }
            continue;
        }
        let start = i;
        while i < bytes.len() && bytes[i].is_ascii_digit() {
            i += 1;
        }
        let digits = &text[start..i];
        if digits.len() <= 2
            && i < bytes.len()
            && bytes[i] == b'.'
            && bytes.get(i + 1).is_some_and(u8::is_ascii_whitespace)
        {
            if let Ok(n) = digits.parse::<u32>() {
                // Consume the dot and the following whitespace.
                let mut body = i + 1;
                while body < bytes.len() && bytes[body].is_ascii_whitespace() {
                    body += 1;
                }
                marks.push((n, start, body));
                i = body;
                continue;
            }
        }
        // Not a marker: skip any decimal tail so `2.18` is consumed whole.
        while i < bytes.len() && (bytes[i].is_ascii_digit() || bytes[i] == b'.') {
            i += 1;
        }
    }
    marks
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
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

        // Modelled. Every finding here is held against the model, so a
        // correct model produces none. An earlier version fired on every
        // connective unconditionally, which meant modelling an article
        // raised its score and a correct and a wrong model were
        // indistinguishable. The check must reward the work, not the silence.
        let mr = mr.unwrap_or(&Value::Null);
        let model_text = render(mr).to_lowercase();
        let branches = branch_count(mr);
        let connectives_in_text: usize = leden
            .iter()
            .map(|(_, body)| {
                let body_lower = body.to_lowercase();
                CONNECTIVES
                    .iter()
                    .filter(|c| contains_phrase(&body_lower, c))
                    .count()
            })
            .sum();

        // A derogation or exception has to show up as a branch somewhere.
        // One branch can carry several connectives, so the test is that
        // there is at least one, not that the counts match.
        if connectives_in_text > 0 && branches == 0 {
            let words: Vec<String> = leden
                .iter()
                .flat_map(|(n, body)| {
                    let body_lower = body.to_lowercase();
                    CONNECTIVES
                        .iter()
                        .filter(move |c| contains_phrase(&body_lower, c))
                        .map(move |c| format!("lid {n}: {c}"))
                        .collect::<Vec<_>>()
                })
                .collect();
            findings.push(Finding::new(
                "coverage",
                Some(&number),
                format!(
                    "text is conditional but the model has no branch ({})",
                    words.join(", ")
                ),
            ));
        }

        for (n, body) in &leden {
            let body_lower = body.to_lowercase();
            for hit in TIME_WORDS
                .iter()
                .copied()
                .filter(|w| contains_phrase(&body_lower, w))
            {
                // A period must be carried by a name the engine can use, not
                // by prose: a `description` mentioning "peildatum" satisfied
                // the old check without changing a single binding.
                if !names_carry(mr, hit) {
                    findings.push(Finding::new(
                        "coverage",
                        Some(&number),
                        format!(
                            "lid {n} is bound to \"{hit}\" but no parameter, input or output names a period"
                        ),
                    ));
                }
            }
        }
        let _ = model_text;
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
        for (regulation, output, has_description) in cross_law_sources(mr) {
            // A source without `regulation` resolves inside this law: it
            // names an output another article here produces. That is
            // legitimate, so check it against this file instead of the
            // corpus.
            if regulation.is_empty() {
                if output.is_empty() {
                    // A source with only a `description` is an input the
                    // model could not bind and said so. That is a different
                    // claim from an empty `source: {}`, and lumping them
                    // together made a motivated external fact score the same
                    // as a silent hole.
                    let described = has_description;
                    findings.push(Finding::new(
                        if described {
                            "external-input"
                        } else {
                            "binding"
                        },
                        Some(&number),
                        if described {
                            "input is declared external and unbound; the engine cannot supply it"
                                .to_string()
                        } else {
                            "empty source: no regulation, no output, no reason".to_string()
                        },
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

/// How many conditional constructs the model carries. Used to test whether
/// a conditional statutory text has any branch at all.
fn branch_count(mr: &Value) -> usize {
    let mut n = 0usize;
    walk(mr, &mut |key, node| {
        if matches!(key, Some("conditions") | Some("cases")) && node.as_sequence().is_some() {
            n += 1;
        }
        if key == Some("operation") {
            if let Some(op) = node.as_str() {
                if matches!(op, "IF" | "IF_ELSE" | "SWITCH" | "CASE") {
                    n += 1;
                }
            }
        }
    });
    n
}

/// Whether a parameter, input or output name (or a `period`/`type_spec`
/// field) mentions `word`. Prose in a `description` deliberately does not
/// count: the point is that the value carries the period, not the comment.
fn names_carry(mr: &Value, word: &str) -> bool {
    let mut found = false;
    walk(mr, &mut |key, node| {
        match key {
            Some("parameters") | Some("input") | Some("output") => {
                if let Some(seq) = node.as_sequence() {
                    for item in seq {
                        if item
                            .get("name")
                            .and_then(Value::as_str)
                            .is_some_and(|n| n.to_lowercase().contains(word))
                        {
                            found = true;
                        }
                    }
                }
            }
            // A declared period on a quantity counts whatever it says: the
            // field itself is the statement that a period was considered.
            Some("period") | Some("reference_date") | Some("tijdvak") if node.is_string() => {
                found = true;
            }
            _ => {}
        }
        // A definition or variable named after the period counts too.
        if let Some(k) = key {
            if k.to_lowercase().contains(word) {
                found = true;
            }
        }
    });
    found
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
    walk_outside_sources(doc, &mut |key, node| {
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
    walk_outside_sources(doc, &mut |key, node| {
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
fn cross_law_sources(v: &Value) -> Vec<(String, String, bool)> {
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
                let described = map.get(Value::from("description")).is_some();
                out.push((reg, output, described));
            }
        }
    });
    out
}

/// Depth-first walk calling `f(key_of_this_node, node)`.
fn walk<'a>(v: &'a Value, f: &mut impl FnMut(Option<&'a str>, &'a Value)) {
    walk_inner(None, v, f);
}

/// Like [`walk`] but never descends into a `source` mapping. Inside a
/// `source`, `output` names what is being *read*; counting that as a
/// declaration let every internal reference certify itself, which made the
/// unresolved-reference branch unreachable.
fn walk_outside_sources<'a>(v: &'a Value, f: &mut impl FnMut(Option<&'a str>, &'a Value)) {
    fn inner<'a>(
        key: Option<&'a str>,
        v: &'a Value,
        f: &mut impl FnMut(Option<&'a str>, &'a Value),
    ) {
        if key == Some("source") {
            return;
        }
        f(key, v);
        match v {
            Value::Mapping(map) => {
                for (k, val) in map {
                    inner(k.as_str(), val, f);
                }
            }
            Value::Sequence(seq) => {
                for item in seq {
                    inner(key, item, f);
                }
            }
            _ => {}
        }
    }
    inner(None, v, f);
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
    walk_outside_sources(&doc, &mut |key, node| {
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
    fn leden_split_works_on_folded_text_without_line_breaks() {
        // A harvest that folds an article onto one line used to collapse
        // every article to a single lid, which silently disabled the
        // per-lid accounting on entire laws.
        let folded = "1. De eerste regel geldt. 2. De tweede regel geldt, tenzij                       artikel 2.18 anders bepaalt. 3. De derde regel geldt.";
        let leden = split_leden(folded);
        assert_eq!(leden.len(), 3, "{leden:?}");
        assert_eq!(leden[1].0, 2);
        assert!(leden[1].1.contains("2.18"), "{:?}", leden[1].1);
        assert!(leden[2].1.starts_with("De derde"));
    }

    #[test]
    fn coverage_stays_silent_when_a_conditional_text_has_a_branch() {
        // The point of the check is to reward the work. An earlier version
        // fired on every connective regardless of the model, so a correct
        // and a wrong model scored the same.
        let yaml = r#"
articles:
  - number: '2'
    text: |-
      1. De verzekerde heeft aanspraak op een zorgtoeslag.

      4. In afwijking van het eerste lid bedraagt de aanspraak vijftig procent.
    machine_readable:
      execution:
        actions:
          - output: hoogte_zorgtoeslag
            operation: IF
            conditions:
              - test: {operation: EQUALS, subject: $partner_is_verzekerde, value: false}
                then: $halve_aanspraak
              - else: $volle_aanspraak
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = coverage(&doc);
        assert!(
            !findings.iter().any(|f| f.detail.contains("no branch")),
            "a branched model must not be reported: {findings:?}"
        );
    }

    #[test]
    fn coverage_is_not_satisfied_by_a_period_mentioned_only_in_prose() {
        let yaml = r#"
articles:
  - number: '2'
    text: |-
      5. De aanspraak wordt voor iedere kalendermaand afzonderlijk bepaald.
    machine_readable:
      execution:
        input:
          - name: bedrag
            source:
              description: wordt per kalendermaand vastgesteld
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = coverage(&doc);
        assert!(
            findings.iter().any(|f| f.detail.contains("kalendermaand")),
            "prose must not satisfy the period requirement: {findings:?}"
        );
    }

    #[test]
    fn a_described_unbound_source_is_not_the_same_as_an_empty_one() {
        let described = r#"
articles:
  - number: '2'
    text: tekst
    machine_readable:
      execution:
        input:
          - name: extern_feit
            source:
              description: dit gegeven komt van buiten het corpus
"#;
        let empty = r#"
articles:
  - number: '2'
    text: tekst
    machine_readable:
      execution:
        input:
          - name: extern_feit
            source: {}
"#;
        let d: Value = serde_yaml_ng::from_str(described).unwrap();
        let e: Value = serde_yaml_ng::from_str(empty).unwrap();
        assert_eq!(binding_integrity(&d, None)[0].check, "external-input");
        assert_eq!(binding_integrity(&e, None)[0].check, "binding");
    }

    #[test]
    fn an_internal_reference_does_not_certify_itself() {
        // `source: {output: X}` reads X; it does not declare it. Counting it
        // as a declaration made the unresolved-reference branch unreachable.
        let yaml = r#"
articles:
  - number: '2'
    text: tekst
    machine_readable:
      execution:
        input:
          - name: premie
            source:
              output: standaardpremie
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = binding_integrity(&doc, None);
        assert!(
            findings
                .iter()
                .any(|f| f.detail.contains("standaardpremie")),
            "expected the dangling internal reference to be reported: {findings:?}"
        );
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
