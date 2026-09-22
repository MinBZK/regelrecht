//! Nep-eHerkenning: inloggen met een KvK-nummer, de naam van de gemachtigde
//! en een machtiging. Er is geen register en geen databasecontrole; wie het
//! formulier invult, is voor deze PoC de gemachtigde.

use serde::{Deserialize, Serialize};
use serde_json::{json, Value};

/// De enige machtiging die deze PoC kent.
pub const MACHTIGING_VOLLEDIG: &str = "volledig";

/// Wat de login meegeeft.
#[derive(Debug, Clone, Deserialize)]
pub struct Login {
    pub kvk: String,
    pub persoon: String,
    pub machtiging: String,
}

/// Een ingelogde gemachtigde.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sessie {
    pub kvk: String,
    pub persoon: String,
    pub machtiging: String,
}

impl Login {
    /// KvK-nummer van acht cijfers, een naam, en machtiging `volledig`.
    pub fn valideer(self) -> Result<Sessie, String> {
        let kvk = self.kvk.trim();
        if kvk.len() != 8 || !kvk.bytes().all(|b| b.is_ascii_digit()) {
            return Err("een KvK-nummer heeft acht cijfers".into());
        }
        let persoon = self.persoon.trim();
        if persoon.is_empty() {
            return Err("de naam van de gemachtigde ontbreekt".into());
        }
        if self.machtiging != MACHTIGING_VOLLEDIG {
            return Err(format!(
                "alleen machtiging '{MACHTIGING_VOLLEDIG}' wordt geaccepteerd"
            ));
        }
        Ok(Sessie {
            kvk: kvk.to_string(),
            persoon: persoon.to_string(),
            machtiging: self.machtiging,
        })
    }
}

impl Sessie {
    /// Het ontvangstkanaal zoals een stroom het leest: `$intake.kanaal` en
    /// `$intake.eherkenning.*`.
    pub fn intake(&self, kanaal: &str) -> Value {
        json!({
            "kanaal": kanaal,
            "eherkenning": {"kvk": self.kvk, "persoon": self.persoon, "machtiging": self.machtiging},
        })
    }
}

/// De paden onder `$intake` die dit kanaal levert.
pub const INTAKE_PADEN: &[&str] = &[
    "kanaal",
    "eherkenning.kvk",
    "eherkenning.persoon",
    "eherkenning.machtiging",
];

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn login(kvk: &str, persoon: &str, machtiging: &str) -> Result<Sessie, String> {
        Login {
            kvk: kvk.into(),
            persoon: persoon.into(),
            machtiging: machtiging.into(),
        }
        .valideer()
    }

    #[test]
    fn geldige_login() {
        let s = login(" 12345678 ", "A. Tester", "volledig").unwrap();
        assert_eq!(s.kvk, "12345678");
        assert_eq!(s.intake("portaal")["eherkenning"]["persoon"], "A. Tester");
    }

    #[test]
    fn ongeldige_logins() {
        assert!(login("1234567", "A", "volledig").is_err());
        assert!(login("1234567a", "A", "volledig").is_err());
        assert!(login("12345678", "  ", "volledig").is_err());
        assert!(login("12345678", "A", "beperkt").is_err());
    }
}
