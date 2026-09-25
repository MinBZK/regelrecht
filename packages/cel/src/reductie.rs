//! Reductie tot lexostatus: de lexostatus-definities laden, en een kroniek
//! reduceren tot de parameters van een artikel.
//!
//! Een lexostatus-definitie beperkt de kroniek met `filter` en kiest met
//! `kies` zo nodig een gram. Per parameter leidt een afleiding een waarde af,
//! op een van twee manieren:
//!
//! - **op het gekozen gram**: `veld`, `jaar_van`, `gevuld`, `gelijk`, `tabel`
//!   met `elke_regel` of `een_regel` (en `alleen_waar`), en `moment`;
//! - **over een verzameling grammen**, met een eigen `filter`: `bestaat`,
//!   `som`, en `kies: laatste` met `veld`, `jaar_van` of `bevat`.
//!
//! Geen gram betekent binnen de eigen kroniek "nee" (`bestaat`, `bevat`) of
//! nul (`som`): de cel spreekt alleen over haar eigen kroniek. Een waarde die
//! er niet is (`veld` op een leeg veld, `kies` zonder gram) blijft weg; er
//! wordt niets aangevuld, tenzij de definitie met `geen_gram` zegt hoe zij
//! afwezigheid leest (bijvoorbeeld null: niet gebeurd).
//!
//! Met `groepeer: zaakkenmerk` is een lexostatus een **lijst**: een regel per
//! zaak waarvan ten minste een gram door `filter` komt, en met `zonder` geen
//! gram door dat filter. `kies` en de afleidingen werken dan per zaak. Een
//! lijst is voor de afnemer, zoals een behandelaar met een werkvoorraad; ze
//! heeft geen parameters en gaat nooit naar de engine.
//!
//! `extra_velden` leidt waarden af die geen parameter zijn, zoals de invoer
//! van een synthese-bron. Ze staan apart in de lexostatus en gaan nooit naar
//! de engine. Veldpaden zijn relatief aan `fields` van het gram, met punten.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::datum;
use crate::laden;
use crate::schema::Soort;
use crate::stroom::Gram;

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
    /// Artikelen van een afnemer (`<regeling>#<artikel>`) waarvan deze
    /// lexostatus parameters levert.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub levert_aan: Vec<String>,
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

/// De filtersleutels die een veld van het gram zelf zijn, geen veldpad.
pub const GRAM_SLEUTELS: &[&str] = &[
    "name",
    "type",
    "soort",
    "stage",
    "zaakkenmerk",
    "recording_actor",
    "chronicle",
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
    pub afleidingen: BTreeMap<String, Afleiding>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_velden: BTreeMap<String, Afleiding>,
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
    /// Het gram met het laatste `op_moment`; een herstel is een nieuw gram.
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
    /// Over de grammen door `filter`: of het lijstveld in het laatste de
    /// waarde bevat.
    LaatsteBevat {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        kies: Kies,
        bevat: Bevat,
    },
    /// Over de grammen door `filter`: of er ten minste een is.
    Bestaat {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        bestaat: bool,
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Moment {
    OpMoment,
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
    pub fn alle_afleidingen(&self) -> impl Iterator<Item = (&String, &Afleiding)> {
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
            | Afleiding::LaatsteJaarVan { filter, .. }
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
            Afleiding::Gevuld { gevuld } => vec![gevuld.as_str()],
            Afleiding::Gelijk { gelijk } => vec![gelijk.veld.as_str()],
            Afleiding::ElkeRegel { tabel, .. } | Afleiding::EenRegel { tabel, .. } => {
                vec![tabel.as_str()]
            }
            Afleiding::Som { som, .. } => vec![som.as_str()],
            Afleiding::Verzamel { verzamel, .. } => verzamel.iter().map(String::as_str).collect(),
            Afleiding::LaatsteBevat { bevat, .. } => vec![bevat.veld.as_str()],
            Afleiding::Bestaat { .. } | Afleiding::Moment { .. } => vec![],
        };
        if let Some(f) = self.filter() {
            paden.extend(filter_paden(f));
        }
        paden
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

    /// Pas een afleiding op het gekozen gram toe. `None`: het gram zegt er
    /// niets over en de parameter blijft weg; er wordt niets aangevuld. Een
    /// afleiding over een verzameling grammen geeft hier `None`; zie
    /// [`Afleiding::pas_toe_op_verzameling`]. Een gram dat niet de vorm heeft
    /// die de afleiding leest (een ongeldig `op_moment`, een tabelregel die
    /// geen object is) is een fout, geen ontbrekende waarde.
    pub fn pas_toe(&self, gram: &Gram) -> Result<Option<Value>, String> {
        Ok(match self {
            Afleiding::Veld { veld } => gram.veld(veld).filter(|w| gevuld(w)).cloned(),
            Afleiding::JaarVan { jaar_van } => gram.veld(jaar_van).and_then(jaar_uit),
            Afleiding::Gevuld { gevuld: pad } => {
                Some(Value::Bool(gram.veld(pad).is_some_and(gevuld)))
            }
            Afleiding::Gelijk { gelijk } => gram
                .veld(&gelijk.veld)
                .filter(|w| gevuld(w))
                .map(|w| Value::Bool(w == &gelijk.aan)),
            Afleiding::ElkeRegel {
                tabel,
                elke_regel,
                alleen_waar,
            } => {
                let rijen = rijen(gram, tabel)?;
                let elk = rijen
                    .iter()
                    .filter(|r| {
                        alleen_waar
                            .as_ref()
                            .is_none_or(|k| r.get(k) == Some(&Value::Bool(true)))
                    })
                    .all(|r| r.get(elke_regel).is_some_and(gevuld));
                Some(Value::Bool(!rijen.is_empty() && elk))
            }
            Afleiding::EenRegel { tabel, een_regel } => Some(Value::Bool(
                rijen(gram, tabel)?
                    .iter()
                    .any(|r| r.get(een_regel) == Some(&Value::Bool(true))),
            )),
            Afleiding::Moment {
                moment: Moment::OpMoment,
            } => Some(Value::String(datum::peildatum(&gram.moment()?))),
            Afleiding::LaatsteVeld { .. }
            | Afleiding::LaatsteJaarVan { .. }
            | Afleiding::LaatsteBevat { .. }
            | Afleiding::Bestaat { .. }
            | Afleiding::Som { .. }
            | Afleiding::Verzamel { .. } => None,
        })
    }

    /// Pas een afleiding over een verzameling toe op de grammen die al door
    /// het filter van de lexostatus kwamen. Ze filtert zelf verder.
    pub fn pas_toe_op_verzameling(
        &self,
        inputs: &Map<String, Value>,
        grammen: &[&Gram],
    ) -> Result<Option<Value>, String> {
        let Some(filter) = self.filter() else {
            return Ok(None);
        };
        let mut door = Vec::new();
        for g in grammen {
            if past(filter, inputs, g)? {
                door.push(*g);
            }
        }
        Ok(match self {
            Afleiding::Bestaat { .. } => Some(Value::Bool(!door.is_empty())),
            Afleiding::Verzamel { verzamel, .. } => Some(Value::Array(
                door.iter()
                    .map(|g| {
                        Value::Object(
                            verzamel
                                .iter()
                                .map(|v| (v.clone(), g.veld(v).cloned().unwrap_or(Value::Null)))
                                .collect(),
                        )
                    })
                    .collect(),
            )),
            Afleiding::Som { som, .. } => {
                let (mut geheel, mut reeel, mut alleen_geheel) = (0_i64, 0.0_f64, true);
                for g in &door {
                    match g.veld(som) {
                        None | Some(Value::Null) => {}
                        Some(Value::Number(n)) => {
                            match n.as_i64() {
                                Some(i) => geheel = geheel.saturating_add(i),
                                None => alleen_geheel = false,
                            }
                            reeel += n.as_f64().unwrap_or(0.0);
                        }
                        // Geen getal: de som is niet af te leiden.
                        Some(_) => return Ok(None),
                    }
                }
                if alleen_geheel {
                    Some(Value::from(geheel))
                } else {
                    serde_json::Number::from_f64(reeel).map(Value::Number)
                }
            }
            Afleiding::LaatsteVeld {
                veld, geen_gram, ..
            } => match laatste(&door)? {
                Some(g) => g.veld(veld).filter(|w| gevuld(w)).cloned(),
                None => geen_gram.clone(),
            },
            Afleiding::LaatsteJaarVan {
                jaar_van,
                geen_gram,
                ..
            } => match laatste(&door)? {
                Some(g) => g.veld(jaar_van).and_then(jaar_uit),
                None => geen_gram.clone(),
            },
            Afleiding::LaatsteBevat { bevat, .. } => Some(Value::Bool(
                laatste(&door)?
                    .and_then(|g| g.veld(&bevat.veld))
                    .and_then(Value::as_array)
                    .is_some_and(|lijst| lijst.contains(&bevat.waarde)),
            )),
            _ => None,
        })
    }
}

/// Het gram met het laatste `op_moment`; bij gelijk moment het later
/// toegevoegde.
fn laatste<'g>(grammen: &[&'g Gram]) -> Result<Option<&'g Gram>, String> {
    let mut gekozen: Option<(DateTime<chrono::FixedOffset>, &Gram)> = None;
    for gram in grammen {
        let moment = gram.moment()?;
        if gekozen.as_ref().is_none_or(|(m, _)| moment >= *m) {
            gekozen = Some((moment, gram));
        }
    }
    Ok(gekozen.map(|(_, g)| g))
}

