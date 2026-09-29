//! The lexogram: the regulations from `REGULATION_PATH`, loaded into the engine.
//!
//! Why a dedicated loader and not the engine's
//! `RuleResolver::load_from_directory`: that one is lenient (a regulation that
//! does not load becomes a warning and is skipped), whereas the runtime must
//! then refuse to start, since a decision would otherwise rest on an
//! incomplete corpus. The runtime also keeps the text of each regulation for
//! the hash in the receipt (RFC-013), and it skips YAML files that are not
//! regulations (scenarios, notes) instead of reporting them as failed
//! regulations. Loading a single regulation does use the engine's
//! (`load_law`).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use regelrecht_engine::{Article, LawExecutionService};
use regelrecht_law_model::{ParameterType, TypeSpec};
use serde::Serialize;
use sha2::{Digest, Sha256};
use walkdir::WalkDir;

use crate::gram::LoadedRegulation;

/// The corpus as the runtime loaded it: the engine with all regulations, and
/// their inventory for the receipt of a decision (RFC-013).
pub struct Corpus {
    pub service: LawExecutionService,
    pub regulations: Vec<LoadedRegulation>,
}

/// Load every regulation (a YAML file with `$id` and `articles`) under a
/// directory. Other YAML files (scenarios, notes) are skipped; a regulation
/// the engine does not load is an error.
pub fn load(map: &Path) -> Result<Corpus, Vec<String>> {
    if !map.is_dir() {
        return Err(vec![format!("{}: not a directory", map.display())]);
    }
    let mut service = LawExecutionService::new();
    let mut loaded: Vec<LoadedRegulation> = Vec::new();
    let mut errors = Vec::new();
    let mut files = Vec::new();
    for item in WalkDir::new(map)
        .into_iter()
        .filter_entry(|e| !e.file_name().to_string_lossy().starts_with('.'))
    {
        match item {
            Ok(e) if e.file_type().is_file() => files.push(e.into_path()),
            Ok(_) => {}
            // A directory or file that cannot be read: a regulation may be
            // missing, so that is an error.
            Err(e) => errors.push(format!("{}: {e}", map.display())),
        }
    }
    files.retain(|p| p.extension().is_some_and(|x| x == "yaml" || x == "yml"));
    files.sort();
    for path in files {
        let text = match std::fs::read_to_string(&path) {
            Ok(t) => t,
            Err(e) => {
                errors.push(format!("{}: {e}", path.display()));
                continue;
            }
        };
        let doc = match serde_yaml_ng::from_str::<serde_yaml_ng::Value>(&text) {
            Ok(d) => d,
            Err(e) => {
                // Not YAML, so not a regulation; still worth reporting.
                tracing::warn!(path = %path.display(), "not valid YAML, skipped: {e}");
                continue;
            }
        };
        if doc.get("$id").is_none() || doc.get("articles").is_none() {
            continue;
        }
        match service.load_law(&text) {
            Ok(id) => {
                // The engine also loads a regulation with an invalid `origin`
                // (it does not read it); the runtime then does not start, with
                // file, article and parameter in the message (RFC-043).
                if let Some(law) = service.resolver().get_law(&id) {
                    errors.extend(
                        crate::origin::validate(law)
                            .into_iter()
                            .map(|f| format!("{}: {f}", path.display())),
                    );
                }
                loaded.push(inventory(&path, &text, &doc, id));
            }
            Err(e) => errors.push(format!("{}: {e}", path.display())),
        }
    }
    loaded.sort();
    if errors.is_empty() {
        Ok(Corpus {
            service,
            regulations: loaded,
        })
    } else {
        Err(errors)
    }
}

