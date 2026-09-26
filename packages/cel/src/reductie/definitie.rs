//! De lexostatus-definities: wat `lexostatussen.yaml` van een cel declareert
//! (`schema/chronolex/v0.1.0/lexostatus.json`), en wat de controles bij het
//! opstarten erover vragen. Het uitvoeren staat in [`super`].

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::laden;
use crate::schema::Soort;

/// De lexostatus-definities van een cel (`schema/chronolex/v0.1.0/lexostatus.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct Lexostatussen {
    pub cel: String,
    pub lexostatus_definitions: Vec<LexostatusDefinitie>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LexostatusDefinitie {
    pub name: String,
    pub inputs: Vec<InputDefinitie>,
    pub reduction: Reductie,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputDefinitie {
    pub name: String,
    #[serde(rename = "type")]
    pub soort: String,
}

/// Gelijkheid op het gram: een sleutel uit [`GRAM_SLEUTELS`] is een veld van
/// het gram zelf, elke andere een veldpad onder `fields`. Een waarde `$x`
/// komt uit de inputs.
pub type Filter = BTreeMap<String, String>;

/// De filtersleutels die een veld van het gram zelf zijn, geen veldpad: elk
/// veld met een tekst als waarde (zie [`crate::gram::Gram::kenmerk`]). Zo kan
/// een lexostatus ook per besluit in een zaak filteren (`besluit`,
/// `besluitkenmerk`, `wijzigt`).
pub const GRAM_SLEUTELS: &[&str] = &[
    "name",
    "type",
    "soort",
    "stage",
    "zaak",
    "zaakkenmerk",
    "besluit",
    "besluitkenmerk",
    "wijzigt",
    "recording_actor",
    "chronicle",
    "legal_character",
    "decision_type",
    "regulation",
    "competent_authority",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Reductie {
    pub kroniek: String,
    #[serde(default, skip_serializing_if = "Filter::is_empty")]
    pub filter: Filter,
    /// Maakt van de lexostatus een lijst met een regel per zaak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub groepeer: Option<Groepeer>,
    /// Alleen met `groepeer`: een zaak met een gram door dit filter valt af.
    #[serde(default, skip_serializing_if = "Filter::is_empty")]
    pub zonder: Filter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kies: Option<Kies>,
    pub afleidingen: BTreeMap<String, Afgeleid>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_velden: BTreeMap<String, Afgeleid>,
}

/// Een afleiding met haar grondslag: de artikelen (`<regeling>#<artikel>`,
/// optioneel met `lid <n>`) waarop zij rust. Dat is het artikel dat het feit
/// vraagt, of het artikel dat de lezing draagt, zoals het register dat de
/// cel bijhoudt ("geen gram is nee"). De naam van een parameter-afleiding is
/// een parameter van een artikel uit de grondslag van het event of uit deze
/// grondslag; de controle bij het opstarten gaat na dat elk artikel geladen
/// is en het lid bestaat (zie [`crate::controle`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Afgeleid {
    #[serde(flatten)]
    pub afleiding: Afleiding,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub grondslag: Vec<String>,
}

impl std::ops::Deref for Afgeleid {
    type Target = Afleiding;
    fn deref(&self) -> &Afleiding {
        &self.afleiding
    }
}

impl From<Afleiding> for Afgeleid {
    fn from(afleiding: Afleiding) -> Self {
        Self {
            afleiding,
            grondslag: Vec::new(),
        }
    }
}

impl<'de> Deserialize<'de> for Afgeleid {
    /// `grondslag` naast de sleutels van de afleiding: eerst eraf, dan de
    /// afleiding uit de rest (een untagged enum met `flatten` negeert
    /// onbekende sleutels niet betrouwbaar).
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut v = Value::deserialize(d)?;
        let grondslag = match v.as_object_mut().and_then(|o| o.remove("grondslag")) {
            Some(g) => serde_json::from_value(g).map_err(serde::de::Error::custom)?,
            None => Vec::new(),
        };
        let afleiding = Afleiding::deserialize(v).map_err(serde::de::Error::custom)?;
        Ok(Self {
            afleiding,
            grondslag,
        })
    }
}

/// Waarop een lijst-lexostatus groepeert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Groepeer {
    /// Een regel per zaak. Alleen events met een zaak hebben een zaakkenmerk.
    Zaakkenmerk,
}

/// Welk gram telt als er meer zijn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kies {
    /// Het laatste gram in de tijd: het laatste `op_moment`, bij gelijk
    /// moment het laatste `vastgelegd_op`. Een herstel is een nieuw gram.
    Laatste,
}