/// Het jaartal van een datum (`JJJJ-MM-DD`, of een moment met tijdzone).
/// Geen datum: niets, en de parameter blijft weg.
pub fn jaar_uit(waarde: &Value) -> Option<Value> {
    datum::jaar_van(waarde.as_str()?).map(Value::from)
}

/// Gevuld: niet null, geen lege tekst, geen lege lijst of leeg object.
pub fn gevuld(w: &Value) -> bool {
    match w {
        Value::Null => false,
        Value::String(s) => !s.trim().is_empty(),
        Value::Array(a) => !a.is_empty(),
        Value::Object(o) => !o.is_empty(),
        _ => true,
    }
}

/// De regels van een tabelveld. Geen tabel (of null): geen regels. Een
/// waarde die geen lijst van objecten is, is een fout.
fn rijen<'g>(gram: &'g Gram, tabel: &str) -> Result<Vec<&'g Map<String, Value>>, String> {
    let regels = match gram.veld(tabel) {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => {
            return Err(format!(
                "gram '{}': tabelveld '{tabel}' is geen lijst van regels",
                gram.name
            ))
        }
    };
    regels
        .iter()
        .enumerate()
        .map(|(i, r)| {
            r.as_object().ok_or_else(|| {
                format!(
                    "gram '{}': regel {tabel}[{i}] is geen object met kolommen",
                    gram.name
                )
            })
        })
        .collect()
}

/// Een lexostatus: de parameters die een reductie oplevert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lexostatus {
    pub naam: String,
    /// Het gekozen gram, als de definitie er een kiest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub zaakkenmerk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_moment: Option<String>,
    pub parameters: BTreeMap<String, Value>,
    /// Waarden die geen parameter zijn; ze gaan nooit naar de engine.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_velden: BTreeMap<String, Value>,
    /// Parameters met een afleiding waarover de kroniek niets zegt.
    #[serde(default)]
    pub niet_afgeleid: Vec<String>,
    /// Bij een lijst-lexostatus (`groepeer`): de regels. Een lijst is voor de
    /// afnemer en gaat nooit naar de engine; `parameters` is dan leeg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lijst: Option<Vec<Regel>>,
}

impl Lexostatus {
    /// Een lexostatus zonder waarden: de kroniek zegt er niets over.
    pub fn leeg(naam: impl Into<String>) -> Self {
        Self {
            naam: naam.into(),
            zaakkenmerk: None,
            op_moment: None,
            parameters: BTreeMap::new(),
            extra_velden: BTreeMap::new(),
            niet_afgeleid: Vec::new(),
            lijst: None,
        }
    }