/// A loaded regulation for the receipt: its `$id`, from when that version
/// applies and the hash of the file as read. In the corpus the file name is
/// the effective date; if it is not there, `valid_from` or
/// `publication_date` from the file itself counts.
fn inventory(path: &Path, text: &str, doc: &serde_yaml_ng::Value, id: String) -> LoadedRegulation {
    let read = |key: &str| {
        doc.get(key)
            .and_then(serde_yaml_ng::Value::as_str)
            .map(str::to_string)
    };
    let stem = path
        .file_stem()
        .map(|s| s.to_string_lossy().to_string())
        .filter(|s| s.len() == 10 && s.split('-').count() == 3);
    let valid_from = stem
        .or_else(|| read("valid_from"))
        .or_else(|| read("publication_date"));
    if valid_from.is_none() {
        tracing::warn!(regulation = %id, file = %path.display(), "regulation without a date in its file name, valid_from or publication_date: the receipt names no version");
    }
    LoadedRegulation {
        id,
        valid_from: valid_from.unwrap_or_default(),
        sha256: hex::encode(Sha256::digest(text.as_bytes())),
    }
}

/// A legal basis, parsed: `<regulation>#<article>` or
/// `<regulation>#<article> lid <n>`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct LegalBasis<'g> {
    pub regulation: &'g str,
    pub article: &'g str,
    pub paragraph: Option<&'g str>,
}

/// Whether a text is a paragraph number: digits, optionally with a letter.
fn is_paragraph_number(text: &str) -> bool {
    let digits = text.trim_end_matches(|c: char| c.is_ascii_lowercase());
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && text.len() - digits.len() <= 1
}

/// Parse a legal basis. An article number may contain a space
/// (`kieswet#G 1`), so the paragraph follows the last ` lid `, and only if a
/// paragraph number stands there.
pub fn parse(legal_basis: &str) -> Result<LegalBasis<'_>, String> {
    let (regulation, rest) = legal_basis.split_once('#').ok_or_else(|| {
        format!("legal basis '{legal_basis}' does not have the form <regulation>#<article>")
    })?;
    let (article, paragraph) = match rest.rsplit_once(" lid ") {
        Some((article, paragraph)) if is_paragraph_number(paragraph) => (article, Some(paragraph)),
        _ => (rest, None),
    };
    Ok(LegalBasis {
        regulation,
        article,
        paragraph,
    })
}

/// Whether an article text has a paragraph: a line that starts with `<n>.`
/// or `<n> `.
pub fn has_paragraph(article: &Article, paragraph: &str) -> bool {
    article.text.lines().any(|row| {
        row.trim_start()
            .strip_prefix(paragraph)
            .is_some_and(|rest| rest.starts_with('.') || rest.starts_with(' '))
    })
}

/// The article behind a legal basis `<regulation>#<article>`, also when the
/// legal basis names a paragraph: a paragraph has no parameters of its own.
/// Whether the paragraph exists is checked by [`has_paragraph`].
pub fn article<'s>(
    service: &'s LawExecutionService,
    legal_basis: &str,
) -> Result<&'s Article, String> {
    let LegalBasis {
        regulation,
        article: number,
        ..
    } = parse(legal_basis)?;
    let law = service.resolver().get_law(regulation).ok_or_else(|| {
        format!("legal basis '{legal_basis}': regulation '{regulation}' is not loaded")
    })?;
    law.find_article_by_number(number).ok_or_else(|| {
        format!("legal basis '{legal_basis}': regulation '{regulation}' has no article {number}")
    })
}

/// The article behind a legal basis, and the article text must have the
/// paragraph it names (see [`has_paragraph`]). Shared by the checks on the
/// legal basis of an event, of a derivation and of a form field.
pub fn valid<'s>(
    service: &'s LawExecutionService,
    legal_basis: &str,
) -> Result<&'s Article, String> {
    let a = article(service, legal_basis)?;
    if let Some(paragraph) = parse(legal_basis)?
        .paragraph
        .filter(|l| !has_paragraph(a, l))
    {
        return Err(format!(
            "legal basis '{legal_basis}': article {} has no paragraph {paragraph} (no line starting with '{paragraph}.' or '{paragraph} ')",
            a.number
        ));
    }
    Ok(a)
}

