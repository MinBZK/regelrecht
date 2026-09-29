//! De lexostatus-definities: wat `lexostatussen.yaml` van een cel declareert
//! (`schema/chronolex/v0.2.0/lexostatus.json`), en wat de controles bij het
//! opstarten erover vragen. Het uitvoeren staat in [`super`].

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::laden;
use crate::schema::Soort;

/// De lexostatus-definities van een cel (`schema/chronolex/v0.2.0/lexostatus.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct Lexostatussen {
    pub cell: String,
    /// Aanvullingen op de lexostatussen die de wet in deze cel leest (zie
    /// [`crate::wet`]): extra velden voor de synthese, geen parameters.
    #[serde(default)]
    pub law: Vec<WetAanvulling>,
    pub lexostatus_definitions: Vec<LexostatusDefinitie>,
}

/// Extra velden bij een lexostatus uit de wet: wat de cel meegeeft als
/// invoer voor een latere bron (een KvK-nummer, een aanduiding), geen
/// parameter van een artikel. Dat is registratie, dus configuratie van de
/// cel, niet de wet.
#[derive(Debug, Clone, Deserialize)]
pub struct WetAanvulling {
    /// `<regeling>#<artikel>`: het lezende artikel.
    pub article: String,
    pub extra_fields: BTreeMap<String, Afgeleid>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LexostatusDefinitie {
    pub name: String,
    pub inputs: Vec<InputDefinitie>,
    pub reduction: Reductie,
    /// Uit de wet (`produces.extensions.chronolex.leest`, zie
    /// [`crate::wet`]): het lezende artikel. Zonder: uit `lexostatussen.yaml`.
    #[serde(skip)]
    pub law: Option<crate::wet::Wetlezing>,
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
/// veld met een tekst als waarde (zie [`crate::gram::Gram::kenmerk`]): het
/// id, de wortel (uit de index van de kroniek) en de vaste velden. Daarnaast
/// is `verwijst.<naam>` het id waarnaar een gram onder die naam verwijst; zo
/// filtert een lexostatus per besluit.
pub const GRAM_SLEUTELS: &[&str] = &[
    "id",
    "root",
    "name",
    "type",
    "subtype",
    "stage",
    "recording_actor",
    "chronicle",
    "legal_character",
    "decision_type",
    "regulation",
    "competent_authority",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Reductie {
    pub chronicle: String,
    #[serde(default, skip_serializing_if = "Filter::is_empty")]
    pub filter: Filter,
    /// Maakt van de lexostatus een lijst met een regel per zaak.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<Groepeer>,
    /// Alleen met `groepeer`: een zaak met een gram door dit filter valt af.
    #[serde(default, skip_serializing_if = "Filter::is_empty")]
    pub without: Filter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<Kies>,
    pub derivations: BTreeMap<String, Afgeleid>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_fields: BTreeMap<String, Afgeleid>,
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
    pub derivation: Afleiding,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legal_basis: Vec<String>,
}

impl std::ops::Deref for Afgeleid {
    type Target = Afleiding;
    fn deref(&self) -> &Afleiding {
        &self.derivation
    }
}

impl From<Afleiding> for Afgeleid {
    fn from(derivation: Afleiding) -> Self {
        Self {
            derivation,
            legal_basis: Vec::new(),
        }
    }
}

impl<'de> Deserialize<'de> for Afgeleid {
    /// `grondslag` naast de sleutels van de afleiding: eerst eraf, dan de
    /// afleiding uit de rest (een untagged enum met `flatten` negeert
    /// onbekende sleutels niet betrouwbaar).
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut v = Value::deserialize(d)?;
        let legal_basis = match v.as_object_mut().and_then(|o| o.remove("legal_basis")) {
            Some(g) => serde_json::from_value(g).map_err(serde::de::Error::custom)?,
            None => Vec::new(),
        };
        let derivation = Afleiding::deserialize(v).map_err(serde::de::Error::custom)?;
        Ok(Self {
            derivation,
            legal_basis,
        })
    }
}

/// Waarop een lijst-lexostatus groepeert.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Groepeer {
    /// Een regel per wortel: de grammen die via hun verwijzingen bij
    /// hetzelfde gram zonder verwijzing uitkomen (zoals een aanvraag en wat
    /// erop volgt).
    Root,
}

/// Welk gram telt als er meer zijn.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kies {
    /// Het laatste gram in de tijd: het laatste `op_moment`, bij gelijk
    /// moment het laatste `vastgelegd_op`. Een herstel is een nieuw gram.
    Latest,
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
        pick: Kies,
        moment: Moment,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: de waarde van `veld` in het laatste.
    /// Komt geen gram door het filter, dan `geen_gram`, als dat er is.
    LaatsteVeld {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Kies,
        field: String,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: het jaartal van de datum in `jaar_van`
    /// in het laatste gram. Geen gram: `geen_gram`, als dat er is.
    LaatsteJaarVan {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Kies,
        year_of: String,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: de periode waarin de datum in
    /// `periode_van` in het laatste gram valt (zie [`Afleiding::PeriodeVan`]).
    /// Geen gram: `geen_gram`, als dat er is.
    LaatstePeriodeVan {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Kies,
        period_of: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        period: Option<Periode>,
        #[serde(
            default,
            deserialize_with = "aanwezig",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over de grammen door `filter`: of het lijstveld in het laatste de
    /// waarde bevat.
    LaatsteBevat {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Kies,
        contains: Bevat,
    },
    /// Over de grammen door `filter`: of er ten minste een is. Met `gevuld`
    /// telt alleen een gram waarin dat veld gevuld is (zie
    /// [`super::gevuld`]): een filter vergelijkt op gelijkheid, en "heeft
    /// een waarde" is geen waarde.
    Bestaat {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        exists: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filled: Option<String>,
    },
    /// Over de grammen door `filter`: de som van een getalveld.
    Som {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        sum: String,
    },
    /// Over de grammen door `filter`: per gram een regel met deze velden, in
    /// de volgorde van de kroniek. Een lijst voor een tabel, bijvoorbeeld de
    /// regels van een uitslag; een veld dat een gram niet heeft, is null.
    Verzamel {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        collect: Vec<String>,
    },
    Veld {
        field: String,
    },
    /// Het jaartal van een datumveld van het gekozen gram.
    JaarVan {
        year_of: String,
    },
    /// De periode (jaar, kwartaal of maand) waarin een datumveld van het
    /// gekozen gram valt, als de eerste dag ervan: een tijdvak als datum die
    /// de engine kan lezen. Zonder `periode` zet de runtime bij het laden de
    /// periode die de regeling noemt (zie [`Periode`]).
    PeriodeVan {
        period_of: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        period: Option<Periode>,
    },
    Gevuld {
        filled: String,
    },
    Gelijk {
        equals: Gelijk,
    },
    ElkeRegel {
        table: String,
        each_row: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only_where: Option<String>,
    },
    EenRegel {
        table: String,
        one_row: String,
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
    pub field: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Bevat {
    pub field: String,
    pub value: Value,
}

/// Een periode van de kalender. Een tijdvak (de periode waarvoor een
/// beschikking wordt gevraagd) kan een jaar zijn, een kwartaal of een maand;
/// de regeling zegt welke met `temporal.period_type` van de parameter
/// (RFC-001: `year`, `month`). Een periode heet naar haar eerste dag.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Periode {
    Year,
    Quarter,
    Month,
}

impl Periode {
    /// De periode die een regeling met `temporal.period_type` noemt.
    pub fn uit_period_type(t: &str) -> Option<Self> {
        match t {
            "year" => Some(Periode::Year),
            "quarter" => Some(Periode::Quarter),
            "month" => Some(Periode::Month),
            _ => None,
        }
    }

    /// De eerste dag van de periode waarin een datum valt.
    pub fn start(self, d: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
        use chrono::Datelike;
        let month = match self {
            Periode::Year => 1,
            Periode::Quarter => (d.month0() / 3) * 3 + 1,
            Periode::Month => d.month(),
        };
        chrono::NaiveDate::from_ymd_opt(d.year(), month, 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Moment {
    /// Wanneer het feit rechtens geldt of plaatsvond.
    EffectiveAt,
    /// Wanneer de cel het vastlegde.
    RecordedAt,
}

/// Lees lexostatus-definities uit tekst en valideer ze tegen het schema.
pub fn parse(tekst: &str, source: &str) -> Result<Lexostatussen, Vec<String>> {
    laden::definitie(tekst, source, Soort::Lexostatus)
}

/// Laad de lexostatus-definities uit een bestand.
pub fn laad(path: &Path) -> Result<Lexostatussen, Vec<String>> {
    laden::laad(path, parse)
}

impl Lexostatussen {
    pub fn lexostatus(&self, name: &str) -> Option<&LexostatusDefinitie> {
        self.lexostatus_definitions.iter().find(|d| d.name == name)
    }
}

impl LexostatusDefinitie {
    /// Alle afleidingen: de parameters en de extra velden.
    pub fn alle_afleidingen(&self) -> impl Iterator<Item = (&String, &Afgeleid)> {
        self.reduction
            .derivations
            .iter()
            .chain(self.reduction.extra_fields.iter())
    }

    /// Of de lexostatus een lijst is (`groepeer`): geen parameters, en nooit
    /// naar de engine.
    pub fn is_lijst(&self) -> bool {
        self.reduction.group_by.is_some()
    }

    /// De namen die deze lexostatus levert: parameters en extra velden.
    pub fn levert(&self, name: &str) -> bool {
        self.reduction.derivations.contains_key(name)
            || self.reduction.extra_fields.contains_key(name)
    }
}

/// Of een filtersleutel een veld van het gram zelf is.
pub fn is_gram_sleutel(sleutel: &str) -> bool {
    GRAM_SLEUTELS.contains(&sleutel) || sleutel.starts_with(crate::gram::VERWIJST)
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
            Afleiding::Veld { field } | Afleiding::LaatsteVeld { field, .. } => {
                vec![field.as_str()]
            }
            Afleiding::JaarVan { year_of } | Afleiding::LaatsteJaarVan { year_of, .. } => {
                vec![year_of.as_str()]
            }
            Afleiding::PeriodeVan { period_of, .. }
            | Afleiding::LaatstePeriodeVan { period_of, .. } => vec![period_of.as_str()],
            Afleiding::Gevuld { filled } => vec![filled.as_str()],
            Afleiding::Gelijk { equals } => vec![equals.field.as_str()],
            Afleiding::ElkeRegel { table, .. } | Afleiding::EenRegel { table, .. } => {
                vec![table.as_str()]
            }
            Afleiding::Som { sum, .. } => vec![sum.as_str()],
            Afleiding::Verzamel { collect, .. } => collect.iter().map(String::as_str).collect(),
            Afleiding::LaatsteBevat { contains, .. } => vec![contains.field.as_str()],
            Afleiding::Bestaat { filled, .. } => filled.iter().map(String::as_str).collect(),
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
            Afleiding::PeriodeVan { period, .. } | Afleiding::LaatstePeriodeVan { period, .. } => {
                Some(period)
            }
            _ => None,
        }
    }

    /// Bij een tabelafleiding: het tabelveld en de kolommen die ze leest
    /// (`elke_regel` of `een_regel`, en `alleen_waar`).
    pub fn tabel_kolommen(&self) -> Option<(&str, Vec<&str>)> {
        match self {
            Afleiding::ElkeRegel {
                table,
                each_row,
                only_where,
            } => {
                let mut k = vec![each_row.as_str()];
                k.extend(only_where.as_deref());
                Some((table, k))
            }
            Afleiding::EenRegel { table, one_row } => Some((table, vec![one_row])),
            _ => None,
        }
    }

    /// Of de afleiding toetst of iets aanwezig is (`gevuld`, `tabel` met
    /// `elke_regel`). Onwaar betekent dan: dit ontbreekt in het gram. Bij
    /// `gelijk` en `een_regel` is onwaar een antwoord, geen gat.
    pub fn toetst_aanwezigheid(&self) -> bool {
        matches!(self, Afleiding::Gevuld { .. } | Afleiding::ElkeRegel { .. })
    }

    /// Hoe de afleiding afwezigheid leest (`geen_gram`), als zij dat zegt.
    pub fn no_gram(&self) -> Option<&Value> {
        match self {
            Afleiding::LaatsteMoment { no_gram, .. }
            | Afleiding::LaatsteVeld { no_gram, .. }
            | Afleiding::LaatsteJaarVan { no_gram, .. }
            | Afleiding::LaatstePeriodeVan { no_gram, .. } => no_gram.as_ref(),
            _ => None,
        }
    }
}