    /// De waarde die de lexostatus onder een naam levert, als parameter of
    /// als extra veld; `None` als die er niet is of null is.
    pub fn veld(&self, naam: &str) -> Option<&Value> {
        self.parameters
            .get(naam)
            .or_else(|| self.extra_velden.get(naam))
            .filter(|w| !w.is_null())
    }
}

/// Een regel van een lijst-lexostatus: een zaak.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regel {
    pub zaakkenmerk: String,
    /// Het gekozen gram van de zaak, als de definitie er een kiest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub op_moment: Option<String>,
    /// De afleidingen en extra velden, per zaak. Geen parameters.
    pub velden: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub niet_afgeleid: Vec<String>,
}

/// De parameters van een lexostatus die zeggen dat iets ontbreekt: een
/// aanwezigheidsafleiding (zie [`Afleiding::toetst_aanwezigheid`]) met de
/// waarde onwaar.
pub fn ontbreekt(
    definitie: &LexostatusDefinitie,
    parameters: &BTreeMap<String, Value>,
) -> Vec<String> {
    definitie
        .reduction
        .afleidingen
        .iter()
        .filter(|(naam, a)| {
            a.toetst_aanwezigheid() && parameters.get(*naam) == Some(&Value::Bool(false))
        })
        .map(|(naam, _)| naam.clone())
        .collect()
}

/// Of een gram door het filter komt. Een waarde `$x` komt uit de inputs.
pub fn past(filter: &Filter, inputs: &Map<String, Value>, gram: &Gram) -> Result<bool, String> {
    for (sleutel, verwacht) in filter {
        let verwacht = match verwacht.strip_prefix('$') {
            Some(input) => inputs
                .get(input)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("input '{input}' ontbreekt"))?,
            None => verwacht.as_str(),
        };
        let gelijk = match sleutel.as_str() {
            "name" => gram.name == verwacht,
            "type" => gram.type_ == verwacht,
            "soort" => gram.soort.as_deref() == Some(verwacht),
            "stage" => gram.stage.as_deref() == Some(verwacht),
            "zaakkenmerk" => gram.zaakkenmerk.as_deref() == Some(verwacht),
            "recording_actor" => gram.recording_actor == verwacht,
            "chronicle" => gram.chronicle == verwacht,
            pad => match gram.veld(pad) {
                Some(Value::String(s)) => s == verwacht,
                Some(w @ (Value::Number(_) | Value::Bool(_))) => *w.to_string() == *verwacht,
                _ => false,
            },
        };
        if !gelijk {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Wat de afleidingen over een groep grammen opleveren.
struct Afgeleid<'g> {
    gekozen: Option<&'g Gram>,
    parameters: BTreeMap<String, Value>,
    extra_velden: BTreeMap<String, Value>,
    niet_afgeleid: Vec<String>,
}

/// Kies zo nodig een gram uit `door` en pas elke afleiding toe. `None`: de
/// definitie kiest een gram en er is er geen.
fn leid_af_uit<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    door: &[&'g Gram],
) -> Result<Option<Afgeleid<'g>>, String> {
    let r = &definitie.reduction;
    let gekozen = match r.kies {
        Some(Kies::Laatste) => match laatste(door)? {
            Some(g) => Some(g),
            None => return Ok(None),
        },
        None => None,
    };
    let mut uit = Afgeleid {
        gekozen,
        parameters: BTreeMap::new(),
        extra_velden: BTreeMap::new(),
        niet_afgeleid: Vec::new(),
    };
    for (naam, afleiding, extra) in r
        .afleidingen
        .iter()
        .map(|(n, a)| (n, a, false))
        .chain(r.extra_velden.iter().map(|(n, a)| (n, a, true)))
    {
        let waarde = if afleiding.op_gekozen_gram() {
            let gram = gekozen.ok_or_else(|| {
                format!(
                    "afleiding '{naam}' leest het gekozen gram, maar lexostatus '{}' kiest er geen",
                    definitie.name
                )
            })?;
            afleiding.pas_toe(gram)?
        } else {
            afleiding.pas_toe_op_verzameling(inputs, door)?
        };
        match (waarde, extra) {
            (Some(w), false) => {
                uit.parameters.insert(naam.clone(), w);
            }
            (Some(w), true) => {
                uit.extra_velden.insert(naam.clone(), w);
            }
            (None, false) => uit.niet_afgeleid.push(naam.clone()),
            (None, true) => {}
        }
    }
    Ok(Some(uit))
}

/// De grammen die door een filter komen.
fn door_filter<'g>(
    filter: &Filter,
    inputs: &Map<String, Value>,
    grammen: impl IntoIterator<Item = &'g Gram>,
) -> Result<Vec<&'g Gram>, String> {
    let mut door = Vec::new();
    for gram in grammen {
        if past(filter, inputs, gram)? {
            door.push(gram);
        }
    }
    Ok(door)
}

/// Reduceer de grammen van een kroniek tot een lexostatus. `None`: de
/// definitie kiest een gram (`kies`) en geen gram komt door het filter. Een
/// lijst-lexostatus (`groepeer`) geeft altijd een lexostatus, met een lege
/// lijst als geen zaak past.
pub fn reduceer<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: impl IntoIterator<Item = &'g Gram>,
) -> Result<Option<Lexostatus>, String> {
    let r = &definitie.reduction;
    let in_kroniek = grammen.into_iter().filter(|g| g.chronicle == r.kroniek);
    if r.groepeer.is_some() {
        return reduceer_lijst(definitie, inputs, in_kroniek).map(Some);
    }
    let door = door_filter(&r.filter, inputs, in_kroniek)?;
    let Some(a) = leid_af_uit(definitie, inputs, &door)? else {
        return Ok(None);
    };
    Ok(Some(Lexostatus {
        zaakkenmerk: a.gekozen.and_then(|g| g.zaakkenmerk.clone()),
        op_moment: a.gekozen.map(|g| g.op_moment.clone()),
        parameters: a.parameters,
        extra_velden: a.extra_velden,
        niet_afgeleid: a.niet_afgeleid,
        ..Lexostatus::leeg(&definitie.name)
    }))
}

