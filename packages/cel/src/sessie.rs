//! Sessies van de rollen van een proces (`rollen` in `proces.yaml`): de
//! aanvrager logt in met nep-eHerkenning ([`crate::eherkenning`]), de
//! behandelaar met een nagebootste medewerkerslogin ([`Medewerker`]). Een
//! cookie per proces,
//! een gebruiker per sessie: wie als de andere rol inlogt, vervangt de sessie.
//! Er is geen register en geen databasecontrole.

use std::collections::HashMap;
use std::sync::Mutex;

use axum::http::HeaderMap;
use serde::{Deserialize, Serialize};

use crate::eherkenning::Sessie;

/// Naam van de sessiecookie.
pub const COOKIE: &str = "cel_sessie";

/// Een medewerker van de cel, alleen met een naam. Geen register: wie een
/// naam invult, is voor deze PoC medewerker.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Medewerker {
    pub naam: String,
}

impl Medewerker {
    /// Een naam, niet leeg.
    pub fn valideer(self) -> Result<Self, String> {
        let naam = self.naam.trim();
        if naam.is_empty() {
            return Err("de naam van de medewerker ontbreekt".into());
        }
        Ok(Self {
            naam: naam.to_string(),
        })
    }
}

/// Wie er is ingelogd, in welke rol.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Gebruiker {
    Aanvrager(Sessie),
    Behandelaar(Medewerker),
}

impl Gebruiker {
    pub fn aanvrager(&self) -> Option<&Sessie> {
        match self {
            Gebruiker::Aanvrager(s) => Some(s),
            Gebruiker::Behandelaar(_) => None,
        }
    }

    pub fn behandelaar(&self) -> Option<&Medewerker> {
        match self {
            Gebruiker::Behandelaar(m) => Some(m),
            Gebruiker::Aanvrager(_) => None,
        }
    }
}

/// Sessies in het geheugen: een herstart logt iedereen uit.
#[derive(Default)]
pub struct Sessies(Mutex<HashMap<String, Gebruiker>>);

impl Sessies {
    pub fn nieuw(&self, gebruiker: Gebruiker) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        if let Ok(mut m) = self.0.lock() {
            m.insert(token.clone(), gebruiker);
        }
        token
    }

    pub fn zoek(&self, headers: &HeaderMap) -> Option<Gebruiker> {
        let token = token(headers)?;
        self.0.lock().ok()?.get(&token).cloned()
    }

    pub fn verwijder(&self, headers: &HeaderMap) {
        if let (Some(token), Ok(mut m)) = (token(headers), self.0.lock()) {
            m.remove(&token);
        }
    }
}

fn token(headers: &HeaderMap) -> Option<String> {
    headers
        .get_all(axum::http::header::COOKIE)
        .iter()
        .filter_map(|v| v.to_str().ok())
        .flat_map(|v| v.split(';'))
        .find_map(|deel| {
            let (naam, waarde) = deel.trim().split_once('=')?;
            (naam == COOKIE).then(|| waarde.to_string())
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::eherkenning::Login;

    #[test]
    fn sessie_via_cookie() {
        let sessies = Sessies::default();
        let sessie = Login {
            kvk: "12345678".into(),
            persoon: "A".into(),
        }
        .valideer()
        .unwrap();
        let token = sessies.nieuw(Gebruiker::Aanvrager(sessie));
        let mut h = HeaderMap::new();
        h.insert(
            axum::http::header::COOKIE,
            format!("ander=1; {COOKIE}={token}").parse().unwrap(),
        );
        assert_eq!(
            sessies.zoek(&h).unwrap().aanvrager().unwrap().kvk,
            "12345678"
        );
        assert!(sessies.zoek(&h).unwrap().behandelaar().is_none());
        sessies.verwijder(&h);
        assert!(sessies.zoek(&h).is_none());
        assert!(sessies.zoek(&HeaderMap::new()).is_none());
    }

    #[test]
    fn medewerker_heeft_een_naam() {
        let m = Medewerker {
            naam: " B. Behandelaar ".into(),
        }
        .valideer()
        .unwrap();
        assert_eq!(m.naam, "B. Behandelaar");
        assert!(Medewerker { naam: "  ".into() }.valideer().is_err());
    }
}