/// The parameters of an article and of every article it calls through an
/// input, transitively: an input with `source.output` points to the article
/// with that output, in `source.regulation` or in the same regulation. A
/// parameter declared only by a called article counts: the engine passes it
/// on when the input does not pass its own `parameters`.
pub fn transitive_parameters(
    service: &LawExecutionService,
    regulation: &str,
    article: &Article,
) -> BTreeSet<String> {
    let mut parameters = BTreeSet::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut to_do: Vec<(String, &Article)> = vec![(regulation.to_string(), article)];
    while let Some((law, a)) = to_do.pop() {
        if !seen.insert((law.clone(), a.number.clone())) {
            continue;
        }
        parameters.extend(a.get_parameters().iter().map(|p| p.name.clone()));
        for input in a.get_inputs() {
            let Some(source) = &input.source else {
                continue;
            };
            let Some(output) = &source.output else {
                continue;
            };
            let target = source.regulation.clone().unwrap_or_else(|| law.clone());
            if let Some(next) = service
                .resolver()
                .get_article_by_output(&target, output, None)
            {
                to_do.push((target, next));
            }
        }
    }
    parameters
}

/// The type of a value according to the regulation: `type` and the unit
/// (`type_spec.unit`, such as `eurocent`). A frontend displays and asks for
/// a value with it, not by its name.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ValueType {
    #[serde(rename = "type")]
    pub kind: ParameterType,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
}

impl ValueType {
    pub fn new(kind: ParameterType, type_spec: Option<&TypeSpec>) -> Self {
        Self {
            kind,
            unit: type_spec.and_then(|t| t.unit.clone()),
        }
    }
}

/// The types of the outputs of an article, by name.
pub fn output_types(service: &LawExecutionService, article: &str) -> BTreeMap<String, ValueType> {
    self::article(service, article)
        .ok()
        .and_then(|a| a.get_execution_spec())
        .and_then(|e| e.output.as_ref())
        .map(|o| {
            o.iter()
                .map(|o| {
                    (
                        o.name.clone(),
                        ValueType::new(o.output_type, o.type_spec.as_ref()),
                    )
                })
                .collect()
        })
        .unwrap_or_default()
}

/// A parameter the caller of an article must supply, with the article that
/// declares it.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Required {
    pub name: String,
    /// `<regulation>#<article>`.
    pub article: String,
    #[serde(flatten)]
    pub typing: ValueType,
    pub nullable: bool,
    /// The description from the regulation, with the provenance according to the model.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// The parameters the caller of an article must supply: those of the article