/// Een lijst-lexostatus: groepeer per zaakkenmerk, houd de zaken met een gram
/// door `filter` en zonder gram door `zonder`, en leid per zaak af. De regels
/// staan op het moment van het gekozen gram, de oudste eerst.
fn reduceer_lijst<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: impl IntoIterator<Item = &'g Gram>,
) -> Result<Lexostatus, String> {
    let r = &definitie.reduction;
    let mut zaken: BTreeMap<&str, Vec<&Gram>> = BTreeMap::new();
    for g in grammen {
        // Een gram zonder zaak hoort in geen regel.
        if let Some(z) = g.zaakkenmerk.as_deref() {
            zaken.entry(z).or_default().push(g);
        }
    }
    let mut regels = Vec::new();
    for (zaak, groep) in zaken {
        if !r.zonder.is_empty()
            && !door_filter(&r.zonder, inputs, groep.iter().copied())?.is_empty()
        {
            continue;
        }
        let door = door_filter(&r.filter, inputs, groep.iter().copied())?;
        if door.is_empty() {
            continue;
        }
        let Some(a) = leid_af_uit(definitie, inputs, &door)? else {
            continue;
        };
        let moment = a.gekozen.map(Gram::moment).transpose()?;
        let mut velden = a.parameters;
        velden.extend(a.extra_velden);
        regels.push((
            moment,
            Regel {
                zaakkenmerk: zaak.to_string(),
                op_moment: a.gekozen.map(|g| g.op_moment.clone()),
                velden,
                niet_afgeleid: a.niet_afgeleid,
            },
        ));
    }
    regels.sort_by(|(a, ra), (b, rb)| a.cmp(b).then_with(|| ra.zaakkenmerk.cmp(&rb.zaakkenmerk)));
    Ok(Lexostatus {
        lijst: Some(regels.into_iter().map(|(_, r)| r).collect()),
        ..Lexostatus::leeg(&definitie.name)
    })
}

