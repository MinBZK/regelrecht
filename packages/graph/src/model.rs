//! Lean deserialisation of a regulation YAML file.
//!
//! The graph builder reads 4.000+ files of on average 50 kB and needs a few
//! hundred bytes from each. `regelrecht_law_model::ArticleBasedLaw` is the full
//! document model and deliberately does not carry `references` at all (the
//! harvested reference block is corpus metadata, not execution semantics), so
//! there is nothing to reuse there. These structs take exactly the fields the
//! graph needs and let serde drop the rest, which keeps both the parse cost and
//! the coupling to schema churn down.
//!
//! Two schema vocabularies have to survive here side by side: the repository
//! corpus still writes v0.5.x (`open_terms` with `delegated_to`), and the
//! harvested corpus arrives as v0.6.0. The fields the graph reads happen to be
//! spelled the same in both; where they are not, the alias is noted below.

use serde::de::Deserializer;
use serde::Deserialize;

/// One regulation file, reduced to what the graph needs.
#[derive(Debug, Clone, Deserialize)]
pub struct LawFile {
    #[serde(rename = "$id")]
    pub id: Option<String>,
    #[serde(default)]
    pub bwb_id: Option<String>,
    #[serde(default)]
    pub name: Option<String>,
    #[serde(default)]
    pub officiele_titel: Option<String>,
    #[serde(default)]
    pub regulatory_layer: Option<String>,
    #[serde(default)]
    pub valid_from: Option<String>,
    #[serde(default)]
    pub articles: Vec<ArticleFile>,
}

/// One article, reduced to its identity, its harvested references and the
/// enrichment constructs that carry a stronger edge.
#[derive(Debug, Clone, Deserialize)]
pub struct ArticleFile {
    #[serde(default, deserialize_with = "flex_string")]
    pub number: Option<String>,
    #[serde(default)]
    pub references: Vec<ReferenceFile>,
    #[serde(default)]
    pub machine_readable: Option<MachineReadableFile>,
}

/// A harvested reference: the pointer the wettekst itself makes to another
/// regulation. `bwb_id` is required by the schema; the positional fields are
/// all optional and describe how deep into the target the pointer goes.
#[derive(Debug, Clone, Deserialize)]
pub struct ReferenceFile {
    #[serde(default, deserialize_with = "flex_string")]
    pub bwb_id: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub artikel: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub lid: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub hoofdstuk: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub afdeling: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub paragraaf: Option<String>,
}

