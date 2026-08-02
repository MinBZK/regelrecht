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

/// Operations the engine has. A marking with `resolution: engine` that asks
/// for one of these is stale rather than true, and the translator took a
/// detour it did not have to take.
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

/// Words that give a marking away as being about content that another
/// regulation supplies rather than about something the format cannot express.
/// A norm a ministerial regulation fills is an `open_term`, and recording it
/// as a marking sends it to the wrong queue: nobody builds an operation for
/// it and the work queue never learns which regulation is wanted.
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
        let own_text = article
            .get("text")
            .and_then(Value::as_str)
            .unwrap_or_default();
        let Some(mr) = article.get("machine_readable") else {
            continue;
        };

        for entry in markings(mr) {
            let about = entry.get("about").and_then(Value::as_str).unwrap_or_default();
            let resolved_by = entry
                .get("resolved_by")
                .and_then(Value::as_str)
                .unwrap_or_default();
            let both = format!("{about} {resolved_by}").to_lowercase();

            if let Some(signal) = CORPUS_GAP_SIGNALS.iter().find(|s| both.contains(**s)) {
                findings.push(Finding::new(
                    "marking",
                    Some(&number),
                    format!(
                        "marking \"{}\" mentions \"{signal}\"; a norm whose content another \
                         regulation supplies is an open_term, not a construct the format cannot \
                         express",
                        truncate(about)
                    ),
                ));
            }

            // `resolution: engine` is itself the claim that the operation does
            // not exist, so it can be held against the operation list without
            // reading the prose for an absence claim first. A `model` marking
            // makes no such claim and is left alone: the eighteenth-birthday
            // rule wants a month boundary, not AGE.
            if entry.get("resolution").and_then(Value::as_str) == Some("engine") {
                for (op, phrases) in AVAILABLE_OPERATIONS {
                    if phrases.iter().any(|p| both.contains(p)) {
                        findings.push(Finding::new(
                            "marking",
                            Some(&number),
                            format!(
                                "marking \"{}\" asks for an engine operation and names something \
                                 {op} already does",
                                truncate(about)
                            ),
                        ));
                        break;
                    }
                }
            }

            // The excerpt is what ties the marking to this provision. A
            // marking that quotes words this article does not contain is
            // about something else, and there is then no way to tell whether
            // the construct it names is even in front of it.
            let quote = entry
                .get("legal_text_excerpt")
                .and_then(Value::as_str)
                .unwrap_or_default();
            if quote.trim().is_empty() {
                findings.push(Finding::new(
                    "marking",
                    Some(&number),
                    format!(
                        "marking \"{}\" quotes no legal text; a marking that cannot quote the \
                         words it is about is about something else",
                        truncate(about)
                    ),
                ));
            } else if !normalised(own_text).contains(&normalised(quote)) {
                findings.push(Finding::new(
                    "marking",
                    Some(&number),
                    format!(
                        "marking \"{}\" quotes {:?}, which does not appear in this article's own \
                         text",
                        truncate(about),
                        truncate(quote)
                    ),
                ));
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

/// Whether a model does anything beyond flagging. Shared by the tally and by
/// the accounting gate so the two can never disagree about what "worked out"
/// means.
fn carries_logic(mr: &Value) -> bool {
    ["execution", "definitions", "requires", "open_terms", "implements"]
        .iter()
        .any(|key| mr.get(key).is_some())
}

/// The values a marking says it blocks.
fn marking_targets(marking: &Value) -> Vec<&str> {
    marking
        .get("target")
        .and_then(Value::as_sequence)
        .map(|seq| seq.iter().filter_map(Value::as_str).collect())
        .unwrap_or_default()
}

/// The markings one model carries. Empty when the model has none, which is
/// the ordinary case: a marking is the exception a translation records, not
/// something every article has.
fn markings(mr: &Value) -> &[Value] {
    mr.get("markings")
        .and_then(Value::as_sequence)
        .map(Vec::as_slice)
        .unwrap_or(&[])
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
///
/// Raises two kinds of finding, and the difference is what they cost to fix.
/// `binding` is an error in this file: a name that resolves to nothing, a
/// source without a claim, an output the target law demonstrably does not
/// produce. `outside-corpus` is a known gap: the value comes from a law that
/// has not been harvested, and no edit to this file resolves it.
///
/// Round 4 ran them together, and all 17 binding findings of variant b turned
/// out to be laws outside the corpus, indistinguishable from a real error.
/// Worse, the two variants recorded the same legal situation differently:
/// variant a demoted an unfindable binding to a bare input and drew 7 findings
/// of a softer kind while variant b, which said which law the value came from,
/// drew 17 of a harder one. Steering on fewer findings would have picked the
/// less honest of the two. Both now land here, and the tally reports which
/// route a run took.
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
/// Outputs whose own description names an article that never reads them.
///
/// The sharpest finding of round 4 and the one no gate had: article 1 of the
/// zorgtoeslag computes `is_verzekerde` in full, with the three ZVW capacities,
/// the eighteen-year threshold and the article 24 exception, and the output
/// appears exactly once in the whole file, as the result of its own action.
/// Article 2 does not read it and has no parameter for it, so the model grants
/// the allowance to a sixteen-year-old and to someone whose cover is suspended.
///
/// What makes this catchable is that the description says so itself: "artikel
/// 2, eerste lid, verbindt de aanspraak aan dit begrip". The text asserts a
/// relation the file does not have, and an assertion the agent wrote is
/// something a check can hold it to.
///
/// Narrow on purpose. It does not ask whether an unread output ought to be
/// read, which is a judgement; it asks whether a promise the file makes about
/// itself is kept.
pub fn output_promises(doc: &Value) -> Vec<Finding> {
    let empty = Vec::new();
    let articles = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty);

    // Which article reads which name, by any route.
    let mut readers: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for entry in articles {
        let Some(number) = entry.get("number").and_then(Value::as_str) else {
            continue;
        };
        let Some(mr) = entry.get("machine_readable") else {
            continue;
        };
        walk(mr, &mut |key, node| {
            // An `output` declaration is not a reading of itself.
            if matches!(key, Some("output")) {
                return;
            }
            if let (Some("name" | "value" | "variable"), Value::String(s)) = (key, node) {
                readers
                    .entry(s.clone())
                    .or_default()
                    .insert(number.to_owned());
            }
        });
    }

    let mut findings = Vec::new();
    for entry in articles {
        let (Some(number), Some(mr)) = (
            entry.get("number").and_then(Value::as_str),
            entry.get("machine_readable"),
        ) else {
            continue;
        };
        let mut outputs: Vec<(String, String)> = Vec::new();
        walk_outside_sources(mr, &mut |key, node| {
            if key != Some("output") {
                return;
            }
            if let Value::Sequence(seq) = node {
                for item in seq {
                    if let (Some(name), Some(desc)) = (
                        item.get("name").and_then(Value::as_str),
                        item.get("description").and_then(Value::as_str),
                    ) {
                        outputs.push((name.to_owned(), desc.to_owned()));
                    }
                }
            }
        });

        for (name, description) in outputs {
            // Only the articles this description names, and only those of this
            // law: "van de Zorgverzekeringswet" points outside and the work
            // queue owns that.
            for cited in claiming_references(&description) {
                let reads = readers.get(&name).is_some_and(|by| {
                    by.iter()
                        .any(|r| r == &cited || r.starts_with(&format!("{cited}.")))
                });
                if reads {
                    continue;
                }
                // The naming article itself reading it proves nothing.
                if cited == number || number.starts_with(&format!("{cited}.")) {
                    continue;
                }
                if !articles.iter().any(|a| {
                    a.get("number")
                        .and_then(Value::as_str)
                        .is_some_and(|n| n == cited || n.starts_with(&format!("{cited}.")))
                }) {
                    continue;
                }
                findings.push(Finding::new(
                    "promise",
                    Some(number),
                    format!(
                        "output {name} says article {cited} uses it, and article {cited} \
                         reads nothing of the sort. Either bind it there or drop the claim"
                    ),
                ));
            }
        }
    }
    findings
}

/// Article references in a description that claim use rather than definition.
///
/// "De normpremie, bedoeld in artikel 1, onderdeel h" says where the term is
/// defined; it does not say that article 1 reads this output. Dutch statutes
/// mark that difference with a fixed formula, and treating every reference as
/// a claim of use turned the check into noise: thirteen findings of which the
/// majority were definitional.
///
/// This is a closed set of formulas and not an open vocabulary. Each one has a
/// settled meaning in Dutch legislative drafting, which is why it can be
/// enumerated without the usual objection to word lists.
fn claiming_references(description: &str) -> Vec<String> {
    const DEFINITIONAL: &[&str] = &[
        "bedoeld in",
        "genoemd in",
        "in de zin van",
        "als bedoeld bij",
        "krachtens",
        "op grond van",
    ];
    let lower = description.to_lowercase();
    super::context::referenced_articles(description)
        .into_iter()
        .filter(|number| {
            // Look at what sits just before each mention of this article.
            let needle = format!("artikel {number}");
            let mut definitional_everywhere = true;
            let mut found = false;
            let mut from = 0;
            while let Some(pos) = lower[from..].find(&needle) {
                let at = from + pos;
                found = true;
                // Only what precedes it in the same sentence. Looking a fixed
                // number of characters back read "in de zin van deze wet?
                // Artikel 2 verbindt..." as definitional, because the formula
                // sat in the previous sentence and belonged to something else.
                let sentence_start = lower[..at].rfind(['.', '?', ';', '!']).map_or(0, |i| i + 1);
                let before = &lower[sentence_start..at];
                if !DEFINITIONAL.iter().any(|d| before.contains(d)) {
                    definitional_everywhere = false;
                }
                from = at + needle.len();
            }
            found && !definitional_everywhere
        })
        .collect()
}

/// What a translation attempted, counted rather than judged.
///
/// The gates say whether something is wrong. Nothing said whether anything was
/// attempted, and round 3 paid for that: the variant that laid no cross-law
/// binding at all drew no binding findings and therefore looked better than the
/// variant that tried. Restraint and emptiness produce the same score under a
/// gate, and only a tally tells them apart.
///
/// Deliberately free of judgement. A high `bindings` is not better than a low
/// one; it is a different translation of the same text, and comparing two runs
/// needs both these numbers and the findings beside them.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Tally {
    /// Article entries in the file.
    pub articles: usize,
    /// Entries whose model carries executable logic.
    pub with_logic: usize,
    /// Entries whose model carries only markings or declarations.
    pub marked_only: usize,
    /// Entries with no outcome at all. Mirrors the `accounted` check, and is
    /// here so a run can be read without cross-referencing the findings.
    pub bare: usize,
    /// Bindings that read from another regulation.
    pub cross_law_bindings: usize,
    /// Distinct regulations this file reads from.
    pub laws_read: usize,
    /// Regulations the statutory text cites, whether read or not. The gap
    /// between this and `laws_read` is what the reference gate reports.
    pub laws_cited: usize,
    /// Sources that name no regulation at all and lean on a `description`.
    /// Beside `cross_law_bindings` this is what tells a translation that says
    /// where a value comes from apart from one that only says it comes from
    /// somewhere. Both are the same known gap, so they draw the same finding,
    /// and only these two numbers say which of the two a run chose.
    pub unnamed_sources: usize,
    /// Markings, in total and split by what has to change before the article
    /// can be translated in full.
    pub markings: usize,
    pub markings_engine: usize,
    pub markings_model: usize,
    /// Markings that name at least one value they block. A marking with an
    /// empty `target` asserts the article stays executable, so the split
    /// between these two is the difference between a flag and a hole.
    pub markings_blocking: usize,
    /// Markings a human has signed off on.
    pub markings_accepted: usize,
    /// Open terms, and how many of them name who fills them. A term the law
    /// leaves to whichever authority is competent is a different kind of work
    /// from one waiting on a named ministerial regulation.
    pub open_terms: usize,
    pub open_terms_delegated: usize,
    pub declares: usize,
    pub overrides: usize,
    /// Outputs declared anywhere in the file.
    pub outputs: usize,
    /// Outputs some other model reads, in this file or through a binding it
    /// declares. An output nothing consumes is either dead or a restriction
    /// that restricts nothing.
    pub outputs_consumed: usize,
}

/// Count what a file attempted.
#[must_use]
pub fn tally(doc: &Value) -> Tally {
    let empty = Vec::new();
    let articles = doc
        .get("articles")
        .and_then(Value::as_sequence)
        .unwrap_or(&empty);
    let own_bwb = doc
        .get("bwb_id")
        .and_then(Value::as_str)
        .unwrap_or_default();

    let mut t = Tally {
        articles: articles.len(),
        ..Tally::default()
    };
    let mut laws_read: BTreeSet<String> = BTreeSet::new();
    let mut laws_cited: BTreeSet<String> = BTreeSet::new();
    // Keyed by article, because an output that only its own model mentions is
    // not consumed by anything: the question is whether some other provision
    // reads it. Counting a declaration as its own consumer put the figure at
    // 60 of 61 and said nothing.
    let mut declared_outputs: BTreeMap<String, String> = BTreeMap::new();
    let mut consumed: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();

    for entry in articles {
        let here = entry
            .get("number")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_owned();
        if let Some(text) = entry.get("text").and_then(Value::as_str) {
            for id in bwb_ids(text) {
                if id != own_bwb {
                    laws_cited.insert(id.to_owned());
                }
            }
        }
        let Some(mr) = entry.get("machine_readable") else {
            if entry
                .get("text")
                .and_then(Value::as_str)
                .is_some_and(|t| !t.trim().is_empty())
            {
                t.bare += 1;
            }
            continue;
        };

        let count = |key: &str| mr.get(key).and_then(Value::as_sequence).map_or(0, Vec::len);
        t.declares += count("declares");
        t.overrides += count("overrides");
        t.open_terms += count("open_terms");
        for term in mr
            .get("open_terms")
            .and_then(Value::as_sequence)
            .map_or(&[][..], Vec::as_slice)
        {
            if term.get("delegated_to").is_some() {
                t.open_terms_delegated += 1;
            }
        }
        for marking in markings(mr) {
            t.markings += 1;
            match marking.get("resolution").and_then(Value::as_str) {
                Some("engine") => t.markings_engine += 1,
                Some("model") => t.markings_model += 1,
                _ => {}
            }
            if !marking_targets(marking).is_empty() {
                t.markings_blocking += 1;
            }
            if marking.get("accepted").and_then(Value::as_bool) == Some(true) {
                t.markings_accepted += 1;
            }
        }

        let has_logic = carries_logic(mr);
        let has_marking = count("markings") + count("declares") + count("overrides") > 0;
        if has_logic {
            t.with_logic += 1;
        } else if has_marking {
            t.marked_only += 1;
        } else {
            t.bare += 1;
        }

        laws_read.extend(bound_regulations(Some(mr)));

        // Outputs declared here, and the values this model reads.
        walk_outside_sources(mr, &mut |key, node| {
            if key != Some("output") {
                return;
            }
            match node {
                Value::Sequence(seq) => {
                    for item in seq {
                        if let Some(n) = item.get("name").and_then(Value::as_str) {
                            declared_outputs.insert(n.to_owned(), here.clone());
                        }
                    }
                }
                Value::String(s) => {
                    declared_outputs.insert(s.clone(), here.clone());
                }
                _ => {}
            }
        });
        // A source can sit at any depth: inputs live under `execution`,
        // under `parameters`, and nested inside operations. Counting only the
        // top-level `input` list reported nought bindings on a file with
        // eight of them.
        walk(mr, &mut |key, node| {
            if key != Some("source") {
                return;
            }
            if node.get("regulation").or_else(|| node.get("law")).is_some() {
                t.cross_law_bindings += 1;
            } else if node.get("description").is_some() {
                t.unnamed_sources += 1;
            }
            if let Some(out) = node.get("output").and_then(Value::as_str) {
                consumed
                    .entry(out.to_owned())
                    .or_default()
                    .insert(here.clone());
            }
        });
        // A model that names a value by the name another article produces is
        // reading it, whether or not it declared a source.
        walk(mr, &mut |key, node| {
            if let (Some("name" | "value" | "variable"), Value::String(s)) = (key, node) {
                consumed.entry(s.clone()).or_default().insert(here.clone());
            }
        });
    }

    t.laws_read = laws_read.len();
    t.laws_cited = laws_cited.len();
    t.outputs = declared_outputs.len();
    t.outputs_consumed = declared_outputs
        .iter()
        .filter(|(name, declared_in)| {
            consumed
                .get(*name)
                .is_some_and(|readers| readers.iter().any(|r| r != *declared_in))
        })
        .count();
    t
}

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
/// Not every citation must become a binding: a reference can be descriptive,
/// or the target may sit outside this corpus. But it must be answered, and
/// there are exactly two answers.
///
/// The second answer used to be any marking whose prose contained the law id
/// or the BWB number anywhere. That is the cheapest sentence an agent can
/// write, and a gate with a cheap exit shapes the output that passes it: 43 of
/// the 101 norm gaps in round 4 were cross-references to laws that exist,
/// written up as gaps, because writing the BWB number into a gap was the short
/// way past this check. A value another law produces is an input with a
/// `source`, never a gap.
///
/// So a marking now has to be demonstrably about *this* reference, and the
/// evidence is the one the schema already requires. Every reference in the
/// harvested text is a markdown link whose label is the words of the citation
/// and whose target carries the BWB number, so the citation has a known
/// literal form. A marking answers the reference when its `legal_text_excerpt`
/// contains those words. Quoting the sentence that carries the citation is
/// something you can only do by having read it; naming the number is not.
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
        // A marking excuses the reference whose words it quotes, and only that
        // one. A marking about rounding says nothing about whether this
        // article reads the Zorgverzekeringswet, and treating any marking as a
        // blanket answer is how a gate stops asking.
        let quotes: Vec<String> = mr
            .map(|mr| {
                markings(mr)
                    .iter()
                    .filter_map(|m| m.get("legal_text_excerpt").and_then(Value::as_str))
                    .map(normalised)
                    .collect()
            })
            .unwrap_or_default();

        for id in cited {
            let Some(law_id) = index.get(id) else {
                // Outside the corpus. RFC-026 calls that a known gap, and the
                // work queue owns it rather than this file.
                continue;
            };
            if bound.contains(law_id.as_str()) {
                continue;
            }
            let labels = citation_labels(entry, text, id);
            let quoted = quotes.iter().any(|quote| {
                if labels.is_empty() {
                    // No markdown link to quote, so the number is all there is.
                    quote.contains(&normalised(id))
                } else {
                    labels.iter().any(|label| quote.contains(label))
                }
            });
            if quoted {
                continue;
            }
            findings.push(Finding::new(
                "reference",
                Some(number),
                format!(
                    "text cites {law_id} ({id}) and the model reads nothing from it. Bind to \
                     the article it names, or mark it and quote the words that cite it"
                ),
            ));
        }
    }
    findings
}

