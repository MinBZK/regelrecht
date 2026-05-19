//! Relation extraction over a corpus of [`ArticleBasedLaw`]s.
//!
//! Walks the parsed law structures and emits a [`Relation`] for every
//! explicit pointer between articles (cross-law dataflow on inputs,
//! `implements` ↔ `open_terms` IoC pairs, `legal_basis`, and intra-law
//! dataflow). Textual references in `articles[].text` are not picked up
//! here — those require regex/NLP and are out of scope.
//!
//! The output is collected into a [`RelationIndex`] with O(1) lookups by
//! law, article, and output, so query endpoints can answer
//! "what is related to X?" without rescanning the corpus.
//!
//! Weights are returned as `f64` with hard-coded defaults per
//! [`RelationType`]. The data model has room for explicit weights later
//! (a future RFC may declare them in YAML); this iteration only uses the
//! defaults.
//!
//! See `corpus/regulation/nl/wet/wet_op_de_zorgtoeslag/2025-01-01.yaml`
//! for a working IoC example that exercises all four relation types.

use crate::article::ArticleBasedLaw;
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Kinds of explicit relations that can be extracted from the corpus.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RelationType {
    /// `input.source.regulation` pointing at another law id — law A
    /// needs the output of law B to execute this article.
    CrossLawDataflow,
    /// `implements` declaration — this article fills an open_term of a
    /// higher article/law (IoC, "Gelet op …").
    Implementation,
    /// `open_terms` declaration — this article leaves a term open for a
    /// lower regulator to fill in (the inverse of [`Self::Implementation`]).
    OpenTermDeclaration,
    /// `legal_basis` at law level — the whole regulation has a basis in
    /// a higher law article.
    LegalBasis,
    /// `input.source.output` without `regulation` — another article in
    /// the same law produces this input.
    IntraLawDataflow,
}

impl RelationType {
    /// Default weight per relation type. Hand-tuned based on how "hard"
    /// the coupling is: dataflow is the hardest (no result without the
    /// source), open_term/implements is slightly softer (delegation),
    /// legal_basis softer still (grounding, not always needed at
    /// runtime), and intra-law dataflow is lowest only because intra-law
    /// relations are typically less valuable in a cross-corpus graph.
    pub fn default_weight(self) -> f64 {
        match self {
            Self::CrossLawDataflow => 1.0,
            Self::Implementation => 0.9,
            Self::OpenTermDeclaration => 0.9,
            Self::LegalBasis => 0.7,
            Self::IntraLawDataflow => 0.6,
        }
    }
}

/// A specific point in the corpus that a relation attaches to or from.
///
/// `law_id` is required. `article` is `None` when the relation attaches
/// to a whole law (e.g. `legal_basis` at law level has no source article).
/// `output` / `input` are only populated when the relation attaches to a
/// specific field in `machine_readable.execution` — otherwise they stay
/// empty so that article-level queries find the endpoint.
#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub struct RelationEndpoint {
    pub law_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub input: Option<String>,
}

impl RelationEndpoint {
    pub fn law(law_id: impl Into<String>) -> Self {
        Self {
            law_id: law_id.into(),
            article: None,
            output: None,
            input: None,
        }
    }

    pub fn article(law_id: impl Into<String>, article: impl Into<String>) -> Self {
        Self {
            law_id: law_id.into(),
            article: Some(article.into()),
            output: None,
            input: None,
        }
    }
}

/// A single directed relation between two endpoints in the corpus.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Relation {
    pub from: RelationEndpoint,
    pub to: RelationEndpoint,
    pub relation_type: RelationType,
    pub weight: f64,
    /// Free-form metadata for type-specific details: e.g.
    /// `{"open_term": "standaardpremie"}` for Implementation, or
    /// `{"input": "toetsingsinkomen", "output": "toetsingsinkomen"}`
    /// for CrossLawDataflow.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub metadata: BTreeMap<String, String>,
}

/// Direction in which a relation is queried relative to a query endpoint.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Direction {
    /// Relations where the query endpoint is the `to` (others point at me).
    Incoming,
    /// Relations where the query endpoint is the `from` (I point at others).
    Outgoing,
    /// Both sides.
    #[default]
    Both,
}

