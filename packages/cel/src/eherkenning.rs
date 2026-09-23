//! Nep-eHerkenning: inloggen met een KvK-nummer en de naam van de persoon.
//! Wie namens de organisatie mag handelen, zegt het handelsregister, niet de
//! login.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// Wat de login meegeeft. Onbekende velden, zoals een oude `machtiging`,
/// worden genegeerd.
#[derive(Debug, Clone, Deserialize)]
pub struct Login {
    pub kvk: String,
    pub persoon: String,
}

/// Een ingelogde persoon, namens een organisatie.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sessie {
    pub kvk: String,
    pub persoon: String,
}

impl Login {
    /// KvK-nummer van acht cijfers en een naam.
    pub fn valideer(self) -> Result<Sessie, String> {
        let kvk = self.kvk.trim();
        if kvk.len() != 8 || !kvk.bytes().all(|b| b.is_ascii_digit()) {
            return Err("een KvK-nummer heeft acht cijfers".into());
        }
        let persoon = self.persoon.trim();
        if persoon.is_empty() {
            return Err("de naam van de persoon ontbreekt".into());
        }
        Ok(Sessie {
            kvk: kvk.to_string(),
            persoon: persoon.to_string(),
        })
    }
}

impl Sessie {
    /// Het ontvangstkanaal zoals een stroom het leest: `$intake.kanaal` en
    /// `$intake.eherkenning.*`.
    pub fn intake(&self, kanaal: &str) -> Value {
        json!({
            "kanaal": kanaal,
            "eherkenning": {"kvk": self.kvk, "persoon": self.persoon},
        })
    }
}

/// De paden onder `$intake` die dit kanaal levert.
pub const INTAKE_PADEN: &[&str] = &["kanaal", "eherkenning.kvk", "eherkenning.persoon"];

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn login(kvk: &str, persoon: &str) -> Result<Sessie, String> {
        Login {
            kvk: kvk.into(),
            persoon: persoon.into(),
        }
        .valideer()
    }

    #[test]
    fn geldige_login() {
        let s = login(" 12345678 ", "A. Tester").unwrap();
        assert_eq!(s.kvk, "12345678");
        assert_eq!(s.intake("portaal")["eherkenning"]["persoon"], "A. Tester");
    }

    #[test]
    fn ongeldige_logins() {
        assert!(login("1234567", "A").is_err());
        assert!(login("1234567a", "A").is_err());
        assert!(login("12345678", "  ").is_err());
    }

    #[test]
    fn de_login_zegt_wie_en_voor_welke_organisatie() {
        let s = login("12345678", "A. Tester").unwrap();
        assert_eq!(
            s.intake("portaal")["eherkenning"],
            serde_json::json!({"kvk": "12345678", "persoon": "A. Tester"})
        );
    }
}
