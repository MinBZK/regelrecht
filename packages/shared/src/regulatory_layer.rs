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

    /// Parse from WTI "soort-regeling" text.
    ///
    /// Returns `(layer, warning)` where the warning is `Some` for ambiguous mappings
    /// or unknown types. Direct matches produce no warning.
    #[must_use]
    pub fn from_soort_regeling(text: &str) -> (Self, Option<String>) {
        match text.to_lowercase().as_str() {
            "grondwet" => (Self::Grondwet, None),
            "wet" => (Self::Wet, None),
            "amvb" | "algemene maatregel van bestuur" => (Self::Amvb, None),
            "ministeriele regeling" | "ministeriële regeling" => {
                (Self::MinisterieleRegeling, None)
            }
            "beleidsregel" => (Self::Beleidsregel, None),
            "eu-verordening" => (Self::EuVerordening, None),
            "eu-richtlijn" => (Self::EuRichtlijn, None),
            "verdrag" => (Self::Verdrag, None),
            "uitvoeringsbeleid" => (Self::Uitvoeringsbeleid, None),
            "gemeentelijke verordening" => (Self::GemeentelijkeVerordening, None),
            "provinciale verordening" => (Self::ProvincialeVerordening, None),
            // Ambiguous mappings - produce warnings
            "koninklijk besluit" | "kb" => (
                Self::Amvb,
                Some(format!(
                    "Mapped soort-regeling '{text}' to AMVB (closest schema match)"
                )),
            ),
            "regeling" => (
                Self::MinisterieleRegeling,
                Some(format!(
                    "Mapped soort-regeling '{text}' to MINISTERIELE_REGELING (closest schema match)"
                )),
            ),
            "verordening" => (
                Self::GemeentelijkeVerordening,
                Some(format!(
                    "Mapped soort-regeling '{text}' to GEMEENTELIJKE_VERORDENING (closest schema match)"
                )),
            ),
            unknown => {
                tracing::warn!(
                    soort_regeling = %unknown,
                    "Unknown soort-regeling type, defaulting to WET"
                );
                (
                    Self::Wet,
                    Some(format!(
                        "Unknown soort-regeling '{unknown}', defaulting to WET"
                    )),
                )
            }
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
    fn test_from_soort_regeling() {
        assert_eq!(
            RegulatoryLayer::from_soort_regeling("wet"),
            (RegulatoryLayer::Wet, None)
        );
        assert_eq!(
            RegulatoryLayer::from_soort_regeling("WET"),
            (RegulatoryLayer::Wet, None)
        );
        assert_eq!(
            RegulatoryLayer::from_soort_regeling("grondwet"),
            (RegulatoryLayer::Grondwet, None)
        );
        assert_eq!(
            RegulatoryLayer::from_soort_regeling("eu-verordening"),
            (RegulatoryLayer::EuVerordening, None)
        );

        // Ambiguous mappings
        let (layer, warning) = RegulatoryLayer::from_soort_regeling("koninklijk besluit");
        assert_eq!(layer, RegulatoryLayer::Amvb);
        assert!(warning.is_some());

        // Unknown defaults to Wet
        let (layer, warning) = RegulatoryLayer::from_soort_regeling("unknown");
        assert_eq!(layer, RegulatoryLayer::Wet);
        assert!(warning.is_some());
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
