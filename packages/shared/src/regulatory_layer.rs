//! Canonical regulatory layer types for Dutch law.
//!
//! This enum is the single source of truth for regulatory layer types,
//! shared across all crates in the workspace.

use serde::{Deserialize, Serialize};

/// Types of regulatory documents in Dutch law.
///
/// Aligned with schema v0.3.1 regulatory_layer enum.
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
}

impl RegulatoryLayer {
    /// Get the string value for YAML/JSON output.
    #[must_use]
    pub fn as_str(&self) -> &'static str {
        match self {
            Self::Grondwet => "GRONDWET",
            Self::Wet => "WET",
            Self::Amvb => "AMVB",
            Self::MinisterieleRegeling => "MINISTERIELE_REGELING",
            Self::Beleidsregel => "BELEIDSREGEL",
            Self::EuVerordening => "EU_VERORDENING",
            Self::EuRichtlijn => "EU_RICHTLIJN",
            Self::Verdrag => "VERDRAG",
            Self::Uitvoeringsbeleid => "UITVOERINGSBELEID",
            Self::GemeentelijkeVerordening => "GEMEENTELIJKE_VERORDENING",
            Self::ProvincialeVerordening => "PROVINCIALE_VERORDENING",
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
            Self::EuVerordening => "eu_verordening",
            Self::EuRichtlijn => "eu_richtlijn",
            Self::Verdrag => "verdrag",
            Self::Uitvoeringsbeleid => "uitvoeringsbeleid",
            Self::GemeentelijkeVerordening => "gemeentelijke_verordening",
            Self::ProvincialeVerordening => "provinciale_verordening",
        }
    }

}

#[cfg(test)]
mod tests {
    use super::*;

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
    fn test_default() {
        assert_eq!(RegulatoryLayer::default(), RegulatoryLayer::Wet);
    }
}
