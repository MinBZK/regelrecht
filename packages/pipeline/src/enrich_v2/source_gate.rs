//! The gate that decides whether a law file may be enriched at all.
//!
//! Enrichment measures a translation against the statutory text in the file
//! it is given. When that text is not the law, the measurement says nothing
//! about the translation. That is not hypothetical: the checked-in
//! `wet_op_de_zorgtoeslag/2025-01-01.yaml` carries an article `1a` that the
//! official toestand does not have, an article 1 whose lettering and
//! definitions differ from the statute, and an article 2 missing the
//! threshold clause ("voorzover dat toetsingsinkomen het drempelinkomen te
//! boven gaat") that the calculation turns on.
//!
//! So this module reads the official BWB toestand and compares. It is
//! deliberately a step *before* the enricher and outside the agent: the
//! agent has no network, and this work belongs to harvesting anyway. It
//! stands here until the harvester takes it back.
//!
//! Two things come out of the same parse. The verdict per article, and the
//! structural placement of each article (chapter, division, paragraph with
//! their headings). The harvester's splitter starts at `artikel` and never
//! sees the containers, so that placement is lost today. It is the reason
//! an agent cannot tell that Awb 3:9 sits in "Afdeling 3.3 Advisering".

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

/// Where an article sits in the document. Every level carries its number
/// and its heading, because the heading is condensed legal classification
/// written by the legislator.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Placement {
    #[serde(skip_serializing_if = "Option::is_none")]
    pub boek: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub deel: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub titeldeel: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<Level>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<Level>,
}

impl Placement {
    /// True when no container encloses the article, which is normal for a
    /// short law.
    pub fn is_empty(&self) -> bool {
        self == &Self::default()
    }

    /// Human-readable path, most general first.
    pub fn path(&self) -> String {
        [
            self.boek.as_ref().map(|l| ("Boek", l)),
            self.deel.as_ref().map(|l| ("Deel", l)),
            self.hoofdstuk.as_ref().map(|l| ("Hoofdstuk", l)),
            self.titeldeel.as_ref().map(|l| ("Titel", l)),
            self.afdeling.as_ref().map(|l| ("Afdeling", l)),
            self.paragraaf.as_ref().map(|l| ("Paragraaf", l)),
        ]
        .into_iter()
        .flatten()
        .map(|(kind, level)| match &level.heading {
            Some(h) => format!("{kind} {} {h}", level.number),
            None => format!("{kind} {}", level.number),
        })
        .collect::<Vec<_>>()
        .join(" > ")
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Level {
    pub number: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
}

/// One article as the official toestand has it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SourceArticle {
    pub number: String,
    /// Heading of the article itself, when it has one.
    pub heading: Option<String>,
    /// The statutory text, leden numbered as `1. `, `2. `, separated by a
    /// blank line, matching the convention the corpus already uses.
    pub text: String,
    pub placement: Placement,
}

/// What the comparison says about one article.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Verdict {
    /// Text matches the official toestand after whitespace normalisation.
    Verified,
    /// Both have the article, the text differs.
    Drift { detail: String },
    /// The official toestand has it, the corpus file does not.
    Missing,
    /// The corpus file has it, the official toestand does not.
    Fabricated,
}

impl Verdict {
    pub fn is_ok(&self) -> bool {
        matches!(self, Verdict::Verified)
    }
}

/// The result of holding a corpus file against the official toestand.
#[derive(Debug, Default)]
pub struct GateReport {
    pub verdicts: BTreeMap<String, Verdict>,
    /// Placement per article number, from the official source.
    pub placements: BTreeMap<String, Placement>,
}

impl GateReport {
    /// The gate passes only when every article is verified. A single
    /// fabricated or drifted article means the file does not carry the law,
    /// and a translation measured against it proves nothing.
    pub fn passes(&self) -> bool {
        !self.verdicts.is_empty() && self.verdicts.values().all(Verdict::is_ok)
    }

    pub fn counts(&self) -> BTreeMap<&'static str, usize> {
        let mut c = BTreeMap::new();
        for v in self.verdicts.values() {
            let key = match v {
                Verdict::Verified => "verified",
                Verdict::Drift { .. } => "drift",
                Verdict::Missing => "missing",
                Verdict::Fabricated => "fabricated",
            };
            *c.entry(key).or_insert(0) += 1;
        }
        c
    }
}