/// Reduceer een enkel gram, zoals een concept dat nog geen feit is. Heeft
/// het gram een zaakkenmerk, dan is dat de input `zaakkenmerk`.
pub fn leid_af(definitie: &LexostatusDefinitie, gram: &Gram) -> Result<Lexostatus, String> {
    let mut inputs = Map::new();
    if let Some(z) = &gram.zaakkenmerk {
        inputs.insert("zaakkenmerk".into(), Value::String(z.clone()));
    }
    reduceer(definitie, &inputs, std::slice::from_ref(gram))?
        .ok_or_else(|| "de reductie vond het gram niet".to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::stroom::{StroomVerwijzing, Zaak};
    use serde_json::json;

    const CEL: &str = include_str!("../tests/fixtures/cellen/instantie/lexostatussen.yaml");

    fn gram(zaak: &str, moment: &str, fields: Value) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            type_: "indiening".into(),
            soort: Some("aanvraag".into()),
            stage: None,
            name: "aanvraag_ontvangen".into(),
            chronicle: "test_kroniek".into(),
            recording_actor: "test_instantie".into(),
            grondslag: vec!["testregeling_aanvraag#1".into()],
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            competent_authority: None,
            op_moment: moment.into(),
            zaak: Zaak::Opent,
            zaakkenmerk: Some(zaak.into()),
            stroom: StroomVerwijzing {
                id: "test_aanvragen".into(),
                sha256: "0".repeat(64),
            },
            herkomst: None,
            fields: fields.as_object().unwrap().clone(),
            inputs: BTreeMap::new(),
            receipt: None,
        }
    }

    fn afl(yaml: &str) -> Afleiding {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    fn een(yaml: &str, fields: Value) -> Option<Value> {
        afl(yaml)
            .pas_toe(&gram("z", "2025-03-12T10:14:03+01:00", fields))
            .unwrap()
    }

    const ZAAK: &str = "00000000-0000-4000-8000-000000000001";

    #[test]
    fn fixture_laadt() {
        let c = parse(CEL, "fixture").unwrap();
        assert_eq!(c.cel, "test_instantie");
        assert_eq!(c.lexostatus_definitions[0].reduction.afleidingen.len(), 9);
    }

    #[test]
    fn ongeldige_afleiding_faalt_op_het_schema() {
        let tekst = CEL.replace(
            "{veld: inhoud.aanvraagjaar}",
            "{optellen: inhoud.aanvraagjaar}",
        );
        let fout = parse(&tekst, "t").unwrap_err();
        assert!(
            fout.iter().any(|f| f.contains("afleidingen/aanvraagjaar")),
            "{fout:?}"
        );
    }

    #[test]
    fn afleiding_veld() {
        let f = json!({"a": {"jaar": 2025, "leeg": ""}});
        assert_eq!(een("{veld: a.jaar}", f.clone()), Some(json!(2025)));
        assert_eq!(een("{veld: a.leeg}", f.clone()), None);
        assert_eq!(een("{veld: a.ontbreekt}", f), None);
    }

    #[test]
    fn afleiding_gevuld() {
        let f = json!({"a": {"naam": "X", "leeg": " ", "lijst": []}});
        assert_eq!(een("{gevuld: a.naam}", f.clone()), Some(json!(true)));
        assert_eq!(een("{gevuld: a.leeg}", f.clone()), Some(json!(false)));
        assert_eq!(een("{gevuld: a.lijst}", f.clone()), Some(json!(false)));
        assert_eq!(een("{gevuld: a.ontbreekt}", f), Some(json!(false)));
    }

    #[test]
    fn afleiding_gelijk() {
        let f = json!({"a": {"reg": "a"}});
        assert_eq!(
            een("{gelijk: {veld: a.reg, aan: a}}", f.clone()),
            Some(json!(true))
        );
        assert_eq!(
            een("{gelijk: {veld: a.reg, aan: b}}", f.clone()),
            Some(json!(false))
        );
        assert_eq!(een("{gelijk: {veld: a.x, aan: b}}", f), None);
    }

    #[test]
    fn afleiding_tabel_elke_regel() {
        let f = json!({"t": [{"o": "raad", "z": 3}, {"o": "staten", "z": null}]});
        assert_eq!(
            een("{tabel: t, elke_regel: o}", f.clone()),
            Some(json!(true))
        );
        assert_eq!(een("{tabel: t, elke_regel: z}", f), Some(json!(false)));
        // Een lege of ontbrekende tabel bevat niets.
        assert_eq!(
            een("{tabel: t, elke_regel: o}", json!({"t": []})),
            Some(json!(false))
        );
        assert_eq!(
            een("{tabel: t, elke_regel: o}", json!({})),
            Some(json!(false))
        );
    }

    #[test]
    fn afleiding_tabel_alleen_waar() {
        let f = json!({"t": [{"s": false}, {"s": true, "n": 2}]});
        assert_eq!(
            een("{tabel: t, elke_regel: n, alleen_waar: s}", f),
            Some(json!(true))
        );
        let f = json!({"t": [{"s": true}, {"s": true, "n": 2}]});
        assert_eq!(
            een("{tabel: t, elke_regel: n, alleen_waar: s}", f),
            Some(json!(false))
        );
        // Geen regel waar: niets te missen, zolang de tabel regels heeft.
        let f = json!({"t": [{"s": false}]});
        assert_eq!(
            een("{tabel: t, elke_regel: n, alleen_waar: s}", f),
            Some(json!(true))
        );
    }

    #[test]
    fn afleiding_tabel_een_regel() {
        let f = json!({"t": [{"s": false}, {"s": true}]});
        assert_eq!(een("{tabel: t, een_regel: s}", f), Some(json!(true)));
        let f = json!({"t": [{"s": false}, {"s": "true"}]});
        assert_eq!(een("{tabel: t, een_regel: s}", f), Some(json!(false)));
    }

    #[test]
    fn afleiding_jaar_van() {
        let f = json!({"a": {"datum": "2026-03-18", "leeg": null, "tekst": "geen datum"}});
        assert_eq!(een("{jaar_van: a.datum}", f.clone()), Some(json!(2026)));
        assert_eq!(een("{jaar_van: a.leeg}", f.clone()), None);
        assert_eq!(een("{jaar_van: a.tekst}", f.clone()), None);
        assert_eq!(een("{jaar_van: a.ontbreekt}", f), None);
        assert_eq!(afl("{jaar_van: a.datum}").gelezen_paden(), vec!["a.datum"]);
    }

    #[test]
    fn afleiding_jaar_van_over_een_verzameling() {
        let a = afl("{filter: {name: x}, kies: laatste, jaar_van: datum, geen_gram: null}");
        assert!(matches!(a, Afleiding::LaatsteJaarVan { .. }));
        assert!(!a.op_gekozen_gram());
        let oud = besluit(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"datum": "2024-03-20"}),
        );
        let nieuw = besluit(
            "x",
            "2025-03-20T09:00:00+01:00",
            json!({"datum": "2025-01-02"}),
        );
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[&oud, &nieuw])
                .unwrap(),
            Some(json!(2025))
        );
        // Geen gram: de lezing van afwezigheid.
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[]).unwrap(),
            Some(Value::Null)
        );
        // Een gram zonder datum levert niets; er wordt niets aangevuld.
        let leeg = besluit("x", "2024-03-20T09:00:00+01:00", json!({"datum": null}));
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[&leeg]).unwrap(),
            None
        );
    }

    #[test]
    fn afleiding_moment() {
        // De datum in de eigen tijdzone van het moment.
        let g = gram("z", "2025-03-12T00:30:00+01:00", json!({}));
        assert_eq!(
            afl("{moment: op_moment}").pas_toe(&g).unwrap(),
            Some(json!("2025-03-12"))
        );
    }

    #[test]
    fn een_ongeldig_op_moment_is_een_fout_en_geen_lege_waarde() {
        let g = gram("z", "12 maart 2025", json!({}));
        let f = afl("{moment: op_moment}").pas_toe(&g).unwrap_err();
        assert!(f.contains("ongeldig op_moment '12 maart 2025'"), "{f}");
        // Ook `kies: laatste` kiest niet stil om zo'n gram heen.
        let def: LexostatusDefinitie = serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {kroniek: test_kroniek, kies: laatste, afleidingen: {x: {veld: a}}}}",
        )
        .unwrap();
        let goed = gram("z", "2025-03-12T10:14:03+01:00", json!({"a": 1}));
        assert!(reduceer(&def, &Map::new(), &[goed, g])
            .unwrap_err()
            .contains("ongeldig op_moment"));
    }

    #[test]
    fn kies_laatste_vergelijkt_momenten_over_tijdzones_heen() {
        // Op de klok van hun eigen tijdzone lijkt de volgorde anders dan ze
        // is: 10:15+02:00 is 08:15 UTC, 10:00+01:00 is 09:00 UTC en
        // 09:30+00:00 is 09:30 UTC, het laatst.
        let def: LexostatusDefinitie = serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {kroniek: test_kroniek, kies: laatste, afleidingen: {x: {veld: a}}}}",
        )
        .unwrap();
        let grammen = [
            gram("z", "2025-03-12T09:30:00+00:00", json!({"a": "utc"})),
            gram("z", "2025-03-12T10:15:00+02:00", json!({"a": "oost"})),
            gram("z", "2025-03-12T10:00:00+01:00", json!({"a": "nl"})),
        ];
        let l = reduceer(&def, &Map::new(), &grammen).unwrap().unwrap();
        assert_eq!(l.parameters["x"], json!("utc"));
        assert_eq!(l.op_moment.as_deref(), Some("2025-03-12T09:30:00+00:00"));
    }

    #[test]
    fn een_tabelregel_die_geen_object_is_is_een_fout() {
        let g = gram(
            "z",
            "2025-03-12T10:14:03+01:00",
            json!({"t": [{"k": true}, 3]}),
        );
        let f = afl("{tabel: t, een_regel: k}").pas_toe(&g).unwrap_err();
        assert!(f.contains("regel t[1] is geen object"), "{f}");
        let g = gram("z", "2025-03-12T10:14:03+01:00", json!({"t": null}));
        assert_eq!(
            afl("{tabel: t, een_regel: k}").pas_toe(&g).unwrap(),
            Some(json!(false))
        );
    }

    #[test]
    fn afleiding_leest_paden() {
        assert_eq!(
            afl("{gelijk: {veld: a.b, aan: 1}}").gelezen_paden(),
            vec!["a.b"]
        );
        assert_eq!(afl("{tabel: t, een_regel: s}").gelezen_paden(), vec!["t"]);
        assert!(afl("{moment: op_moment}").gelezen_paden().is_empty());
    }

    #[test]
    fn reductie_kiest_laatste_per_zaak() {
        let c = parse(CEL, "fixture").unwrap();
        let def = &c.lexostatus_definitions[0];
        let grammen = vec![
            gram(
                ZAAK,
                "2025-03-01T09:00:00+01:00",
                json!({"inhoud": {"naam": "Eerst"}}),
            ),
            gram(
                ZAAK,
                "2025-03-02T09:00:00+01:00",
                json!({"inhoud": {"naam": "Herstel"}}),
            ),
            gram(
                "00000000-0000-4000-8000-000000000002",
                "2025-03-03T09:00:00+01:00",
                json!({"inhoud": {}}),
            ),
        ];
        let inputs = json!({"zaakkenmerk": ZAAK});
        let l = reduceer(def, inputs.as_object().unwrap(), &grammen)
            .unwrap()
            .unwrap();
        assert_eq!(l.op_moment.as_deref(), Some("2025-03-02T09:00:00+01:00"));
        assert_eq!(l.parameters["bevat_naam"], json!(true));
        assert_eq!(l.parameters["aanvraagdatum"], json!("2025-03-02"));
        assert!(l.niet_afgeleid.contains(&"aanvraagjaar".to_string()));
    }

    #[test]
    fn ontbreekt_noemt_alleen_aanwezigheidsafleidingen() {
        let c = parse(CEL, "fixture").unwrap();
        let def = &c.lexostatus_definitions[0];
        let g = gram(
            ZAAK,
            "2025-03-01T09:00:00+01:00",
            json!({"inhoud": {"naam": "X", "registratie": "b",
                "organen": [{"orgaan": "raad", "samengevoegd": false}]}}),
        );
        let l = leid_af(def, &g).unwrap();
        // Onwaar, maar een antwoord: registratie_categorie_a (gelijk) en
        // is_samengevoegd (een_regel). Onwaar en een gat: de rest.
        assert_eq!(l.parameters["registratie_categorie_a"], json!(false));
        assert_eq!(l.parameters["is_samengevoegd"], json!(false));
        assert_eq!(
            ontbreekt(def, &l.parameters),
            vec!["bevat_aanduiding", "bevat_aantal_zetels"]
        );
    }

    #[test]
    fn reductie_zonder_passend_gram() {
        let c = parse(CEL, "fixture").unwrap();
        let inputs = json!({"zaakkenmerk": ZAAK});
        let l = reduceer(
            &c.lexostatus_definitions[0],
            inputs.as_object().unwrap(),
            &[],
        )
        .unwrap();
        assert!(l.is_none());
    }

    #[test]
    fn reductie_zonder_input_is_een_fout() {
        let c = parse(CEL, "fixture").unwrap();
        let g = gram(ZAAK, "2025-03-01T09:00:00+01:00", json!({}));
        let fout = reduceer(&c.lexostatus_definitions[0], &Map::new(), &[g]).unwrap_err();
        assert!(fout.contains("zaakkenmerk"), "{fout}");
    }

    #[test]
    fn filter_op_zaakkenmerk_laat_een_gram_zonder_zaak_niet_door() {
        let mut filter = Filter::new();
        filter.insert("zaakkenmerk".into(), "$zaakkenmerk".into());
        let inputs = json!({"zaakkenmerk": ZAAK});
        let inputs = inputs.as_object().unwrap();
        let mut g = gram(ZAAK, "2025-03-01T09:00:00+01:00", json!({}));
        assert!(past(&filter, inputs, &g).unwrap());
        g.zaak = Zaak::Geen;
        g.zaakkenmerk = None;
        assert!(!past(&filter, inputs, &g).unwrap());
    }

    const REGISTER: &str = include_str!("../tests/fixtures/cellen/register/lexostatussen.yaml");

    fn besluit(name: &str, moment: &str, fields: Value) -> Gram {
        let mut g = gram(ZAAK, moment, fields);
        g.zaak = Zaak::Geen;
        g.zaakkenmerk = None;
        g.type_ = "decretogram".into();
        g.soort = None;
        g.name = name.into();
        g.chronicle = "test_register".into();
        g
    }

    fn register() -> Vec<Gram> {
        vec![
            besluit(
                "aanduiding_ingeschreven",
                "2024-01-10T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "orgaan": "raad", "gebied": "A"}),
            ),
            besluit(
                "uitslag_vastgesteld",
                "2024-03-20T09:00:00+01:00",
                json!({"lijst": "VOORBEELD", "orgaan": "raad", "gebied": "A", "zetels": 4}),
            ),
            besluit(
                "uitslag_vastgesteld",
                "2024-03-21T09:00:00+01:00",
                json!({"lijst": "VOORBEELD", "orgaan": "raad", "gebied": "B", "zetels": 2}),
            ),
            besluit(
                "uitslag_vastgesteld",
                "2024-03-21T09:00:00+01:00",
                json!({"lijst": "ANDERS", "orgaan": "raad", "gebied": "B", "zetels": 7}),
            ),
            besluit(
                "mededeling_gedaan",
                "2024-11-01T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "datum": "2024-11-01", "geblokkeerd_voor": ["staten"]}),
            ),
            besluit(
                "mededeling_gedaan",
                "2024-12-01T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "datum": "2024-12-01", "geblokkeerd_voor": ["raad"]}),
            ),
        ]
    }

    /// `verzamel` maakt van de grammen door het filter een lijst met een
    /// regel per gram; een veld dat een gram niet heeft, is null.
    #[test]
    fn verzamel_geeft_een_regel_per_gram() {
        let a: Afleiding = serde_yaml_ng::from_str(
            "{filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, verzamel: [gebied, zetels, samengevoegd]}",
        )
        .unwrap();
        let grammen = register();
        let refs: Vec<&Gram> = grammen.iter().collect();
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let w = a
            .pas_toe_op_verzameling(inputs.as_object().unwrap(), &refs)
            .unwrap();
        assert_eq!(
            w,
            Some(json!([
                {"gebied": "A", "zetels": 4, "samengevoegd": null},
                {"gebied": "B", "zetels": 2, "samengevoegd": null}
            ]))
        );
    }

    fn registerstatus(aanduiding: &str) -> Lexostatus {
        let c = parse(REGISTER, "register").unwrap();
        let inputs = json!({"aanduiding": aanduiding});
        reduceer(
            &c.lexostatus_definitions[0],
            inputs.as_object().unwrap(),
            &register(),
        )
        .unwrap()
        .unwrap()
    }

    #[test]
    fn filter_per_afleiding_bestaat_som_en_laatste() {
        let l = registerstatus("VOORBEELD");
        assert_eq!(l.zaakkenmerk, None, "zonder kies wordt geen gram gekozen");
        assert_eq!(l.parameters["is_ingeschreven_raad"], json!(true));
        assert_eq!(l.parameters["is_geschrapt_raad"], json!(false));
        // Alleen de uitslagen met deze aanduiding boven de lijst, over alle gebieden.
        assert_eq!(l.parameters["zetels_op_lijst"], json!(6));
        assert_eq!(l.parameters["datum_mededeling"], json!("2024-12-01"));
        // Het laatste gram telt: daar staat raad wel in de lijst.
        assert_eq!(l.parameters["geblokkeerd_raad"], json!(true));
        assert!(l.niet_afgeleid.is_empty(), "{:?}", l.niet_afgeleid);
    }

    #[test]
    fn afwezigheid_in_de_eigen_kroniek_is_nee() {
        let l = registerstatus("ONBEKEND");
        assert_eq!(l.parameters["is_ingeschreven_raad"], json!(false));
        assert_eq!(l.parameters["zetels_op_lijst"], json!(0));
        assert_eq!(l.parameters["geblokkeerd_raad"], json!(false));
        // Een datum is er niet: die blijft weg, er wordt niets aangevuld.
        assert!(!l.parameters.contains_key("datum_mededeling"));
        assert_eq!(l.niet_afgeleid, vec!["datum_mededeling", "jaar"]);
    }

    #[test]
    fn som_zonder_getal_is_niet_af_te_leiden() {
        let a = afl("{filter: {name: uitslag_vastgesteld}, som: zetels}");
        let g = besluit(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": "vier"}),
        );
        assert_eq!(a.pas_toe_op_verzameling(&Map::new(), &[&g]).unwrap(), None);
        let h = besluit(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": 1.5}),
        );
        let i = besluit(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": 2}),
        );
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[&h, &i]).unwrap(),
            Some(json!(3.5))
        );
    }

    #[test]
    fn filter_op_een_veldpad_vergelijkt_ook_getallen() {
        let g = besluit(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"a": {"jaar": 2024, "ja": true}}),
        );
        let f: Filter = serde_json::from_value(json!({"a.jaar": "2024", "a.ja": "true"})).unwrap();
        assert!(past(&f, &Map::new(), &g).unwrap());
        let f: Filter = serde_json::from_value(json!({"a.jaar": "2025"})).unwrap();
        assert!(!past(&f, &Map::new(), &g).unwrap());
        let f: Filter = serde_json::from_value(json!({"a.ontbreekt": "x"})).unwrap();
        assert!(!past(&f, &Map::new(), &g).unwrap());
    }

    #[test]
    fn afleidingen_over_een_verzameling_lezen_ook_hun_filter() {
        let a = afl("{filter: {name: x, orgaan: raad, aanduiding: $aanduiding}, bestaat: true}");
        assert!(!a.op_gekozen_gram());
        assert_eq!(a.gelezen_paden(), vec!["aanduiding", "orgaan"]);
        let a = afl("{filter: {name: x}, kies: laatste, bevat: {veld: lijst, waarde: raad}}");
        assert!(matches!(a, Afleiding::LaatsteBevat { .. }));
        assert_eq!(a.gelezen_paden(), vec!["lijst"]);
        let a = afl("{filter: {name: x}, kies: laatste, veld: datum}");
        assert!(matches!(a, Afleiding::LaatsteVeld { .. }));
        assert!(afl("{veld: datum}").op_gekozen_gram());
    }

    #[test]
    fn extra_velden_staan_apart_van_de_parameters() {
        let mut c = parse(CEL, "fixture").unwrap();
        let def = &mut c.lexostatus_definitions[0];
        def.reduction
            .extra_velden
            .insert("aanduiding".into(), afl("{veld: inhoud.aanduiding}"));
        let g = gram(
            ZAAK,
            "2025-03-01T09:00:00+01:00",
            json!({"inhoud": {"aanduiding": "X"}}),
        );
        let l = leid_af(def, &g).unwrap();
        assert_eq!(l.extra_velden["aanduiding"], json!("X"));
        assert!(!l.parameters.contains_key("aanduiding"));
        assert!(def.levert("aanduiding") && def.levert("bevat_naam"));
    }

    #[test]
    fn afleiding_op_het_gram_zonder_kies_is_een_fout() {
        let mut c = parse(REGISTER, "register").unwrap();
        let def = &mut c.lexostatus_definitions[0];
        def.reduction
            .afleidingen
            .insert("x".into(), afl("{veld: aanduiding}"));
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let fout = reduceer(def, inputs.as_object().unwrap(), &register()).unwrap_err();
        assert!(fout.contains("kiest er geen"), "{fout}");
    }

    #[test]
    fn register_fixture_valideert_tegen_het_schema() {
        let c = parse(REGISTER, "register").unwrap();
        assert_eq!(c.cel, "test_register");
        assert_eq!(
            c.lexostatus_definitions[0].levert_aan,
            vec![
                "testregeling_afnemer#1",
                "testregeling_afnemer#2",
                "testregeling_afnemer#3"
            ]
        );
    }

    // --- Lijst-lexostatus: groepeer en zonder ---

    const WERKVOORRAAD: &str = "cel: c\nlexostatus_definitions:\n  - name: werkvoorraad\n    inputs: []\n    reduction:\n      kroniek: test_kroniek\n      filter: {type: indiening, soort: aanvraag}\n      groepeer: zaakkenmerk\n      zonder: {stage: BESLUIT}\n      kies: laatste\n      afleidingen:\n        ontvangen_op: {moment: op_moment}\n        naam: {veld: inhoud.naam}\n";

    fn stage(zaak: &str, moment: &str, stage: &str) -> Gram {
        let mut g = gram(zaak, moment, json!({}));
        g.type_ = "decretogram".into();
        g.soort = None;
        g.stage = Some(stage.into());
        g.name = "besluit".into();
        g.zaak = Zaak::Volgt;
        g
    }

    #[test]
    fn groepeer_per_zaak_zonder_besluit() {
        let c = parse(WERKVOORRAAD, "w").unwrap();
        let def = &c.lexostatus_definitions[0];
        assert!(def.is_lijst());
        let (a, b, d) = (
            "00000000-0000-4000-8000-00000000000a",
            "00000000-0000-4000-8000-00000000000b",
            "00000000-0000-4000-8000-00000000000d",
        );
        let mut zonder_zaak = gram(
            a,
            "2025-01-01T09:00:00+01:00",
            json!({"inhoud": {"naam": "Geen"}}),
        );
        zonder_zaak.zaak = Zaak::Geen;
        zonder_zaak.zaakkenmerk = None;
        let grammen = vec![
            // Zaak b is later ingediend maar komt eerst op kenmerk; de lijst
            // staat op moment.
            gram(
                b,
                "2025-03-05T09:00:00+01:00",
                json!({"inhoud": {"naam": "B"}}),
            ),
            gram(
                a,
                "2025-03-01T09:00:00+01:00",
                json!({"inhoud": {"naam": "A eerst"}}),
            ),
            gram(
                a,
                "2025-03-02T09:00:00+01:00",
                json!({"inhoud": {"naam": "A herstel"}}),
            ),
            // Zaak d heeft een besluit en valt af; een andere stage niet.
            gram(
                d,
                "2025-03-03T09:00:00+01:00",
                json!({"inhoud": {"naam": "D"}}),
            ),
            stage(d, "2025-03-04T09:00:00+01:00", "BESLUIT"),
            stage(b, "2025-03-06T09:00:00+01:00", "BEKENDMAKING"),
            zonder_zaak,
        ];
        let l = reduceer(def, &Map::new(), &grammen).unwrap().unwrap();
        assert!(l.parameters.is_empty());
        let lijst = l.lijst.unwrap();
        let zaken: Vec<&str> = lijst.iter().map(|r| r.zaakkenmerk.as_str()).collect();
        assert_eq!(zaken, [a, b]);
        assert_eq!(lijst[0].velden["naam"], json!("A herstel"));
        assert_eq!(lijst[0].velden["ontvangen_op"], json!("2025-03-02"));
        assert_eq!(
            lijst[0].op_moment.as_deref(),
            Some("2025-03-02T09:00:00+01:00")
        );
        // Een zaak met alleen een besluit en geen aanvraag telt niet.
        let alleen = vec![stage(a, "2025-03-04T09:00:00+01:00", "BEKENDMAKING")];
        let l = reduceer(def, &Map::new(), &alleen).unwrap().unwrap();
        assert!(l.lijst.unwrap().is_empty());
    }

    #[test]
    fn zonder_vraagt_groepeer() {
        let tekst = WERKVOORRAAD.replace("      groepeer: zaakkenmerk\n", "");
        let fout = parse(&tekst, "w").unwrap_err();
        assert!(fout.iter().any(|f| f.contains("groepeer")), "{fout:?}");
    }

    #[test]
    fn geen_gram_leest_afwezigheid() {
        let a = afl("{filter: {name: x}, kies: laatste, veld: datum, geen_gram: null}");
        assert!(matches!(
            &a,
            Afleiding::LaatsteVeld {
                geen_gram: Some(Value::Null),
                ..
            }
        ));
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[]).unwrap(),
            Some(Value::Null)
        );
        let g = besluit(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"datum": "2024-03-20"}),
        );
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[&g]).unwrap(),
            Some(json!("2024-03-20"))
        );
        // Een gram met een leeg veld is geen afwezigheid van het gram: weg.
        let leeg = besluit("x", "2024-03-20T09:00:00+01:00", json!({"datum": null}));
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[&leeg]).unwrap(),
            None
        );
        let onwaar = afl("{filter: {name: x}, kies: laatste, veld: ja, geen_gram: false}");
        assert_eq!(
            onwaar.pas_toe_op_verzameling(&Map::new(), &[]).unwrap(),
            Some(json!(false))
        );
        // Zonder geen_gram blijft de parameter weg.
        let zonder = afl("{filter: {name: x}, kies: laatste, veld: datum}");
        assert!(matches!(
            &zonder,
            Afleiding::LaatsteVeld {
                geen_gram: None,
                ..
            }
        ));
        assert_eq!(
            zonder.pas_toe_op_verzameling(&Map::new(), &[]).unwrap(),
            None
        );
    }

    #[test]
    fn filter_op_stage() {
        let f: Filter = serde_json::from_value(json!({"stage": "BESLUIT"})).unwrap();
        let z = "00000000-0000-4000-8000-00000000000a";
        assert!(past(
            &f,
            &Map::new(),
            &stage(z, "2025-03-04T09:00:00+01:00", "BESLUIT")
        )
        .unwrap());
        assert!(!past(
            &f,
            &Map::new(),
            &stage(z, "2025-03-04T09:00:00+01:00", "BEKENDMAKING")
        )
        .unwrap());
        assert!(!past(
            &f,
            &Map::new(),
            &gram(z, "2025-03-04T09:00:00+01:00", json!({}))
        )
        .unwrap());
    }
}
