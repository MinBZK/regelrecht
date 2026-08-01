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
        // A definition provision states what words mean. Its chapeau caveat
        // ("tenzij anders is geregeld") and its enumerations ("dan wel")
        // are not conditions on a value, so expecting a branch there is
        // noise, and noise is what teaches a reader to stop looking.
        let is_definition = is_definition_text(text);
        if connectives_in_text > 0 && branches == 0 && !is_definition {
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

/// Operations the engine has. An `untranslatable` whose reason is that one
/// of these does not exist is stale rather than true, and the translator
/// took a detour it did not have to take.
///
/// Kept here rather than derived from the schema because the schema's enums
/// are not one list: rounding, date arithmetic and comparison live in
/// different definitions. Out of date is better than absent, and the check
/// only ever suggests.
pub const AVAILABLE_OPERATIONS: &[(&str, &[&str])] = &[
    ("ROUND", &["afronden", "afronding", "rounding", "round"]),
    (
        "CEIL",
        &["naar boven afronden", "ceil", "afronden naar boven"],
    ),
    (
        "FLOOR",
        &["naar beneden afronden", "floor", "afronden naar beneden"],
    ),
    (
        "DATE_DIFF",
        &[
            "datumverschil",
            "verschil tussen data",
            "date difference",
            "aantal dagen tussen",
        ],
    ),
    (
        "DATE_ADD",
        &["datum optellen", "date add", "termijn optellen bij"],
    ),
    ("AGE", &["leeftijd berekenen", "leeftijd op", "age"]),
    (
        "DAY_OF_WEEK",
        &["dag van de week", "day of week", "weekdag"],
    ),
];

/// Ways of saying "the engine cannot do this". An untranslatable that does
/// not make that claim is about the shape of the model rather than about a
/// missing operation, and comparing it against the operation list would
/// only produce noise.
const ABSENCE_CLAIMS: &[&str] = &[
    "kent geen",
    "heeft geen",
    "ondersteunt geen",
    "bestaat niet",
    "is niet beschikbaar",
    "niet beschikbaar als",
    "geen operatie",
    "not available as an engine operation",
    "the engine cannot",
    "no such operation",
];

/// Words that give a marking away as being about the corpus rather than
/// about the engine's language. A norm filled by a regulation that has not
/// been harvested is a `norm_gap`, and recording it as an `untranslatable`
/// sends it to the wrong queue: nobody builds an operation for it and the
/// harvest never learns it is wanted.
const CORPUS_GAP_SIGNALS: &[&str] = &[
    "niet in het corpus",
    "niet geoogst",
    "ministeriële regeling",
    "ministeriele regeling",
    "beleidsregel",
    "amvb",
    "algemene maatregel van bestuur",
    "bij regeling",
    "nadere regels",
    "nog niet beschikbaar",
];

/// Patterns that look like a citation of a document outside the corpus.
/// An agent without network cannot have read one, so a citation is either
/// something the worker put in front of it or something it recalled, and
/// the two are indistinguishable to a reader.
const CITATION_SIGNALS: &[&str] = &[
    "kst-",
    "kamerstukken",
    "zoek.officielebekendmakingen.nl",
    "stb-",
    "staatsblad",
    "stcrt-",
    "staatscourant",
    "ecli:",
];

/// Whether anything the agent wrote beside the law names a source it cannot
/// have read.
///
/// The law YAML is not the only file an agent produces. It writes scenario
/// files, result envelopes and notes, and a check that only looks at the law
/// misses whatever landed elsewhere. That is not hypothetical: a fabricated
/// kamerstuk reference with a working URL appeared in a `.feature` file while
/// the law itself was clean.
///
/// `provided` is every text the agent was given, so a reference that occurs
/// there is one it read rather than recalled.
pub fn citations_in_companion_files(dir: &Path, provided: &str) -> Vec<Finding> {
    let provided_lower = provided.to_lowercase();
    let mut findings = Vec::new();

    let mut stack = vec![dir.to_path_buf()];
    while let Some(current) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&current) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            // The law itself is covered by `marking_discipline`, which knows
            // its structure and can say which article a finding belongs to.
            let name = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or_default();
            if name.ends_with(".yaml") && !name.starts_with('.') {
                continue;
            }
            // The context brief is written by the worker, not the agent, and
            // citing it is allowed by construction. Scanning it would flag
            // the worker's own work as an unsupported claim.
            if name == super::context::CONTEXT_BRIEF {
                continue;
            }
            let Ok(body) = std::fs::read_to_string(&path) else {
                continue;
            };
            let lower = body.to_lowercase();
            for signal in CITATION_SIGNALS {
                if lower.contains(signal) && !provided_lower.contains(signal) {
                    findings.push(Finding::new(
                        "citation",
                        None,
                        format!(
                            "{name} cites \"{signal}\", which appears in no text that was \
                             provided; a source that was not read may be named as a lead but \
                             not as a citation"
                        ),
                    ));
                    break;
                }
            }
        }
    }
    findings.sort_by(|a, b| a.detail.cmp(&b.detail));
    findings.dedup();
    findings
}

