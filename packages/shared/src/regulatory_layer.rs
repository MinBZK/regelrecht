//! Canonical regulatory layer types for Dutch law.
//!
//! This enum is the single source of truth for regulatory layer types,
//! shared across all crates in the workspace.

use serde::{Deserialize, Serialize};

/// Types of regulatory documents in Dutch law.
///
/// Aligned with schema v0.5.2 regulatory_layer enum.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, Default)]
pub enum RegulatoryLayer {
    /// Constitutional law (Grondwet).
    #[serde(rename = "GRONDWET")]
    Grondwet,

    /// Formal law (wet).
    #[serde(rename = "WET")]
    #[default]
    Wet,

    /// General administrative measure (Algemene Maatregel van Bestuur).
    #[serde(rename = "AMVB")]
    Amvb,

    /// Ministerial regulation (Ministeriële regeling).
    #[serde(rename = "MINISTERIELE_REGELING")]
    MinisterieleRegeling,

    /// Policy rule (Beleidsregel).
    #[serde(rename = "BELEIDSREGEL")]
    Beleidsregel,

    /// Royal decree (Koninklijk Besluit).
    #[serde(rename = "KONINKLIJK_BESLUIT")]
    KoninklijkBesluit,

    /// EU regulation (EU-verordening).
    #[serde(rename = "EU_VERORDENING")]
    EuVerordening,

    /// EU directive (EU-richtlijn).
    #[serde(rename = "EU_RICHTLIJN")]
    EuRichtlijn,

    /// International treaty (Verdrag).
    #[serde(rename = "VERDRAG")]
    Verdrag,

    /// Implementation policy (Uitvoeringsbeleid).
    #[serde(rename = "UITVOERINGSBELEID")]
    Uitvoeringsbeleid,

    /// Municipal ordinance (Gemeentelijke verordening).
    #[serde(rename = "GEMEENTELIJKE_VERORDENING")]
    GemeentelijkeVerordening,

    /// Provincial ordinance (Provinciale verordening).
    #[serde(rename = "PROVINCIALE_VERORDENING")]
    ProvincialeVerordening,

    /// Water board ordinance (Waterschapsverordening).
    #[serde(rename = "WATERSCHAPS_VERORDENING")]
    WaterschapsVerordening,
}

