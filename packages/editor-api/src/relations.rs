//! Helpers for building and querying the corpus relation graph.
//!
//! The actual extraction logic lives in `regelrecht_engine::relations`.
//! This module wraps it for editor-api's needs: parsing all currently
//! loaded laws from the [`SourceMap`] into [`ArticleBasedLaw`] structs and
//! handing back a [`RelationIndex`].
//!
//! A YAML that fails to parse (e.g. a half-edited file in a local source)
//! is logged and skipped — we never panic during startup or reload. A
//! corpus with a single broken law still produces a valid (smaller) index
//! for all other laws, so the relations endpoint keeps working for the
//! rest of the corpus.

use regelrecht_corpus::SourceMap;
use regelrecht_engine::{ArticleBasedLaw, RelationIndex};

/// Parse every law in the [`SourceMap`] and build a [`RelationIndex`].
///
/// Returns an empty index when the source map is empty — callers can
/// always swap an `Arc<RelationIndex>` without special-casing startup
/// or reload paths.
pub fn build_index_from_source_map(source_map: &SourceMap) -> RelationIndex {
    let mut parsed: Vec<ArticleBasedLaw> = Vec::new();
    for law in source_map.laws() {
        match ArticleBasedLaw::from_yaml_str(&law.yaml_content) {
            Ok(parsed_law) => parsed.push(parsed_law),
            Err(e) => {
                // Skip-and-log: a broken YAML must not take down the
                // whole relations subsystem.
                tracing::warn!(
                    law_id = %law.law_id,
                    source_id = %law.source_id,
                    error = %e,
                    "skipping law during relation index build (parse failed)"
                );
            }
        }
    }
    let count = parsed.len();
    let index = RelationIndex::build(&parsed);
    tracing::info!(
        laws = count,
        relations = index.len(),
        "built relation index"
    );
    index
}

#[cfg(test)]
mod tests {
    //! Integration-flavoured tests over the real corpus YAML files. These
    //! parse two co-operating laws (Wet op de zorgtoeslag + Regeling
    //! standaardpremie) and assert that the cross-law relations show up
    //! exactly as we expect. If a future schema change breaks the YAML
    //! shape these tests will surface that immediately.

    use super::*;
    use regelrecht_engine::{Direction, RelationType};
    use std::path::PathBuf;

    fn corpus_path() -> PathBuf {
        // CARGO_MANIFEST_DIR points at packages/editor-api/ — climb out to
        // the repo root, then into corpus/regulation.
        PathBuf::from(env!("CARGO_MANIFEST_DIR"))
            .join("..")
            .join("..")
            .join("corpus")
            .join("regulation")
    }

    fn load(rel: &str) -> ArticleBasedLaw {
        let path = corpus_path().join(rel);
        let yaml = std::fs::read_to_string(&path)
            .unwrap_or_else(|e| panic!("failed to read {}: {e}", path.display()));
        ArticleBasedLaw::from_yaml_str(&yaml)
            .unwrap_or_else(|e| panic!("failed to parse {}: {e}", path.display()))
    }

    #[test]
    fn zorgtoeslag_art4_is_implemented_by_regeling_standaardpremie() {
        let laws = vec![
            load("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml"),
            load("nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml"),
        ];
        let index = RelationIndex::build(&laws);

        // Incoming on zorgtoeslagwet art 4 must contain an Implementation
        // edge from regeling_standaardpremie art 1.
        let incoming = index.for_article("zorgtoeslagwet", "4", Direction::Incoming);
        let impls: Vec<_> = incoming
            .iter()
            .filter(|r| r.relation_type == RelationType::Implementation)
            .collect();
        assert!(
            !impls.is_empty(),
            "expected an Implementation edge into zorgtoeslagwet art 4"
        );
        assert!(
            impls
                .iter()
                .any(|r| r.from.law_id == "regeling_standaardpremie"
                    && r.from.article.as_deref() == Some("1")
                    && r.metadata.get("open_term").map(String::as_str) == Some("standaardpremie")),
            "missing expected implementation edge: {:?}",
            impls
        );
    }

    #[test]
    fn regeling_standaardpremie_has_three_legal_basis_edges() {
        let laws = vec![load(
            "nl/ministeriele_regeling/regeling_standaardpremie/2025-01-01.yaml",
        )];
        let index = RelationIndex::build(&laws);
        let outgoing = index.for_law("regeling_standaardpremie", Direction::Outgoing);
        let basis: Vec<_> = outgoing
            .iter()
            .filter(|r| r.relation_type == RelationType::LegalBasis)
            .collect();
        // YAML declares three legal_basis entries (zorgtoeslag art 4,
        // zorgverzekeringswet art 18d and 18e).
        assert_eq!(
            basis.len(),
            3,
            "expected 3 legal_basis edges, got {basis:#?}"
        );
    }

    #[test]
    fn zorgtoeslag_has_cross_law_dataflow_to_awir() {
        // wet_op_de_zorgtoeslag art 2 pulls multiple inputs from
        // algemene_wet_inkomensafhankelijke_regelingen. Loading both laws
        // lets the extractor resolve the target article number too.
        let laws = vec![
            load("nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml"),
            load("nl/wet/algemene_wet_inkomensafhankelijke_regelingen/2025-01-01.yaml"),
        ];
        let index = RelationIndex::build(&laws);
        let outgoing = index.for_law("zorgtoeslagwet", Direction::Outgoing);
        let cross_law: Vec<_> = outgoing
            .iter()
            .filter(|r| {
                r.relation_type == RelationType::CrossLawDataflow
                    && r.to.law_id == "algemene_wet_inkomensafhankelijke_regelingen"
            })
            .collect();
        assert!(
            !cross_law.is_empty(),
            "expected at least one CrossLawDataflow edge to AWIR"
        );
    }
}