/// Whether the markings are in the channel they belong to, and whether the
/// file cites anything it cannot have read.
///
/// Neither is a defect in the translation. Both are a defect in the record,
/// and the record is what a reader has to trust.
pub fn marking_discipline(doc: &Value, text_corpus: &str) -> Vec<Finding> {
    let mut findings = Vec::new();
    let corpus_lower = text_corpus.to_lowercase();

    for article in articles(doc).iter() {
        let number = article_number(article).unwrap_or_default();
        let Some(mr) = article.get("machine_readable") else {
            continue;
        };

        // Untranslatables that describe a corpus gap or a solved one.
        if let Some(seq) = mr.get("untranslatables").and_then(Value::as_sequence) {
            for entry in seq {
                let construct = entry
                    .get("construct")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let reason = entry
                    .get("reason")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let both = format!("{construct} {reason}").to_lowercase();

                if let Some(signal) = CORPUS_GAP_SIGNALS.iter().find(|s| both.contains(**s)) {
                    findings.push(Finding::new(
                        "marking",
                        Some(&number),
                        format!(
                            "untranslatable \"{}\" mentions \"{signal}\"; a norm filled elsewhere                              is a norm_gap, not a gap in the engine's language",
                            truncate(construct)
                        ),
                    ));
                }

                // Only when the reason asserts an absence. A construct that
                // merely mentions a date says nothing about which operation
                // it needs, and flagging on that alone cries wolf: the
                // eighteenth-birthday rule wants a month boundary, not AGE.
                let reason_lower = reason.to_lowercase();
                let claims_absence = ABSENCE_CLAIMS.iter().any(|c| reason_lower.contains(c));
                if claims_absence {
                    for (op, phrases) in AVAILABLE_OPERATIONS {
                        if phrases.iter().any(|p| reason_lower.contains(p)) {
                            findings.push(Finding::new(
                                "marking",
                                Some(&number),
                                format!(
                                    "untranslatable \"{}\" says the engine cannot do this, and names                                      something {op} does",
                                    truncate(construct)
                                ),
                            ));
                            break;
                        }
                    }
                }
            }
        }

        // Anything that reads as a citation of a document outside the corpus.
        let rendered = render(mr).to_lowercase();
        for signal in CITATION_SIGNALS {
            if rendered.contains(signal) && !corpus_lower.contains(signal) {
                findings.push(Finding::new(
                    "citation",
                    Some(&number),
                    format!(
                        "cites \"{signal}\", which appears in no text that was provided;                          a source that was not read may be named as a lead but not as a citation"
                    ),
                ));
                break;
            }
        }
    }
    findings
}

fn truncate(s: &str) -> String {
    let cut: String = s.chars().take(60).collect();
    if s.chars().count() > 60 {
        format!("{cut}…")
    } else {
        cut
    }
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

/// Whether an article is a definition provision. Recognised by the fixed
/// words the legislator uses, because the scope of what it defines and the
/// absence of a rule are both consequences of that.
fn is_definition_text(text: &str) -> bool {
    let lower = text.to_lowercase();
    lower.contains("wordt verstaan onder")
        || lower.contains("verstaan onder:")
        || lower.contains("wordt in deze")
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
/// Articles whose text points at another law that the model never reads.
///
/// This is the gate that round 3 lacked, and its absence flattered the wrong
/// variant. The run without a context brief laid no cross-law binding at all,
/// so it drew no binding findings and scored better than the run that tried.
/// A model that attempts nothing cannot be wrong about anything, and no check
/// then distinguishes restraint from emptiness.
///
/// The hook is sturdier than a word list. The harvested text carries the
/// reference links of the statute, and every one of them names a BWB
/// identifier: "artikel 1 van de Zorgverzekeringswet" arrives with
/// `BWBR0018450` beside it. That identifier is what the corpus files carry in
/// their own header, so the question "does this article read the law it cites"
/// is a lookup rather than an interpretation.
///
/// Soft on purpose. Not every citation must become a binding: a reference can
/// be descriptive, or the target may sit outside this corpus. But it must be
/// answered, and an answer may be a marking.
pub fn cross_law_references(doc: &Value, corpus_root: Option<&Path>) -> Vec<Finding> {
    let own_bwb = doc
        .get("bwb_id")
        .and_then(Value::as_str)
        .unwrap_or_default();
    let empty = Vec::new();
    let articles = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty);
    let Some(root) = corpus_root else {
        return Vec::new();
    };
    let index = bwb_index(root);
    let mut findings = Vec::new();

    for entry in articles {
        let (Some(number), Some(text)) = (
            entry.get("number").and_then(Value::as_str),
            entry.get("text").and_then(Value::as_str),
        ) else {
            continue;
        };
        let cited: BTreeSet<&str> = bwb_ids(text)
            .into_iter()
            .filter(|id| *id != own_bwb)
            .collect();
        if cited.is_empty() {
            continue;
        }
        let mr = entry.get("machine_readable");
        // An article that says nothing at all is the `accounted` check's
        // business; saying it twice about the same article helps nobody.
        if mr.is_none() {
            continue;
        }
        let bound = bound_regulations(mr);
        // A marking excuses the reference it names, and only that one. An
        // untranslatable about rounding says nothing about whether this
        // article reads the Zorgverzekeringswet, and treating any marking as
        // a blanket answer is how a gate stops asking.
        let marking_text = mr.map(marking_prose).unwrap_or_default();

        for id in cited {
            let Some(law_id) = index.get(id) else {
                // Outside the corpus. RFC-026 calls that a known gap, and the
                // work queue owns it rather than this file.
                continue;
            };
            if bound.contains(law_id.as_str()) {
                continue;
            }
            if marking_text.contains(law_id.as_str()) || marking_text.contains(id) {
                continue;
            }
            findings.push(Finding::new(
                "reference",
                Some(number),
                format!(
                    "text cites {law_id} ({id}) and the model reads nothing from it. Bind to \
                     the article it names, or record why the reference carries no value"
                ),
            ));
        }
    }
    findings
}

/// Everything the markings of one model say, as one lowercased string.
///
/// Used to check whether a marking mentions the law a reference points at.
/// Crude by design: the question is only whether the agent addressed this
/// reference somewhere, and a substring answers that without prescribing where
/// in the entry the name has to sit.
fn marking_prose(mr: &Value) -> String {
    let mut out = String::new();
    for key in ["untranslatables", "norm_gaps", "structural_choices"] {
        if let Some(seq) = mr.get(key).and_then(Value::as_sequence) {
            for item in seq {
                if let Ok(text) = serde_yaml_ng::to_string(item) {
                    out.push_str(&text.to_lowercase());
                }
            }
        }
    }
    out
}

/// BWB identifiers appearing in a text, in order of first appearance.
fn bwb_ids(text: &str) -> Vec<&str> {
    let mut out = Vec::new();
    let bytes = text.as_bytes();
    let mut i = 0;
    while let Some(pos) = text[i..].find("BWBR") {
        let start = i + pos;
        let mut end = start + 4;
        while end < bytes.len() && bytes[end].is_ascii_digit() {
            end += 1;
        }
        if end > start + 4 {
            let id = &text[start..end];
            if !out.contains(&id) {
                out.push(id);
            }
        }
        i = end.max(start + 4);
    }
    out
}

/// Regulations this model reads from, through any channel.
fn bound_regulations(mr: Option<&Value>) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    let Some(mr) = mr else { return out };
    walk(mr, &mut |key, node| {
        if !matches!(key, Some("source") | Some("overrides") | Some("implements")) {
            return;
        }
        let mut take = |v: &Value| {
            if let Some(s) = v
                .get("regulation")
                .or_else(|| v.get("law"))
                .and_then(Value::as_str)
            {
                out.insert(s.to_owned());
            }
        };
        match node {
            Value::Sequence(seq) => seq.iter().for_each(&mut take),
            other => take(other),
        }
    });
    out
}

