//! Hoe de cel aan haar antwoord kwam: de herkomst naast de uitkomst.
//!
//! Een lexostatus zegt wát er vastgesteld is. Dit zegt waaróp: welke gegevens
//! de cel gelezen heeft en hoe ze die tot dit ene antwoord teruggebracht heeft.
//! Dat kan hier en alleen hier — een reductie leest uitsluitend de eigen
//! kronieken van de eigen cel (RFC-022 §4.1), dus de cel kan precies zeggen wat
//! ze las zonder ergens anders te hoeven kijken. Een consument die dit blok
//! naast het antwoord legt, kan de reductie nalopen zonder de cel binnen te
//! gaan.
//!
//! **Geen tweede administratie.** Elk gram in deze uitleg is een *verwijzing*
//! ([`GramRef`]) naar een gram dat in een kroniek van deze cel ligt, en geen
//! kopie van zijn inhoud. Dezelfde keuze als in het journaal, en om dezelfde
//! reden: wat er in het gram staat, staat in de kroniek.
//!
//! **Alleen eigen grammen.** Elke verwijzing hierin noemt de cel die het
//! antwoord gaf. Een reductie kan geen andere cel bereiken, dus een uitleg die
//! er een noemt, zou een cel beschrijven die niet bestaat. `tests/invarianten.rs`
//! meet dat over elk scenario.
//!
//! **Geen wandkloktijd.** Wat hier staat, hangt alleen van de kronieken en van
//! het gevraagde moment af. Twee keer dezelfde vraag levert dezelfde uitleg, en
//! daarmee is dit net zo goed een contract als het antwoord zelf.

use super::besluit::REGULATION_VALID_FROM;
use super::chronicle::{self, ChronicleEvent};
use crate::journal::GramRef;
// De twee vragen die een gram in het beeld van de wereld identificeren: welke
// soort het is, en waaronder het terug te vinden is. Ze horen bij het gram en
// niet bij deze uitleg, dus ze worden geleend van waar ze al beantwoord worden
// — een tweede antwoord ernaast zou stil uiteen kunnen lopen met het beeld waar
// deze verwijzingen naartoe wijzen.
use crate::snapshot::{gram_id, gram_kind};
use chrono::NaiveDate;
use regelrecht_engine::Value;
use serde::Serialize;
use std::collections::BTreeMap;

/// Hoe de cel aan dit antwoord kwam.
///
/// Twee vormen, precies de twee die een cel kent (zie [`super::Reduction`]), en
/// in beide gevallen de grammen die ze ervoor gelezen heeft. "Niets vastgesteld"
/// is geen derde vorm: dat is hetzelfde kroniekfilter met geen enkel gram, plus
/// [`Self::gemist`] — wat er wél lag en waarom het niet meedeed.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Reductie {
    /// Wat de cel gedaan heeft om tot dit antwoord te komen.
    pub vorm: ReductieVorm,
    /// De grammen die ze daarbij gelezen heeft, in de volgorde waarin ze in de
    /// kroniek liggen.
    ///
    /// Leeg is een uitspraak en geen gat: bij een kroniekfilter dat niets vond,
    /// is dit precies wat er te zeggen valt — er is niets gelezen.
    pub grammen: Vec<GebruiktGram>,
    /// Wat er in de stroom lag en tóch niet meedeed; alleen bij een
    /// kroniekfilter dat niets vaststelde.
    ///
    /// Zonder dit zou "niets vastgesteld" niet te onderscheiden zijn van "hier
    /// ligt helemaal niets": een stroom vol grammen over een ander onderwerp
    /// geeft hetzelfde antwoord als een lege stroom.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub gemist: Option<Gemist>,
}

impl Reductie {
    /// Een kroniekfilter dat een antwoord opleverde.
    pub(crate) fn kroniekfilter(filter: Kroniekfilter, grammen: Vec<GebruiktGram>) -> Self {
        Self {
            vorm: ReductieVorm::Kroniekfilter(filter),
            grammen,
            gemist: None,
        }
    }

    /// Een kroniekfilter dat niets vaststelde, met wat er wél lag.
    pub(crate) fn niets_vastgesteld(
        filter: Kroniekfilter,
        grammen: Vec<GebruiktGram>,
        gemist: Gemist,
    ) -> Self {
        Self {
            vorm: ReductieVorm::Kroniekfilter(filter),
            grammen,
            gemist: Some(gemist),
        }
    }

    /// Een wetsvorm: de eigen engine rekende over de eigen feiten.
    pub(crate) fn wetsvorm(wetsvorm: Wetsvorm, grammen: Vec<GebruiktGram>) -> Self {
        Self {
            vorm: ReductieVorm::Wetsvorm(wetsvorm),
            grammen,
            gemist: None,
        }
    }

