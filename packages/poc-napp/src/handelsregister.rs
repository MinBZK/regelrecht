//! MOCK van de KvK Handelsregister-raadpleging.
//!
//! Bij registratie van een aanduiding eist de Kieswet (G-1) bewijs van
//! inschrijving in het Handelsregister: een vereniging met volledige
//! rechtsbevoegdheid (notariële statuten). Het Handelsregister kent voor
//! politieke organisaties SBI-code 94.92. In het echt zou de Napp de KvK-API
//! raadplegen; deze module simuleert die raadpleging deterministisch zodat
//! de claim-flow demonstreerbaar is zonder externe afhankelijkheid.
//!
//! Het resultaat (`HrToets`) is serde-serialiseerbaar en wordt bij de claim
//! opgeslagen, zodat de beoordelaar precies ziet waarop de toets is gebaseerd.

use serde::{Deserialize, Serialize};

/// SBI-code voor politieke organisaties in het Handelsregister.
pub const SBI_POLITIEKE_ORGANISATIES: &str = "94.92";

/// Result of the (mocked) Handelsregister check, stored with the claim.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HrToets {
    /// The KvK number that was looked up.
    pub kvk_nummer: String,
    /// Whether the number resolves to a registration.
    pub gevonden: bool,
    pub statutaire_naam: Option<String>,
    pub rechtsvorm: Option<String>,
    pub sbi_code: Option<String>,
    pub sbi_omschrijving: Option<String>,
    /// Normalized comparison between the statutaire naam and the claimed
    /// aanduiding.
    pub naam_match: bool,
}

/// Lowercase and strip everything but letters and digits, so that
/// "Vereniging Leefbaar Capelle" matches "Leefbaar Capelle".
fn normaliseer(s: &str) -> String {
    s.chars()
        .filter(|c| c.is_alphanumeric())
        .flat_map(char::to_lowercase)
        .collect()
}

/// MOCK: consult the Handelsregister for a KvK number and compare the
/// registration against the claimed aanduiding. Deterministic: a valid
/// 8-digit number is always found as a vereniging met volledige
/// rechtsbevoegdheid with SBI 94.92 and statutaire naam
/// "Vereniging {aanduiding}".
pub fn raadpleeg(kvk: &str, aanduiding: &str) -> HrToets {
    let kvk = kvk.trim();
    let geldig = kvk.len() == 8 && kvk.chars().all(|c| c.is_ascii_digit());
    if !geldig {
        return HrToets {
            kvk_nummer: kvk.to_string(),
            gevonden: false,
            statutaire_naam: None,
            rechtsvorm: None,
            sbi_code: None,
            sbi_omschrijving: None,
            naam_match: false,
        };
    }
    let statutaire_naam = format!("Vereniging {aanduiding}");
    let naam_match = normaliseer(&statutaire_naam).contains(&normaliseer(aanduiding));
    HrToets {
        kvk_nummer: kvk.to_string(),
        gevonden: true,
        statutaire_naam: Some(statutaire_naam),
        rechtsvorm: Some("Vereniging met volledige rechtsbevoegdheid".to_string()),
        sbi_code: Some(SBI_POLITIEKE_ORGANISATIES.to_string()),
        sbi_omschrijving: Some("Politieke organisaties".to_string()),
        naam_match,
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used, clippy::panic)]
mod tests {
    use super::*;

    #[test]
    fn geldig_nummer_wordt_gevonden_met_sbi_9492() {
        let toets = raadpleeg("90000001", "Leefbaar Capelle");
        assert!(toets.gevonden);
        assert_eq!(
            toets.statutaire_naam.as_deref(),
            Some("Vereniging Leefbaar Capelle")
        );
        assert_eq!(
            toets.rechtsvorm.as_deref(),
            Some("Vereniging met volledige rechtsbevoegdheid")
        );
        assert_eq!(toets.sbi_code.as_deref(), Some("94.92"));
        assert!(toets.naam_match);
    }

    #[test]
    fn ongeldig_nummer_wordt_niet_gevonden() {
        for kvk in ["1234", "abcdefgh", "123456789", ""] {
            let toets = raadpleeg(kvk, "Partij");
            assert!(!toets.gevonden, "{kvk} hoort niet gevonden te worden");
            assert!(toets.statutaire_naam.is_none());
            assert!(!toets.naam_match);
        }
    }

    #[test]
    fn raadpleging_is_deterministisch_en_serialiseerbaar() {
        let a = raadpleeg("12345678", "EVB (Echt voor Barendrecht)");
        let b = raadpleeg("12345678", "EVB (Echt voor Barendrecht)");
        assert_eq!(a, b);
        let json = serde_json::to_string(&a).expect("serialiseerbaar");
        let terug: HrToets = serde_json::from_str(&json).expect("deserialiseerbaar");
        assert_eq!(a, terug);
    }
}