impl ReferenceFile {
    /// The most specific anchor this reference names, if any. Used to point a
    /// citation edge at a concrete article node instead of at the law as a
    /// whole.
    pub fn anchor(&self) -> Option<&str> {
        self.artikel.as_deref()
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct MachineReadableFile {
    #[serde(default)]
    pub execution: Option<ExecutionFile>,
    #[serde(default)]
    pub implements: Vec<ImplementsFile>,
    #[serde(default)]
    pub open_terms: Vec<OpenTermFile>,
    /// Everything else the section carries: `definitions`, `requires`,
    /// `markings`, `competent_authority`, and whatever a later schema version
    /// adds. Kept as raw values because the graph does not read them; it only
    /// needs to know whether they are there.
    #[serde(flatten)]
    pub overige: std::collections::BTreeMap<String, serde_yaml_ng::Value>,
}

impl MachineReadableFile {
    /// Does this section actually say anything?
    ///
    /// The presence of the key is not the question. An enrichment run that
    /// writes `machine_readable: {}` on an article, or leaves a section behind
    /// with only empty lists in it, has modelled nothing, and counting it as
    /// enriched turns the corpus map green while the work is still undone. The
    /// pipeline's own coverage counter takes the looser reading
    /// (`machine_readable.is_some()`); this is the strict one, because a map
    /// that overstates progress is worse than no map.
    pub fn is_substantive(&self) -> bool {
        if self.execution.is_some() || !self.implements.is_empty() || !self.open_terms.is_empty() {
            return true;
        }
        self.overige.values().any(is_meaningful)
    }
}

/// A value counts when it carries content. Null, an empty string, an empty list
/// and an empty mapping all mean the same thing here: nothing was written.
fn is_meaningful(value: &serde_yaml_ng::Value) -> bool {
    match value {
        serde_yaml_ng::Value::Null => false,
        serde_yaml_ng::Value::String(s) => !s.trim().is_empty(),
        serde_yaml_ng::Value::Sequence(items) => items.iter().any(is_meaningful),
        serde_yaml_ng::Value::Mapping(map) => map.values().any(is_meaningful),
        _ => true,
    }
}

#[derive(Debug, Clone, Default, Deserialize)]
pub struct ExecutionFile {
    #[serde(default)]
    pub input: Vec<InputFile>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct InputFile {
    #[serde(default)]
    pub source: Option<SourceFile>,
}

/// `source.regulation` on an input: the strongest edge the corpus can state,
/// because it says this article computes nothing without that other law.
#[derive(Debug, Clone, Deserialize)]
pub struct SourceFile {
    #[serde(default)]
    pub regulation: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub output: Option<String>,
}

/// The IoC counterpart: a lower regulation filling in an `open_term` of a
/// higher one.
#[derive(Debug, Clone, Deserialize)]
pub struct ImplementsFile {
    #[serde(default)]
    pub law: Option<String>,
    #[serde(default, deserialize_with = "flex_string")]
    pub article: Option<String>,
    #[serde(default)]
    pub open_term: Option<String>,
}

/// A term the law leaves open. Three resolve states follow from it (filled,
/// expected, per case); see [`crate::build`].
#[derive(Debug, Clone, Deserialize)]
pub struct OpenTermFile {
    #[serde(default)]
    pub id: Option<String>,
    #[serde(default)]
    pub delegated_to: Option<String>,
    #[serde(default)]
    pub delegation_type: Option<String>,
    #[serde(default)]
    pub expected_source: Option<String>,
    #[serde(default)]
    pub decided_per_case_by: Option<String>,
}

/// Accept a scalar that the corpus sometimes quotes and sometimes does not.
///
/// `artikel: '4'` and `artikel: 4` both occur in harvested YAML, and `lid: 2`
/// is routinely unquoted. A plain `Option<String>` fails on the second form and
/// takes a whole file down with it, so every scalar the graph reads goes
/// through here.
fn flex_string<'de, D>(deserializer: D) -> Result<Option<String>, D::Error>
where
    D: Deserializer<'de>,
{
    let value = serde_yaml_ng::Value::deserialize(deserializer)?;
    Ok(match value {
        serde_yaml_ng::Value::String(s) => Some(s),
        serde_yaml_ng::Value::Number(n) => Some(n.to_string()),
        serde_yaml_ng::Value::Bool(b) => Some(b.to_string()),
        serde_yaml_ng::Value::Null => None,
        _ => None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_a_harvested_law_with_references() {
        let yaml = r#"
$id: wet_op_de_zorgtoeslag
regulatory_layer: WET
publication_date: '2024-10-16'
valid_from: '2026-01-01'
bwb_id: BWBR0018451
preamble:
  text: irrelevant
articles:
  - number: '1'
    text: |-
      wat tekst met [een verwijzing][ref1]
    references:
      - id: ref1
        bwb_id: BWBR0018450
        artikel: '1'
      - id: ref2
        bwb_id: BWBR0011353
        afdeling: '5.3'
"#;
        let law: LawFile = serde_yaml_ng::from_str(yaml).expect("parse");
        assert_eq!(law.id.as_deref(), Some("wet_op_de_zorgtoeslag"));
        assert_eq!(law.bwb_id.as_deref(), Some("BWBR0018451"));
        assert_eq!(law.articles.len(), 1);
        assert_eq!(law.articles[0].references.len(), 2);
        assert_eq!(law.articles[0].references[0].anchor(), Some("1"));
        assert_eq!(law.articles[0].references[1].anchor(), None);
    }

    #[test]
    fn accepts_unquoted_numeric_scalars() {
        let yaml = r#"
$id: test
articles:
  - number: 4
    text: x
    references:
      - id: ref1
        bwb_id: BWBR0000001
        artikel: 12
        lid: 2
"#;
        let law: LawFile = serde_yaml_ng::from_str(yaml).expect("parse");
        assert_eq!(law.articles[0].number.as_deref(), Some("4"));
        assert_eq!(law.articles[0].references[0].artikel.as_deref(), Some("12"));
        assert_eq!(law.articles[0].references[0].lid.as_deref(), Some("2"));
    }

    #[test]
    fn an_empty_section_is_not_enrichment() {
        let yaml = r#"
$id: test
articles:
  - number: '1'
    text: x
    machine_readable: {}
  - number: '2'
    text: y
    machine_readable:
      implements: []
      open_terms: []
  - number: '3'
    text: z
    machine_readable:
      definitions:
        drempelinkomen: iets
"#;
        let law: LawFile = serde_yaml_ng::from_str(yaml).expect("parse");
        let substantive: Vec<bool> = law
            .articles
            .iter()
            .map(|a| {
                a.machine_readable
                    .as_ref()
                    .is_some_and(|m| m.is_substantive())
            })
            .collect();
        assert_eq!(substantive, vec![false, false, true]);
    }

    #[test]
    fn parses_enrichment_constructs() {
        let yaml = r#"
$id: regeling_standaardpremie
regulatory_layer: MINISTERIELE_REGELING
publication_date: '2024-01-01'
articles:
  - number: '1'
    text: x
    machine_readable:
      implements:
        - law: wet_op_de_zorgtoeslag
          article: '4'
          open_term: standaardpremie
      execution:
        input:
          - name: premie
            source:
              regulation: zorgverzekeringswet
              output: is_verzekerd
  - number: '2'
    text: y
    machine_readable:
      open_terms:
        - id: gemeentelijke_afstand_cm
          delegated_to: gemeenteraad
          delegation_type: GEMEENTELIJKE_VERORDENING
"#;
        let law: LawFile = serde_yaml_ng::from_str(yaml).expect("parse");
        let mr = law.articles[0]
            .machine_readable
            .as_ref()
            .expect("machine_readable");
        assert_eq!(
            mr.implements[0].law.as_deref(),
            Some("wet_op_de_zorgtoeslag")
        );
        let exec = mr.execution.as_ref().expect("execution");
        assert_eq!(
            exec.input[0]
                .source
                .as_ref()
                .and_then(|s| s.regulation.as_deref()),
            Some("zorgverzekeringswet")
        );
        let mr2 = law.articles[1]
            .machine_readable
            .as_ref()
            .expect("machine_readable");
        assert_eq!(
            mr2.open_terms[0].delegated_to.as_deref(),
            Some("gemeenteraad")
        );
    }
}
