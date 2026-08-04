//! Corpus metrics — the numbers behind the Corpusstand dashboard.
//!
//! # Why this lives in the engine
//!
//! Every figure here could be produced by walking the YAML again in the frontend
//! or in a script. That is exactly what must not happen: a second reader
//! eventually disagrees with the engine, and a metric that disagrees is worse
//! than no metric — it grants confidence that something is covered while the
//! engine loads something else. The clearest case is a `source:` block placed
//! under `parameters:` instead of `input:`. [`Parameter`] has no `source` field,
//! so serde drops it at parse time and the value silently becomes a plain
//! direct parameter; a naive counter reports a working cross-law binding that
//! never fires.
//!
//! So the metrics read the *already parsed* structures the engine resolves
//! against, and nothing else.
//!
//! # Selections, not scalars
//!
//! The report is a set of **indexes** — one row per regulation, per article, per
//! binding — and every headline number is the length of a filter over them. A
//! count can therefore never disagree with the list behind it, because it *is*
//! that list, and a dashboard tile can drill down without a second query.
//!
//! # Provenance
//!
//! Everything in [`CorpusMetrics`] is engine-derived: it comes from laws loaded
//! into the resolver. Scenario coverage is deliberately absent — scenarios are
//! files living beside a law, not part of the model, so they belong to a
//! corpus-derived report that a caller merges in. Mixing the two here would
//! hide which figures the engine actually vouches for.
//!
//! # Determinism
//!
//! Equal input yields equal output, byte for byte: every collection is sorted
//! before it is returned, and no map iteration order reaches the result.

use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

// Re-exported by the engine so the historical `crate::article::*` paths keep
// working; the document model itself lives in `regelrecht_law_model`.
use crate::article::ArticleBasedLaw;
use crate::resolver::RuleResolver;
use crate::types::RegulatoryLayer;

/// Description markers that say "this ought to be a cross-law binding but is not".
///
/// Deliberately narrow. Generic phrasings like "forward naar" legitimately
/// describe leaf parameters that *feed* a binding's parameter mapping, so
/// matching on those would flood the report with false positives.
const PLAIN_PARAM_MARKERS: [&str; 2] = ["conceptueel", "tijdelijk als directe parameter"];

/// How a binding fares when the engine tries to resolve it.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingIntegrity {
    /// Resolvable: the target exists and produces what is asked of it.
    Clean,
    /// The target law does not produce that output; execution fails at resolution.
    Dangling,
    /// An `implements` pointing at a law/article that does not declare the term.
    ImplDangling,
}

/// What kind of relation a binding expresses.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BindingKind {
    /// `input.source` — the engine fetches a value.
    Source,
    /// `implements` — a lower regulation fills an open term (RFC-003).
    Implements,
}

/// A modelling defect. Every variant is a modelling error, never an engine
/// limitation: the YAML says something the engine cannot honour.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum FindingClass {
    /// `source:` under `parameters:` — dropped at parse time, cross-law never fires.
    ///
    /// **This module can never produce this variant, and that is the point.**
    /// [`crate::article::Execution`]'s `Parameter` has no `source` field, so
    /// serde discards the block before any engine code sees it. A defect that
    /// consists of the engine being blind cannot be found by asking the engine.
    /// Detecting it requires reading the raw document, which is what
    /// `script/cross-law-integriteit.py` does — that script therefore is not a
    /// redundant second implementation but the only place this check can live.
    /// The variant exists so a caller can merge those document-level findings
    /// into one report.
    Misplaced,
    /// A `source` whose target does not produce that output.
    Dangling,
    /// A parameter whose description names another regulation but carries no `source`.
    PlainParam,
    /// An `implements` pointing at an undeclared open term.
    ImplDangling,
    /// A regulation carrying `implements` but no `valid_from`. RFC-003's temporal
    /// filter then matches it for every calculation date, silently overriding the
    /// correct version.
    ImplNoDate,
    /// An open term that no loaded regulation implements: delegation left unfinished.
    ///
    /// Unlike the five above this has no counterpart in
    /// `script/cross-law-integriteit.py`, which only checks that an `implements`
    /// points somewhere real — not that every open term is reached. Kept separate
    /// so the differential test against that script stays a like-for-like
    /// comparison.
    OpenTermUnfilled,
}

/// One defect, anchored where it can be repaired.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Finding {
    pub class: FindingClass,
    pub law_id: String,
    /// Absent for law-level findings (`ImplNoDate`).
    pub article: Option<String>,
    /// Where the binding was meant to point, when known. Without it a
    /// `Misplaced` finding says something is broken but not what it should reach.
    pub target: Option<String>,
    pub detail: String,
}

