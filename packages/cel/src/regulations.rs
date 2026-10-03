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
use std::path::{Path, PathBuf};

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
    /// The file of every loaded regulation, per `(id, version)` with the
    /// version as the engine knows it ([`version_key`]); see [`file_of`].
    pub files: BTreeMap<(String, String), PathBuf>,
}

/// The version of a regulation as the engine selects it: `valid_from`,
/// otherwise `publication_date`. Not the file name, which the receipt
/// prefers (see `inventory`): in the corpus they differ.
pub fn version_key(valid_from: Option<&str>, publication_date: &str) -> String {
    valid_from.unwrap_or(publication_date).to_string()
}

/// The file of the version of `id` that the engine applies on `date` (the
/// newest without one).
pub fn file_of<'f>(
    files: &'f BTreeMap<(String, String), PathBuf>,
    service: &LawExecutionService,
    id: &str,
    date: Option<chrono::NaiveDate>,
) -> Option<&'f PathBuf> {
    let resolver = service.resolver();
    let law = resolver
        .get_law_for_date(id, date)
        .or_else(|| resolver.get_law(id))?;
    let version = version_key(law.valid_from.as_deref(), &law.publication_date);
    files.get(&(id.to_string(), version))
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
    let mut regulation_files = BTreeMap::new();
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
                // file, article and parameter in the message (RFC-048). The
                // version of this file, not the newest of the regulation.
                // A version the runtime cannot find back is not checked
                // silently: that is an error too.
                let read = |key: &str| doc.get(key).and_then(serde_yaml_ng::Value::as_str);
                match service.resolver().all_law_versions().find(|l| {
                    l.id == id
                        && l.valid_from.as_deref() == read("valid_from")
                        && l.publication_date == read("publication_date").unwrap_or_default()
                }) {
                    Some(law) => errors.extend(
                        crate::origin::validate(law)
                            .into_iter()
                            .map(|f| format!("{}: {f}", path.display())),
                    ),
                    None => errors.push(format!(
                        "{}: regulation '{id}' loaded, but no loaded version has valid_from {:?} and publication_date {:?} of this file: its origin cannot be checked",
                        path.display(),
                        read("valid_from"),
                        read("publication_date")
                    )),
                }
                let version = version_key(
                    read("valid_from"),
                    read("publication_date").unwrap_or_default(),
                );
                regulation_files.insert((id.clone(), version), path.clone());
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
            files: regulation_files,
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

impl LegalBasis<'_> {
    /// `<regulation>#<article>`: the article, without a paragraph.
    pub fn article_ref(&self) -> String {
        format!("{}#{}", self.regulation, self.article)
    }
}

/// Whether a text is a paragraph number: digits, optionally with a letter.
fn is_paragraph_number(text: &str) -> bool {
    let digits = text.trim_end_matches(|c: char| c.is_ascii_lowercase());
    !digits.is_empty()
        && digits.bytes().all(|b| b.is_ascii_digit())
        && text.len() - digits.len() <= 1
}

/// Whether a text is a regulation id: `[a-z0-9_]+`.
fn is_regulation_id(text: &str) -> bool {
    !text.is_empty()
        && text
            .bytes()
            .all(|b| b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_')
}

/// Whether a text is an article number: letters, digits, `:`, `.`, `_`,
/// `-` and spaces, starting and ending with a letter or digit (`4:13`,
/// `G 1`, `2.1a`). The schema's `provisionGround` says the same.
fn is_article_number(text: &str) -> bool {
    let edge = |c: Option<char>| c.is_some_and(|c| c.is_ascii_alphanumeric());
    edge(text.chars().next())
        && edge(text.chars().last())
        && text
            .chars()
            .all(|c| c.is_ascii_alphanumeric() || matches!(c, ':' | '.' | '_' | '-' | ' '))
}

/// Parse a legal basis. An article number may contain a space
/// (`kieswet#G 1`), so the paragraph follows the last ` lid `, and only if a
/// paragraph number stands there. The regulation is a regulation id and the
/// article an article number ([`is_article_number`]); anything else (a
/// trailing space, a second `#`, an empty article) is refused, not looked up.
pub fn parse(legal_basis: &str) -> Result<LegalBasis<'_>, String> {
    let (regulation, rest) = legal_basis.split_once('#').ok_or_else(|| {
        format!("legal basis '{legal_basis}' does not have the form <regulation>#<article>")
    })?;
    let (article, paragraph) = match rest.rsplit_once(" lid ") {
        Some((article, paragraph)) if is_paragraph_number(paragraph) => (article, Some(paragraph)),
        _ => (rest, None),
    };
    if !is_regulation_id(regulation) || !is_article_number(article) {
        return Err(format!(
            "legal basis '{legal_basis}' does not have the form <regulation>#<article>: the regulation is a-z, 0-9 and _, the article letters, digits, ':', '.', '_', '-' and spaces, starting and ending with a letter or digit, optionally followed by ' lid <n>'"
        ));
    }
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
    walk_calls(
        service,
        regulation,
        article,
        |_, _, _| true,
        |_, a| parameters.extend(a.get_parameters().iter().map(|p| p.name.clone())),
    );
    parameters
}