/// The words with which an article cites one law, normalised.
///
/// The harvest renders every reference as a markdown link: the label is the
/// citation as the legislator wrote it ("artikel 2.18 van de Wet
/// inkomstenbelasting 2001") and the link target carries the BWB number. The
/// article's `references` list maps the link id to that number, so the citation
/// of a given law has a known literal form and a quotation can be held against
/// it.
///
/// Returns every label for this law, because one article can cite the same law
/// in several places and quoting any one of them addresses that citation.
fn citation_labels(article: &Value, text: &str, bwb_id: &str) -> Vec<String> {
    let mut out = Vec::new();
    let refs = article
        .get("references")
        .and_then(Value::as_sequence)
        .map_or(&[][..], Vec::as_slice);
    for reference in refs {
        if reference.get("bwb_id").and_then(Value::as_str) != Some(bwb_id) {
            continue;
        }
        let Some(id) = reference.get("id").and_then(Value::as_str) else {
            continue;
        };
        let needle = format!("][{id}]");
        let mut from = 0;
        while let Some(pos) = text[from..].find(&needle) {
            let close = from + pos;
            if let Some(open) = text[..close].rfind('[') {
                let label = normalised(&text[open + 1..close]);
                if !label.is_empty() && !out.contains(&label) {
                    out.push(label);
                }
            }
            from = close + needle.len();
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
        let has_logic = mr.is_some_and(carries_logic);
        if has_logic || carries("markings") || carries("declares") || carries("overrides") {
            continue;
        }
        findings.push(Finding::new(
            "accounted",
            Some(number),
            "carries no outcome: no logic, no marking, no open term and no \
             declaration. Passing an article over without a word cannot be told \
             apart from not having read it",
        ));
    }
    findings
}

/// Values a marking declares blocked and the same article computes anyway.
///
/// `target` is the marking's pointer: it names the values in this article that
/// cannot be produced, and an empty list is a statement rather than an
/// omission, namely that the article stays executable. Either way the claim
/// has a consequence the file itself can be held to, and in round 4 it had
/// none: of the 72 outputs recorded as blocked not one was left out, and every
/// one of them was computed by an action in the same article. A declaration
/// that costs nothing to write and contradicts what sits beside it is worse
/// than no declaration, because a reader believes it.
///
/// Hard on purpose. This is not a judgement about the translation but a
/// contradiction inside one file, and no reading of the statute makes both
/// halves true.
pub fn blocked_values_are_absent(doc: &Value) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (number, mr) in articles_with_models(doc) {
        let actions = action_subtrees(mr);
        if actions.is_empty() {
            continue;
        }
        for marking in markings(mr) {
            for name in marking_targets(marking) {
                if actions.iter().any(|node| produces(node, name)) {
                    findings.push(Finding::new(
                        "contradiction",
                        Some(&number),
                        format!(
                            "marking blocks {name}, and an action in this article computes it. \
                             A value you declared unproducible cannot also be produced"
                        ),
                    ));
                } else if actions.iter().any(|node| reads(node, name)) {
                    findings.push(Finding::new(
                        "contradiction",
                        Some(&number),
                        format!(
                            "marking blocks {name}, and an action in this article calculates \
                             with it. Whatever that action produces rests on a value the \
                             marking says is not there"
                        ),
                    ));
                }
            }
        }
    }
    findings
}