/// In-memory index over all [`Relation`]s in the corpus, with side
/// indexes per granularity so that `for_*` queries are O(1) lookups.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RelationIndex {
    relations: Vec<Relation>,
    /// `law_id` → indexes in `relations` where this law is the `from`.
    out_by_law: HashMap<String, Vec<usize>>,
    /// `law_id` → indexes in `relations` where this law is the `to`.
    in_by_law: HashMap<String, Vec<usize>>,
    /// `(law_id, article)` → indexes where this article is the `from`.
    out_by_article: HashMap<(String, String), Vec<usize>>,
    /// `(law_id, article)` → indexes where this article is the `to`.
    in_by_article: HashMap<(String, String), Vec<usize>>,
    /// `(law_id, article, output)` → indexes where this output is the `from`.
    out_by_output: HashMap<(String, String, String), Vec<usize>>,
    /// `(law_id, article, output)` → indexes where this output is the `to`.
    in_by_output: HashMap<(String, String, String), Vec<usize>>,
}

impl RelationIndex {
    /// Build the index from a slice of parsed laws.
    pub fn build(laws: &[ArticleBasedLaw]) -> Self {
        let mut idx = Self::default();
        for law in laws {
            extract_legal_basis(law, &mut idx);
            extract_article_relations(law, laws, &mut idx);
        }
        idx
    }

    /// Total relation count — handy for diagnostics/logging.
    pub fn len(&self) -> usize {
        self.relations.len()
    }

    pub fn is_empty(&self) -> bool {
        self.relations.is_empty()
    }

    /// Read-only view of all relations. Order is not stable across builds.
    pub fn all(&self) -> &[Relation] {
        &self.relations
    }

    /// Relations involving a whole law (any article).
    pub fn for_law(&self, law_id: &str, direction: Direction) -> Vec<&Relation> {
        self.collect(
            direction,
            self.out_by_law.get(law_id),
            self.in_by_law.get(law_id),
        )
    }

    /// Relations involving a specific article.
    pub fn for_article(&self, law_id: &str, article: &str, direction: Direction) -> Vec<&Relation> {
        let key = (law_id.to_string(), article.to_string());
        self.collect(
            direction,
            self.out_by_article.get(&key),
            self.in_by_article.get(&key),
        )
    }

    /// Relations that touch a specific named output on an article.
    pub fn for_output(
        &self,
        law_id: &str,
        article: &str,
        output: &str,
        direction: Direction,
    ) -> Vec<&Relation> {
        let key = (law_id.to_string(), article.to_string(), output.to_string());
        self.collect(
            direction,
            self.out_by_output.get(&key),
            self.in_by_output.get(&key),
        )
    }

    /// Relations that touch a specific named input on an article.
    ///
    /// Inputs themselves are not held in a side index (an article
    /// typically has many inputs and the main pivot is always the
    /// source output); we therefore filter the article set on
    /// `metadata["input"]`. For the current corpus size that is <1ms
    /// and it keeps the index small.
    pub fn for_input(
        &self,
        law_id: &str,
        article: &str,
        input: &str,
        direction: Direction,
    ) -> Vec<&Relation> {
        // Canonical source of truth for "which input" is the metadata
        // field — the extractor sets it at the same site where `from.input`
        // is set for dataflow edges, and no extractor populates `to.input`.
        // Checking only `metadata["input"]` keeps the predicate aligned
        // with the producer and prevents future divergence.
        self.for_article(law_id, article, direction)
            .into_iter()
            .filter(|rel| rel.metadata.get("input").map(String::as_str) == Some(input))
            .collect()
    }

    fn collect(
        &self,
        direction: Direction,
        outgoing: Option<&Vec<usize>>,
        incoming: Option<&Vec<usize>>,
    ) -> Vec<&Relation> {
        let mut seen = std::collections::HashSet::new();
        let mut out = Vec::new();
        let mut push_from = |idxs: &Vec<usize>| {
            for &i in idxs {
                if seen.insert(i) {
                    out.push(&self.relations[i]);
                }
            }
        };
        match direction {
            Direction::Outgoing => {
                if let Some(idxs) = outgoing {
                    push_from(idxs);
                }
            }
            Direction::Incoming => {
                if let Some(idxs) = incoming {
                    push_from(idxs);
                }
            }
            Direction::Both => {
                if let Some(idxs) = outgoing {
                    push_from(idxs);
                }
                if let Some(idxs) = incoming {
                    push_from(idxs);
                }
            }
        }
        out
    }