/// URL of the official toestand XML for a BWB id on a date.
pub fn toestand_url(bwb_id: &str, valid_from: &str) -> String {
    format!(
        "https://repository.officiele-overheidspublicaties.nl/bwb/{bwb_id}/{valid_from}_0/xml/{bwb_id}_{valid_from}_0.xml"
    )
}

/// Container elements, from most general to most specific. The harvester's
/// splitter starts at `artikel` and therefore never records any of these.
const CONTAINERS: &[&str] = &[
    "boek",
    "deel",
    "hoofdstuk",
    "titeldeel",
    "afdeling",
    "paragraaf",
];

/// Parse an official BWB toestand into articles with their placement.
pub fn parse_toestand(xml: &str) -> Result<Vec<SourceArticle>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| format!("toestand is not XML: {e}"))?;
    let mut out = Vec::new();
    walk(doc.root_element(), &Placement::default(), &mut out);
    Ok(out)
}

fn walk(node: roxmltree::Node, inherited: &Placement, out: &mut Vec<SourceArticle>) {
    let tag = node.tag_name().name();

    if tag == "artikel" {
        if let Some(article) = read_article(node, inherited) {
            out.push(article);
        }
        return;
    }

    // A container contributes its own level to everything below it.
    let placement = if CONTAINERS.contains(&tag) {
        let mut p = inherited.clone();
        let level = read_level(node);
        match tag {
            "boek" => p.boek = level,
            "deel" => p.deel = level,
            "hoofdstuk" => p.hoofdstuk = level,
            "titeldeel" => p.titeldeel = level,
            "afdeling" => p.afdeling = level,
            "paragraaf" => p.paragraaf = level,
            _ => {}
        }
        p
    } else {
        inherited.clone()
    };

    for child in node.children().filter(roxmltree::Node::is_element) {
        walk(child, &placement, out);
    }
}

/// The `kop` of a container holds `nr` and usually `titel`.
fn read_level(node: roxmltree::Node) -> Option<Level> {
    let kop = node
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "kop")?;
    let number = child_text(kop, "nr").unwrap_or_default();
    let heading = child_text(kop, "titel").filter(|s| !s.is_empty());
    if number.is_empty() && heading.is_none() {
        return None;
    }
    Some(Level { number, heading })
}

fn read_article(node: roxmltree::Node, placement: &Placement) -> Option<SourceArticle> {
    let kop = node
        .children()
        .find(|c| c.is_element() && c.tag_name().name() == "kop");
    let number = kop.and_then(|k| child_text(k, "nr")).unwrap_or_default();
    if number.is_empty() {
        return None;
    }
    let heading = kop
        .and_then(|k| child_text(k, "titel"))
        .filter(|s| !s.is_empty());

    // Leden get their number back in front, which is how the corpus writes
    // them and what the per-lid accounting relies on. An article with a
    // single lid carries no number in the corpus convention, and adding one
    // here would report a formatting difference as drift on every such
    // article, drowning the real ones.
    let lid_nodes: Vec<_> = node
        .children()
        .filter(|c| c.is_element() && c.tag_name().name() == "lid")
        .collect();
    let single = lid_nodes.len() == 1;
    let leden: Vec<String> = lid_nodes
        .into_iter()
        .map(|lid| {
            let nr = child_text(lid, "lidnr").unwrap_or_default();
            let body = element_text_excluding(lid, &["lidnr", "meta-data"]);
            if nr.is_empty() || single {
                body
            } else {
                format!("{nr}. {body}")
            }
        })
        .filter(|s| !s.trim().is_empty())
        .collect();

    let text = if leden.is_empty() {
        element_text_excluding(node, &["kop", "meta-data"])
    } else {
        leden.join("\n\n")
    };

    Some(SourceArticle {
        number,
        heading,
        text: normalize_ws(&text),
        placement: placement.clone(),
    })
}

fn child_text(node: roxmltree::Node, name: &str) -> Option<String> {
    node.children()
        .find(|c| c.is_element() && c.tag_name().name() == name)
        .map(|c| normalize_ws(&all_text(c)))
}

fn all_text(node: roxmltree::Node) -> String {
    node.descendants()
        .filter(roxmltree::Node::is_text)
        .filter_map(|n| n.text())
        .collect::<Vec<_>>()
        .join(" ")
}