    /// De cellen die deze uitleg noemt.
    ///
    /// Eén cel, altijd: die van het antwoord. Dat is geen aanname maar een
    /// meting — zie de moduledocs en `tests/invarianten.rs`.
    pub fn genoemde_cellen(&self) -> impl Iterator<Item = &str> {
        self.grammen.iter().map(|gram| gram.gram.cell.as_str())
    }
}

/// De twee reductievormen, zoals ze in het antwoord verschijnen.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "soort", rename_all = "snake_case")]
pub enum ReductieVorm {
    /// Een filter over één eigen kroniek; geen engine in zicht.
    Kroniekfilter(Kroniekfilter),
    /// Een eigen regeling die over de eigen feiten rekende.
    Wetsvorm(Wetsvorm),
}

/// Waar het kroniekfilter naar gezocht heeft, en met welke regel.
///
/// Precies de vraag die de cel stelde: over welke stroom, over welk onderwerp,
/// onder welke voorwaarden en tot welk moment. Genoeg om hem na te lopen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Kroniekfilter {
    /// De stroom die nagekeken is.
    pub chronicle: String,
    /// Het sleutelveld van die stroom.
    pub key: String,
    /// De waarde die de vraag voor dat veld meegaf: het onderwerp.
    pub key_value: Value,
    /// De extra gelijkheden waaraan een vastlegging moest voldoen.
    ///
    /// Leeg is het gewone geval; wat hier staat, komt letterlijk uit de
    /// definitie en niet uit de vraag.
    #[serde(rename = "where")]
    pub conditions: BTreeMap<String, Value>,
    /// Wat er van de overgebleven vastleggingen gemaakt is.
    pub regel: Regel,
    /// Het moment waarop gevraagd is: later vastgelegde feiten bestaan voor dit
    /// antwoord niet.
    pub op_moment: NaiveDate,
}

/// Wat er van de vastleggingen gemaakt is die aan het filter voldeden.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "regel", rename_all = "snake_case")]
pub enum Regel {
    /// De laatste vastlegging op of vóór het moment, in haar geheel.
    Laatste,
    /// Alle vastleggingen tellen mee; dit veld is opgeteld.
    Som {
        /// Het veld dat gesommeerd is.
        field: String,
    },
}

/// Wat er in de stroom lag maar buiten het filter viel.
///
/// Drie getallen en geen lijst: het zijn grammen waar dit antwoord juist *niet*
/// over gaat, en een uitleg die ze opsomt zou een antwoord geven op een vraag
/// die niemand stelde. Het aantal is wat een lezer nodig heeft om te weten waar
/// hij moet kijken — een verkeerd moment, een verkeerd onderwerp, of een
/// voorwaarde die niet uitkwam.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
pub struct Gemist {
    /// Hoeveel grammen er in deze stroom liggen, over alle onderwerpen en
    /// momenten heen.
    pub in_de_stroom: usize,
    /// Hoeveel daarvan ná het gevraagde moment liggen.
    pub na_het_moment: usize,
    /// Hoeveel er op tijd lagen maar over een ander onderwerp gaan.
    pub andere_sleutel: usize,
    /// Hoeveel er op tijd lagen en over dit onderwerp gaan, maar niet aan
    /// `where` voldeden.
    pub buiten_de_voorwaarden: usize,
}

/// De wetsvorm: welke regeling in welke versie, en waar haar inputs vandaan
/// kwamen.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Wetsvorm {
    /// De uitgevoerde regeling, bij `$id`.
    pub regulation: String,
    /// De versie van die regeling die op het gevraagde moment gold.
    ///
    /// `None` als de regeling geen versiedatum draagt. Dit is wat een antwoord
    /// over een moment in het verleden van een herberekening onderscheidt: er is
    /// gerekend onder het recht van toen.
    pub regulation_valid_from: Option<String>,
    /// De uitkomst die gevraagd is.
    pub output: String,
    /// Per input van de uitgevoerde regeling waar hij vandaan kwam.
    ///
    /// De inputs van deze uitvoering, niet die van de regelingen die zij op haar
    /// beurt aanriep: elke aanroep heeft haar eigen herkomst, en die uitvouwen
    /// zou van deze uitleg een trace maken. Een input die geen enkele bron
    /// beantwoordde, staat er niet in — daar valt geen herkomst over te melden.
    pub inputs: Vec<GebruikteInput>,
    /// Het moment waarop gerekend is.
    pub op_moment: NaiveDate,
}