    fn push(&mut self, rel: Relation) {
        let idx = self.relations.len();
        // out-side: keyed by rel.from
        self.out_by_law
            .entry(rel.from.law_id.clone())
            .or_default()
            .push(idx);
        if let Some(art) = &rel.from.article {
            self.out_by_article
                .entry((rel.from.law_id.clone(), art.clone()))
                .or_default()
                .push(idx);
            if let Some(out) = &rel.from.output {
                self.out_by_output
                    .entry((rel.from.law_id.clone(), art.clone(), out.clone()))
                    .or_default()
                    .push(idx);
            }
        }
        // in-side: keyed by rel.to
        self.in_by_law
            .entry(rel.to.law_id.clone())
            .or_default()
            .push(idx);
        if let Some(art) = &rel.to.article {
            self.in_by_article
                .entry((rel.to.law_id.clone(), art.clone()))
                .or_default()
                .push(idx);
            if let Some(out) = &rel.to.output {
                self.in_by_output
                    .entry((rel.to.law_id.clone(), art.clone(), out.clone()))
                    .or_default()
                    .push(idx);
            }
        }
        self.relations.push(rel);
    }
}

fn extract_legal_basis(law: &ArticleBasedLaw, idx: &mut RelationIndex) {
    let Some(bases) = law.legal_basis.as_ref() else {
        return;
    };
    for basis in bases {
        let mut meta = BTreeMap::new();
        if let Some(desc) = &basis.description {
            meta.insert("description".to_string(), desc.clone());
        }
        idx.push(Relation {
            from: RelationEndpoint::law(&law.id),
            to: RelationEndpoint::article(&basis.law_id, &basis.article),
            relation_type: RelationType::LegalBasis,
            weight: RelationType::LegalBasis.default_weight(),
            metadata: meta,
        });
    }
}