/// Map from BWB identifier to law id, built by walking the corpus once.
fn bwb_index(root: &Path) -> BTreeMap<String, String> {
    let mut index = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        let Ok(entries) = std::fs::read_dir(&dir) else {
            continue;
        };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
                continue;
            }
            if path.extension().and_then(|e| e.to_str()) != Some("yaml") {
                continue;
            }
            let Ok(raw) = std::fs::read_to_string(&path) else {
                continue;
            };
            // Read the header only: these files run to hundreds of kilobytes
            // and both fields sit at the top.
            let head: String = raw.lines().take(40).collect::<Vec<_>>().join("\n");
            let Ok(doc) = serde_yaml_ng::from_str::<Value>(&head) else {
                continue;
            };
            if let (Some(bwb), Some(id)) = (
                doc.get("bwb_id").and_then(Value::as_str),
                doc.get("$id").and_then(Value::as_str),
            ) {
                index.insert(bwb.to_owned(), id.to_owned());
            }
        }
    }
    index
}

/// Articles that carry no outcome at all.
///
/// An article passed over without a word is indistinguishable from an article
/// nobody read, and the reviewer cannot tell which happened. The skill now asks
/// for one of four outcomes per article; this is what makes that checkable
/// rather than merely instructed.
///
/// The fourth outcome deserves its own note, because looking for "articles that
/// cannot be executed" turned up almost none. A definition by reference is a
/// cross-law binding. A naming provision belongs to an output computed
/// elsewhere. A statement that the amount depends on income and assets is a
/// property the model must have, and in round 3 it was the property the model
/// broke. Even the citation title fixes what every trace calls this law. What
/// looks like an empty article is usually a provision of a kind nobody looked
/// for yet, which is why `declares` exists and why this check reports the
/// remainder rather than excusing it.
pub fn every_article_accounted(doc: &Value) -> Vec<Finding> {
    let empty = Vec::new();
    let articles = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty);
    let mut findings = Vec::new();

    for entry in articles {
        let Some(number) = entry.get("number").and_then(Value::as_str) else {
            continue;
        };
        let text = entry
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        if text.trim().is_empty() {
            continue;
        }
        let mr = entry.get("machine_readable");
        let carries = |key: &str| {
            mr.and_then(|m| m.get(key))
                .and_then(Value::as_sequence)
                .is_some_and(|s| !s.is_empty())
        };
        let has_logic = mr.is_some_and(|m| {
            m.get("execution").is_some()
                || m.get("definitions").is_some()
                || m.get("requires").is_some()
                || m.get("open_terms").is_some()
                || m.get("implements").is_some()
        });
        if has_logic
            || carries("untranslatables")
            || carries("norm_gaps")
            || carries("declares")
            || carries("overrides")
        {
            continue;
        }
        findings.push(Finding::new(
            "accounted",
            Some(number),
            "carries no outcome: no logic, no untranslatable, no norm gap and no \
             declaration. Passing an article over without a word cannot be told \
             apart from not having read it",
        ));
    }
    findings
}