fn element_text_excluding(node: roxmltree::Node, skip: &[&str]) -> String {
    let mut parts = Vec::new();
    for child in node.children() {
        if child.is_element() && skip.contains(&child.tag_name().name()) {
            continue;
        }
        if child.is_text() {
            if let Some(t) = child.text() {
                parts.push(t.to_string());
            }
        } else if child.is_element() {
            parts.push(all_text(child));
        }
    }
    normalize_ws(&parts.join(" "))
}

fn normalize_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// Compare a corpus law document against the official articles.
///
/// Comparison is on normalised whitespace only. Anything else (accents,
/// punctuation, casing) is treated as a difference on purpose: a
/// translation is checked word for word against this text, so a silent
/// tolerance here would be a silent tolerance there.
pub fn verify(corpus: &serde_yaml_ng::Value, official: &[SourceArticle]) -> GateReport {
    let mut report = GateReport::default();
    let mut official_by_number: BTreeMap<&str, &SourceArticle> = BTreeMap::new();
    for a in official {
        official_by_number.insert(a.number.as_str(), a);
        report
            .placements
            .insert(a.number.clone(), a.placement.clone());
    }

    let empty = Vec::new();
    let articles = corpus
        .get("articles")
        .and_then(serde_yaml_ng::Value::as_sequence)
        .unwrap_or(&empty);

    let mut seen = std::collections::BTreeSet::new();
    for article in articles {
        let number = match article.get("number") {
            Some(serde_yaml_ng::Value::String(s)) => s.clone(),
            Some(other) => serde_yaml_ng::to_string(other)
                .unwrap_or_default()
                .trim()
                .to_string(),
            None => continue,
        };
        seen.insert(number.clone());
        let corpus_text = normalize_ws(
            article
                .get("text")
                .and_then(serde_yaml_ng::Value::as_str)
                .unwrap_or_default(),
        );
        match official_by_number.get(number.as_str()) {
            None => {
                report.verdicts.insert(number, Verdict::Fabricated);
            }
            Some(source) => {
                let verdict = if corpus_text == source.text {
                    Verdict::Verified
                } else {
                    Verdict::Drift {
                        detail: describe_difference(&corpus_text, &source.text),
                    }
                };
                report.verdicts.insert(number, verdict);
            }
        }
    }

    for a in official {
        if !seen.contains(&a.number) {
            report.verdicts.insert(a.number.clone(), Verdict::Missing);
        }
    }

    report
}

/// The context sidecar: everything the official source knows about an
/// article that the law YAML has no field for.
///
/// This does not go into the law file. `machine_readable` and the article
/// object are `additionalProperties: false` on a released schema version,
/// so carrying placement there needs a version bump plus a corpus-wide
/// migration. Until that is decided, the worker reads this file and puts
/// the placement in front of the agent, which is what layer 3 of RFC-026
/// describes anyway: the worker assembles, the agent reads.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ContextSidecar {
    pub bwb_id: String,
    pub valid_from: String,
    /// Article number to what the source says about it.
    pub articles: BTreeMap<String, ArticleContext>,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct ArticleContext {
    /// Readable path, e.g. "Hoofdstuk 3 Besluiten > Afdeling 3.3 Advisering".
    #[serde(skip_serializing_if = "String::is_empty", default)]
    pub path: String,
    #[serde(skip_serializing_if = "Placement::is_empty", default)]
    pub placement: Placement,
    /// The article's own heading, when the law gives it one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
    /// What the gate said about this article before the rewrite.
    pub verdict: String,
}

/// Filename of the context sidecar, beside the law YAML.
pub const CONTEXT_SIDECAR: &str = ".source-context.yaml";

