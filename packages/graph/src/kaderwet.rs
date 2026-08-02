//! Kaderwetten: aangewezen, niet opgespoord.
//!
//! Being a framework law is a legal qualification, not a property of a degree
//! distribution. The Awb is a framework law because it declares itself
//! applicable to virtually every besluit, not because 867 laws happen to cite
//! it. Those two things correlate and they are not the same thing, and a
//! threshold over the citation count cannot tell them apart: measured on the
//! harvested corpus, 20% of the corpus qualifies exactly one law and 5%
//! qualifies six, while the seven immediately below the cut differ in no
//! meaningful way from the six above it. Worse, a threshold moves on its own as
//! the corpus grows, so the same law silently changes character between two
//! harvests.
//!
//! So there is no detection here. There are two ways a law becomes a framework
//! law, and both of them are statements rather than measurements:
//!
//! 1. **It is designated.** Someone with the authority to say so puts it on the
//!    list, with the card that RFC-026 calls for: what its scope is, how you
//!    recognise that it applies, and what has to be taken along when it does.
//!    The list is data with an owner, it lives next to the corpus, and it is
//!    read here.
//! 2. **The corpus says so.** An [`EdgeType::Applicability`] edge is a law
//!    declaring itself applicable to another, which is the relation that makes
//!    a framework law, whether there is one of them or a thousand.
//!
//! The second route produces nothing today, and that is worth being plain
//! about: the harvested `references` block records that an article points at
//! another law and not what the pointer means, so no applicability edge can be
//! derived from it. Until an enriched corpus states it, the qualification comes
//! from the list alone. What is not on the list is missed, and that is a
//! limitation you can see and fix rather than one that hides inside a constant.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// One kaderwetkaart: the law, and what a reader of the graph has to know about
/// it.
///
/// Only `bwb_id` or `law_id` is needed to make the designation work. The prose
/// fields are what make the designation reviewable, and leaving them empty is
/// how a card announces that nobody has done that work yet.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Kaderwetkaart {
    /// BWB identifier. Preferred, because it survives a renamed slug.
    #[serde(default)]
    pub bwb_id: Option<String>,
    /// Corpus `$id` of the law, for regulations without a BWB number.
    #[serde(default)]
    pub law_id: Option<String>,
    /// What the law is called, for the report and the legend.
    #[serde(default)]
    pub naam: Option<String>,
    /// Which decisions, bodies or procedures the law reaches.
    #[serde(default)]
    pub toepassingsbereik: Option<String>,
    /// How you recognise, in another law, that this one is in play.
    #[serde(default)]
    pub herkenningspatroon: Option<String>,
    /// What has to be taken along once it is in play.
    #[serde(default)]
    pub meenemen: Vec<String>,
    /// Who designated it. A designation without a name behind it is an opinion.
    #[serde(default)]
    pub aangewezen_door: Option<String>,
    /// When, so a stale card is visible as stale.
    #[serde(default)]
    pub datum: Option<String>,
}

/// The designation list as a whole.
#[derive(Debug, Clone, Default, Deserialize, Serialize)]
pub struct Kaderwetten {
    #[serde(default)]
    pub version: u32,
    /// Who owns the list. Not decoration: the point of designating rather than
    /// detecting is that there is someone to ask.
    #[serde(default)]
    pub beheerder: Option<String>,
    #[serde(default)]
    pub kaderwetten: Vec<Kaderwetkaart>,
}

/// Where the list is looked for when no path is given: next to the corpus it
/// describes, because that is what it is about and who owns it follows the
/// corpus.
pub const DEFAULT_FILE: &str = "kaderwetten.yaml";

#[derive(Debug)]
pub enum LoadError {
    Missing(PathBuf),
    Unreadable(PathBuf, String),
}

impl std::fmt::Display for LoadError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            LoadError::Missing(path) => {
                write!(f, "geen kaderwetlijst op {}", path.display())
            }
            LoadError::Unreadable(path, error) => {
                write!(f, "kaderwetlijst {} niet te lezen: {error}", path.display())
            }
        }
    }
}

impl Kaderwetten {
    /// Read a list from a file.
    ///
    /// A missing file is reported rather than silently treated as an empty
    /// list. An empty list is a legitimate state and so is a missing one, but
    /// they mean different things and the builder says which it got.
    pub fn load(path: &Path) -> Result<Kaderwetten, LoadError> {
        if !path.exists() {
            return Err(LoadError::Missing(path.to_path_buf()));
        }
        let text = std::fs::read_to_string(path)
            .map_err(|e| LoadError::Unreadable(path.to_path_buf(), e.to_string()))?;
        serde_yaml_ng::from_str(&text)
            .map_err(|e| LoadError::Unreadable(path.to_path_buf(), e.to_string()))
    }

    /// Is this law designated? Matches on BWB identifier first, then on the
    /// corpus id.
    pub fn designates(&self, bwb_id: Option<&str>, law_id: &str) -> bool {
        self.kaderwetten.iter().any(|kaart| {
            match (kaart.bwb_id.as_deref(), bwb_id) {
                (Some(a), Some(b)) if a == b => return true,
                _ => {}
            }
            // A versioned node carries `slug@date`; the card names the law.
            let base = law_id.split('@').next().unwrap_or(law_id);
            kaart.law_id.as_deref() == Some(base)
        })
    }

    pub fn len(&self) -> usize {
        self.kaderwetten.len()
    }

    pub fn is_empty(&self) -> bool {
        self.kaderwetten.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SAMPLE: &str = r#"
version: 1
beheerder: nog niemand
kaderwetten:
  - bwb_id: BWBR0005537
    law_id: algemene_wet_bestuursrecht
    naam: Algemene wet bestuursrecht
    toepassingsbereik: elk besluit van een bestuursorgaan
    herkenningspatroon: de wet spreekt van een besluit, beschikking of bezwaar
    meenemen:
      - de bezwaar- en beroepsprocedure
    aangewezen_door: nog niemand
    datum: '2026-08-02'
"#;

    #[test]
    fn designation_matches_on_bwb_and_on_slug() {
        let list: Kaderwetten = serde_yaml_ng::from_str(SAMPLE).expect("parse");
        assert_eq!(list.len(), 1);
        assert!(list.designates(Some("BWBR0005537"), "wat_dan_ook"));
        assert!(list.designates(None, "algemene_wet_bestuursrecht"));
        assert!(!list.designates(Some("BWBR0000001"), "participatiewet"));
    }

    #[test]
    fn a_versioned_node_is_still_the_same_law() {
        let list: Kaderwetten = serde_yaml_ng::from_str(SAMPLE).expect("parse");
        assert!(list.designates(None, "algemene_wet_bestuursrecht@1994-01-01"));
    }

    #[test]
    fn a_missing_file_is_reported_and_not_taken_for_an_empty_list() {
        let err = Kaderwetten::load(Path::new("/geen/pad/kaderwetten.yaml"));
        assert!(matches!(err, Err(LoadError::Missing(_))));
    }

    #[test]
    fn an_empty_list_is_a_valid_list() {
        let list: Kaderwetten =
            serde_yaml_ng::from_str("version: 1\nkaderwetten: []\n").expect("parse");
        assert!(list.is_empty());
        assert!(!list.designates(Some("BWBR0005537"), "algemene_wet_bestuursrecht"));
    }
}