/// Declarations that contradict the document header they fix.
///
/// A `declares` entry is the provision that decides a top-level property, so a
/// mismatch means one of the two is wrong and neither side knows it. Compares
/// only what is present: an article may fix a property the header omits, and
/// that is a finding on the header rather than on the article.
pub fn declaration_agrees_with_header(doc: &Value) -> Vec<Finding> {
    let empty = Vec::new();
    let mut findings = Vec::new();
    for entry in doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty)
    {
        let Some(number) = entry.get("number").and_then(Value::as_str) else {
            continue;
        };
        let Some(declares) = entry
            .get("machine_readable")
            .and_then(|m| m.get("declares"))
            .and_then(Value::as_sequence)
        else {
            continue;
        };
        for d in declares {
            let (Some(property), Some(value)) = (
                d.get("property").and_then(Value::as_str),
                d.get("value").and_then(Value::as_str),
            ) else {
                continue;
            };
            match doc.get(property).and_then(Value::as_str) {
                None => findings.push(Finding::new(
                    "declares",
                    Some(number),
                    format!(
                        "fixes {property} as {value:?}, and the document header does not \
                         carry {property} at all"
                    ),
                )),
                Some(header) if header.trim() != value.trim() => {
                    findings.push(Finding::new(
                        "declares",
                        Some(number),
                        format!(
                            "fixes {property} as {value:?} while the header says {header:?}. \
                             The article decides; the header is a copy"
                        ),
                    ));
                }
                Some(_) => {}
            }
        }
    }
    findings
}

/// Overrides that name an article the corpus does not have.
///
/// An override displaces another article's output, and the resolver matches it
/// on the literal triple of regulation, article number and output name. Miss
/// the article number and the override is inert: it neither fires nor fails,
/// and the engine returns both the displaced value and the displacing one
/// without complaint.
///
/// The failure has one cause, measured on round 3. The corpus splits below
/// article level, so the producer of `hoogte_zorgtoeslag` sits at entry `2.1`
/// while the statute cites it as "artikel 2". The agent writes what the statute
/// says. It gets `requires` right (72 of 72 targets resolve in the Awir,
/// including `2.1.e.1°`), so it knows the fragment numbering; it only falls
/// back to the statutory citation here, because nothing ever told it otherwise.
/// `binding_integrity` walks `source` and has never looked at `overrides`.
pub fn override_targets(doc: &Value, corpus_root: Option<&Path>) -> Vec<Finding> {
    let own_id = doc.get("$id").and_then(Value::as_str).unwrap_or_default();
    let mut findings = Vec::new();

    for (article, mr) in articles_with_models(doc) {
        let Some(overrides) = mr.get("overrides").and_then(Value::as_sequence) else {
            continue;
        };
        for entry in overrides {
            let Some(target_article) = entry.get("article").and_then(Value::as_str) else {
                continue;
            };
            let law = entry
                .get("regulation")
                .or_else(|| entry.get("law"))
                .and_then(Value::as_str)
                .unwrap_or(own_id);
            let output = entry.get("output").and_then(Value::as_str);

            // Same file: resolve against the document in hand.
            let target_doc = if law == own_id || law.is_empty() {
                Some(std::borrow::Cow::Borrowed(doc))
            } else {
                match corpus_root.and_then(|root| find_law_file(root, law)) {
                    Some(path) => std::fs::read_to_string(&path)
                        .ok()
                        .and_then(|raw| serde_yaml_ng::from_str::<Value>(&raw).ok())
                        .map(std::borrow::Cow::Owned),
                    // A law outside the corpus is not this file's problem, and
                    // `binding_integrity` already reports the unresolvable
                    // regulation itself. Saying it twice trains people to skim.
                    None => continue,
                }
            };
            let Some(target_doc) = target_doc else {
                continue;
            };

            let numbers = article_numbers(&target_doc);
            if !numbers.iter().any(|n| n == target_article) {
                let near = numbers
                    .iter()
                    .filter(|n| {
                        n.starts_with(&format!("{target_article}."))
                            || target_article.starts_with(&format!("{n}."))
                    })
                    .take(3)
                    .cloned()
                    .collect::<Vec<_>>();
                let hint = if near.is_empty() {
                    String::new()
                } else {
                    format!(", did you mean {}", near.join(" or "))
                };
                findings.push(Finding::new(
                    "override",
                    Some(&article),
                    format!(
                        "overrides {law} article {target_article}, which this corpus does not \
                         have{hint}. The override never fires and both values are returned"
                    ),
                ));
                continue;
            }

            if let Some(output) = output {
                if !doc_article_defines_output(&target_doc, target_article, output) {
                    findings.push(Finding::new(
                        "override",
                        Some(&article),
                        format!(
                            "overrides output {output} on {law} article {target_article}, which \
                             that article does not produce"
                        ),
                    ));
                }
            }
        }
    }
    findings
}