/// One edge in the dependency graph.
///
/// A `Misplaced` binding is deliberately **not** a row here. The engine discards
/// it, so drawing it would show a connection that does not exist at execution
/// time. It surfaces as a [`Finding`] on the article instead.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BindingRow {
    pub from_law: String,
    pub from_article: String,
    pub to_law: String,
    /// Output name for a source binding, open-term id for an implements binding.
    pub label: String,
    pub kind: BindingKind,
    pub integrity: BindingIntegrity,
}

/// An untranslatable construct, with the flag saying whether a human signed off.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UntranslatableRow {
    pub construct: String,
    pub reason: String,
    pub accepted: bool,
}

/// One article. The unit every drill-down eventually lands on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleRow {
    pub law_id: String,
    pub valid_from: Option<String>,
    pub number: String,
    /// Whether this article carries an executable model at all. The share of
    /// articles for which this holds is the dashboard's headline figure.
    pub has_logic: bool,
    pub outputs: Vec<String>,
    pub parameter_count: usize,
    pub input_count: usize,
    /// Inputs carrying a `source`. The gap with `input_count` is how much is fed
    /// from outside rather than derived.
    pub bound_input_count: usize,
    pub open_terms: Vec<String>,
    pub implements_count: usize,
    pub untranslatables: Vec<UntranslatableRow>,
    pub findings: Vec<Finding>,
}

/// One loaded regulation version. A law with three versions yields three rows —
/// "regulations" and "versions" are different counts and the dashboard shows both.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct RegulationRow {
    pub law_id: String,
    pub valid_from: Option<String>,
    pub valid_to: Option<String>,
    pub layer: RegulatoryLayer,
    /// Schema version parsed out of the `$schema` URL (e.g. `v0.5.6`). Reveals a
    /// corpus drifting behind the engine, and whether it drifts unevenly.
    pub schema_version: Option<String>,
    pub content_hash: Option<String>,
    pub article_count: usize,
    pub articles_with_logic: usize,
    pub output_count: usize,
    pub parameter_count: usize,
    pub input_count: usize,
    pub bound_input_count: usize,
    pub open_term_count: usize,
    pub untranslatable_count: usize,
    pub untranslatables_accepted: usize,
    pub incoming_bindings: usize,
    pub outgoing_bindings: usize,
    /// False when this regulation is only referenced as a binding target and is
    /// not itself loaded. A binding into the void must stay visible, otherwise
    /// the graph looks complete while a regulation is missing.
    pub loaded: bool,
}

/// Headline numbers. Derived from the indexes in one place so a tile can never
/// contradict the list it opens.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Totals {
    pub regulations: usize,
    pub versions: usize,
    pub articles: usize,
    pub articles_with_logic: usize,
    pub outputs: usize,
    pub parameters: usize,
    pub inputs: usize,
    pub bound_inputs: usize,
    pub open_terms: usize,
    pub untranslatables: usize,
    pub untranslatables_accepted: usize,
    pub bindings: usize,
    pub bindings_clean: usize,
    /// Loaded regulations nothing points at: modelled but outside every
    /// calculation path. Not a defect, but a signal.
    pub uncalled_regulations: usize,
    pub findings_by_class: BTreeMap<FindingClass, usize>,
}

/// The whole engine-derived report.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct CorpusMetrics {
    /// The date the caller asked about. Carried through untouched: without it
    /// the report cannot be compared with an earlier one.
    pub as_of: Option<String>,
    pub totals: Totals,
    pub regulations: Vec<RegulationRow>,
    pub articles: Vec<ArticleRow>,
    pub bindings: Vec<BindingRow>,
    pub findings: Vec<Finding>,
}

/// Pull `v1.2.3` out of a `$schema` URL, ignoring the rest of the path.
fn schema_version(url: &str) -> Option<String> {
    url.split(['/', '-'])
        .find(|seg| {
            let Some(rest) = seg.strip_prefix('v') else {
                return false;
            };
            let parts: Vec<&str> = rest.split('.').collect();
            parts.len() == 3
                && parts
                    .iter()
                    .all(|p| !p.is_empty() && p.bytes().all(|b| b.is_ascii_digit()))
        })
        .map(str::to_string)
}

/// Every output name a law produces, from `actions[].output` and `output[].name`.
fn law_outputs(law: &ArticleBasedLaw) -> BTreeSet<String> {
    let mut out = BTreeSet::new();
    for article in &law.articles {
        let Some(exec) = article
            .machine_readable
            .as_ref()
            .and_then(|mr| mr.execution.as_ref())
        else {
            continue;
        };
        for action in exec.actions.iter().flatten() {
            if let Some(name) = &action.output {
                out.insert(name.clone());
            }
        }
        for output in exec.output.iter().flatten() {
            out.insert(output.name.clone());
        }
    }
    out
}

/// Open-term ids declared per article number.
fn law_open_terms(law: &ArticleBasedLaw) -> BTreeMap<String, BTreeSet<String>> {
    let mut idx: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for article in &law.articles {
        let Some(mr) = article.machine_readable.as_ref() else {
            continue;
        };
        for term in mr.open_terms.iter().flatten() {
            idx.entry(article.number.clone())
                .or_default()
                .insert(term.id.clone());
        }
    }
    idx
}