fn extract_article_relations(
    law: &ArticleBasedLaw,
    all_laws: &[ArticleBasedLaw],
    idx: &mut RelationIndex,
) {
    for article in &law.articles {
        // Implementation (this article fills an open_term elsewhere) +
        // matching OpenTermDeclaration on the target side.
        if let Some(impls) = article.get_implements() {
            for decl in impls {
                let mut meta = BTreeMap::new();
                meta.insert("open_term".to_string(), decl.open_term.clone());
                if let Some(gelet) = &decl.gelet_op {
                    meta.insert("gelet_op".to_string(), gelet.clone());
                }
                idx.push(Relation {
                    from: RelationEndpoint::article(&law.id, &article.number),
                    to: RelationEndpoint::article(&decl.law, &decl.article),
                    relation_type: RelationType::Implementation,
                    weight: RelationType::Implementation.default_weight(),
                    metadata: meta.clone(),
                });
                idx.push(Relation {
                    from: RelationEndpoint::article(&decl.law, &decl.article),
                    to: RelationEndpoint::article(&law.id, &article.number),
                    relation_type: RelationType::OpenTermDeclaration,
                    weight: RelationType::OpenTermDeclaration.default_weight(),
                    metadata: meta,
                });
            }
        }

        // Dataflow on inputs (cross-law or intra-law).
        for input in article.get_inputs() {
            let Some(source) = input.source.as_ref() else {
                continue;
            };
            let Some(source_output) = source.output.as_deref() else {
                // `source: {}` resolves via DataSourceRegistry at runtime —
                // not a corpus relation we can express statically.
                continue;
            };
            match source.regulation.as_deref() {
                Some(other_law) => {
                    let target_article = all_laws
                        .iter()
                        .find(|l| l.id == other_law)
                        .and_then(|l| l.find_article_by_output(source_output))
                        .map(|a| a.number.clone());
                    // If the target article cannot be resolved (the law
                    // is not loaded), we still ship `output` — that is a
                    // useful trail for the client. `push()` keeps the
                    // output-side index empty as long as `article` is
                    // missing.
                    let to = RelationEndpoint {
                        law_id: other_law.to_string(),
                        article: target_article,
                        output: Some(source_output.to_string()),
                        input: None,
                    };
                    let mut meta = BTreeMap::new();
                    meta.insert("input".to_string(), input.name.clone());
                    meta.insert("output".to_string(), source_output.to_string());
                    idx.push(Relation {
                        from: RelationEndpoint {
                            law_id: law.id.clone(),
                            article: Some(article.number.clone()),
                            output: None,
                            input: Some(input.name.clone()),
                        },
                        to,
                        relation_type: RelationType::CrossLawDataflow,
                        weight: RelationType::CrossLawDataflow.default_weight(),
                        metadata: meta,
                    });
                }
                None => {
                    // Same-law reference: find the producing article within
                    // this law. If the output is unresolvable, skip — the
                    // engine would surface this elsewhere.
                    let Some(target_article) = law.find_article_by_output(source_output) else {
                        continue;
                    };
                    if target_article.number == article.number {
                        // Self-reference is not a useful relation.
                        continue;
                    }
                    let mut meta = BTreeMap::new();
                    meta.insert("input".to_string(), input.name.clone());
                    meta.insert("output".to_string(), source_output.to_string());
                    idx.push(Relation {
                        from: RelationEndpoint {
                            law_id: law.id.clone(),
                            article: Some(article.number.clone()),
                            output: None,
                            input: Some(input.name.clone()),
                        },
                        to: RelationEndpoint {
                            law_id: law.id.clone(),
                            article: Some(target_article.number.clone()),
                            output: Some(source_output.to_string()),
                            input: None,
                        },
                        relation_type: RelationType::IntraLawDataflow,
                        weight: RelationType::IntraLawDataflow.default_weight(),
                        metadata: meta,
                    });
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const ZORGTOESLAG_YAML: &str = r#"
$schema: "https://example.com/v0.5.0/schema.json"
$id: zorgtoeslagwet
regulatory_layer: WET
publication_date: "2025-01-01"
name: Wet op de zorgtoeslag
legal_basis:
  - law_id: grondwet
    article: "120"
    description: "Grondslag"
articles:
  - number: "2"
    text: "Recht op zorgtoeslag."
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
        input:
          - name: toetsingsinkomen
            type: number
            source:
              regulation: awir
              output: toetsingsinkomen
          - name: standaardpremie
            type: number
            source:
              output: standaardpremie
        output:
          - name: zorgtoeslag
            type: number
        actions:
          - output: zorgtoeslag
            value: 0
  - number: "4"
    text: "De standaardpremie wordt bij ministeriële regeling vastgesteld."
    machine_readable:
      open_terms:
        - id: standaardpremie
          type: number
          delegated_to: minister
      execution:
        parameters: []
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 0
"#;

    const REGELING_YAML: &str = r#"
$schema: "https://example.com/v0.5.0/schema.json"
$id: regeling_standaardpremie
regulatory_layer: MINISTERIELE_REGELING
publication_date: "2025-01-01"
name: Regeling standaardpremie
articles:
  - number: "1"
    text: "De standaardpremie bedraagt €1500."
    machine_readable:
      implements:
        - law: zorgtoeslagwet
          article: "4"
          open_term: standaardpremie
      execution:
        parameters: []
        output:
          - name: standaardpremie
            type: number
        actions:
          - output: standaardpremie
            value: 1500
"#;

    const AWIR_YAML: &str = r#"
$schema: "https://example.com/v0.5.0/schema.json"
$id: awir
regulatory_layer: WET
publication_date: "2025-01-01"
name: AWIR
articles:
  - number: "8"
    text: "Toetsingsinkomen."
    machine_readable:
      execution:
        parameters:
          - name: bsn
            type: string
        output:
          - name: toetsingsinkomen
            type: number
        actions:
          - output: toetsingsinkomen
            value: 0
"#;

    fn parse_corpus() -> Vec<ArticleBasedLaw> {
        vec![
            ArticleBasedLaw::from_yaml_str(ZORGTOESLAG_YAML).expect("zorgtoeslag"),
            ArticleBasedLaw::from_yaml_str(REGELING_YAML).expect("regeling"),
            ArticleBasedLaw::from_yaml_str(AWIR_YAML).expect("awir"),
        ]
    }

    #[test]
    fn extracts_cross_law_dataflow() {
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        let outgoing = idx.for_article("zorgtoeslagwet", "2", Direction::Outgoing);
        let cross_law: Vec<_> = outgoing
            .iter()
            .filter(|r| r.relation_type == RelationType::CrossLawDataflow)
            .collect();
        assert_eq!(cross_law.len(), 1, "expected one cross-law dataflow edge");
        let rel = cross_law[0];
        assert_eq!(rel.to.law_id, "awir");
        assert_eq!(rel.to.article.as_deref(), Some("8"));
        assert_eq!(rel.to.output.as_deref(), Some("toetsingsinkomen"));
        assert_eq!(rel.metadata.get("input").unwrap(), "toetsingsinkomen");
        assert_eq!(rel.weight, RelationType::CrossLawDataflow.default_weight());
    }

    #[test]
    fn extracts_implementation_and_open_term_pair() {
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        let from_regeling = idx.for_article("regeling_standaardpremie", "1", Direction::Outgoing);
        assert!(from_regeling
            .iter()
            .any(|r| r.relation_type == RelationType::Implementation
                && r.to.law_id == "zorgtoeslagwet"
                && r.to.article.as_deref() == Some("4")));
        let to_regeling = idx.for_article("regeling_standaardpremie", "1", Direction::Incoming);
        assert!(to_regeling
            .iter()
            .any(|r| r.relation_type == RelationType::OpenTermDeclaration
                && r.from.law_id == "zorgtoeslagwet"
                && r.from.article.as_deref() == Some("4")));
    }

    #[test]
    fn incoming_lookup_finds_who_implements_my_open_term() {
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        let incoming = idx.for_article("zorgtoeslagwet", "4", Direction::Incoming);
        let impls: Vec<_> = incoming
            .iter()
            .filter(|r| r.relation_type == RelationType::Implementation)
            .collect();
        assert_eq!(impls.len(), 1);
        assert_eq!(impls[0].from.law_id, "regeling_standaardpremie");
        assert_eq!(
            impls[0].metadata.get("open_term").unwrap(),
            "standaardpremie"
        );
    }

    #[test]
    fn extracts_intra_law_dataflow() {
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        let outgoing = idx.for_article("zorgtoeslagwet", "2", Direction::Outgoing);
        let intra: Vec<_> = outgoing
            .iter()
            .filter(|r| r.relation_type == RelationType::IntraLawDataflow)
            .collect();
        assert_eq!(intra.len(), 1);
        assert_eq!(intra[0].to.law_id, "zorgtoeslagwet");
        assert_eq!(intra[0].to.article.as_deref(), Some("4"));
        assert_eq!(intra[0].to.output.as_deref(), Some("standaardpremie"));
    }

    #[test]
    fn extracts_legal_basis() {
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        let outgoing = idx.for_law("zorgtoeslagwet", Direction::Outgoing);
        let basis: Vec<_> = outgoing
            .iter()
            .filter(|r| r.relation_type == RelationType::LegalBasis)
            .collect();
        assert_eq!(basis.len(), 1);
        assert_eq!(basis[0].to.law_id, "grondwet");
        assert_eq!(basis[0].to.article.as_deref(), Some("120"));
    }

    #[test]
    fn for_output_finds_dataflow_consumers() {
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        // AWIR.toetsingsinkomen output is the `to` side of zorgtoeslag's dataflow.
        let consumers = idx.for_output("awir", "8", "toetsingsinkomen", Direction::Incoming);
        assert!(consumers
            .iter()
            .any(|r| r.relation_type == RelationType::CrossLawDataflow
                && r.from.law_id == "zorgtoeslagwet"
                && r.from.article.as_deref() == Some("2")));
    }

    #[test]
    fn for_input_filters_by_input_name() {
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        let matches = idx.for_input(
            "zorgtoeslagwet",
            "2",
            "toetsingsinkomen",
            Direction::Outgoing,
        );
        assert_eq!(matches.len(), 1);
        assert_eq!(matches[0].relation_type, RelationType::CrossLawDataflow);
    }

    #[test]
    fn both_direction_does_not_duplicate() {
        // A whole-law query for zorgtoeslagwet hits both incoming (e.g.
        // implementation from regeling) and outgoing (e.g. dataflow to
        // awir). Both must not double-count any single edge.
        let laws = parse_corpus();
        let idx = RelationIndex::build(&laws);
        let both = idx.for_law("zorgtoeslagwet", Direction::Both);
        let mut seen = std::collections::HashSet::new();
        for rel in &both {
            // Combine (from, to, type) as a poor-man's edge identity.
            let key = (rel.from.clone(), rel.to.clone(), rel.relation_type);
            assert!(seen.insert(key), "edge appears twice in Both result");
        }
    }
}