/// Cross-law bindings whose two sides label the same quantity differently.
///
/// `unit` is a label and never a conversion (RFC-023), so two different labels
/// on one binding mean a factor is missing. Within one article the engine
/// checks this; across a law boundary `resolver.rs` checks nothing, which is
/// why the failure is silent rather than loud.
///
/// Measured cost of the silence: the zorgtoeslag counts in eurocent 42 times
/// and the Awir in euro 67 times, and the same person on the same income comes
/// out at € 827,63 or € 1.550,46 depending on which convention wins. Nothing
/// warns.
///
/// Says nothing when either side omits `unit`. A year number and a month index
/// are properly dimensionless, and demanding a label there would train people
/// to add meaningless ones.
pub fn binding_units(doc: &Value, corpus_root: Option<&Path>) -> Vec<Finding> {
    let Some(corpus_root) = corpus_root else {
        return Vec::new();
    };
    let mut findings = Vec::new();
    let mut cache: BTreeMap<String, Option<Value>> = BTreeMap::new();

    for (article, mr) in articles_with_models(doc) {
        for (name, input) in declared_inputs(mr) {
            let Some(source) = input.get("source") else {
                continue;
            };
            let (Some(law), Some(output)) = (
                source
                    .get("regulation")
                    .or_else(|| source.get("law"))
                    .and_then(Value::as_str),
                source.get("output").and_then(Value::as_str),
            ) else {
                continue;
            };
            let Some(here) = unit_of(input) else { continue };

            let target = cache.entry(law.to_owned()).or_insert_with(|| {
                find_law_file(corpus_root, law)
                    .and_then(|p| std::fs::read_to_string(p).ok())
                    .and_then(|raw| serde_yaml_ng::from_str::<Value>(&raw).ok())
            });
            let Some(target) = target.as_ref() else {
                continue;
            };
            let Some(there) = output_unit(target, output) else {
                continue;
            };

            if here != there {
                findings.push(Finding::new(
                    "unit",
                    Some(&article),
                    format!(
                        "input {name} is labelled {here} and reads {law}.{output}, which is \
                         labelled {there}. A unit is a label and never a conversion, so a factor \
                         is missing on one side"
                    ),
                ));
            }
        }
    }
    findings
}

/// Article numbers this document carries, in document order.
fn article_numbers(doc: &Value) -> Vec<String> {
    doc.get("articles")
        .and_then(Value::as_sequence)
        .map(|arts| {
            arts.iter()
                .filter_map(|a| a.get("number").and_then(Value::as_str))
                .map(str::to_owned)
                .collect()
        })
        .unwrap_or_default()
}

/// Whether one named article of a document declares one named output.
fn doc_article_defines_output(doc: &Value, article: &str, output: &str) -> bool {
    let Some(arts) = doc.get("articles").and_then(Value::as_sequence) else {
        return false;
    };
    for entry in arts {
        if entry.get("number").and_then(Value::as_str) != Some(article) {
            continue;
        }
        let Some(mr) = entry.get("machine_readable") else {
            return false;
        };
        let mut found = false;
        walk_outside_sources(mr, &mut |key, node| {
            if key != Some("output") {
                return;
            }
            match node {
                Value::Sequence(seq) => {
                    if seq
                        .iter()
                        .any(|item| item.get("name").and_then(Value::as_str) == Some(output))
                    {
                        found = true;
                    }
                }
                Value::String(s) if s == output => found = true,
                _ => {}
            }
        });
        return found;
    }
    false
}

/// The `unit` a declared value carries, if it carries one.
fn unit_of(node: &Value) -> Option<&str> {
    node.get("type_spec")
        .and_then(|t| t.get("unit"))
        .and_then(Value::as_str)
}

/// The unit declared on one named output of a document.
fn output_unit<'a>(doc: &'a Value, output: &str) -> Option<&'a str> {
    let arts = doc.get("articles").and_then(Value::as_sequence)?;
    for entry in arts {
        let mr = entry.get("machine_readable")?;
        let outputs = mr.get("output").and_then(Value::as_sequence);
        if let Some(seq) = outputs {
            for item in seq {
                if item.get("name").and_then(Value::as_str) == Some(output) {
                    return unit_of(item);
                }
            }
        }
    }
    None
}

/// The declared inputs of a model, by name.
fn declared_inputs(mr: &Value) -> Vec<(String, &Value)> {
    mr.get("input")
        .and_then(Value::as_sequence)
        .map(|seq| {
            seq.iter()
                .filter_map(|i| {
                    i.get("name")
                        .and_then(Value::as_str)
                        .map(|n| (n.to_owned(), i))
                })
                .collect()
        })
        .unwrap_or_default()
}