/// Rewrite a corpus law document so its `text` is the official text and its
/// article set is the official article set.
///
/// A `machine_readable` section is carried over by article number when the
/// article still exists. It is deliberately kept even when the text
/// drifted: dropping it would silently discard work, and the checks that
/// run afterwards are what decide whether the translation still holds
/// against the corrected text.
///
/// Returns the rewritten document and the sidecar.
pub fn rewrite(
    corpus: &serde_yaml_ng::Value,
    official: &[SourceArticle],
    report: &GateReport,
) -> (serde_yaml_ng::Value, ContextSidecar) {
    use serde_yaml_ng::{Mapping, Value};

    let mut existing_mr: BTreeMap<String, Value> = BTreeMap::new();
    let mut existing_url: BTreeMap<String, Value> = BTreeMap::new();
    if let Some(seq) = corpus.get("articles").and_then(Value::as_sequence) {
        for article in seq {
            let Some(number) = article.get("number").and_then(Value::as_str) else {
                continue;
            };
            if let Some(mr) = article.get("machine_readable") {
                existing_mr.insert(number.to_string(), mr.clone());
            }
            if let Some(url) = article.get("url") {
                existing_url.insert(number.to_string(), url.clone());
            }
        }
    }

    let base_url = corpus
        .get("url")
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string();

    let mut sidecar = ContextSidecar {
        bwb_id: corpus
            .get("bwb_id")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        valid_from: corpus
            .get("valid_from")
            .and_then(Value::as_str)
            .unwrap_or_default()
            .to_string(),
        articles: BTreeMap::new(),
    };

    let mut articles = Vec::new();
    for source in official {
        let mut map = Mapping::new();
        map.insert(Value::from("number"), Value::from(source.number.as_str()));
        map.insert(Value::from("text"), Value::from(source.text.as_str()));
        let url = existing_url
            .get(&source.number)
            .cloned()
            .unwrap_or_else(|| {
                Value::from(format!(
                    "{base_url}#Artikel{}",
                    source.number.replace(' ', "")
                ))
            });
        map.insert(Value::from("url"), url);
        if let Some(mr) = existing_mr.get(&source.number) {
            map.insert(Value::from("machine_readable"), mr.clone());
        }
        articles.push(Value::Mapping(map));

        sidecar.articles.insert(
            source.number.clone(),
            ArticleContext {
                path: source.placement.path(),
                placement: source.placement.clone(),
                heading: source.heading.clone(),
                verdict: match report.verdicts.get(&source.number) {
                    Some(Verdict::Verified) => "verified".into(),
                    Some(Verdict::Drift { .. }) => "text replaced (had drifted)".into(),
                    Some(Verdict::Missing) => "text added (was absent)".into(),
                    Some(Verdict::Fabricated) | None => "new".into(),
                },
            },
        );
    }

    let mut doc = corpus.clone();
    if let Value::Mapping(map) = &mut doc {
        map.insert(Value::from("articles"), Value::Sequence(articles));
    }
    (doc, sidecar)
}

/// A short, quotable description of how two texts differ: the first point
/// where they diverge, with a little context on both sides.
fn describe_difference(corpus: &str, source: &str) -> String {
    let common = corpus
        .char_indices()
        .zip(source.chars())
        .take_while(|((_, a), b)| a == b)
        .last()
        .map_or(0, |((i, c), _)| i + c.len_utf8());

    let tail = |s: &str| -> String {
        let rest: String = s[common.min(s.len())..].chars().take(90).collect();
        if rest.is_empty() {
            "<einde>".to_string()
        } else {
            rest
        }
    };

    format!(
        "diverges after {common} chars; corpus has \"{}\", source has \"{}\"",
        tail(corpus),
        tail(source)
    )
}

#[cfg(test)]
#[allow(clippy::unwrap_used)]
mod tests {
    use super::*;

    const XML: &str = r#"<toestand bwb-id="BWBR0000001">
      <wet-besluit>
        <wettekst>
          <hoofdstuk><kop><label>Hoofdstuk</label><nr>3</nr><titel>Besluiten</titel></kop>
            <afdeling><kop><label>Afdeling</label><nr>3.3</nr><titel>Advisering</titel></kop>
              <artikel><kop><label>Artikel</label><nr>3:5</nr></kop>
                <lid><lidnr>1</lidnr><al>In deze afdeling wordt verstaan onder adviseur: ...</al></lid>
              </artikel>
              <artikel><kop><label>Artikel</label><nr>3:9</nr></kop>
                <al>Het bestuursorgaan vergewist zich ervan dat het onderzoek zorgvuldig is verricht.</al>
              </artikel>
            </afdeling>
          </hoofdstuk>
          <artikel><kop><label>Artikel</label><nr>1</nr></kop>
            <lid><lidnr>1</lidnr><al>Eerste lid.</al></lid>
            <lid><lidnr>2</lidnr><al>Tweede lid.</al></lid>
          </artikel>
        </wettekst>
      </wet-besluit>
    </toestand>"#;