/// Articles that carry a marking and nothing else.
///
/// A marking is a flag on an article that is otherwise worked out: it names
/// the one thing that does not fit and leaves everything that does fit
/// standing. An article whose whole model is a marking made the opposite move,
/// and in round 4 that was the largest failure class: the chapeau of article 1
/// of the zorgtoeslag got one marking and nothing more, while the definitions
/// under it were perfectly translatable.
///
/// One exception holds, and it is read off `target` rather than off a field of
/// its own. A marking with `resolution: model` says the format has no shape
/// for this construct, and when it also names the values that consequently
/// cannot be produced, it has said that nothing is left to write down. An
/// empty `target` claims the opposite in the same breath — the article stays
/// executable — so it cannot excuse an article that produces nothing. And a
/// marking with `resolution: engine` never excuses one: a missing operation
/// blocks a step, so the inputs, the parameters and the rest of the
/// calculation can all be written down and only that step is missing.
pub fn markings_leave_something_standing(doc: &Value) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (number, mr) in articles_with_models(doc) {
        let marks = markings(mr);
        if marks.is_empty() || carries_logic(mr) {
            continue;
        }
        let carries = |key: &str| {
            mr.get(key)
                .and_then(Value::as_sequence)
                .is_some_and(|s| !s.is_empty())
        };
        if carries("declares") || carries("overrides") {
            continue;
        }
        if marks.iter().any(|m| {
            m.get("resolution").and_then(Value::as_str) == Some("model")
                && !marking_targets(m).is_empty()
        }) {
            continue;
        }
        findings.push(Finding::new(
            "marking-only",
            Some(&number),
            "the whole model is a marking. A marking flags the one thing that does not \
             fit and leaves what does fit standing; only a model-resolution marking that \
             names what it blocks can say there is nothing left",
        ));
    }
    findings
}