/// Eén input van een uitgevoerde regeling, met zijn herkomst.
///
/// De herkomst en niet de waarde. Waar de waarde vandaan komt, is precies wat
/// een uitvoering niet laat zien en wat deze uitleg toevoegt; de waarde zelf
/// staat waar ze vandaan kwam — in het gram waarnaar
/// [`InputHerkomst::EigenKroniek`] wijst, of in de vraag die de consument zelf
/// stelde. Een kopie ernaast zou een tweede administratie zijn, en voor een
/// input die een andere regeling berekende zou ze leeg blijven: de engine geeft
/// er geen tussenwaarden van terug.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GebruikteInput {
    /// De naam van de input in de regeling.
    pub name: String,
    /// Waar hij vandaan kwam.
    pub herkomst: InputHerkomst,
}

/// De herkomst van één input van een wetsvorm.
///
/// De tiers van de resolutievolgorde (RFC-022 §4.2), in de woorden van deze
/// opstelling: een kroniekstroom is tier 1, een eigen regeling tier 2, een cel
/// tier 3.
#[derive(Debug, Clone, PartialEq, Serialize)]
#[serde(tag = "herkomst", rename_all = "snake_case")]
pub enum InputHerkomst {
    /// Uit de vraag: de reductie gaf hem aan de engine mee.
    Parameter {
        /// De gedocumenteerde parameter die hem vulde; `None` als de definitie
        /// er een vaste waarde voor invult en de vraag er dus niet over ging.
        parameter: Option<String>,
    },
    /// Uit een eigen kroniek van deze cel: de stroom stond als databron klaar.
    EigenKroniek {
        /// De stroom waaruit gelezen is.
        chronicle: String,
        /// Het gram dat op dit moment over dit onderwerp gold, waaronder het in
        /// [`Reductie::grammen`] staat; `None` als er over dit onderwerp niets
        /// in deze stroom lag.
        gram: Option<String>,
    },
    /// Berekend door een andere regeling die deze cel zelf laadt.
    Regeling {
        /// De aangeroepen regeling, bij `$id`.
        regulation: String,
        /// De uitkomst die zij leverde.
        output: String,
    },
    /// Geaccepteerd van een andere cel.
    ///
    /// In een reductie onbereikbaar: de reduce-engine krijgt geen cel-tier (zie
    /// `Cell::grant_cell_tier`), en een reductie die er toch een bereikt, wordt
    /// geweigerd. De variant staat er omdat de engine deze herkomst kent — hem
    /// weglaten zou hem stil als iets anders laten verschijnen, en dat is
    /// precies het onderscheid dat invariant I5 maakt.
    Cel {
        /// De cel die de waarde vaststelde.
        cell: String,
        /// De uitkomst waaronder ze dat deed.
        output: String,
    },
}

/// Eén gram dat een reductie gelezen heeft.
///
/// De verwijzing is die van het journaal, zodat een lezer één vocabulaire heeft
/// en één sleutel om het gram in het beeld van de wereld mee terug te vinden.
/// Wat er hier bijkomt, is wat een reductie erover te zeggen heeft: wanneer het
/// vastgelegd is, onder welke versie van welke regeling er besloten is, en —
/// bij een som — wat dit gram aan het totaal bijdroeg.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct GebruiktGram {
    /// Waar dit gram ligt en hoe het heet.
    #[serde(flatten)]
    pub gram: GramRef,
    /// De plek in de kroniek, vanaf 0: het volgnummer waaronder het beeld van de
    /// wereld het geeft. Ook te lezen uit [`GramRef::id`], en hier los omdat een
    /// lezer het nummer wil zien en niet hoeft te ontleden.
    pub volgnummer: usize,
    /// Het moment waarop dit feit in deze cel feit werd.
    pub op_moment: NaiveDate,
    /// Bij een decretogram: de versie van de regeling waaronder besloten is.
    ///
    /// `None` bij elk ander gram, en ook bij een decretogram van een cel zonder
    /// engine: die legt vast wat zij vaststelde, zonder uitvoering eronder.
    pub regulation_valid_from: Option<String>,
    /// Bij een som: wat dit gram aan het totaal bijdroeg. `None` bij elke andere
    /// regel.
    pub bijdrage: Option<Value>,
}

impl GebruiktGram {
    /// De verwijzing naar één gelezen gram, op zijn plek in de kroniek.
    pub(crate) fn new(cell: &str, chronicle: &str, index: usize, event: &ChronicleEvent) -> Self {
        Self {
            gram: GramRef {
                cell: cell.to_string(),
                chronicle: chronicle.to_string(),
                id: gram_id(cell, chronicle, index),
                kind: gram_kind(event.intake),
                name: event.name.clone(),
            },
            volgnummer: index,
            op_moment: event.op_moment,
            regulation_valid_from: chronicle::field(&event.fields, REGULATION_VALID_FROM)
                .and_then(Value::as_str)
                .map(str::to_string),
            bijdrage: None,
        }
    }

    /// Hetzelfde gram, met wat het aan een som bijdroeg.
    pub(crate) fn met_bijdrage(mut self, bijdrage: Value) -> Self {
        self.bijdrage = Some(bijdrage);
        self
    }
}