/// Build the report over every law version the resolver holds.
///
/// Note on versions: a law loaded in several versions is walked once per
/// version, because a binding can be sound in one version and dangling in the
/// next. That differs from `script/cross-law-integriteit.py`, which keeps one
/// document per `$id`; the differential test between them therefore runs on a
/// corpus with a single version per law.
pub fn corpus_metrics(resolver: &RuleResolver, as_of: Option<&str>) -> CorpusMetrics {
    let laws: Vec<&ArticleBasedLaw> = resolver.all_law_versions().collect();

    let outputs_by_law: BTreeMap<&str, BTreeSet<String>> = laws
        .iter()
        .map(|law| (law.id.as_str(), law_outputs(law)))
        .collect();
    let open_terms_by_law: BTreeMap<&str, BTreeMap<String, BTreeSet<String>>> = laws
        .iter()
        .map(|law| (law.id.as_str(), law_open_terms(law)))
        .collect();

    let mut articles: Vec<ArticleRow> = Vec::new();
    let mut bindings: Vec<BindingRow> = Vec::new();
    let mut findings: Vec<Finding> = Vec::new();
    let mut regulations: Vec<RegulationRow> = Vec::new();
    // Open terms reached by some `implements`, so the unfilled ones can be
    // spotted after the walk. Keyed (law, article, term).
    let mut filled_terms: BTreeSet<(String, String, String)> = BTreeSet::new();

    for law in &laws {
        let mut reg = RegulationRow {
            law_id: law.id.clone(),
            valid_from: law.valid_from.clone(),
            valid_to: law.valid_to.clone(),
            layer: law.regulatory_layer,
            schema_version: law.schema.as_deref().and_then(schema_version),
            content_hash: law.content_hash.clone(),
            article_count: law.articles.len(),
            articles_with_logic: 0,
            output_count: 0,
            parameter_count: 0,
            input_count: 0,
            bound_input_count: 0,
            open_term_count: 0,
            untranslatable_count: 0,
            untranslatables_accepted: 0,
            incoming_bindings: 0,
            outgoing_bindings: 0,
            loaded: true,
        };

        let has_implements = law.articles.iter().any(|a| {
            a.machine_readable
                .as_ref()
                .is_some_and(|mr| mr.implements.is_some())
        });
        if has_implements && law.valid_from.is_none() {
            findings.push(Finding {
                class: FindingClass::ImplNoDate,
                law_id: law.id.clone(),
                article: None,
                target: None,
                detail: "implements without valid_from (matches every calculation date)".into(),
            });
        }

        for article in &law.articles {
            let mut row = ArticleRow {
                law_id: law.id.clone(),
                valid_from: law.valid_from.clone(),
                number: article.number.clone(),
                has_logic: article.machine_readable.is_some(),
                outputs: Vec::new(),
                parameter_count: 0,
                input_count: 0,
                bound_input_count: 0,
                open_terms: Vec::new(),
                implements_count: 0,
                untranslatables: Vec::new(),
                findings: Vec::new(),
            };

            // No early `continue` past this point: the tail of this loop body
            // is what moves an article's findings into the corpus-wide list and
            // folds its counts into the regulation row. Skipping it once left
            // `totals` disagreeing with the per-article rows — exactly the
            // count-versus-list divergence this module exists to prevent.
            if let Some(mr) = article.machine_readable.as_ref() {
                reg.articles_with_logic += 1;

                for term in mr.open_terms.iter().flatten() {
                    row.open_terms.push(term.id.clone());
                }
                for u in mr.untranslatables.iter().flatten() {
                    row.untranslatables.push(UntranslatableRow {
                        construct: u.construct.clone(),
                        reason: u.reason.clone(),
                        accepted: u.accepted,
                    });
                }

                // implements — the IoC binding must land on a declared open term.
                for decl in mr.implements.iter().flatten() {
                    row.implements_count += 1;
                    let declared = open_terms_by_law
                        .get(decl.law.as_str())
                        .and_then(|per_article| per_article.get(&decl.article));
                    let integrity = if !open_terms_by_law.contains_key(decl.law.as_str()) {
                        row.findings.push(Finding {
                            class: FindingClass::ImplDangling,
                            law_id: law.id.clone(),
                            article: Some(article.number.clone()),
                            target: Some(decl.law.clone()),
                            detail: format!("implements unknown law {}", decl.law),
                        });
                        BindingIntegrity::ImplDangling
                    } else if !declared.is_some_and(|terms| terms.contains(&decl.open_term)) {
                        row.findings.push(Finding {
                            class: FindingClass::ImplDangling,
                            law_id: law.id.clone(),
                            article: Some(article.number.clone()),
                            target: Some(decl.law.clone()),
                            detail: format!(
                                "{} art {} does not declare open_term \"{}\"",
                                decl.law, decl.article, decl.open_term
                            ),
                        });
                        BindingIntegrity::ImplDangling
                    } else {
                        filled_terms.insert((
                            decl.law.clone(),
                            decl.article.clone(),
                            decl.open_term.clone(),
                        ));
                        BindingIntegrity::Clean
                    };
                    bindings.push(BindingRow {
                        from_law: law.id.clone(),
                        from_article: article.number.clone(),
                        to_law: decl.law.clone(),
                        label: decl.open_term.clone(),
                        kind: BindingKind::Implements,
                        integrity,
                    });
                }

                if let Some(exec) = mr.execution.as_ref() {
                    for output in exec.output.iter().flatten() {
                        row.outputs.push(output.name.clone());
                    }

                    // parameters — neither branch yields a binding: the engine sees nothing here.
                    for param in exec.parameters.iter().flatten() {
                        row.parameter_count += 1;
                        let description = param.description.as_deref().unwrap_or("").to_lowercase();
                        if PLAIN_PARAM_MARKERS.iter().any(|m| description.contains(m)) {
                            row.findings.push(Finding {
                                class: FindingClass::PlainParam,
                                law_id: law.id.clone(),
                                article: Some(article.number.clone()),
                                target: None,
                                detail: param.name.clone(),
                            });
                        }
                    }

                    for input in exec.input.iter().flatten() {
                        row.input_count += 1;
                        let Some(source) = input.source.as_ref() else {
                            continue;
                        };
                        // `source: {}` is a data-registry binding, not a cross-law reference.
                        if source.regulation.is_none() && source.output.is_none() {
                            continue;
                        }
                        row.bound_input_count += 1;

                        let (target_law, resolvable) = match source.regulation.as_deref() {
                            // Intra-law: points at an output of this same law.
                            None => (
                                law.id.as_str(),
                                source
                                    .output
                                    .as_ref()
                                    .is_some_and(|o| outputs_by_law[law.id.as_str()].contains(o)),
                            ),
                            Some(target) => {
                                let ok = outputs_by_law.get(target).is_some_and(|outs| {
                                    source.output.as_ref().is_none_or(|o| outs.contains(o))
                                });
                                (target, ok)
                            }
                        };
                        let label = source.output.clone().unwrap_or_default();

                        if !resolvable {
                            row.findings.push(Finding {
                                class: FindingClass::Dangling,
                                law_id: law.id.clone(),
                                article: Some(article.number.clone()),
                                target: Some(target_law.to_string()),
                                detail: if source.regulation.is_none() {
                                    format!("intra-law {label} does not exist")
                                } else {
                                    format!("{target_law}.{label} does not exist in the target law")
                                },
                            });
                        }
                        bindings.push(BindingRow {
                            from_law: law.id.clone(),
                            from_article: article.number.clone(),
                            to_law: target_law.to_string(),
                            label,
                            kind: BindingKind::Source,
                            integrity: if resolvable {
                                BindingIntegrity::Clean
                            } else {
                                BindingIntegrity::Dangling
                            },
                        });
                    }
                } // execution
            } // machine_readable

            reg.output_count += row.outputs.len();
            reg.parameter_count += row.parameter_count;
            reg.input_count += row.input_count;
            reg.bound_input_count += row.bound_input_count;
            reg.open_term_count += row.open_terms.len();
            reg.untranslatable_count += row.untranslatables.len();
            reg.untranslatables_accepted +=
                row.untranslatables.iter().filter(|u| u.accepted).count();
            findings.extend(row.findings.iter().cloned());
            articles.push(row);
        }

        regulations.push(reg);
    }

    // A `source` under `parameters:` is only reachable through the raw document,
    // because serde has already dropped it. The engine cannot see it, so neither
    // can we — the check lives in `script/cross-law-integriteit.py`, which reads
    // the YAML directly. Recording the gap here rather than pretending coverage.

    // Open terms nobody implements: delegation declared but never completed.
    for law in &laws {
        for (article_number, terms) in &open_terms_by_law[law.id.as_str()] {
            for term in terms {
                let key = (law.id.clone(), article_number.clone(), term.clone());
                if !filled_terms.contains(&key) {
                    findings.push(Finding {
                        class: FindingClass::OpenTermUnfilled,
                        law_id: law.id.clone(),
                        article: Some(article_number.clone()),
                        target: None,
                        detail: format!(
                            "open_term \"{term}\" is not implemented by any loaded regulation"
                        ),
                    });
                }
            }
        }
    }

    // Regulations referenced as a target but never loaded get a row too, so a
    // binding into the void stays countable instead of vanishing.
    let loaded: BTreeSet<&str> = laws.iter().map(|l| l.id.as_str()).collect();
    let mut phantom: BTreeSet<&str> = BTreeSet::new();
    for binding in &bindings {
        if !loaded.contains(binding.to_law.as_str()) {
            phantom.insert(binding.to_law.as_str());
        }
    }
    for law_id in phantom {
        regulations.push(RegulationRow {
            law_id: law_id.to_string(),
            valid_from: None,
            valid_to: None,
            layer: RegulatoryLayer::default(),
            schema_version: None,
            content_hash: None,
            article_count: 0,
            articles_with_logic: 0,
            output_count: 0,
            parameter_count: 0,
            input_count: 0,
            bound_input_count: 0,
            open_term_count: 0,
            untranslatable_count: 0,
            untranslatables_accepted: 0,
            incoming_bindings: 0,
            outgoing_bindings: 0,
            loaded: false,
        });
    }

    // Degree counts. A self-reference is a fact about one node, not a connection
    // between two, so it does not make a regulation "called".
    for binding in &bindings {
        if binding.from_law == binding.to_law {
            continue;
        }
        for reg in regulations.iter_mut() {
            if reg.law_id == binding.to_law {
                reg.incoming_bindings += 1;
            }
            if reg.law_id == binding.from_law {
                reg.outgoing_bindings += 1;
            }
        }
    }

    regulations.sort_by(|a, b| {
        a.law_id
            .cmp(&b.law_id)
            .then_with(|| a.valid_from.cmp(&b.valid_from))
    });
    articles.sort_by(|a, b| {
        a.law_id
            .cmp(&b.law_id)
            .then_with(|| a.valid_from.cmp(&b.valid_from))
            .then_with(|| compare_article_numbers(&a.number, &b.number))
    });
    bindings.sort_by(|a, b| {
        a.from_law
            .cmp(&b.from_law)
            .then_with(|| compare_article_numbers(&a.from_article, &b.from_article))
            .then_with(|| a.to_law.cmp(&b.to_law))
            .then_with(|| a.label.cmp(&b.label))
    });
    findings.sort_by(|a, b| {
        a.class
            .cmp(&b.class)
            .then_with(|| a.law_id.cmp(&b.law_id))
            .then_with(|| match (&a.article, &b.article) {
                (Some(x), Some(y)) => compare_article_numbers(x, y),
                (l, r) => l.cmp(r),
            })
            .then_with(|| a.detail.cmp(&b.detail))
    });

    let totals = totals(&regulations, &articles, &bindings, &findings);
    CorpusMetrics {
        as_of: as_of.map(str::to_string),
        totals,
        regulations,
        articles,
        bindings,
        findings,
    }
}