/// Every `actions` list in a model, at any depth.
fn action_subtrees(mr: &Value) -> Vec<&Value> {
    let mut out = Vec::new();
    walk(mr, &mut |key, node| {
        if key == Some("actions") && node.as_sequence().is_some() {
            out.push(node);
        }
    });
    out
}

/// Whether a subtree declares `name` as something it produces.
fn produces(node: &Value, name: &str) -> bool {
    let mut found = false;
    walk_outside_sources(node, &mut |key, node| {
        if key != Some("output") {
            return;
        }
        match node {
            Value::String(s) if s == name => found = true,
            Value::Sequence(seq) => {
                if seq
                    .iter()
                    .any(|item| item.get("name").and_then(Value::as_str) == Some(name))
                {
                    found = true;
                }
            }
            _ => {}
        }
    });
    found
}

/// Whether a subtree calculates with `name`.
fn reads(node: &Value, name: &str) -> bool {
    let reference = format!("${name}");
    let mut found = false;
    walk(node, &mut |_, node| {
        if node.as_str() == Some(reference.as_str()) {
            found = true;
        }
    });
    found
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

            // A voiding override says the entitlement does not arise at all,
            // and the ground for that has to be the words of this article. A
            // quotation only has to be copied; a category would have to be
            // invented, and inventing is what this format exists to prevent.
            if entry.get("voids").and_then(Value::as_bool) == Some(true) {
                let quote = entry
                    .get("legal_text_excerpt")
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                let own_text = doc
                    .get("articles")
                    .and_then(Value::as_sequence)
                    .and_then(|arts| {
                        arts.iter().find(|a| {
                            a.get("number").and_then(Value::as_str) == Some(article.as_str())
                        })
                    })
                    .and_then(|a| a.get("text"))
                    .and_then(Value::as_str)
                    .unwrap_or_default();
                if quote.trim().is_empty() {
                    findings.push(Finding::new(
                        "override",
                        Some(&article),
                        "voids an output without quoting the words of this article that                          establish it",
                    ));
                } else if !normalised(own_text).contains(&normalised(quote)) {
                    findings.push(Finding::new(
                        "override",
                        Some(&article),
                        format!(
                            "voids an output on the ground {quote:?}, which does not appear                              in this article's own text"
                        ),
                    ));
                }
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

/// Collapse whitespace and case so a quotation can be held against the text it
/// came from.
///
/// The harvested text wraps at column boundaries and carries markdown link
/// syntax, so a verbatim quotation rarely matches byte for byte even when it is
/// honest.
fn normalised(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    let mut last_space = true;
    for ch in text.chars() {
        let ch = if ch == '\u{00a0}' { ' ' } else { ch };
        if ch.is_whitespace() {
            if !last_space {
                out.push(' ');
                last_space = true;
            }
        } else if !matches!(ch, '[' | ']' | '*' | '_') {
            out.extend(ch.to_lowercase());
            last_space = false;
        }
    }
    out.trim().to_owned()
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
    findings.extend(blocked_values_are_absent(&doc));
    findings.extend(markings_leave_something_standing(&doc));
    findings.extend(cross_law_references(&doc, corpus_root));
    findings.extend(output_promises(&doc));
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
    fn an_output_whose_description_names_a_reader_that_reads_nothing_is_flagged() {
        // Round 4, the heaviest finding of the legal review. Article 1 computes
        // `is_verzekerde` in full and article 2 never reads it, so the model
        // grants the allowance to a sixteen-year-old. The description says
        // article 2 does read it, and that claim is what makes it catchable.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1.1.c"
    text: verzekerde
    machine_readable:
      execution:
        output:
          - name: is_verzekerde
            type: boolean
            description: >-
              Is de persoon verzekerde in de zin van deze wet? Artikel 2, eerste lid,
              verbindt de aanspraak op zorgtoeslag aan dit begrip.
  - number: "2.1"
    text: aanspraak
    machine_readable:
      execution:
        output:
          - name: bestaat_aanspraak
"#,
        )
        .expect("yaml");
        let f = output_promises(&doc);
        assert_eq!(f.len(), 1, "{f:?}");
        assert_eq!(f[0].check, "promise");
        assert!(f[0].detail.contains("is_verzekerde"));
    }

    #[test]
    fn a_definitional_reference_is_not_a_claim_of_use() {
        // "De normpremie, bedoeld in artikel 1, onderdeel h" says where the
        // term is defined and not that article 1 reads this output. Treating
        // every reference as a claim turned the check into noise.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: begrippen
  - number: "2.2"
    text: normpremie
    machine_readable:
      execution:
        output:
          - name: normpremie
            description: De normpremie, bedoeld in artikel 1, onderdeel h.
"#,
        )
        .expect("yaml");
        assert!(output_promises(&doc).is_empty());
    }

    #[test]
    fn a_kept_promise_is_silent() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: verzekerde
    machine_readable:
      execution:
        output:
          - name: is_verzekerde
            description: Artikel 2 verbindt de aanspraak aan dit begrip.
  - number: "2"
    text: aanspraak
    machine_readable:
      execution:
        input:
          - name: is_verzekerde
"#,
        )
        .expect("yaml");
        assert!(output_promises(&doc).is_empty());
    }

    #[test]
    fn a_promise_about_an_article_this_law_does_not_have_is_silent() {
        // "Artikel 24 van de Zorgverzekeringswet" points outside this file and
        // the work queue owns that.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: iets
    machine_readable:
      execution:
        output:
          - name: x
            description: Artikel 24 van de Zorgverzekeringswet gebruikt dit.
"#,
        )
        .expect("yaml");
        assert!(output_promises(&doc).is_empty());
    }

    #[test]
    fn the_tally_separates_restraint_from_emptiness() {
        // Round 3: the variant without a context brief laid nought cross-law
        // bindings and therefore drew nought binding findings, which made it
        // look better than the variant that tried. Only a tally tells those
        // two apart.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0018451
articles:
  - number: "1"
    text: "bedoeld in [ref]: https://wetten.overheid.nl/BWBR0018472#Artikel8"
    machine_readable:
      execution:
        output:
          - name: toeslag
        input:
          - name: inkomen
            source: {regulation: awir, output: toetsingsinkomen}
  - number: "2"
    text: iets
    machine_readable:
      markings:
        - about: kwantificeren over personen
          resolution: model
          target: [aantal_personen]
          legal_text_excerpt: iets
  - number: "3"
    text: iets zonder uitkomst
"#,
        )
        .expect("yaml");
        let t = tally(&doc);
        assert_eq!(t.articles, 3);
        assert_eq!(t.with_logic, 1);
        assert_eq!(t.marked_only, 1);
        assert_eq!(t.bare, 1);
        assert_eq!(t.cross_law_bindings, 1);
        assert_eq!(t.laws_read, 1);
        assert_eq!(t.laws_cited, 1);
        assert_eq!(t.markings, 1);
        assert_eq!(t.markings_model, 1);
        assert_eq!(t.markings_engine, 0);
        assert_eq!(t.markings_blocking, 1);
    }

    #[test]
    fn the_tally_separates_a_marking_that_blocks_from_one_that_only_explains() {
        // An empty `target` is the marking saying the article stays
        // executable. Counting both as one number would hide the difference
        // between a flag and a hole, which is the whole point of the field.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: iets
    machine_readable:
      execution:
        output:
          - name: x
      markings:
        - about: afronden op hele euro
          resolution: engine
          target: []
          legal_text_excerpt: iets
        - about: een regel over een verzameling
          resolution: model
          target: [x]
          legal_text_excerpt: iets
          accepted: true
"#,
        )
        .expect("yaml");
        let t = tally(&doc);
        assert_eq!(t.markings, 2);
        assert_eq!(t.markings_engine, 1);
        assert_eq!(t.markings_model, 1);
        assert_eq!(t.markings_blocking, 1);
        assert_eq!(t.markings_accepted, 1);
    }

    #[test]
    fn the_tally_says_whether_a_source_named_the_law_it_could_not_reach() {
        // Variant a of round 4 demoted an unreachable binding to a bare
        // input. Both draw the same finding now, so only these two numbers
        // say which of the two a run wrote.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: iets
    machine_readable:
      execution:
        input:
          - name: a
            source: {regulation: wet_inkomstenbelasting_2001, output: verzamelinkomen}
          - name: b
            source:
              description: komt van buiten
"#,
        )
        .expect("yaml");
        let t = tally(&doc);
        assert_eq!(t.cross_law_bindings, 1);
        assert_eq!(t.unnamed_sources, 1);
    }

    #[test]
    fn the_tally_separates_a_delegated_open_term_from_a_free_one() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: iets
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: amount
          delegated_to: Onze Minister
          delegation_type: MINISTERIELE_REGELING
        - id: redelijkerwijs
          type: boolean