/// Article number paired with its model, for every article that has one.
fn articles_with_models(doc: &Value) -> Vec<(String, &Value)> {
    doc.get("articles")
        .and_then(Value::as_sequence)
        .map(|arts| {
            arts.iter()
                .filter_map(|a| {
                    let number = a.get("number").and_then(Value::as_str)?;
                    let mr = a.get("machine_readable")?;
                    Some((number.to_owned(), mr))
                })
                .collect()
        })
        .unwrap_or_default()
}

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
///
/// `companion_dir`, when given, is the directory the agent worked in: every
/// non-law file under it is checked for citations too, because an agent
/// writes more than the law and a check that only reads the law misses it.
pub fn run_with_companions(
    yaml: &str,
    corpus_root: Option<&Path>,
    companion_dir: Option<&Path>,
) -> Report {
    let mut report = run(yaml, corpus_root);
    if let Some(dir) = companion_dir {
        let statutory_text = serde_yaml_ng::from_str::<Value>(yaml)
            .map(|doc| all_article_text(&doc))
            .unwrap_or_default();
        report
            .findings
            .extend(citations_in_companion_files(dir, &statutory_text));
    }
    report
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
    let statutory_text = all_article_text(&doc);
    let mut findings = coverage(&doc);
    findings.extend(enum_provenance(&doc));
    findings.extend(binding_integrity(&doc, corpus_root));
    findings.extend(override_targets(&doc, corpus_root));
    findings.extend(every_article_accounted(&doc));
    findings.extend(cross_law_references(&doc, corpus_root));
    findings.extend(declaration_agrees_with_header(&doc));
    findings.extend(binding_units(&doc, corpus_root));
    findings.extend(marking_discipline(&doc, &statutory_text));
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

    /// Round 3, wet_op_de_zorgtoeslag article 2.4: the override names "2"
    /// while the producer sits at entry "2.1", so it never fires and the
    /// engine returns both the halved and the unhalved amount.
    /// A corpus with one law the citing file can resolve.
    fn corpus_with_awir() -> tempfile::TempDir {
        let dir = tempfile::tempdir().expect("tempdir");
        let awir = dir.path().join("regulation/nl/wet/awir");
        std::fs::create_dir_all(&awir).expect("mkdir");
        std::fs::write(
            awir.join("2026-01-01.yaml"),
            "$id: awir\nbwb_id: BWBR0018472\narticles: []\n",
        )
        .expect("write");
        dir
    }

    #[test]
    fn an_article_that_cites_a_law_it_never_reads_is_flagged() {
        // Round 3, variant without a context brief: nought cross-law bindings
        // across the whole file, and therefore nought binding findings. A
        // model that attempts nothing cannot be wrong about anything.
        let dir = corpus_with_awir();
        let doc: Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0018451
articles:
  - number: "5.2"
    text: "het toetsingsinkomen, bedoeld in artikel 8 van de Awir [ref]: https://wetten.overheid.nl/BWBR0018472#Artikel8"
    machine_readable:
      execution:
        output: iets
"#,
        )
        .expect("yaml");
        let f = cross_law_references(&doc, Some(dir.path()));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].check, "reference");
        assert!(f[0].detail.contains("awir"));
    }

    #[test]
    fn a_binding_to_the_cited_law_settles_it() {
        let dir = corpus_with_awir();
        let doc: Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0018451
articles:
  - number: "5.2"
    text: "bedoeld in artikel 8 [ref]: https://wetten.overheid.nl/BWBR0018472#Artikel8"
    machine_readable:
      input:
        - name: toetsingsinkomen
          source: {regulation: awir, output: toetsingsinkomen}
"#,
        )
        .expect("yaml");
        assert!(cross_law_references(&doc, Some(dir.path())).is_empty());
    }

    #[test]
    fn a_marking_excuses_only_the_reference_it_names() {
        // An untranslatable about rounding says nothing about whether this
        // article reads the law it cites.
        let dir = corpus_with_awir();
        let unrelated: Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0018451
articles:
  - number: "5.2"
    text: "bedoeld in artikel 8 [ref]: https://wetten.overheid.nl/BWBR0018472#Artikel8"
    machine_readable:
      untranslatables:
        - construct: rounding
          reason: the engine cannot round
"#,
        )
        .expect("yaml");
        assert_eq!(cross_law_references(&unrelated, Some(dir.path())).len(), 1);

        let named: Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0018451
articles:
  - number: "5.2"
    text: "bedoeld in artikel 8 [ref]: https://wetten.overheid.nl/BWBR0018472#Artikel8"
    machine_readable:
      norm_gaps:
        - norm: toetsingsinkomen
          kind: delegated
          blocks: [x]
          expected_source: awir article 8, not yet enriched