/// Article numbers are strings that read as numbers ("2", "10", "2a"). Compare
/// the leading digits numerically so article 10 sorts after article 2.
fn compare_article_numbers(a: &str, b: &str) -> std::cmp::Ordering {
    let head = |s: &str| -> (u64, String) {
        let digits: String = s.chars().take_while(char::is_ascii_digit).collect();
        (digits.parse().unwrap_or(u64::MAX), s.to_string())
    };
    let (na, sa) = head(a);
    let (nb, sb) = head(b);
    na.cmp(&nb).then_with(|| sa.cmp(&sb))
}

fn totals(
    regulations: &[RegulationRow],
    articles: &[ArticleRow],
    bindings: &[BindingRow],
    findings: &[Finding],
) -> Totals {
    let loaded: Vec<&RegulationRow> = regulations.iter().filter(|r| r.loaded).collect();
    let distinct: BTreeSet<&str> = loaded.iter().map(|r| r.law_id.as_str()).collect();
    // Uncalled is a property of a regulation, not of one of its versions: a law
    // whose 2024 version nobody points at but whose 2025 version is called is
    // not sitting outside the calculation path. Counting rows would inflate the
    // figure by exactly the number of superseded versions.
    let called: BTreeSet<&str> = loaded
        .iter()
        .filter(|r| r.incoming_bindings > 0)
        .map(|r| r.law_id.as_str())
        .collect();
    let mut findings_by_class: BTreeMap<FindingClass, usize> = BTreeMap::new();
    for finding in findings {
        *findings_by_class.entry(finding.class).or_insert(0) += 1;
    }
    Totals {
        regulations: distinct.len(),
        versions: loaded.len(),
        articles: articles.len(),
        articles_with_logic: articles.iter().filter(|a| a.has_logic).count(),
        outputs: articles.iter().map(|a| a.outputs.len()).sum(),
        parameters: articles.iter().map(|a| a.parameter_count).sum(),
        inputs: articles.iter().map(|a| a.input_count).sum(),
        bound_inputs: articles.iter().map(|a| a.bound_input_count).sum(),
        open_terms: articles.iter().map(|a| a.open_terms.len()).sum(),
        untranslatables: articles.iter().map(|a| a.untranslatables.len()).sum(),
        untranslatables_accepted: articles
            .iter()
            .flat_map(|a| a.untranslatables.iter())
            .filter(|u| u.accepted)
            .count(),
        bindings: bindings.len(),
        bindings_clean: bindings
            .iter()
            .filter(|b| b.integrity == BindingIntegrity::Clean)
            .count(),
        uncalled_regulations: distinct.difference(&called).count(),
        findings_by_class,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::resolver::RuleResolver;

    const SCHEMA: &str =
        "https://raw.githubusercontent.com/MinBZK/regelrecht/refs/tags/schema-v0.5.6/schema/v0.5.6/schema.json";

    /// Minimal law document. Names are invented: the fixtures carry no
    /// organisation, statute, amount, person or sector from any real dossier.
    fn law(id: &str, valid_from: Option<&str>, articles: &str) -> String {
        let dated = match valid_from {
            Some(d) => format!("valid_from: '{d}'\n"),
            None => String::new(),
        };
        format!(
            "$schema: {SCHEMA}\n\
             $id: {id}\n\
             regulatory_layer: WET\n\
             publication_date: '2024-01-01'\n\
             {dated}\
             name: Example regulation\n\
             articles:\n{articles}"
        )
    }

    fn resolver_with(laws: &[String]) -> RuleResolver {
        let mut resolver = RuleResolver::new();
        for yaml in laws {
            resolver
                .load_from_yaml(yaml)
                .unwrap_or_else(|e| panic!("fixture failed to load: {e}\n{yaml}"));
        }
        resolver
    }

    fn produces(number: &str, output: &str) -> String {
        format!(
            "  - number: '{number}'\n    text: text\n    machine_readable:\n      execution:\n        output:\n          - name: {output}\n            type: number\n"
        )
    }

    fn binds(number: &str, regulation: Option<&str>, output: &str) -> String {
        // `regulation`/`output` sit two levels deeper than `source:`. Getting
        // this wrong makes them siblings, serde drops the source entirely, and
        // the fixture silently proves nothing.
        let reg = match regulation {
            Some(r) => format!("              regulation: {r}\n"),
            None => String::new(),
        };
        format!(
            "  - number: '{number}'\n    text: text\n    machine_readable:\n      execution:\n        input:\n          - name: value\n            type: number\n            source:\n{reg}              output: {output}\n"
        )
    }

    fn count(metrics: &CorpusMetrics, class: FindingClass) -> usize {
        metrics.findings_by_class_or_zero(class)
    }

    impl CorpusMetrics {
        fn findings_by_class_or_zero(&self, class: FindingClass) -> usize {
            self.totals
                .findings_by_class
                .get(&class)
                .copied()
                .unwrap_or(0)
        }
    }

    #[test]
    fn resolvable_cross_law_binding_is_clean_and_becomes_an_edge() {
        let m = corpus_metrics(
            &resolver_with(&[
                law(
                    "law_a",
                    Some("2025-01-01"),
                    &binds("1", Some("law_b"), "amount"),
                ),
                law("law_b", Some("2025-01-01"), &produces("1", "amount")),
            ]),
            None,
        );
        assert_eq!(m.totals.bindings_clean, 1);
        assert!(
            m.findings.is_empty(),
            "unexpected findings: {:?}",
            m.findings
        );
        assert_eq!(m.bindings[0].to_law, "law_b");
        assert_eq!(m.bindings[0].integrity, BindingIntegrity::Clean);
        assert_eq!(m.bindings[0].kind, BindingKind::Source);
    }

    #[test]
    fn binding_to_a_missing_output_is_dangling() {
        let m = corpus_metrics(
            &resolver_with(&[
                law(
                    "law_a",
                    Some("2025-01-01"),
                    &binds("2", Some("law_b"), "nowhere"),
                ),
                law(
                    "law_b",
                    Some("2025-01-01"),
                    &produces("1", "something_else"),
                ),
            ]),
            None,
        );
        assert_eq!(count(&m, FindingClass::Dangling), 1);
        assert_eq!(m.findings[0].article.as_deref(), Some("2"));
        assert_eq!(m.bindings[0].integrity, BindingIntegrity::Dangling);
    }

    #[test]
    fn intra_law_reference_to_a_missing_own_output_is_dangling() {
        let m = corpus_metrics(
            &resolver_with(&[law(
                "law_a",
                Some("2025-01-01"),
                &binds("1", None, "nowhere"),
            )]),
            None,
        );
        assert_eq!(count(&m, FindingClass::Dangling), 1);
        assert!(m.findings[0].detail.contains("intra-law"));
    }

    /// `source: {}` resolves through the data-source registry, so it is neither
    /// a cross-law edge nor a defect.
    #[test]
    fn empty_source_is_a_data_registry_binding_not_an_edge() {
        let articles = "  - number: '1'\n    text: text\n    machine_readable:\n      execution:\n        input:\n          - name: value\n            type: number\n            source: {}\n";
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), articles)]),
            None,
        );
        assert!(m.bindings.is_empty());
        assert!(m.findings.is_empty());
        assert_eq!(m.totals.inputs, 1);
        assert_eq!(m.totals.bound_inputs, 0);
    }

    #[test]
    fn implements_pointing_at_an_unknown_law_is_impl_dangling() {
        let articles = "  - number: '1'\n    text: text\n    machine_readable:\n      implements:\n        - law: law_missing\n          article: '2'\n          open_term: rate\n";
        let m = corpus_metrics(
            &resolver_with(&[law("regulation_a", Some("2025-01-01"), articles)]),
            None,
        );
        assert_eq!(count(&m, FindingClass::ImplDangling), 1);
        assert!(m.findings[0].detail.contains("unknown law"));
    }

    #[test]
    fn implements_pointing_at_an_undeclared_term_is_impl_dangling() {
        let impl_article = "  - number: '1'\n    text: text\n    machine_readable:\n      implements:\n        - law: law_b\n          article: '2'\n          open_term: other\n";
        let term_article = "  - number: '2'\n    text: text\n    machine_readable:\n      open_terms:\n        - id: rate\n          type: number\n";
        let m = corpus_metrics(
            &resolver_with(&[
                law("regulation_a", Some("2025-01-01"), impl_article),
                law("law_b", Some("2025-01-01"), term_article),
            ]),
            None,
        );
        assert_eq!(count(&m, FindingClass::ImplDangling), 1);
        assert!(m.findings[0].detail.contains("does not declare open_term"));
    }

    #[test]
    fn a_matching_implements_is_clean_and_fills_the_term() {
        let impl_article = "  - number: '1'\n    text: text\n    machine_readable:\n      implements:\n        - law: law_b\n          article: '2'\n          open_term: rate\n";
        let term_article = "  - number: '2'\n    text: text\n    machine_readable:\n      open_terms:\n        - id: rate\n          type: number\n";
        let m = corpus_metrics(
            &resolver_with(&[
                law("regulation_a", Some("2025-01-01"), impl_article),
                law("law_b", Some("2025-01-01"), term_article),
            ]),
            None,
        );
        assert_eq!(count(&m, FindingClass::ImplDangling), 0);
        // Filled, so it must not also be reported as unfilled.
        assert_eq!(count(&m, FindingClass::OpenTermUnfilled), 0);
        assert_eq!(m.bindings[0].kind, BindingKind::Implements);
        assert_eq!(m.bindings[0].integrity, BindingIntegrity::Clean);
    }

    /// RFC-003's temporal filter matches an undated implementing regulation for
    /// every calculation date, silently overriding the correct version.
    #[test]
    fn implements_without_valid_from_is_flagged() {
        let impl_article = "  - number: '1'\n    text: text\n    machine_readable:\n      implements:\n        - law: law_b\n          article: '2'\n          open_term: rate\n";
        let term_article = "  - number: '2'\n    text: text\n    machine_readable:\n      open_terms:\n        - id: rate\n          type: number\n";
        let m = corpus_metrics(
            &resolver_with(&[
                law("regulation_a", None, impl_article),
                law("law_b", Some("2025-01-01"), term_article),
            ]),
            None,
        );
        assert_eq!(count(&m, FindingClass::ImplNoDate), 1);
    }

    #[test]
    fn an_open_term_nobody_implements_is_reported() {
        let term_article = "  - number: '2'\n    text: text\n    machine_readable:\n      open_terms:\n        - id: rate\n          type: number\n";
        let m = corpus_metrics(
            &resolver_with(&[law("law_b", Some("2025-01-01"), term_article)]),
            None,
        );
        assert_eq!(count(&m, FindingClass::OpenTermUnfilled), 1);
        assert_eq!(m.totals.open_terms, 1);
    }

    #[test]
    fn a_parameter_naming_another_regulation_without_source_is_a_plain_param() {
        let articles = "  - number: '1'\n    text: text\n    machine_readable:\n      execution:\n        parameters:\n          - name: rate\n            type: number\n            description: Conceptueel, komt later uit een andere regeling\n";
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), articles)]),
            None,
        );
        assert_eq!(count(&m, FindingClass::PlainParam), 1);
    }

    #[test]
    fn an_ordinary_parameter_is_not_flagged() {
        let articles = "  - number: '1'\n    text: text\n    machine_readable:\n      execution:\n        parameters:\n          - name: rate\n            type: number\n            description: An ordinary input value\n";
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), articles)]),
            None,
        );
        assert_eq!(count(&m, FindingClass::PlainParam), 0);
        assert_eq!(m.totals.parameters, 1);
    }

    /// The dashboard's headline figure: an article without `machine_readable`
    /// still counts as an article, but not as a translated one.
    #[test]
    fn coverage_counts_articles_with_and_without_logic() {
        let articles = format!(
            "{}  - number: '2'\n    text: not yet modelled\n",
            produces("1", "amount")
        );
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), &articles)]),
            None,
        );
        assert_eq!(m.totals.articles, 2);
        assert_eq!(m.totals.articles_with_logic, 1);
        assert_eq!(m.articles[1].has_logic, false);
        assert_eq!(m.regulations[0].articles_with_logic, 1);
    }

    #[test]
    fn untranslatables_are_split_by_acceptance() {
        let articles = "  - number: '1'\n    text: text\n    machine_readable:\n      untranslatables:\n        - construct: open norm\n          reason: requires judgement\n          accepted: true\n        - construct: discretion\n          reason: no calculable path\n          accepted: false\n";
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), articles)]),
            None,
        );
        assert_eq!(m.totals.untranslatables, 2);
        assert_eq!(m.totals.untranslatables_accepted, 1);
    }

    #[test]
    fn schema_version_is_read_from_the_schema_url() {
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), &produces("1", "amount"))]),
            None,
        );
        assert_eq!(m.regulations[0].schema_version.as_deref(), Some("v0.5.6"));
    }

    /// §3.1: modelled but outside every calculation path. Not a defect, a signal.
    #[test]
    fn a_regulation_nothing_points_at_is_uncalled() {
        let m = corpus_metrics(
            &resolver_with(&[
                law(
                    "law_a",
                    Some("2025-01-01"),
                    &binds("1", Some("law_b"), "amount"),
                ),
                law("law_b", Some("2025-01-01"), &produces("1", "amount")),
            ]),
            None,
        );
        // law_b is called, law_a calls but is never called itself.
        assert_eq!(m.totals.uncalled_regulations, 1);
        let law_a = m.regulations.iter().find(|r| r.law_id == "law_a").unwrap();
        assert_eq!(law_a.incoming_bindings, 0);
        assert_eq!(law_a.outgoing_bindings, 1);
    }

    /// A binding into the void must stay visible, otherwise the graph reads as
    /// complete while a regulation is missing.
    #[test]
    fn a_target_that_is_not_loaded_still_gets_a_row() {
        let m = corpus_metrics(
            &resolver_with(&[law(
                "law_a",
                Some("2025-01-01"),
                &binds("1", Some("law_absent"), "x"),
            )]),
            None,
        );
        let absent = m
            .regulations
            .iter()
            .find(|r| r.law_id == "law_absent")
            .unwrap();
        assert!(!absent.loaded);
        assert_eq!(count(&m, FindingClass::Dangling), 1);
        // Only loaded regulations count as regulations.
        assert_eq!(m.totals.regulations, 1);
    }

    #[test]
    fn an_intra_law_edge_does_not_make_a_regulation_called() {
        let articles = format!("{}{}", produces("1", "own"), binds("2", None, "own"));
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), &articles)]),
            None,
        );
        assert_eq!(m.regulations[0].incoming_bindings, 0);
        assert_eq!(m.totals.uncalled_regulations, 1);
    }

    #[test]
    fn article_rows_sort_numerically_not_lexically() {
        let articles = format!("{}{}", produces("10", "a"), produces("2", "b"));
        let m = corpus_metrics(
            &resolver_with(&[law("law_a", Some("2025-01-01"), &articles)]),
            None,
        );
        let numbers: Vec<&str> = m.articles.iter().map(|a| a.number.as_str()).collect();
        assert_eq!(numbers, vec!["2", "10"]);
    }

    /// Equal input, equal output (§5). Load order must not reach the result.
    #[test]
    fn the_report_is_independent_of_load_order() {
        let a = law(
            "law_a",
            Some("2025-01-01"),
            &binds("1", Some("law_b"), "amount"),
        );
        let b = law("law_b", Some("2025-01-01"), &produces("1", "amount"));
        let forwards = corpus_metrics(&resolver_with(&[a.clone(), b.clone()]), Some("2026-01-01"));
        let backwards = corpus_metrics(&resolver_with(&[b, a]), Some("2026-01-01"));
        assert_eq!(forwards, backwards);
    }

    #[test]
    fn as_of_is_carried_through_untouched() {
        let m = corpus_metrics(&resolver_with(&[]), Some("2026-08-04"));
        assert_eq!(m.as_of.as_deref(), Some("2026-08-04"));
        assert_eq!(m.totals.regulations, 0);
    }

    #[test]
    fn schema_version_parsing() {
        assert_eq!(
            schema_version("…/schema-v0.5.6/schema/v0.5.6/schema.json").as_deref(),
            Some("v0.5.6")
        );
        assert_eq!(
            schema_version("…/schema/v1.10.2/schema.json").as_deref(),
            Some("v1.10.2")
        );
        assert_eq!(schema_version("…/schema/latest/schema.json"), None);
        assert_eq!(schema_version("…/vNOPE/schema.json"), None);
    }
}