"#,
        )
        .expect("yaml");
        let t = tally(&doc);
        assert_eq!(t.open_terms, 2);
        assert_eq!(t.open_terms_delegated, 1);
    }

    #[test]
    fn an_output_only_its_own_article_mentions_is_not_consumed() {
        // Counting a declaration as its own reader put the figure at 60 of 61
        // and said nothing. The question is whether another provision reads
        // it, which is what makes a dangling restriction visible.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: iets
    machine_readable:
      execution:
        output:
          - name: heeft_vermogen_boven_grens
  - number: "2"
    text: iets
    machine_readable:
      execution:
        output:
          - name: toeslag
        input:
          - name: heeft_vermogen_boven_grens
"#,
        )
        .expect("yaml");
        let t = tally(&doc);
        assert_eq!(t.outputs, 2);
        // `heeft_vermogen_boven_grens` is read by article 2; `toeslag` by
        // nobody.
        assert_eq!(t.outputs_consumed, 1);
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

    /// One article citing the Awir through a markdown link, with `{marking}`
    /// spliced into its model.
    fn citing_article(marking: &str) -> Value {
        let yaml = format!(
            r#"
bwb_id: BWBR0018451
articles:
  - number: "5.2"
    text: |-
      het toetsingsinkomen, bedoeld in [artikel 8 van de Algemene wet
      inkomensafhankelijke regelingen][ref1], wordt afgerond.

      [ref1]: https://wetten.overheid.nl/BWBR0018472#Artikel8
    references:
      - id: ref1
        bwb_id: BWBR0018472
    machine_readable:
{marking}
"#
        );
        serde_yaml_ng::from_str(&yaml).expect("yaml")
    }

    #[test]
    fn a_marking_excuses_only_the_reference_whose_words_it_quotes() {
        // A marking about rounding says nothing about whether this article
        // reads the law it cites, and quoting the rounding sentence must not
        // buy off the citation sitting elsewhere in the same article.
        let dir = corpus_with_awir();
        let unrelated = citing_article(
            "      markings:\n        - about: afronden\n          resolution: engine\n          \
             target: []\n          legal_text_excerpt: wordt afgerond",
        );
        assert_eq!(cross_law_references(&unrelated, Some(dir.path())).len(), 1);

        let quoting = citing_article(
            "      markings:\n        - about: het toetsingsinkomen\n          resolution: model\n          \
             target: [toetsingsinkomen]\n          legal_text_excerpt: >-\n            het toetsingsinkomen, \
             bedoeld in artikel 8 van de Algemene wet inkomensafhankelijke regelingen",
        );
        assert!(
            cross_law_references(&quoting, Some(dir.path())).is_empty(),
            "quoting the words that carry the citation answers it"
        );
    }

    #[test]
    fn naming_the_bwb_number_in_a_marking_no_longer_buys_off_a_reference() {
        // The measured failure of round 4: 43 of 101 norm gaps were
        // cross-references to laws that exist, written up as gaps, because
        // writing the number into a gap was the short way past this gate.
        let dir = corpus_with_awir();
        let doc = citing_article(
            "      markings:\n        - about: toetsingsinkomen komt uit BWBR0018472 (awir)\n          \
             resolution: model\n          target: [toetsingsinkomen]\n          \
             legal_text_excerpt: wordt afgerond",
        );
        let findings = cross_law_references(&doc, Some(dir.path()));
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].check, "reference");
    }

    #[test]
    fn a_binding_still_answers_a_reference() {
        let dir = corpus_with_awir();
        let doc = citing_article(
            "      execution:\n        input:\n          - name: toetsingsinkomen\n            \
             source: {regulation: awir, output: toetsingsinkomen}",
        );
        assert!(cross_law_references(&doc, Some(dir.path())).is_empty());
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
            "machine_readable:\n      markings:\n        - about: foreach\n          resolution: model\n          target: [x]\n          legal_text_excerpt: iets",
            "machine_readable:\n      open_terms:\n        - id: standaardpremie\n          type: amount",
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
        // Writing `markings: []` is the cheapest way to silence a check and
        // says nothing at all.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: "1"
    text: iets
    machine_readable:
      markings: []
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
    fn a_voiding_override_must_quote_this_articles_own_words() {
        // The ground for "the entitlement does not arise" is in the article
        // itself: "bestaat geen aanspraak op een zorgtoeslag". Copying is
        // checkable; inventing a category is not.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: wzt
articles:
  - number: "2"
    text: Aanspraak ter grootte van dat verschil.
    machine_readable:
      execution:
        output:
          - name: aanspraak
  - number: "3"
    text: In afwijking daarvan bestaat geen aanspraak op een zorgtoeslag indien het vermogen te hoog is.
    machine_readable:
      overrides:
        - article: "2"
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak op een zorgtoeslag
"#,
        )
        .expect("yaml");
        assert!(override_targets(&doc, None).is_empty());
    }

    #[test]
    fn a_voiding_override_without_a_quotation_is_flagged() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: wzt