"#,
        )
        .expect("yaml");
        assert!(cross_law_references(&named, Some(dir.path())).is_empty());
    }

    #[test]
    fn a_citation_outside_the_corpus_is_a_known_gap_and_stays_quiet() {
        // The Zorgverzekeringswet is cited 38 times by the zorgtoeslag and is
        // not in this corpus. RFC-026 calls that the work queue's business.
        let dir = corpus_with_awir();
        let doc: Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0018451
articles:
  - number: "1.1"
    text: "de schadeverzekering [ref]: https://wetten.overheid.nl/BWBR0018450#Artikel1"
    machine_readable:
      execution:
        output: iets
"#,
        )
        .expect("yaml");
        assert!(cross_law_references(&doc, Some(dir.path())).is_empty());
    }

    #[test]
    fn an_article_without_any_outcome_is_reported() {
        // Round 3, zorgtoeslag 1.2: "De hoogte van de zorgtoeslag is
        // afhankelijk van de draagkracht op basis van het inkomen en het
        // vermogen." Passed over in silence, and it is exactly the property
        // the model went on to break.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1.2"
    text: De hoogte van de zorgtoeslag is afhankelijk van inkomen en vermogen.
"#,
        )
        .expect("yaml");
        let f = every_article_accounted(&doc);
        assert_eq!(f.len(), 1);
        assert_eq!(f[0].check, "accounted");
        assert_eq!(f[0].article.as_deref(), Some("1.2"));
    }

    #[test]
    fn any_of_the_four_outcomes_settles_an_article() {
        for outcome in [
            "machine_readable:\n      execution:\n        output: x",
            "machine_readable:\n      untranslatables:\n        - construct: foreach",
            "machine_readable:\n      norm_gaps:\n        - norm: de standaardpremie\n          kind: delegated\n          blocks: [x]",
            "machine_readable:\n      declares:\n        - property: name\n          value: Testwet",
        ] {
            let doc: Value = serde_yaml_ng::from_str(&format!(
                "articles:\n  - number: \"1\"\n    text: iets\n    {outcome}\n"
            ))
            .expect("yaml");
            assert!(
                every_article_accounted(&doc).is_empty(),
                "outcome should settle the article: {outcome}"
            );
        }
    }

    #[test]
    fn an_empty_marking_list_does_not_count_as_an_outcome() {
        // Writing `untranslatables: []` is the cheapest way to silence a check
        // and says nothing at all.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: iets
    machine_readable:
      untranslatables: []
"#,
        )
        .expect("yaml");
        assert_eq!(every_article_accounted(&doc).len(), 1);
    }

    #[test]
    fn a_declaration_that_contradicts_the_header_is_reported() {
        // Awir article 51 fixes what this law is called, and every trace that
        // names the law is quoting it. If the header disagrees, one of the two
        // is wrong and nothing says which.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
name: Iets anders
articles:
  - number: "51"
    text: "Deze wet wordt aangehaald als: Algemene wet inkomensafhankelijke regelingen."
    machine_readable:
      declares:
        - property: name
          value: Algemene wet inkomensafhankelijke regelingen
"#,
        )
        .expect("yaml");
        let f = declaration_agrees_with_header(&doc);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].detail.contains("Iets anders"));
    }

    #[test]
    fn a_declaration_that_matches_the_header_is_silent() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
name: Algemene wet inkomensafhankelijke regelingen
articles:
  - number: "51"
    text: "Deze wet wordt aangehaald als: Algemene wet inkomensafhankelijke regelingen."
    machine_readable:
      declares:
        - property: name
          value: Algemene wet inkomensafhankelijke regelingen
"#,
        )
        .expect("yaml");
        assert!(declaration_agrees_with_header(&doc).is_empty());
    }

    #[test]
    fn override_naming_an_article_the_corpus_splits_is_flagged() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: wet_op_de_zorgtoeslag
articles:
  - number: "2.1"
    machine_readable:
      output:
        - name: hoogte_zorgtoeslag
  - number: "2.4"
    machine_readable:
      overrides:
        - article: "2"
          output: hoogte_zorgtoeslag
"#,
        )
        .expect("yaml");
        let f = override_targets(&doc, None);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].check, "override");
        assert_eq!(f[0].article.as_deref(), Some("2.4"));
        assert!(
            f[0].detail.contains("2.1"),
            "should point at the real entry"
        );
    }

    #[test]
    fn override_naming_an_article_that_exists_is_left_alone() {
        // Article 4 is not fragmented in the real corpus, and that override
        // is the one of five that works. A check that flags it too is worse
        // than no check.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: wet_op_de_zorgtoeslag
articles:
  - number: "4"
    machine_readable:
      output:
        - name: standaardpremie
  - number: "4a.1"
    machine_readable:
      overrides:
        - article: "4"
          output: standaardpremie
"#,
        )
        .expect("yaml");
        assert!(override_targets(&doc, None).is_empty());
    }

    #[test]
    fn override_on_an_article_that_does_not_produce_that_output_is_flagged() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: test_wet
articles:
  - number: "2"
    machine_readable:
      output:
        - name: bedrag
  - number: "5"
    machine_readable:
      overrides:
        - article: "2"
          output: iets_anders