impl RegulatoryLayer {
    /// Every variant, for callers that must enumerate the whole set rather
    /// than match on one — the document-conversion prompt in
    /// `regelrecht-pipeline` is the live case. Add a variant above and add it
    /// here too; `all_variants_is_exhaustive` below fails otherwise.
    pub const ALL_VARIANTS: &'static [Self] = &[
        Self::Grondwet,
        Self::Wet,
        Self::Amvb,
        Self::MinisterieleRegeling,
        Self::Beleidsregel,
        Self::KoninklijkBesluit,
        Self::EuVerordening,
        Self::EuRichtlijn,
        Self::Verdrag,
        Self::Uitvoeringsbeleid,
        Self::GemeentelijkeVerordening,
        Self::ProvincialeVerordening,
        Self::WaterschapsVerordening,
    ];

    /// Get the string value for YAML/JSON output.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Grondwet => "GRONDWET",
            Self::Wet => "WET",
            Self::Amvb => "AMVB",
            Self::MinisterieleRegeling => "MINISTERIELE_REGELING",
            Self::Beleidsregel => "BELEIDSREGEL",
            Self::KoninklijkBesluit => "KONINKLIJK_BESLUIT",
            Self::EuVerordening => "EU_VERORDENING",
            Self::EuRichtlijn => "EU_RICHTLIJN",
            Self::Verdrag => "VERDRAG",
            Self::Uitvoeringsbeleid => "UITVOERINGSBELEID",
            Self::GemeentelijkeVerordening => "GEMEENTELIJKE_VERORDENING",
            Self::ProvincialeVerordening => "PROVINCIALE_VERORDENING",
            Self::WaterschapsVerordening => "WATERSCHAPS_VERORDENING",
        }
    }

    /// Get the directory name for file output.
    #[must_use]
    pub fn as_dir_name(&self) -> &'static str {
        match self {
            Self::Grondwet => "grondwet",
            Self::Wet => "wet",
            Self::Amvb => "amvb",
            Self::MinisterieleRegeling => "ministeriele_regeling",
            Self::Beleidsregel => "beleidsregel",
            Self::KoninklijkBesluit => "koninklijk_besluit",
            Self::EuVerordening => "eu_verordening",
            Self::EuRichtlijn => "eu_richtlijn",
            Self::Verdrag => "verdrag",
            Self::Uitvoeringsbeleid => "uitvoeringsbeleid",
            Self::GemeentelijkeVerordening => "gemeentelijke_verordening",
            Self::ProvincialeVerordening => "provinciale_verordening",
            Self::WaterschapsVerordening => "waterschaps_verordening",
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The enum declaration is the ground truth: every variant carries a
    /// `#[serde(rename = …)]`, so counting those in this file's own source
    /// catches a variant that was added to the enum and forgotten in
    /// `ALL_VARIANTS`. A `match` cannot do that — the compiler forces an arm,
    /// not a list entry.
    #[test]
    fn all_variants_lists_every_variant() {
        let source = include_str!("regulatory_layer.rs");
        let body = source
            .split_once("pub enum RegulatoryLayer {")
            .expect("enum declaration must be present")
            .1
            .split_once("\n}")
            .expect("enum declaration must be closed")
            .0;
        let declared = body.matches("#[serde(rename = ").count();

        assert_eq!(
            RegulatoryLayer::ALL_VARIANTS.len(),
            declared,
            "the enum declares {declared} variants, ALL_VARIANTS lists {}",
            RegulatoryLayer::ALL_VARIANTS.len()
        );

        let mut names: Vec<&str> = RegulatoryLayer::ALL_VARIANTS
            .iter()
            .map(RegulatoryLayer::as_str)
            .collect();
        names.sort_unstable();
        let unique = names.len();
        names.dedup();
        assert_eq!(names.len(), unique, "ALL_VARIANTS contains a duplicate");
    }

    #[test]
    fn test_as_str() {
        assert_eq!(RegulatoryLayer::Wet.as_str(), "WET");
        assert_eq!(RegulatoryLayer::Amvb.as_str(), "AMVB");
        assert_eq!(
            RegulatoryLayer::MinisterieleRegeling.as_str(),
            "MINISTERIELE_REGELING"
        );
        assert_eq!(
            RegulatoryLayer::ProvincialeVerordening.as_str(),
            "PROVINCIALE_VERORDENING"
        );
    }

    #[test]
    fn test_as_dir_name() {
        assert_eq!(RegulatoryLayer::Wet.as_dir_name(), "wet");
        assert_eq!(
            RegulatoryLayer::MinisterieleRegeling.as_dir_name(),
            "ministeriele_regeling"
        );
    }

    #[test]
    fn test_serialization() {
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::Wet).unwrap(),
            "\"WET\""
        );
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::MinisterieleRegeling).unwrap(),
            "\"MINISTERIELE_REGELING\""
        );
    }

    #[test]
    fn test_deserialization() {
        let layer: RegulatoryLayer = serde_json::from_str("\"WET\"").unwrap();
        assert_eq!(layer, RegulatoryLayer::Wet);
        let layer: RegulatoryLayer = serde_json::from_str("\"MINISTERIELE_REGELING\"").unwrap();
        assert_eq!(layer, RegulatoryLayer::MinisterieleRegeling);
    }

    #[test]
    fn test_koninklijk_besluit_serialization() {
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::KoninklijkBesluit).unwrap(),
            "\"KONINKLIJK_BESLUIT\""
        );
        let layer: RegulatoryLayer = serde_json::from_str("\"KONINKLIJK_BESLUIT\"").unwrap();
        assert_eq!(layer, RegulatoryLayer::KoninklijkBesluit);
        assert_eq!(
            RegulatoryLayer::KoninklijkBesluit.as_str(),
            "KONINKLIJK_BESLUIT"
        );
        assert_eq!(
            RegulatoryLayer::KoninklijkBesluit.as_dir_name(),
            "koninklijk_besluit"
        );
    }

    #[test]
    fn test_waterschaps_verordening_serialization() {
        assert_eq!(
            serde_json::to_string(&RegulatoryLayer::WaterschapsVerordening).unwrap(),
            "\"WATERSCHAPS_VERORDENING\""
        );
        let layer: RegulatoryLayer = serde_json::from_str("\"WATERSCHAPS_VERORDENING\"").unwrap();
        assert_eq!(layer, RegulatoryLayer::WaterschapsVerordening);
        assert_eq!(
            RegulatoryLayer::WaterschapsVerordening.as_str(),
            "WATERSCHAPS_VERORDENING"
        );
        assert_eq!(
            RegulatoryLayer::WaterschapsVerordening.as_dir_name(),
            "waterschaps_verordening"
        );
    }

    #[test]
    fn test_default() {
        assert_eq!(RegulatoryLayer::default(), RegulatoryLayer::Wet);
    }
}