/// itself, and those of every article in the same regulation that it calls
/// through an input without its own `parameters`, transitively; such a call
/// shares the parameters. An input with `parameters` binds the parameters of
/// the called article itself, and a call to another regulation receives only
/// what `parameters` passes; the caller is not asked for those. Per name, the
/// first article that declares it.
pub fn required_parameters(
    service: &LawExecutionService,
    regulation: &str,
    article: &Article,
) -> BTreeMap<String, Required> {
    let mut out: BTreeMap<String, Required> = BTreeMap::new();
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut to_do: Vec<(String, &Article)> = vec![(regulation.to_string(), article)];
    while let Some((law, a)) = to_do.pop() {
        if !seen.insert((law.clone(), a.number.clone())) {
            continue;
        }
        for p in a.get_parameters() {
            out.entry(p.name.clone()).or_insert_with(|| Required {
                name: p.name.clone(),
                article: format!("{law}#{}", a.number),
                typing: ValueType::new(p.param_type, p.type_spec.as_ref()),
                nullable: p.is_nullable(),
                description: p.description.clone(),
            });
        }
        for input in a.get_inputs() {
            let Some(source) = &input.source else {
                continue;
            };
            let Some(output) = &source.output else {
                continue;
            };
            let target = source.regulation.clone().unwrap_or_else(|| law.clone());
            if target != law || source.parameters.as_ref().is_some_and(|p| !p.is_empty()) {
                continue;
            }
            if let Some(next) = service
                .resolver()
                .get_article_by_output(&target, output, None)
            {
                to_do.push((target, next));
            }
        }
    }
    out
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn fixtures() -> std::path::PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/regulation")
    }

    #[test]
    fn loads_the_test_regulations() {
        let c = load(&fixtures()).unwrap();
        assert!(c.service.has_law("testregeling_aanvraag"));
        assert!(c.service.has_law("testregeling_awb"));
        // The inventory for the receipt: every regulation with its hash.
        assert!(c.regulations.iter().any(|r| r.id == "testregeling_aanvraag"
            && r.valid_from == "2025-01-01"
            && r.sha256.len() == 64));
    }

    #[test]
    fn legal_basis_to_article() {
        let s = load(&fixtures()).unwrap().service;
        let a = article(&s, "testregeling_aanvraag#1").unwrap();
        assert!(a.get_parameters().iter().any(|p| p.name == "bevat_naam"));
        assert!(article(&s, "testregeling_aanvraag#9")
            .unwrap_err()
            .contains("no article 9"));
        assert!(article(&s, "onbekend#1")
            .unwrap_err()
            .contains("not loaded"));
        assert!(article(&s, "zonder_hekje").is_err());
    }

    #[test]
    fn legal_basis_with_a_paragraph() {
        assert_eq!(
            parse("een_wet#102 lid 1").unwrap(),
            LegalBasis {
                regulation: "een_wet",
                article: "102",
                paragraph: Some("1")
            }
        );
        assert_eq!(parse("een_wet#4:2 lid 2a").unwrap().paragraph, Some("2a"));
        // An article number with a space, with and without a paragraph.
        assert_eq!(
            parse("kieswet#G 1 lid 3").unwrap(),
            LegalBasis {
                regulation: "kieswet",
                article: "G 1",
                paragraph: Some("3")
            }
        );
        assert_eq!(parse("kieswet#G 1").unwrap().article, "G 1");
        // No paragraph number: then it belongs to the article number.
        assert_eq!(parse("een_wet#A lid B").unwrap().article, "A lid B");
        assert_eq!(parse("een_wet#1 lid 12ab").unwrap().paragraph, None);

        let s = load(&fixtures()).unwrap().service;
        let a = article(&s, "testregeling_aanvraag#1 lid 1").unwrap();
        assert_eq!(a.number, "1");
        assert!(has_paragraph(a, "1"), "{}", a.text);
        assert!(!has_paragraph(a, "9"));
    }

    #[test]
    fn transitive_parameters_follow_the_input() {
        let s = load(&fixtures()).unwrap().service;
        let a = article(&s, "testregeling_afnemer#1").unwrap();
        let p = transitive_parameters(&s, "testregeling_afnemer", a);
        // Own parameters, those of article 2 (same regulation, no binding)
        // and those of the test regulation register (another regulation).
        for name in [
            "bevat_aanduiding",
            "zetels_op_lijst",
            "is_ingeschreven_in_register",
        ] {
            assert!(p.contains(name), "{name} is missing from {p:?}");
        }
        assert!(!p.contains("datum_vaststelling"));
    }

    #[test]
    fn required_parameters_without_what_an_input_binds() {
        let s = load(&fixtures()).unwrap().service;
        let a = article(&s, "testregeling_afnemer#1").unwrap();
        let p = required_parameters(&s, "testregeling_afnemer", a);
        // Article 2 is called without parameters: its parameter counts.
        assert_eq!(p["zetels_op_lijst"].article, "testregeling_afnemer#2");
        assert_eq!(p["datum_mededeling"].typing.kind, ParameterType::Date);
        // The test regulation register gets its parameters from article 1.
        assert!(!p.contains_key("is_ingeschreven_in_register"));
    }
}
