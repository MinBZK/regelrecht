//! Sessies van de rollen van een proces (`rollen` in `proces.yaml`): de
//! aanvrager logt in met nep-eHerkenning ([`crate::eherkenning`]), de
//! behandelaar met een nagebootste medewerkerslogin ([`Medewerker`]). Een
//! cookie per proces,
//! een gebruiker per sessie: wie als de andere rol inlogt, vervangt de sessie.
//! Er is geen register en geen databasecontrole.
//!
//! Een sessie vervalt na [`VERVAL`] zonder gebruik, en er zijn er hooguit
//! [`MAXIMUM`] tegelijk: wie dan inlogt, verdringt de langst ongebruikte. Zo
//! groeit het geheugen niet met elke login.

use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard, PoisonError};
use std::time::{Duration, Instant};

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

/// Hoe lang een sessie zonder gebruik geldig blijft.
pub const VERVAL: Duration = Duration::from_secs(8 * 60 * 60);

/// Hoeveel sessies er per proces tegelijk zijn.
pub const MAXIMUM: usize = 10_000;

/// Een sessie en wanneer ze voor het laatst gebruikt is.
struct Actief {
    gebruiker: Gebruiker,
    laatst: Instant,
}

/// Sessies in het geheugen: een herstart logt iedereen uit.
pub struct Sessies {
    sessies: Mutex<HashMap<String, Actief>>,
    verval: Duration,
    maximum: usize,
}

impl Default for Sessies {
    fn default() -> Self {
        Self::met(VERVAL, MAXIMUM)
    }
}

impl Sessies {
    /// Sessies met een eigen verval en maximum.
    pub fn met(verval: Duration, maximum: usize) -> Self {
        Self {
            sessies: Mutex::new(HashMap::new()),
            verval,
            maximum: maximum.max(1),
        }
    }

    /// De map; een draad die onder het slot paniekte, laat de sessies
    /// bruikbaar achter (er wordt nooit half geschreven).
    fn slot(&self) -> MutexGuard<'_, HashMap<String, Actief>> {
        self.sessies.lock().unwrap_or_else(PoisonError::into_inner)
    }

    pub fn nieuw(&self, gebruiker: Gebruiker) -> String {
        self.nieuw_op(gebruiker, Instant::now())
    }

    fn nieuw_op(&self, gebruiker: Gebruiker, nu: Instant) -> String {
        let token = uuid::Uuid::new_v4().to_string();
        let mut m = self.slot();
        m.retain(|_, s| nu.saturating_duration_since(s.laatst) < self.verval);
        while m.len() >= self.maximum {
            let Some(oudste) = m
                .iter()
                .min_by_key(|(_, s)| s.laatst)
                .map(|(t, _)| t.clone())
            else {
                break;
            };
            m.remove(&oudste);
        }
        m.insert(
            token.clone(),
            Actief {
                gebruiker,
                laatst: nu,
            },
        );
        token
    }

    pub fn zoek(&self, headers: &HeaderMap) -> Option<Gebruiker> {
        self.zoek_op(&token(headers)?, Instant::now())
    }

    /// De gebruiker van een sessie die nog geldt; elk gebruik verlengt haar.
    fn zoek_op(&self, token: &str, nu: Instant) -> Option<Gebruiker> {
        let mut m = self.slot();
        let s = m.get_mut(token)?;
        if nu.saturating_duration_since(s.laatst) >= self.verval {
            m.remove(token);
            return None;
        }
        s.laatst = nu;
        Some(s.gebruiker.clone())
    }

    pub fn verwijder(&self, headers: &HeaderMap) {
        if let Some(token) = token(headers) {
            self.slot().remove(&token);
        }
    }

    /// Hoeveel sessies er nu zijn, verlopen of niet.
    pub fn aantal(&self) -> usize {
        self.slot().len()
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

    fn medewerker(naam: &str) -> Gebruiker {
        Gebruiker::Behandelaar(Medewerker { naam: naam.into() })
    }

    #[test]
    fn een_sessie_vervalt_zonder_gebruik() {
        let sessies = Sessies::met(Duration::from_secs(60), 100);
        let t0 = Instant::now();
        let token = sessies.nieuw_op(medewerker("A"), t0);
        // Gebruik verlengt haar.
        assert!(sessies
            .zoek_op(&token, t0 + Duration::from_secs(50))
            .is_some());
        assert!(sessies
            .zoek_op(&token, t0 + Duration::from_secs(100))
            .is_some());
        // Daarna een minuut niets: weg, en uit de map.
        assert!(sessies
            .zoek_op(&token, t0 + Duration::from_secs(161))
            .is_none());
        assert_eq!(sessies.aantal(), 0);
    }

    #[test]
    fn verlopen_sessies_worden_bij_een_nieuwe_login_opgeruimd() {
        let sessies = Sessies::met(Duration::from_secs(60), 100);
        let t0 = Instant::now();
        for i in 0..10 {
            sessies.nieuw_op(medewerker(&format!("M{i}")), t0);
        }
        assert_eq!(sessies.aantal(), 10);
        sessies.nieuw_op(medewerker("later"), t0 + Duration::from_secs(120));
        assert_eq!(sessies.aantal(), 1);
    }

    #[test]
    fn het_maximum_verdringt_de_langst_ongebruikte() {
        let sessies = Sessies::met(Duration::from_secs(3600), 3);
        let t0 = Instant::now();
        let s = |i: u64| t0 + Duration::from_secs(i);
        let a = sessies.nieuw_op(medewerker("A"), s(0));
        let b = sessies.nieuw_op(medewerker("B"), s(1));
        let c = sessies.nieuw_op(medewerker("C"), s(2));
        // A is net nog gebruikt, dus B is de langst ongebruikte.
        assert!(sessies.zoek_op(&a, s(3)).is_some());
        let d = sessies.nieuw_op(medewerker("D"), s(4));
        assert_eq!(sessies.aantal(), 3);
        assert!(sessies.zoek_op(&b, s(5)).is_none());
        for t in [&a, &c, &d] {
            assert!(sessies.zoek_op(t, s(5)).is_some());
        }
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