/// Een afleiding: hoe een parameter uit de kroniek volgt.
///
/// De volgorde telt: serde probeert de varianten van boven naar beneden en
/// negeert onbekende sleutels, dus de varianten met meer sleutels staan
/// eerst. Het schema heeft de vorm dan al gecontroleerd.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Afleiding {
    /// Over de grammen door `filter`: de datum van een moment van het
    /// laatste (zoals de ontvangst van de aanvraag, in een lijst die ook de
    /// andere grammen van een zaak leest). Geen gram: `geen_gram`, als dat er
    /// is.
    LaatsteMoment {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        kies: Kies,
        moment: Moment,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        geen_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: de waarde van `veld` in het laatste.
    /// Komt geen gram door het filter, dan `geen_gram`, als dat er is.
    LaatsteVeld {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        kies: Kies,
        veld: String,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        geen_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: het jaartal van de datum in `jaar_van`
    /// in het laatste gram. Geen gram: `geen_gram`, als dat er is.
    LaatsteJaarVan {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        kies: Kies,
        jaar_van: String,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        geen_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: de periode waarin de datum in
    /// `periode_van` in het laatste gram valt (zie [`Afleiding::PeriodeVan`]).
    /// Geen gram: `geen_gram`, als dat er is.
    LaatstePeriodeVan {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        kies: Kies,
        periode_van: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        periode: Option<Periode>,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        geen_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: of het lijstveld in het laatste de
    /// waarde bevat.
    LaatsteBevat {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        kies: Kies,
        bevat: Bevat,
    },
    /// Over de grammen door `filter`: of er ten minste een is. Met `gevuld`
    /// telt alleen een gram waarin dat veld gevuld is (zie
    /// [`super::gevuld`]): een filter vergelijkt op gelijkheid, en "heeft
    /// een waarde" is geen waarde.
    Bestaat {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        bestaat: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        gevuld: Option<String>,
    },
    /// Over de grammen door `filter`: de som van een getalveld.
    Som {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        som: String,
    },
    /// Over de grammen door `filter`: per gram een regel met deze velden, in
    /// de volgorde van de kroniek. Een lijst voor een tabel, bijvoorbeeld de
    /// regels van een uitslag; een veld dat een gram niet heeft, is null.
    Verzamel {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        verzamel: Vec<String>,
    },
    Veld {
        veld: String,
    },
    /// Het jaartal van een datumveld van het gekozen gram.
    JaarVan {
        jaar_van: String,
    },
    /// De periode (jaar, kwartaal of maand) waarin een datumveld van het
    /// gekozen gram valt, als de eerste dag ervan: een tijdvak als datum die
    /// de engine kan lezen. Zonder `periode` zet de runtime bij het laden de
    /// periode die de regeling noemt (zie [`Periode`]).
    PeriodeVan {
        periode_van: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        periode: Option<Periode>,
    },
    Gevuld {
        gevuld: String,
    },
    Gelijk {
        gelijk: Gelijk,
    },
    ElkeRegel {
        tabel: String,
        elke_regel: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        alleen_waar: Option<String>,
    },
    EenRegel {
        tabel: String,
        een_regel: String,
    },
    Moment {
        moment: Moment,
    },
}

/// Een sleutel die er staat, ook met de waarde null: `Some(Value::Null)`.
fn aanwezig<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Gelijk {
    pub veld: String,
    pub aan: Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Bevat {
    pub veld: String,
    pub waarde: Value,
}

/// Een periode van de kalender. Een tijdvak (de periode waarvoor een
/// beschikking wordt gevraagd) kan een jaar zijn, een kwartaal of een maand;
/// de regeling zegt welke met `temporal.period_type` van de parameter
/// (RFC-001: `year`, `month`). Een periode heet naar haar eerste dag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Periode {
    Jaar,
    Kwartaal,
    Maand,
}

impl Periode {
    /// De periode die een regeling met `temporal.period_type` noemt.
    pub fn uit_period_type(t: &str) -> Option<Self> {
        match t {
            "year" => Some(Periode::Jaar),
            "quarter" => Some(Periode::Kwartaal),
            "month" => Some(Periode::Maand),
            _ => None,
        }
    }

    /// De eerste dag van de periode waarin een datum valt.
    pub fn begin(self, d: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
        use chrono::Datelike;
        let maand = match self {
            Periode::Jaar => 1,
            Periode::Kwartaal => (d.month0() / 3) * 3 + 1,
            Periode::Maand => d.month(),
        };
        chrono::NaiveDate::from_ymd_opt(d.year(), maand, 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Moment {
    /// Wanneer het feit rechtens geldt of plaatsvond.
    OpMoment,
    /// Wanneer de cel het vastlegde.
    VastgelegdOp,
}

/// Lees lexostatus-definities uit tekst en valideer ze tegen het schema.
pub fn parse(tekst: &str, bron: &str) -> Result<Lexostatussen, Vec<String>> {
    laden::definitie(tekst, bron, Soort::Lexostatus)
}

/// Laad de lexostatus-definities uit een bestand.
pub fn laad(pad: &Path) -> Result<Lexostatussen, Vec<String>> {
    laden::laad(pad, parse)
}

impl Lexostatussen {
    pub fn lexostatus(&self, naam: &str) -> Option<&LexostatusDefinitie> {
        self.lexostatus_definitions.iter().find(|d| d.name == naam)
    }
}

impl LexostatusDefinitie {
    /// Alle afleidingen: de parameters en de extra velden.
    pub fn alle_afleidingen(&self) -> impl Iterator<Item = (&String, &Afgeleid)> {
        self.reduction
            .afleidingen
            .iter()
            .chain(self.reduction.extra_velden.iter())
    }

    /// Of de lexostatus een lijst is (`groepeer`): geen parameters, en nooit
    /// naar de engine.
    pub fn is_lijst(&self) -> bool {
        self.reduction.groepeer.is_some()
    }

    /// De namen die deze lexostatus levert: parameters en extra velden.
    pub fn levert(&self, naam: &str) -> bool {
        self.reduction.afleidingen.contains_key(naam)
            || self.reduction.extra_velden.contains_key(naam)
    }
}

/// Of een filtersleutel een veld van het gram zelf is.
pub fn is_gram_sleutel(sleutel: &str) -> bool {
    GRAM_SLEUTELS.contains(&sleutel)
}

/// De veldpaden onder `fields` waarop een filter selecteert.
pub fn filter_paden(filter: &Filter) -> Vec<&str> {
    filter
        .keys()
        .map(String::as_str)
        .filter(|k| !is_gram_sleutel(k))
        .collect()
}

impl Afleiding {
    /// Het eigen filter van een afleiding over een verzameling grammen;
    /// `None` bij een afleiding op het gekozen gram.
    pub fn filter(&self) -> Option<&Filter> {
        match self {
            Afleiding::LaatsteVeld { filter, .. }
            | Afleiding::LaatsteMoment { filter, .. }
            | Afleiding::LaatsteJaarVan { filter, .. }
            | Afleiding::LaatstePeriodeVan { filter, .. }
            | Afleiding::LaatsteBevat { filter, .. }
            | Afleiding::Bestaat { filter, .. }
            | Afleiding::Som { filter, .. }
            | Afleiding::Verzamel { filter, .. } => Some(filter),
            _ => None,
        }
    }

    /// Of de afleiding het gekozen gram leest (en dus `kies` vraagt).
    pub fn op_gekozen_gram(&self) -> bool {
        self.filter().is_none()
    }

    /// De veldpaden die deze afleiding leest, ook die van haar filter.
    pub fn gelezen_paden(&self) -> Vec<&str> {
        let mut paden = match self {
            Afleiding::Veld { veld } | Afleiding::LaatsteVeld { veld, .. } => vec![veld.as_str()],
            Afleiding::JaarVan { jaar_van } | Afleiding::LaatsteJaarVan { jaar_van, .. } => {
                vec![jaar_van.as_str()]
            }
            Afleiding::PeriodeVan { periode_van, .. }
            | Afleiding::LaatstePeriodeVan { periode_van, .. } => vec![periode_van.as_str()],
            Afleiding::Gevuld { gevuld } => vec![gevuld.as_str()],
            Afleiding::Gelijk { gelijk } => vec![gelijk.veld.as_str()],
            Afleiding::ElkeRegel { tabel, .. } | Afleiding::EenRegel { tabel, .. } => {
                vec![tabel.as_str()]
            }
            Afleiding::Som { som, .. } => vec![som.as_str()],
            Afleiding::Verzamel { verzamel, .. } => verzamel.iter().map(String::as_str).collect(),
            Afleiding::LaatsteBevat { bevat, .. } => vec![bevat.veld.as_str()],
            Afleiding::Bestaat { gevuld, .. } => gevuld.iter().map(String::as_str).collect(),
            Afleiding::Moment { .. } | Afleiding::LaatsteMoment { .. } => vec![],
        };
        if let Some(f) = self.filter() {
            paden.extend(filter_paden(f));
        }
        paden
    }

    /// Bij een periode-afleiding: de periode, die de runtime zo nodig bij
    /// het laden uit de regeling zet.
    pub fn periode_mut(&mut self) -> Option<&mut Option<Periode>> {
        match self {
            Afleiding::PeriodeVan { periode, .. }
            | Afleiding::LaatstePeriodeVan { periode, .. } => Some(periode),
            _ => None,
        }
    }

    /// Bij een tabelafleiding: het tabelveld en de kolommen die ze leest
    /// (`elke_regel` of `een_regel`, en `alleen_waar`).
    pub fn tabel_kolommen(&self) -> Option<(&str, Vec<&str>)> {
        match self {
            Afleiding::ElkeRegel {
                tabel,
                elke_regel,
                alleen_waar,
            } => {
                let mut k = vec![elke_regel.as_str()];
                k.extend(alleen_waar.as_deref());
                Some((tabel, k))
            }
            Afleiding::EenRegel { tabel, een_regel } => Some((tabel, vec![een_regel])),
            _ => None,
        }
    }

    /// Of de afleiding toetst of iets aanwezig is (`gevuld`, `tabel` met
    /// `elke_regel`). Onwaar betekent dan: dit ontbreekt in het gram. Bij
    /// `gelijk` en `een_regel` is onwaar een antwoord, geen gat.
    pub fn toetst_aanwezigheid(&self) -> bool {
        matches!(self, Afleiding::Gevuld { .. } | Afleiding::ElkeRegel { .. })
    }
}