articles:
  - number: "2"
    text: Aanspraak.
    machine_readable:
      execution:
        output:
          - name: aanspraak
  - number: "3"
    text: In afwijking daarvan bestaat geen aanspraak.
    machine_readable:
      overrides:
        - article: "2"
          output: aanspraak
          voids: true
"#,
        )
        .expect("yaml");
        let f = override_targets(&doc, None);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].detail.contains("without quoting"));
    }

    #[test]
    fn a_quotation_that_is_not_in_the_article_is_flagged() {
        // The failure this guards against: a ground that reads plausibly and
        // comes from nowhere.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: wzt
articles:
  - number: "2"
    text: Aanspraak.
    machine_readable:
      execution:
        output:
          - name: aanspraak
  - number: "3"
    text: In afwijking daarvan bestaat geen aanspraak.
    machine_readable:
      overrides:
        - article: "2"
          output: aanspraak
          voids: true
          legal_text_excerpt: het recht vervalt van rechtswege
"#,
        )
        .expect("yaml");
        let f = override_targets(&doc, None);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(f[0].detail.contains("does not appear"));
    }

    #[test]
    fn a_quotation_survives_the_harvesters_line_wrapping_and_link_syntax() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
$id: wzt
articles:
  - number: "2"
    text: Aanspraak.
    machine_readable:
      execution:
        output:
          - name: aanspraak
  - number: "3"
    text: >-
      In afwijking van [artikel 7, derde lid][ref1], bestaat geen
      aanspraak op een zorgtoeslag indien de rendementsgrondslag te hoog is.
    machine_readable:
      overrides:
        - article: "2"
          output: aanspraak
          voids: true
          legal_text_excerpt: bestaat geen aanspraak op een zorgtoeslag