    #[test]
    fn placement_is_carried_down_from_the_containers() {
        let arts = parse_toestand(XML).unwrap();
        let by: BTreeMap<_, _> = arts.iter().map(|a| (a.number.as_str(), a)).collect();

        let a39 = by["3:9"];
        assert_eq!(a39.placement.hoofdstuk.as_ref().unwrap().number, "3");
        assert_eq!(
            a39.placement.afdeling.as_ref().unwrap().heading.as_deref(),
            Some("Advisering")
        );
        // This is the whole point: an agent reading 3:9 can now see that it
        // sits in the division on advice, and therefore that the adviseur
        // definition of 3:5 scopes it.
        assert_eq!(
            a39.placement.path(),
            "Hoofdstuk 3 Besluiten > Afdeling 3.3 Advisering"
        );

        // An article outside every container has an empty placement.
        assert!(by["1"].placement.is_empty());
    }

    #[test]
    fn leden_keep_their_numbering() {
        let arts = parse_toestand(XML).unwrap();
        let a1 = arts.iter().find(|a| a.number == "1").unwrap();
        assert_eq!(a1.text, "1. Eerste lid. 2. Tweede lid.");

        // A single-lid article carries no number, matching the corpus
        // convention. Numbering it would report formatting as drift.
        let a39 = arts.iter().find(|a| a.number == "3:5").unwrap();
        assert!(
            a39.text.starts_with("In deze afdeling"),
            "single lid must not be numbered: {:?}",
            a39.text
        );
    }

    #[test]
    fn an_article_the_source_does_not_have_is_fabricated() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "articles:\n  - number: '1'\n    text: 1. Eerste lid. 2. Tweede lid.\n  - number: 1a\n    text: verzonnen\n",
        )
        .unwrap();
        let report = verify(&corpus, &official);
        assert_eq!(report.verdicts["1a"], Verdict::Fabricated);
        assert!(!report.passes());
    }

    #[test]
    fn an_article_the_corpus_lacks_is_missing() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "articles:\n  - number: '1'\n    text: 1. Eerste lid. 2. Tweede lid.\n",
        )
        .unwrap();
        let report = verify(&corpus, &official);
        assert_eq!(report.verdicts["3:9"], Verdict::Missing);
    }

    #[test]
    fn drift_names_the_point_of_divergence() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            "articles:\n  - number: '1'\n    text: 1. Eerste lid. 2. Een heel ander tweede lid.\n",
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let Verdict::Drift { detail } = &report.verdicts["1"] else {
            panic!("expected drift, got {:?}", report.verdicts["1"]);
        };
        assert!(detail.contains("Een heel ander"), "{detail}");
    }

    #[test]
    fn rewrite_replaces_the_text_and_keeps_the_translation() {
        let official = parse_toestand(XML).unwrap();
        let corpus: serde_yaml_ng::Value = serde_yaml_ng::from_str(
            r#"
bwb_id: BWBR0000001
url: https://wetten.overheid.nl/BWBR0000001
articles:
  - number: '1'
    text: iets heel anders
    url: u
    machine_readable: {a: 1}
  - number: 1a
    text: verzonnen artikel
    url: u
"#,
        )
        .unwrap();
        let report = verify(&corpus, &official);
        let (doc, sidecar) = rewrite(&corpus, &official, &report);

        let arts = doc.get("articles").unwrap().as_sequence().unwrap();
        let numbers: Vec<&str> = arts
            .iter()
            .filter_map(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str))
            .collect();
        // The fabricated article is gone and the ones the law has are there.
        assert!(!numbers.contains(&"1a"), "{numbers:?}");
        assert!(numbers.contains(&"3:9"), "{numbers:?}");

        let a1 = arts
            .iter()
            .find(|a| a.get("number").and_then(serde_yaml_ng::Value::as_str) == Some("1"))
            .unwrap();
        assert_eq!(
            a1.get("text").and_then(serde_yaml_ng::Value::as_str),
            Some("1. Eerste lid. 2. Tweede lid.")
        );
        // Existing work is carried over rather than silently discarded; the
        // checks that follow decide whether it still holds.
        assert!(a1.get("machine_readable").is_some());

        assert_eq!(
            sidecar.articles["3:9"].path,
            "Hoofdstuk 3 Besluiten > Afdeling 3.3 Advisering"
        );
        assert_eq!(sidecar.articles["1"].verdict, "text replaced (had drifted)");
    }

    #[test]
    fn toestand_url_matches_the_repository_layout() {
        assert_eq!(
            toestand_url("BWBR0018451", "2025-01-01"),
            "https://repository.officiele-overheidspublicaties.nl/bwb/BWBR0018451/2025-01-01_0/xml/BWBR0018451_2025-01-01_0.xml"
        );
    }
}