/// Visit an article and every article it calls through an input with
/// `source.output` (in `source.regulation` or in the same regulation),
/// transitively and each once, as `(regulation, article)`. `follow` says per
/// call `(calling regulation, called regulation, source)` whether to go on.
fn walk_calls<'s>(
    service: &'s LawExecutionService,
    regulation: &str,
    article: &'s Article,
    follow: impl Fn(&str, &str, &regelrecht_law_model::Source) -> bool,
    mut visit: impl FnMut(&str, &'s Article),
) {
    let mut seen: BTreeSet<(String, String)> = BTreeSet::new();
    let mut to_do: Vec<(String, &Article)> = vec![(regulation.to_string(), article)];
    while let Some((law, a)) = to_do.pop() {
        if !seen.insert((law.clone(), a.number.clone())) {
            continue;
        }
        visit(&law, a);
        for input in a.get_inputs() {
            let Some(source) = &input.source else {
                continue;
            };
            let Some(output) = &source.output else {
                continue;
            };
            let target = source.regulation.clone().unwrap_or_else(|| law.clone());
            if !follow(&law, &target, source) {
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
    walk_calls(
        service,
        regulation,
        article,
        |law, target, source| {
            target == law && source.parameters.as_ref().is_none_or(|p| p.is_empty())
        },
        |law, a| {
            for p in a.get_parameters() {
                out.entry(p.name.clone()).or_insert_with(|| Required {
                    name: p.name.clone(),
                    article: format!("{law}#{}", a.number),
                    typing: ValueType::new(p.param_type, p.type_spec.as_ref()),
                    nullable: p.is_nullable(),
                    description: p.description.clone(),
                });
            }
        },
    );
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

    /// Two versions of one regulation whose file names are not their
    /// `valid_from`, crosswise: the file follows the version the engine
    /// applies, not the file name.
    #[test]
    fn the_file_of_the_version_the_engine_applies() {
        // Under a plain directory: the loader skips hidden ones, like `.tmp…`.
        let tmp = tempfile::tempdir().unwrap();
        let dir = tmp.path().join("corpus");
        let version = |valid_from: &str| {
            format!("$id: twee_versies\nname: Twee versies\nregulatory_layer: WET\npublication_date: '2019-01-01'\nvalid_from: '{valid_from}'\nurl: https://example.com\narticles:\n  - number: '1'\n    text: versie {valid_from}\n")
        };
        std::fs::create_dir_all(dir.join("twee_versies")).unwrap();
        for (file, valid_from) in [
            ("2021-01-01.yaml", "2020-01-01"),
            ("2020-01-01.yaml", "2021-01-01"),
        ] {
            std::fs::write(dir.join("twee_versies").join(file), version(valid_from)).unwrap();
        }
        let c = load(&dir).unwrap();
        let on = |date: &str| {
            let date = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d").unwrap();
            let file = file_of(&c.files, &c.service, "twee_versies", Some(date)).unwrap();
            file.file_name().unwrap().to_string_lossy().to_string()
        };
        assert_eq!(on("2020-06-01"), "2021-01-01.yaml");
        assert_eq!(on("2021-06-01"), "2020-01-01.yaml");
        assert!(file_of(&c.files, &c.service, "bestaat_niet", None).is_none());
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
        assert_eq!(parse("awb#4:13").unwrap().article, "4:13");
        assert_eq!(parse("een_wet#2.1a-b lid 3").unwrap().article, "2.1a-b");
    }

    /// The grammar of a legal basis: a regulation id, an article number that
    /// starts and ends with a letter or digit; nothing else is looked up.
    #[test]
    fn a_legal_basis_outside_the_grammar_is_refused() {
        for wrong in [
            "zonder_hekje",
            "#1",
            "een_wet#",
            "Een_Wet#1",
            "een-wet#1",
            "een_wet#1 ",
            "een_wet# 1",
            "een_wet#1#2",
            "een_wet#1 lid 1 ",
            "een_wet#1/2",
            "een_wet#-1",
        ] {
            let f = parse(wrong).expect_err(wrong);
            assert!(f.contains("does not have the form"), "{wrong}: {f}");
        }

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