"#,
        )
        .expect("yaml");
        assert!(override_targets(&doc, None).is_empty());
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
    fn a_marking_that_asks_for_an_operation_the_engine_has_is_flagged() {
        let yaml = r#"
articles:
  - number: '1'
    text: De uitkomst wordt afgerond op hele euro's.
    machine_readable:
      markings:
        - about: afronden op hele euro's
          resolution: engine
          target: [uitkomst]
          legal_text_excerpt: De uitkomst wordt afgerond op hele euro's.
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
    fn a_marking_that_quotes_words_this_article_does_not_have_is_flagged() {
        // The excerpt is what ties the marking to this provision. Without
        // that tie there is no way to tell whether the construct it names is
        // even in front of it.
        let yaml = r#"
articles:
  - number: '1'
    text: Verzekerde is degene die verzekerd is ingevolge de zorgverzekering.
    machine_readable:
      markings:
        - about: kwantificeren over personen
          resolution: model
          target: [aantal]
          legal_text_excerpt: voor elk van de tot het huishouden behorende personen
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = marking_discipline(&doc, "Verzekerde is degene die verzekerd is.");
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].detail.contains("does not appear"));
    }

    #[test]
    fn a_marking_that_quotes_the_article_verbatim_is_left_alone() {
        // The harvest wraps lines and adds markdown link syntax, so an honest
        // quotation rarely matches byte for byte and must still pass.
        let yaml = r#"
articles:
  - number: '1'
    text: |-
      De aanspraak wordt vastgesteld voor [elk van de tot het huishouden
      behorende personen][ref1].

      [ref1]: https://wetten.overheid.nl/BWBR0018472#Artikel8
    machine_readable:
      markings:
        - about: kwantificeren over personen
          resolution: model
          target: [aanspraak]
          legal_text_excerpt: elk van de tot het huishouden behorende personen
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(marking_discipline(&doc, "").is_empty());
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
      markings:
        - about: de eerste dag van de kalendermaand volgend op de achttiende verjaardag
          resolution: model
          target: [verzekerd_vanaf]
          legal_text_excerpt: de eerste dag van de kalendermaand volgend op zijn achttiende verjaardag
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        assert!(marking_discipline(&doc, "kalendermaand achttiende").is_empty());
    }

    #[test]
    fn a_norm_another_regulation_fills_does_not_belong_in_a_marking() {
        let yaml = r#"
articles:
  - number: '4'
    text: Bij ministeriële regeling wordt de standaardpremie vastgesteld.
    machine_readable:
      markings:
        - about: de standaardpremie
          resolution: model
          target: [standaardpremie]
          resolved_by: Wordt bij ministeriële regeling vastgesteld.
          legal_text_excerpt: Bij ministeriële regeling wordt de standaardpremie vastgesteld.
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = marking_discipline(
            &doc,
            "Bij ministeriële regeling wordt de standaardpremie vastgesteld.",
        );
        assert!(
            findings.iter().any(|f| f.detail.contains("open_term")),
            "a norm filled elsewhere belongs in open_terms: {findings:?}"
        );
    }

    #[test]
    fn a_citation_the_agent_cannot_have_read_is_flagged() {
        let yaml = r#"
articles:
  - number: '2'
    text: De normpremie bedraagt een percentage.
    machine_readable:
      markings:
        - about: de percentages
          resolution: model
          target: [normpremie]
          resolved_by: 'Zie Kamerstukken II 2004/05, 29 762, nr. 3 voor de bedoeling.'
          legal_text_excerpt: De normpremie bedraagt een percentage.
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
      markings:
        - about: iets
          resolution: model
          target: [iets]
          resolved_by: 'Verwijst naar Staatsblad 2005, 358, zoals het artikel zelf doet.'
          legal_text_excerpt: 'Zie Staatsblad 2005, 358.'
"#;
        let doc: Value = serde_yaml_ng::from_str(yaml).unwrap();
        let findings = marking_discipline(&doc, "Zie Staatsblad 2005, 358.");
        assert!(
            findings.iter().all(|f| f.check != "citation"),
            "{findings:?}"
        );
    }

    #[test]
    fn a_blocked_value_the_same_article_computes_is_a_contradiction() {
        // Round 4: of the 72 outputs recorded as blocked, all 72 were
        // computed by an action in the same article.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '2.1'
    text: iets
    machine_readable:
      markings:
        - about: kwantificeren over personen
          resolution: model
          target: [hoogte_zorgtoeslag]
          legal_text_excerpt: iets
      execution:
        actions:
          - output: hoogte_zorgtoeslag
            operation: ADD
            values: [$a, $b]
"#,
        )
        .expect("yaml");
        let findings = blocked_values_are_absent(&doc);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].check, "contradiction");
        assert!(findings[0].detail.contains("computes it"));
    }

    #[test]
    fn a_blocked_value_an_action_calculates_with_is_a_contradiction() {
        // Blocking an input and then adding it up is the same contradiction
        // read from the other side: whatever comes out rests on a value the
        // marking says is not there.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '2.1'
    text: iets
    machine_readable:
      markings:
        - about: de standaardpremie
          resolution: model
          target: [standaardpremie]
          legal_text_excerpt: iets
      execution:
        actions:
          - output: hoogte
            operation: SUBTRACT
            values: [$standaardpremie, $normpremie]
"#,
        )
        .expect("yaml");
        let findings = blocked_values_are_absent(&doc);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert!(findings[0].detail.contains("calculates with it"));
    }

    #[test]
    fn a_blocked_value_that_is_genuinely_left_out_passes() {
        // The honest shape: the article computes what it can and the blocked
        // value appears nowhere in its actions.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '2.1'
    text: iets
    machine_readable:
      markings:
        - about: de standaardpremie
          resolution: model
          target: [standaardpremie]
          legal_text_excerpt: iets
      execution:
        actions:
          - output: normpremie
            operation: MULTIPLY
            values: [$inkomen, $percentage]
"#,
        )
        .expect("yaml");
        assert!(blocked_values_are_absent(&doc).is_empty());
    }

    #[test]
    fn an_article_whose_whole_model_is_a_marking_is_reported() {
        // Round 4: the chapeau of article 1 of the zorgtoeslag got one
        // marking and nothing else, while the definitions under it were
        // perfectly translatable.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '1'
    text: In deze wet wordt verstaan onder verzekerde…
    machine_readable:
      markings:
        - about: de begripsbepalingen
          resolution: engine
          target: []
          legal_text_excerpt: In deze wet wordt verstaan onder
"#,
        )
        .expect("yaml");
        let findings = markings_leave_something_standing(&doc);
        assert_eq!(findings.len(), 1, "{findings:?}");
        assert_eq!(findings[0].check, "marking-only");
    }

    #[test]
    fn a_model_marking_that_names_what_it_blocks_may_stand_alone() {
        // The one defensible bare article: the format has no shape for this
        // provision at all, and the marking says which values fall away with
        // it. Everything written beside that would be padding.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '3'
    text: Voor elk van de tot het huishouden behorende personen wordt…
    machine_readable:
      markings:
        - about: kwantificeren over personen
          resolution: model
          target: [aanspraak_per_persoon]
          legal_text_excerpt: Voor elk van de tot het huishouden behorende personen
"#,
        )
        .expect("yaml");
        assert!(markings_leave_something_standing(&doc).is_empty());
    }

    #[test]
    fn an_empty_target_cannot_excuse_an_article_that_produces_nothing() {
        // An empty `target` says the article stays executable. Saying that
        // and producing nothing is a contradiction in one breath.
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '3'
    text: iets
    machine_readable:
      markings:
        - about: een regel over een verzameling
          resolution: model
          target: []
          legal_text_excerpt: iets
"#,
        )
        .expect("yaml");
        assert_eq!(markings_leave_something_standing(&doc).len(), 1);
    }

    #[test]
    fn a_marking_beside_a_worked_out_model_is_left_alone() {
        let doc: Value = serde_yaml_ng::from_str(
            r#"
articles:
  - number: '3'
    text: iets
    machine_readable:
      markings:
        - about: afronden
          resolution: engine
          target: []
          legal_text_excerpt: iets
      execution:
        actions:
          - output: x
            operation: ADD
            values: [$a, $b]
"#,
        )
        .expect("yaml");
        assert!(markings_leave_something_standing(&doc).is_empty());
    }

    #[test]
    fn schema_errors_rejects_a_file_without_a_schema_declaration() {
        let errors = schema_errors("articles: []\n");
        assert!(!errors.is_empty());
        assert!(errors[0].contains("$schema"));
    }
}
