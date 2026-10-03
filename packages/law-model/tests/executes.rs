//! `executes` on a policy article (RFC-047): which article of law it
//! executes, and optionally as which subject of a beleidsregel in Awb 1:3
//! lid 4. Only the law's three subjects are a kind; an entry with another
//! value still loads, as an invalid entry.

#![allow(clippy::expect_used, clippy::unwrap_used)]

use regelrecht_law_model::{ArticleBasedLaw, ExecutesKind};

const BELEID: &str = r#"
$id: een_beleid
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het beleid.
    machine_readable:
      executes:
        - {article: 'een_wet#1', as: fact_finding}
        - {article: 'een_wet#2'}
        - {article: 'een_wet#3', as: procedure}
"#;

#[test]
fn as_is_optional_and_only_the_awb_kinds_are_valid() {
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(BELEID).unwrap();
    let a = &law.articles[0];
    let valid: Vec<_> = a
        .get_executes()
        .map(|e| (e.article.as_str(), e.kind))
        .collect();
    assert_eq!(
        valid,
        [
            ("een_wet#1", Some(ExecutesKind::FactFinding)),
            ("een_wet#2", None)
        ]
    );
    assert_eq!(a.get_invalid_executes().len(), 1, "procedure is not a kind");
}

#[test]
fn every_kind_reads_as_its_awb_term() {
    assert_eq!(
        ExecutesKind::note(Some(ExecutesKind::FactFinding)),
        " (vaststelling van feiten, Awb 1:3 lid 4)"
    );
    assert_eq!(
        ExecutesKind::note(Some(ExecutesKind::Interpretation)),
        " (uitleg van wettelijke voorschriften, Awb 1:3 lid 4)"
    );
    assert_eq!(
        ExecutesKind::note(Some(ExecutesKind::Weighing)),
        " (afweging van belangen, Awb 1:3 lid 4)"
    );
    assert_eq!(ExecutesKind::note(None), "");
}

#[test]
fn an_entry_without_as_round_trips_without_it() {
    let law: ArticleBasedLaw = serde_yaml_ng::from_str(BELEID).unwrap();
    let out = serde_yaml_ng::to_string(&law.articles[0].get_declared_executes()[1]).unwrap();
    assert_eq!(out.trim(), "article: een_wet#2");
}
