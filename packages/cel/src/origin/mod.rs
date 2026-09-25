//! Wie een parameter levert, volgens de wet: `origin` op een parameter en
//! `origins` in uitvoeringsbeleid (RFC-043).
//!
//! De wet geeft per parameter de herkomst met een grondslag. Uitvoeringsbeleid
//! van de actor van het proces kan die overschrijven. Bij het opstarten gaat
//! het proces na dat elke parameter die de aanroeper van een uitgevoerde
//! uitkomst (toets, aanbod, elke uitkomst van het besluit) moet leveren, een
//! leverancier heeft die bij zijn herkomst past:
//!
//! | herkomst | leverancier |
//! |---|---|
//! | `BELANGHEBBENDE` | een afleiding van een eigen lexostatus die alleen indieningen leest (`type: indiening`: wat de aanvrager aanlevert, zijn inhoud of zijn login); het tijdvak (`rol: TIJDVAK`) bij het aanbod ook de keuze in het portaal |
//! | `DOSSIER` | een afleiding van een eigen lexostatus die alleen andere grammen van de eigen actor leest (het verloop van de zaak), of de stand bij besluit |
//! | `OORDEEL` | het besluitformulier, alleen bij het besluit, en verder niemand |
//! | `REGISTER` | een synthese-bron die de parameter levert (onder de naam die de synthese van het proces eraan geeft), en die een kroniek bijhoudt met een grondslag in `register` |
//! | `KANAAL` | een afleiding die alleen `$intake` leest |
//!
//! Een parameter die een leverancier heeft die niet bij zijn herkomst past, is
//! altijd een fout, ook naast een leverancier die wel past: de bron is dan
//! verkeerd. Heeft hij geen leverancier, dan is dat een fout, behalve bij
//! `required: false`: dan krijgt de engine hem niet, en rekent ze met een
//! onbekende waarde (RFC-036); dat is een waarschuwing. Een parameter zonder
//! `origin` is een waarschuwing, en in een proces met `herkomst: streng` een
//! fout. Een `BELANGHEBBENDE`-parameter zonder `required: false` is een
//! waarschuwing (RFC-036), behalve het tijdvak.
//!
//! Wat de runtime niet kan nagaan (een bron met een url, een interne cel die
//! niet draait) telt als leverancier, met een waarschuwing die zegt waarom het
//! niet na te gaan is.
//!
//! [`valideer`] controleert de vorm van `origin` en `origins` in een regeling
//! bij het laden, met het bestand in de melding: een ongeldige waarde houdt
//! de engine niet tegen (zie `Declared` in law-model), de runtime wel.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use regelrecht_engine::{Article, LawExecutionService, RegulatoryLayer};
use regelrecht_law_model::{
    ArticleBasedLaw, Declared, Origin, OriginOverride, OriginRole, OriginValue, Parameter,
};

use crate::cel::Cel;
use crate::config::{
    HandelingDefinitie, Handelingsoort, Herkomstcontrole, Oordeel, ProcesDefinitie, RijenDefinitie,
};
use crate::gezag;
use crate::reductie::{Afleiding, Filter, LexostatusDefinitie};
use crate::regelingen::{self, Benodigd};
use crate::stroom::{Binding, Event, Stroom};

mod controle;
mod levering;
mod vorm;

#[cfg(test)]
mod tests;

pub use controle::{controleer, label_uit, oordelen};
pub use levering::Uitvoering;

// Wat de controle van de levering gebruikt.
use levering::{leverancier, vooraf_bekend, Leveranciers, Uitslag};
pub use vorm::{overschrijvingen, parameter, valideer, Overschrijvingen};

/// Het type van een gram dat een belanghebbende indient (RFC-022 par. 1,
/// `schema/chronolex/v0.1.0/stream.json`): wat de aanvrager aanlevert.
const INDIENING: &str = "indiening";

/// De herkomst die voor een parameter geldt, en waar ze staat.
#[derive(Debug, Clone, PartialEq)]
pub struct Geldend {
    pub origin: Origin,
    /// Het artikel van het beleid dat haar overschrijft; `None` als ze uit
    /// de wet komt.
    pub beleid: Option<String>,
}

impl Geldend {
    /// Voor een melding: `REGISTER, register een_registerwet, grondslag x#1`.
    pub fn beschrijving(&self) -> String {
        let mut s = self.origin.waarde.as_str().to_string();
        if let Some(r) = &self.origin.register {
            s.push_str(&format!(", register {r}"));
        }
        if let Some(r) = self.origin.rol {
            s.push_str(&format!(", rol {}", r.as_str()));
        }
        s.push_str(&format!(", grondslag {}", self.origin.grondslag));
        if let Some(b) = &self.beleid {
            s.push_str(&format!(", uit {b}"));
        }
        s
    }

    /// Of de parameter het tijdvak van de gevraagde beschikking is
    /// (`rol: TIJDVAK`). Los van de grondslag: Awb 4:2 lid 1 is ook de
    /// grondslag van andere onderdelen van de aanvraag.
    pub fn is_tijdvak(&self) -> bool {
        self.origin.rol == Some(OriginRole::Tijdvak)
    }
}

/// Wat de controle oplevert.
#[derive(Debug, Default)]
pub struct Controle {
    pub fouten: Vec<String>,
    pub waarschuwingen: Vec<String>,
    /// Per uitvoering de parameters met hun geldende herkomst, in de volgorde
    /// van declaratie; bij een handeling over alle uitkomsten samen,
    /// onder de naam van de handeling.
    pub parameters: BTreeMap<String, Vec<(Benodigd, Option<Geldend>)>>,
    /// De parameter van het aanbod-artikel die het tijdvak is: `rol:
    /// TIJDVAK`.
    pub tijdvak: Option<String>,
}