"#,
        )
        .expect("yaml");
        let f = override_targets(&doc, None);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].detail.contains("does not produce"));
    }

    #[test]
    fn override_into_a_law_outside_the_corpus_is_not_reported_twice() {
        // binding_integrity already reports the unresolvable regulation.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: test_wet
articles:
  - number: "1"
    machine_readable:
      overrides:
        - regulation: wet_die_niet_bestaat
          article: "3"
          output: x
"#,
        )
        .expect("yaml");
        assert!(override_targets(&doc, None).is_empty());
    }

    #[test]
    fn a_binding_across_a_unit_boundary_is_flagged() {
        // The measured silence: eurocent on one side, euro on the other, and
        // the same person comes out at 827,63 or 1550,46 with no warning.
        let dir = tempfile::tempdir().expect("tempdir");
        let awir = dir.path().join("regulation/nl/wet/awir");
        std::fs::create_dir_all(&awir).expect("mkdir");
        std::fs::write(
            awir.join("2026-01-01.yaml"),
            r#"
$id: awir
articles:
  - number: "8.1"
    machine_readable:
      output:
        - name: toetsingsinkomen
          type_spec:
            unit: euro
"#,
        )
        .expect("write");
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: wet_op_de_zorgtoeslag
articles:
  - number: "2.2"
    machine_readable:
      input:
        - name: toetsingsinkomen
          type_spec:
            unit: eurocent
          source:
            regulation: awir
            output: toetsingsinkomen
"#,
        )
        .expect("yaml");
        let f = binding_units(&doc, Some(dir.path()));
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].check, "unit");
        assert!(f[0].detail.contains("eurocent") && f[0].detail.contains("euro"));
    }

    #[test]
    fn matching_units_and_missing_units_are_both_silent() {
        let dir = tempfile::tempdir().expect("tempdir");
        let awir = dir.path().join("regulation/nl/wet/awir");
        std::fs::create_dir_all(&awir).expect("mkdir");
        std::fs::write(
            awir.join("2026-01-01.yaml"),
            r#"
$id: awir
articles:
  - number: "8.1"
    machine_readable:
      output:
        - name: toetsingsinkomen
          type_spec:
            unit: euro
        - name: berekeningsjaar
"#,
        )
        .expect("write");
        // Same label: nothing to say.
        let same: Value = serde_yaml_ng::from_str(
            r#"
$id: wzt
articles:
  - number: "1"
    machine_readable:
      input:
        - name: toetsingsinkomen
          type_spec:
            unit: euro
          source: {regulation: awir, output: toetsingsinkomen}
"#,
        )
        .expect("yaml");
        assert!(binding_units(&same, Some(dir.path())).is_empty());

        // No label on either side: a year is properly dimensionless, and
        // demanding a unit there teaches people to add meaningless ones.
        let bare: Value = serde_yaml_ng::from_str(
            r#"
$id: wzt
articles:
  - number: "1"
    machine_readable:
      input:
        - name: berekeningsjaar
          source: {regulation: awir, output: berekeningsjaar}
"#,
        )
        .expect("yaml");
        assert!(binding_units(&bare, Some(dir.path())).is_empty());
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
    fn a_marking_that_claims_a_missing_operation_the_engine_has_is_flagged() {
        let yaml = r#"
articles:
  - number: '1'
    text: De uitkomst wordt afgerond op hele euro's.
    machine_readable:
      untranslatables:
        - construct: afronden op hele euro's
          reason: De engine kent geen operatie voor afronding.
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = marking_discipline(&doc, "De uitkomst wordt afgerond.");
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(
            findings[0].detail.contains("ROUND"),
            "{}",
            findings[0].detail
        );
    }

    #[test]
    fn a_marking_about_the_shape_of_the_model_is_left_alone() {
        // The eighteenth-birthday rule wants a month boundary rather than
        // an age, and it makes no claim about a missing operation. Flagging
        // it on the word "kalendermaand" alone would cry wolf, and a check
        // that cries wolf gets ignored.
        let yaml = r#"
articles:
  - number: '1'
    text: Verzekerde is hij, vanaf de eerste dag van de kalendermaand volgend op zijn achttiende verjaardag.
    machine_readable:
      untranslatables:
        - construct: de eerste dag van de kalendermaand volgend op de achttiende verjaardag
          kind: model_form
          reason: De regel vraagt om afkappen naar een maandgrens, wat het model niet als grootheid draagt.
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(marking_discipline(&doc, "kalendermaand achttiende").is_empty());
    }

    #[test]
    fn a_corpus_gap_recorded_as_an_untranslatable_is_flagged() {
        let yaml = r#"
articles:
  - number: '4'
    text: Bij ministeriële regeling wordt de standaardpremie vastgesteld.
    machine_readable:
      untranslatables:
        - construct: de standaardpremie
          reason: Wordt bij ministeriële regeling vastgesteld en die regeling zit niet in het corpus.
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = marking_discipline(
            &doc,
            "Bij ministeriële regeling wordt de standaardpremie vastgesteld.",
        );
        assert!(
            findings.iter().any(|f| f.detail.contains("norm_gap")),
            "a norm filled elsewhere belongs in norm_gaps: {findings:?}"
        );
    }

    #[test]
    fn a_citation_the_agent_cannot_have_read_is_flagged() {
        let yaml = r#"
articles:
  - number: '2'
    text: De normpremie bedraagt een percentage.
    machine_readable:
      untranslatables:
        - construct: de percentages
          reason: 'Zie Kamerstukken II 2004/05, 29 762, nr. 3 voor de bedoeling.'
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = marking_discipline(&doc, "De normpremie bedraagt een percentage.");
        assert!(
            findings.iter().any(|f| f.check == "citation"),
            "a source that was not provided may be a lead, not a citation: {findings:?}"
        );
    }

    #[test]
    fn a_citation_that_is_in_the_statutory_text_is_not_flagged() {
        let yaml = r#"
articles:
  - number: '2'
    text: 'Zie Staatsblad 2005, 358.'
    machine_readable:
      untranslatables:
        - construct: iets
          reason: 'Verwijst naar Staatsblad 2005, 358, zoals het artikel zelf doet.'
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = marking_discipline(&doc, "Zie Staatsblad 2005, 358.");
        assert!(
            findings.iter().all(|f| f.check != "citation"),
            "{findings:?}"
        );
    }

    #[test]
    fn schema_errors_rejects_a_file_without_a_schema_declaration() {
        let errors = schema_errors("articles: []\n");
        assert!(!errors.is_empty());
        assert!(errors[0].contains("$schema"));
    }
}
