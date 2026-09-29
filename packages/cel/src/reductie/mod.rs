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
//! Met `groepeer: wortel` is een lexostatus een **lijst**: een regel per
//! wortel (een aanvraag en wat erop volgt) waarvan ten minste een gram door
//! `filter` komt, en met `zonder` geen gram door dat filter. `kies` en de
//! afleidingen werken dan per wortel. Een
//! lijst is voor de afnemer, zoals een behandelaar met een werkvoorraad; ze
//! heeft geen parameters en gaat nooit naar de engine.
//!
//! `extra_velden` leidt waarden af die geen parameter zijn, zoals de invoer
//! van een synthese-bron. Ze staan apart in de lexostatus en gaan nooit naar
//! de engine. Veldpaden zijn relatief aan `fields` van het gram, met punten.
//!
//! `kies: laatste` kiest in de tijd: het gram met het laatste `op_moment`
//! (wanneer het feit rechtens geldt), bij gelijk moment het laatst
//! vastgelegde (`vastgelegd_op`), en daarna het laatst toegevoegde. Een
//! reductie kan op een eerder moment peilen ([`Peil`]): dan tellen alleen de
//! grammen van dat moment.

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use chrono::Datelike;

use crate::datum;
use crate::gram::Gram;

mod definitie;
mod peil;
mod zaakstand;

pub use definitie::*;
pub use peil::*;
pub use zaakstand::*;

impl Afleiding {
    /// Pas een afleiding op het gekozen gram toe. `None`: het gram zegt er
    /// niets over en de parameter blijft weg; er wordt niets aangevuld. Een
    /// afleiding over een verzameling grammen geeft hier `None`; zie
    /// [`Afleiding::pas_toe_op_verzameling`]. Een gram dat niet de vorm heeft
    /// die de afleiding leest (een ongeldig `op_moment`, een tabelregel die
    /// geen object is) is een fout, geen ontbrekende waarde.
    pub fn pas_toe(&self, gram: &Gram) -> Result<Option<Value>, String> {
        Ok(match self {
            Afleiding::Veld { field } => gram.field(field).filter(|w| filled(w)).cloned(),
            Afleiding::JaarVan { year_of } => jaar_uit(year_of, gram.field(year_of))?,
            Afleiding::PeriodeVan { period_of, period } => {
                periode_uit(period_of, gram.field(period_of), *period)?
            }
            Afleiding::Gevuld { filled: path } => {
                Some(Value::Bool(gram.field(path).is_some_and(filled)))
            }
            Afleiding::Gelijk { equals } => gram
                .field(&equals.field)
                .filter(|w| filled(w))
                .map(|w| Value::Bool(w == &equals.value)),
            Afleiding::ElkeRegel {
                table,
                each_row,
                only_where,
            } => {
                let rows = rows(gram, table)?;
                let elk = rows
                    .iter()
                    .filter(|r| {
                        only_where
                            .as_ref()
                            .is_none_or(|k| r.get(k) == Some(&Value::Bool(true)))
                    })
                    .all(|r| r.get(each_row).is_some_and(filled));
                Some(Value::Bool(!rows.is_empty() && elk))
            }
            Afleiding::EenRegel { table, one_row } => Some(Value::Bool(
                rows(gram, table)?
                    .iter()
                    .any(|r| r.get(one_row) == Some(&Value::Bool(true))),
            )),
            Afleiding::Moment {
                moment: Moment::EffectiveAt,
            } => Some(Value::String(datum::reference_date(&gram.moment()?))),
            Afleiding::Moment {
                moment: Moment::RecordedAt,
            } => Some(Value::String(datum::reference_date(&gram.recorded()?))),
            Afleiding::LaatsteVeld { .. }
            | Afleiding::LaatsteMoment { .. }
            | Afleiding::LaatsteJaarVan { .. }
            | Afleiding::LaatstePeriodeVan { .. }
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
        grams: &[&Gram],
    ) -> Result<Option<Value>, String> {
        let Some(filter) = self.filter() else {
            return Ok(None);
        };
        let mut door = Vec::new();
        for g in grams {
            if past(filter, inputs, g)? {
                door.push(*g);
            }
        }
        Ok(match self {
            Afleiding::Bestaat { filled: None, .. } => Some(Value::Bool(!door.is_empty())),
            Afleiding::Bestaat {
                filled: Some(field),
                ..
            } => Some(Value::Bool(
                door.iter().any(|g| g.field(field).is_some_and(filled)),
            )),
            Afleiding::Verzamel { collect, .. } => Some(Value::Array(
                door.iter()
                    .map(|g| {
                        Value::Object(
                            collect
                                .iter()
                                .map(|v| (v.clone(), g.field(v).cloned().unwrap_or(Value::Null)))
                                .collect(),
                        )
                    })
                    .collect(),
            )),
            Afleiding::Som { sum, .. } => {
                let (mut geheel, mut reeel, mut alleen_geheel) = (0_i64, 0.0_f64, true);
                for g in &door {
                    match g.field(sum) {
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
            Afleiding::LaatsteVeld { field, no_gram, .. } => match laatste(&door)? {
                Some(g) => g.field(field).filter(|w| filled(w)).cloned(),
                None => no_gram.clone(),
            },
            Afleiding::LaatsteMoment {
                moment, no_gram, ..
            } => match laatste(&door)? {
                Some(g) => Some(Value::String(datum::reference_date(&match moment {
                    Moment::EffectiveAt => g.moment()?,
                    Moment::RecordedAt => g.recorded()?,
                }))),
                None => no_gram.clone(),
            },
            Afleiding::LaatsteJaarVan {
                year_of, no_gram, ..
            } => match laatste(&door)? {
                Some(g) => jaar_uit(year_of, g.field(year_of))?,
                None => no_gram.clone(),
            },
            Afleiding::LaatstePeriodeVan {
                period_of,
                period,
                no_gram,
                ..
            } => match laatste(&door)? {
                Some(g) => periode_uit(period_of, g.field(period_of), *period)?,
                None => no_gram.clone(),
            },
            Afleiding::LaatsteBevat { contains, .. } => Some(Value::Bool(
                laatste(&door)?
                    .and_then(|g| g.field(&contains.field))
                    .and_then(Value::as_array)
                    .is_some_and(|list| list.contains(&contains.value)),
            )),
            _ => None,
        })
    }
}

/// Het laatste gram in de tijd: het laatste `op_moment`, bij gelijk moment
/// het laatste `vastgelegd_op`, en daarna het later toegevoegde.
fn laatste<'g>(grams: &[&'g Gram]) -> Result<Option<&'g Gram>, String> {
    let mut gekozen: Option<&Gram> = None;
    for gram in grams {
        // Ook een enkel gram wordt gelezen: een ongeldig moment is een fout.
        gram.moment()?;
        gram.recorded()?;
        if gekozen.map_or(Ok(true), |g| gram.tijdvolgorde(g).map(|o| o.is_ge()))? {
            gekozen = Some(gram);
        }
    }
    Ok(gekozen)
}

/// De datum in het veld `pad` van een gram (`JJJJ-MM-DD`, of een moment
/// met tijdzone). Geen waarde (het veld ontbreekt of is null): niets, en de
/// parameter blijft weg. Een waarde die geen datum is, is een fout: het gram
/// heeft niet de vorm die de afleiding leest.
fn datum_in(path: &str, value: Option<&Value>) -> Result<Option<chrono::NaiveDate>, String> {
    match value {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) => datum::datum_van(t)
            .map(Some)
            .ok_or_else(|| format!("veld '{path}': '{t}' is geen datum")),
        Some(w) => Err(format!("veld '{path}': {w} is geen datum")),
    }
}

/// Of elk veld van een gram dat een lexostatus van zijn kroniek als datum
/// leest (`jaar_van`, `periode_van`), een datum is of leeg. Een kroniek wordt
/// niet herschreven: zo'n gram zou elke reductie over die kroniek laten
/// falen, dus de cel weigert het bij het vastleggen.
pub fn datums_in_orde<'d>(
    definities: impl IntoIterator<Item = &'d LexostatusDefinitie>,
    gram: &Gram,
) -> Result<(), String> {
    for def in definities {
        if def.reduction.chronicle != gram.chronicle {
            continue;
        }
        for (name, a) in def.alle_afleidingen() {
            let path = match &a.derivation {
                Afleiding::JaarVan { year_of } | Afleiding::LaatsteJaarVan { year_of, .. } => {
                    year_of
                }
                Afleiding::PeriodeVan { period_of, .. }
                | Afleiding::LaatstePeriodeVan { period_of, .. } => period_of,
                _ => continue,
            };
            datum_in(path, gram.field(path))
                .map_err(|f| format!("{f} (lexostatus '{}' leidt '{name}' eruit af)", def.name))?;
        }
    }
    Ok(())
}

/// Het jaartal van de datum in het veld `pad` (zie `datum_in`).
pub fn jaar_uit(path: &str, value: Option<&Value>) -> Result<Option<Value>, String> {
    Ok(datum_in(path, value)?.map(|d| Value::from(i64::from(d.year()))))
}

/// De eerste dag van de periode waarin de datum in het veld `pad` valt (zie
/// `datum_in`), als `JJJJ-MM-DD`. Een afleiding zonder periode is een
/// fout: het laden zet haar uit de regeling, en lukte dat niet, dan is er
/// geen periode om te kiezen.
pub fn periode_uit(
    path: &str,
    value: Option<&Value>,
    period: Option<Periode>,
) -> Result<Option<Value>, String> {
    let period = period
        .ok_or_else(|| format!("periode_van '{path}' zonder periode (jaar, kwartaal of maand)"))?;
    Ok(datum_in(path, value)?
        .and_then(|d| period.start(d))
        .map(|b| Value::String(b.format("%Y-%m-%d").to_string())))
}

/// Gevuld: niet null, geen lege tekst, geen lege lijst of leeg object.
pub fn filled(w: &Value) -> bool {
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
fn rows<'g>(gram: &'g Gram, table: &str) -> Result<Vec<&'g Map<String, Value>>, String> {
    let rows = match gram.field(table) {
        None | Some(Value::Null) => return Ok(Vec::new()),
        Some(Value::Array(a)) => a,
        Some(_) => {
            return Err(format!(
                "gram '{}': tabelveld '{table}' is geen lijst van regels",
                gram.name
            ))
        }
    };
    rows.iter()
        .enumerate()
        .map(|(i, r)| {
            r.as_object().ok_or_else(|| {
                format!(
                    "gram '{}': regel {table}[{i}] is geen object met kolommen",
                    gram.name
                )
            })
        })
        .collect()
}

/// Een lexostatus: de parameters die een reductie oplevert.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Lexostatus {
    pub name: String,
    /// De wortel van het gekozen gram, als de definitie er een kiest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recorded_at: Option<String>,
    /// Het peil waarop gereduceerd is (zie [`Peil`]); weggelaten zonder
    /// peil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub as_of: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub known_at: Option<String>,
    pub parameters: BTreeMap<String, Value>,
    /// Waarden die geen parameter zijn; ze gaan nooit naar de engine.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_fields: BTreeMap<String, Value>,
    /// Parameters met een afleiding waarover de kroniek niets zegt.
    #[serde(default)]
    pub not_derived: Vec<String>,
    /// Bij een lijst-lexostatus (`groepeer`): de regels. Een lijst is voor de
    /// afnemer en gaat nooit naar de engine; `parameters` is dan leeg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub list: Option<Vec<Regel>>,
    /// Langs welke route de cel reduceerde: alleen in een runtime met de
    /// engine-route (`CEL_REDUCTIE`, zie [`crate::lexostatus_engine`]).
    /// Weggelaten bij de reductie-DSL zonder schakelaar, zodat die
    /// uitvoer gelijk blijft.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reduction: Option<Reductieroute>,
}

/// De route van een reductie: de reductie-DSL of een engine-run van een
/// regeling (experiment A).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Reductieroute {
    /// `engine` of `dsl`.
    pub route: String,
    /// Bij `engine`: de regeling die de lexostatus is.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    /// Bij `dsl` in een runtime met de engine-route: waarom deze lexostatus
    /// toch langs de DSL gaat (uit het koppelbestand).
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// Hoe lang de reductie duurde, in microseconden.
    pub duration_us: u64,
    /// Bij `vergelijk`: hoe lang dezelfde reductie langs de DSL duurde.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub dsl_duration_us: Option<u64>,
    /// De trace van de engine-run, als die gevraagd was.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

impl Lexostatus {
    /// Een lexostatus zonder waarden: de kroniek zegt er niets over.
    pub fn leeg(name: impl Into<String>) -> Self {
        Self {
            name: name.into(),
            root: None,
            effective_at: None,
            recorded_at: None,
            as_of: None,
            known_at: None,
            parameters: BTreeMap::new(),
            extra_fields: BTreeMap::new(),
            not_derived: Vec::new(),
            list: None,
            reduction: None,
        }
    }

    /// De waarde die de lexostatus onder een naam levert, als parameter of
    /// als extra veld; `None` als die er niet is of null is.
    pub fn field(&self, name: &str) -> Option<&Value> {
        self.parameters
            .get(name)
            .or_else(|| self.extra_fields.get(name))
            .filter(|w| !w.is_null())
    }
}

/// Een regel van een lijst-lexostatus: een wortel.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Regel {
    pub root: String,
    /// Het gekozen gram van de wortel, als de definitie er een kiest.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<String>,
    /// De afleidingen en extra velden, per zaak. Geen parameters.
    pub fields: BTreeMap<String, Value>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub not_derived: Vec<String>,
}

/// De parameters van een lexostatus die zeggen dat iets ontbreekt: een
/// aanwezigheidsafleiding (zie [`Afleiding::toetst_aanwezigheid`]) met de
/// waarde onwaar.
pub fn absent(
    definitie: &LexostatusDefinitie,
    parameters: &BTreeMap<String, Value>,
) -> Vec<String> {
    definitie
        .reduction
        .derivations
        .iter()
        .filter(|(name, a)| {
            a.toetst_aanwezigheid() && parameters.get(*name) == Some(&Value::Bool(false))
        })
        .map(|(name, _)| name.clone())
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
        let equals = match gram.kenmerk(sleutel) {
            Some(value) => value == Some(verwacht),
            None => match gram.field(sleutel) {
                Some(Value::String(s)) => s == verwacht,
                Some(w @ (Value::Number(_) | Value::Bool(_))) => *w.to_string() == *verwacht,
                _ => false,
            },
        };
        if !equals {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Wat de afleidingen over een groep grammen opleveren.
struct Opbrengst<'g> {
    gekozen: Option<&'g Gram>,
    parameters: BTreeMap<String, Value>,
    extra_fields: BTreeMap<String, Value>,
    not_derived: Vec<String>,
}

/// Kies zo nodig een gram uit `door` en pas elke afleiding toe. `None`: de
/// definitie kiest een gram en er is er geen.
fn leid_af_uit<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    door: &[&'g Gram],
) -> Result<Option<Opbrengst<'g>>, String> {
    let r = &definitie.reduction;
    let gekozen = match r.pick {
        Some(Kies::Latest) => match laatste(door)? {
            Some(g) => Some(g),
            None => return Ok(None),
        },
        None => None,
    };
    let mut uit = Opbrengst {
        gekozen,
        parameters: BTreeMap::new(),
        extra_fields: BTreeMap::new(),
        not_derived: Vec::new(),
    };
    for (name, derivation, extra) in r
        .derivations
        .iter()
        .map(|(n, a)| (n, a, false))
        .chain(r.extra_fields.iter().map(|(n, a)| (n, a, true)))
    {
        let value = if derivation.op_gekozen_gram() {
            let gram = gekozen.ok_or_else(|| {
                format!(
                    "afleiding '{name}' leest het gekozen gram, maar lexostatus '{}' kiest er geen",
                    definitie.name
                )
            })?;
            derivation.pas_toe(gram)?
        } else {
            derivation.pas_toe_op_verzameling(inputs, door)?
        };
        match (value, extra) {
            (Some(w), false) => {
                uit.parameters.insert(name.clone(), w);
            }
            (Some(w), true) => {
                uit.extra_fields.insert(name.clone(), w);
            }
            (None, false) => uit.not_derived.push(name.clone()),
            (None, true) => {}
        }
    }
    Ok(Some(uit))
}

/// De grammen die door een filter komen.
fn door_filter<'g>(
    filter: &Filter,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
) -> Result<Vec<&'g Gram>, String> {
    let mut door = Vec::new();
    for gram in grams {
        if past(filter, inputs, gram)? {
            door.push(gram);
        }
    }
    Ok(door)
}

/// Reduceer de grammen van een kroniek tot een lexostatus, zonder peil: elk
/// gram telt. Zie [`reduceer_op`].
pub fn reduceer<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
) -> Result<Option<Lexostatus>, String> {
    reduceer_op(definitie, inputs, grams, &Peil::default())
}

/// Reduceer de grammen van een kroniek tot een lexostatus, op een peil: alleen
/// de grammen die bij het peil tellen doen mee. `None`: de definitie kiest
/// een gram (`kies`) en geen gram komt door het filter. Een lijst-lexostatus
/// (`groepeer`) geeft altijd een lexostatus, met een lege lijst als geen zaak
/// past.
pub fn reduceer_op<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
    peil: &Peil,
) -> Result<Option<Lexostatus>, String> {
    let r = &definitie.reduction;
    let mut in_kroniek = Vec::new();
    for g in grams {
        if g.chronicle == r.chronicle && peil.laat_door(g)? {
            in_kroniek.push(g);
        }
    }
    let gepeild = |l: Lexostatus| Lexostatus {
        as_of: peil.as_of.map(|t| t.to_string()),
        known_at: peil.known_at.map(|t| t.to_string()),
        ..l
    };
    if r.group_by.is_some() {
        return reduceer_lijst(definitie, inputs, in_kroniek).map(|l| Some(gepeild(l)));
    }
    let door = door_filter(&r.filter, inputs, in_kroniek)?;
    let Some(a) = leid_af_uit(definitie, inputs, &door)? else {
        return Ok(None);
    };
    Ok(Some(gepeild(Lexostatus {
        root: a.gekozen.and_then(|g| g.root.clone()),
        effective_at: a.gekozen.map(|g| g.effective_at.clone()),
        recorded_at: a.gekozen.map(|g| g.recorded_at.clone()),
        parameters: a.parameters,
        extra_fields: a.extra_fields,
        not_derived: a.not_derived,
        ..Lexostatus::leeg(&definitie.name)
    })))
}

/// Een lijst-lexostatus: groepeer per wortel, houd de wortels met een gram
/// door `filter` en zonder gram door `zonder`, en leid per wortel af. De
/// regels staan op het moment van het gekozen gram (zonder `kies`: het eerste
/// gram van de wortel), de oudste eerst.
fn reduceer_lijst<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grams: impl IntoIterator<Item = &'g Gram>,
) -> Result<Lexostatus, String> {
    let r = &definitie.reduction;
    let mut zaken: BTreeMap<&str, Vec<&Gram>> = BTreeMap::new();
    for g in grams {
        // Een gram zonder bekende wortel hoort in geen regel.
        if let Some(z) = g.root.as_deref() {
            zaken.entry(z).or_default().push(g);
        }
    }
    let mut rows = Vec::new();
    for (case, group) in zaken {
        if !r.without.is_empty()
            && !door_filter(&r.without, inputs, group.iter().copied())?.is_empty()
        {
            continue;
        }
        let door = door_filter(&r.filter, inputs, group.iter().copied())?;
        if door.is_empty() {
            continue;
        }
        let Some(a) = leid_af_uit(definitie, inputs, &door)? else {
            continue;
        };
        // Zonder gekozen gram staat de zaak op haar eerste gram: de opening.
        let moment = match a.gekozen {
            Some(g) => Some(g.moment()?),
            None => door
                .iter()
                .map(|g| g.moment())
                .collect::<Result<Vec<_>, _>>()?
                .into_iter()
                .min(),
        };
        let mut fields = a.parameters;
        fields.extend(a.extra_fields);
        rows.push((
            moment,
            Regel {
                root: case.to_string(),
                effective_at: a.gekozen.map(|g| g.effective_at.clone()),
                fields,
                not_derived: a.not_derived,
            },
        ));
    }
    rows.sort_by(|(a, ra), (b, rb)| a.cmp(b).then_with(|| ra.root.cmp(&rb.root)));
    Ok(Lexostatus {
        list: Some(rows.into_iter().map(|(_, r)| r).collect()),
        ..Lexostatus::leeg(&definitie.name)
    })
}

/// Reduceer een enkel gram, zoals een concept dat nog geen feit is. Heeft
/// het gram een wortel, dan is dat de input `wortel`.
pub fn leid_af(definitie: &LexostatusDefinitie, gram: &Gram) -> Result<Lexostatus, String> {
    let mut inputs = Map::new();
    if let Some(z) = &gram.root {
        inputs.insert("root".into(), Value::String(z.clone()));
    }
    reduceer(definitie, &inputs, std::slice::from_ref(gram))?
        .ok_or_else(|| "de reductie vond het gram niet".to_string())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::StroomVerwijzing;
    use serde_json::json;

    const CEL: &str = include_str!("../../tests/fixtures/cells/instantie/lexostatuses.yaml");

    fn gram(case: &str, moment: &str, fields: Value) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            id: uuid::Uuid::now_v7().to_string(),
            type_: "submission".into(),
            subtype: Some("aanvraag".into()),
            stage: None,
            name: "aanvraag_ontvangen".into(),
            chronicle: "test_kroniek".into(),
            recording_actor: "test_instantie".into(),
            legal_basis: vec!["testregeling_aanvraag#1".into()],
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            competent_authority: None,
            acting_actor: None,
            effective_at: moment.into(),
            effective_at_legal_basis: None,
            recorded_at: moment.into(),
            refers_to: BTreeMap::new(),
            stream: StroomVerwijzing {
                id: "test_aanvragen".into(),
                sha256: "0".repeat(64),
            },
            provenance: None,
            fields: fields.as_object().unwrap().clone(),
            inputs: BTreeMap::new(),
            receipt: None,
            tijden: Default::default(),
            root: Some(case.into()),
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
        assert_eq!(c.cell, "test_instantie");
        assert_eq!(c.lexostatus_definitions[0].reduction.derivations.len(), 9);
    }

    #[test]
    fn ongeldige_afleiding_faalt_op_het_schema() {
        let tekst = CEL.replace(
            "{field: content.aanvraagjaar}",
            "{optellen: content.aanvraagjaar}",
        );
        let error = parse(&tekst, "t").unwrap_err();
        assert!(
            error.iter().any(|f| f.contains("derivations/aanvraagjaar")),
            "{error:?}"
        );
    }

    #[test]
    fn afleiding_veld() {
        let f = json!({"a": {"jaar": 2025, "leeg": ""}});
        assert_eq!(een("{field: a.jaar}", f.clone()), Some(json!(2025)));
        assert_eq!(een("{field: a.leeg}", f.clone()), None);
        assert_eq!(een("{field: a.ontbreekt}", f), None);
    }

    #[test]
    fn afleiding_gevuld() {
        let f = json!({"a": {"naam": "X", "leeg": " ", "lijst": []}});
        assert_eq!(een("{filled: a.naam}", f.clone()), Some(json!(true)));
        assert_eq!(een("{filled: a.leeg}", f.clone()), Some(json!(false)));
        assert_eq!(een("{filled: a.lijst}", f.clone()), Some(json!(false)));
        assert_eq!(een("{filled: a.ontbreekt}", f), Some(json!(false)));
    }

    #[test]
    fn afleiding_gelijk() {
        let f = json!({"a": {"reg": "a"}});
        assert_eq!(
            een("{equals: {field: a.reg, value: a}}", f.clone()),
            Some(json!(true))
        );
        assert_eq!(
            een("{equals: {field: a.reg, value: b}}", f.clone()),
            Some(json!(false))
        );
        assert_eq!(een("{equals: {field: a.x, value: b}}", f), None);
    }

    #[test]
    fn afleiding_tabel_elke_regel() {
        let f = json!({"t": [{"o": "raad", "z": 3}, {"o": "staten", "z": null}]});
        assert_eq!(een("{table: t, each_row: o}", f.clone()), Some(json!(true)));
        assert_eq!(een("{table: t, each_row: z}", f), Some(json!(false)));
        // Een lege of ontbrekende tabel bevat niets.
        assert_eq!(
            een("{table: t, each_row: o}", json!({"t": []})),
            Some(json!(false))
        );
        assert_eq!(
            een("{table: t, each_row: o}", json!({})),
            Some(json!(false))
        );
    }

    #[test]
    fn afleiding_tabel_alleen_waar() {
        let f = json!({"t": [{"s": false}, {"s": true, "n": 2}]});
        assert_eq!(
            een("{table: t, each_row: n, only_where: s}", f),
            Some(json!(true))
        );
        let f = json!({"t": [{"s": true}, {"s": true, "n": 2}]});
        assert_eq!(
            een("{table: t, each_row: n, only_where: s}", f),
            Some(json!(false))
        );
        // Geen regel waar: niets te missen, zolang de tabel regels heeft.
        let f = json!({"t": [{"s": false}]});
        assert_eq!(
            een("{table: t, each_row: n, only_where: s}", f),
            Some(json!(true))
        );
    }

    #[test]
    fn afleiding_tabel_een_regel() {
        let f = json!({"t": [{"s": false}, {"s": true}]});
        assert_eq!(een("{table: t, one_row: s}", f), Some(json!(true)));
        let f = json!({"t": [{"s": false}, {"s": "true"}]});
        assert_eq!(een("{table: t, one_row: s}", f), Some(json!(false)));
    }

    #[test]
    fn afleiding_jaar_van() {
        let f = json!({"a": {"datum": "2026-03-18", "leeg": null, "tekst": "geen datum"}});
        assert_eq!(een("{year_of: a.datum}", f.clone()), Some(json!(2026)));
        assert_eq!(een("{year_of: a.leeg}", f.clone()), None);
        assert_eq!(een("{year_of: a.ontbreekt}", f.clone()), None);
        // Een waarde die geen datum is, is een fout, geen ontbrekende waarde.
        let error = afl("{year_of: a.tekst}")
            .pas_toe(&gram("z", "2025-03-12T10:14:03+01:00", f))
            .unwrap_err();
        assert!(error.contains("'geen datum' is geen datum"), "{error}");
        assert_eq!(afl("{year_of: a.datum}").gelezen_paden(), vec!["a.datum"]);
    }

    /// Een periode heet naar haar eerste dag: een maand, een kwartaal, een
    /// jaar. Een waarde die geen datum is, en een afleiding zonder periode
    /// (niet uit de regeling gezet), zijn een fout.
    #[test]
    fn afleiding_periode_van() {
        let f = json!({"a": {"datum": "2026-08-18", "moment": "2026-11-30T23:30:00+01:00", "tekst": "geen datum"}});
        let per =
            |p: &str, field: &str| een(&format!("{{period_of: {field}, period: {p}}}"), f.clone());
        assert_eq!(per("month", "a.datum"), Some(json!("2026-08-01")));
        assert_eq!(per("quarter", "a.datum"), Some(json!("2026-07-01")));
        assert_eq!(per("year", "a.datum"), Some(json!("2026-01-01")));
        assert_eq!(per("month", "a.moment"), Some(json!("2026-11-01")));
        assert_eq!(per("quarter", "a.moment"), Some(json!("2026-10-01")));
        let error = |yaml: &str| {
            afl(yaml)
                .pas_toe(&gram("z", "2025-03-12T10:14:03+01:00", f.clone()))
                .unwrap_err()
        };
        assert!(error("{period_of: a.tekst, period: month}").contains("geen datum"));
        assert!(error("{period_of: a.datum}").contains("zonder periode"));
        assert_eq!(afl("{period_of: a.datum}").gelezen_paden(), vec!["a.datum"]);
        let a = afl(
            "{filter: {name: x}, pick: latest, period_of: datum, period: month, no_gram: null}",
        );
        assert!(matches!(a, Afleiding::LaatstePeriodeVan { .. }));
    }

    #[test]
    fn afleiding_jaar_van_over_een_verzameling() {
        let a = afl("{filter: {name: x}, pick: latest, year_of: datum, no_gram: null}");
        assert!(matches!(a, Afleiding::LaatsteJaarVan { .. }));
        assert!(!a.op_gekozen_gram());
        let oud = decision(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"datum": "2024-03-20"}),
        );
        let nieuw = decision(
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
        let leeg = decision("x", "2024-03-20T09:00:00+01:00", json!({"datum": null}));
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
            afl("{moment: effective_at}").pas_toe(&g).unwrap(),
            Some(json!("2025-03-12"))
        );
    }

    #[test]
    fn een_ongeldig_op_moment_is_een_fout_en_geen_lege_waarde() {
        let g = gram("z", "12 maart 2025", json!({}));
        let f = afl("{moment: effective_at}").pas_toe(&g).unwrap_err();
        assert!(f.contains("ongeldig effective_at '12 maart 2025'"), "{f}");
        // Ook `kies: laatste` kiest niet stil om zo'n gram heen.
        let def: LexostatusDefinitie = serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {chronicle: test_kroniek, pick: latest, derivations: {x: {field: a}}}}",
        )
        .unwrap();
        let goed = gram("z", "2025-03-12T10:14:03+01:00", json!({"a": 1}));
        assert!(reduceer(&def, &Map::new(), &[goed, g])
            .unwrap_err()
            .contains("ongeldig effective_at"));
    }

    #[test]
    fn kies_laatste_vergelijkt_momenten_over_tijdzones_heen() {
        // Op de klok van hun eigen tijdzone lijkt de volgorde anders dan ze
        // is: 10:15+02:00 is 08:15 UTC, 10:00+01:00 is 09:00 UTC en
        // 09:30+00:00 is 09:30 UTC, het laatst.
        let def: LexostatusDefinitie = serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {chronicle: test_kroniek, pick: latest, derivations: {x: {field: a}}}}",
        )
        .unwrap();
        let grams = [
            gram("z", "2025-03-12T09:30:00+00:00", json!({"a": "utc"})),
            gram("z", "2025-03-12T10:15:00+02:00", json!({"a": "oost"})),
            gram("z", "2025-03-12T10:00:00+01:00", json!({"a": "nl"})),
        ];
        let l = reduceer(&def, &Map::new(), &grams).unwrap().unwrap();
        assert_eq!(l.parameters["x"], json!("utc"));
        assert_eq!(l.effective_at.as_deref(), Some("2025-03-12T09:30:00+00:00"));
    }

    #[test]
    fn een_tabelregel_die_geen_object_is_is_een_fout() {
        let g = gram(
            "z",
            "2025-03-12T10:14:03+01:00",
            json!({"t": [{"k": true}, 3]}),
        );
        let f = afl("{table: t, one_row: k}").pas_toe(&g).unwrap_err();
        assert!(f.contains("regel t[1] is geen object"), "{f}");
        let g = gram("z", "2025-03-12T10:14:03+01:00", json!({"t": null}));
        assert_eq!(
            afl("{table: t, one_row: k}").pas_toe(&g).unwrap(),
            Some(json!(false))
        );
    }

    #[test]
    fn afleiding_leest_paden() {
        assert_eq!(
            afl("{equals: {field: a.b, value: 1}}").gelezen_paden(),
            vec!["a.b"]
        );
        assert_eq!(afl("{table: t, one_row: s}").gelezen_paden(), vec!["t"]);
        assert!(afl("{moment: effective_at}").gelezen_paden().is_empty());
    }

    #[test]
    fn reductie_kiest_laatste_per_zaak() {
        let c = parse(CEL, "fixture").unwrap();
        let def = &c.lexostatus_definitions[0];
        let grams = vec![
            gram(
                ZAAK,
                "2025-03-01T09:00:00+01:00",
                json!({"content": {"naam": "Eerst"}}),
            ),
            gram(
                ZAAK,
                "2025-03-02T09:00:00+01:00",
                json!({"content": {"naam": "Herstel"}}),
            ),
            gram(
                "00000000-0000-4000-8000-000000000002",
                "2025-03-03T09:00:00+01:00",
                json!({"content": {}}),
            ),
        ];
        let inputs = json!({"root": ZAAK});
        let l = reduceer(def, inputs.as_object().unwrap(), &grams)
            .unwrap()
            .unwrap();
        assert_eq!(l.effective_at.as_deref(), Some("2025-03-02T09:00:00+01:00"));
        assert_eq!(l.parameters["bevat_naam"], json!(true));
        assert_eq!(l.parameters["aanvraagdatum"], json!("2025-03-02"));
        assert!(l.not_derived.contains(&"aanvraagjaar".to_string()));
    }

    #[test]
    fn ontbreekt_noemt_alleen_aanwezigheidsafleidingen() {
        let c = parse(CEL, "fixture").unwrap();
        let def = &c.lexostatus_definitions[0];
        let g = gram(
            ZAAK,
            "2025-03-01T09:00:00+01:00",
            json!({"content": {"naam": "X", "registratie": "b",
                "organen": [{"orgaan": "raad", "samengevoegd": false}]}}),
        );
        let l = leid_af(def, &g).unwrap();
        // Onwaar, maar een antwoord: registratie_categorie_a (gelijk) en
        // is_samengevoegd (een_regel). Onwaar en een gat: de rest.
        assert_eq!(l.parameters["registratie_categorie_a"], json!(false));
        assert_eq!(l.parameters["is_samengevoegd"], json!(false));
        assert_eq!(
            absent(def, &l.parameters),
            vec!["bevat_aanduiding", "bevat_aantal_zetels"]
        );
    }

    #[test]
    fn reductie_zonder_passend_gram() {
        let c = parse(CEL, "fixture").unwrap();
        let inputs = json!({"root": ZAAK});
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
        let error = reduceer(&c.lexostatus_definitions[0], &Map::new(), &[g]).unwrap_err();
        assert!(error.contains("root"), "{error}");
    }

    #[test]
    fn filter_op_de_wortel_laat_een_gram_zonder_wortel_niet_door() {
        let mut filter = Filter::new();
        filter.insert("root".into(), "$root".into());
        let inputs = json!({"root": ZAAK});
        let inputs = inputs.as_object().unwrap();
        let mut g = gram(ZAAK, "2025-03-01T09:00:00+01:00", json!({}));
        assert!(past(&filter, inputs, &g).unwrap());
        g.root = None;
        assert!(!past(&filter, inputs, &g).unwrap());
    }

    /// Een filter kan per besluit selecteren: op de verwijzing naar dat besluit.
    #[test]
    fn filter_op_een_verwijzing() {
        let mut filter = Filter::new();
        filter.insert("refers_to.decision".into(), "$besluit".into());
        let inputs = json!({"besluit": "b1"});
        let inputs = inputs.as_object().unwrap();
        let mut g = gram(ZAAK, "2025-03-01T09:00:00+01:00", json!({}));
        assert!(!past(&filter, inputs, &g).unwrap(), "zonder verwijzing");
        g.refers_to.insert("decision".into(), "b1".into());
        assert!(past(&filter, inputs, &g).unwrap());
        g.refers_to.insert("decision".into(), "b2".into());
        assert!(!past(&filter, inputs, &g).unwrap(), "een ander besluit");
    }

    /// Een veld dat een lexostatus als datum leest, moet bij het vastleggen
    /// een datum of leeg zijn; een ander veld of een andere kroniek niet.
    #[test]
    fn een_datumveld_is_een_datum_of_leeg() {
        let def: LexostatusDefinitie = serde_yaml_ng::from_str(
            "name: l\ninputs: []\nreduction:\n  chronicle: test_kroniek\n  pick: latest\n  derivations:\n    jaar: {year_of: a.datum}\n",
        )
        .unwrap();
        let g = |f: Value| gram(ZAAK, "2025-03-01T09:00:00+01:00", f);
        datums_in_orde([&def], &g(json!({"a": {"datum": "2025-03-01"}}))).unwrap();
        datums_in_orde([&def], &g(json!({"a": {"datum": null}, "b": "geen datum"}))).unwrap();
        let error = datums_in_orde([&def], &g(json!({"a": {"datum": "morgen"}}))).unwrap_err();
        assert!(error.contains("'morgen' is geen datum"), "{error}");
        assert!(error.contains("lexostatus 'l'"), "{error}");
        let mut ander = g(json!({"a": {"datum": "morgen"}}));
        ander.chronicle = "andere_kroniek".into();
        datums_in_orde([&def], &ander).unwrap();
    }

    /// Elke sleutel van het gram zelf heeft een waarde in `Gram::kenmerk`;
    /// een andere sleutel is een veldpad.
    #[test]
    fn elke_gramsleutel_is_een_kenmerk() {
        let g = gram(ZAAK, "2025-03-01T09:00:00+01:00", json!({}));
        for k in GRAM_SLEUTELS {
            assert!(g.kenmerk(k).is_some(), "{k}");
        }
        assert!(g.kenmerk("content.naam").is_none());
    }

    const REGISTER: &str = include_str!("../../tests/fixtures/cells/register/lexostatuses.yaml");

    fn decision(name: &str, moment: &str, fields: Value) -> Gram {
        let mut g = gram(ZAAK, moment, fields);
        g.root = None;
        g.type_ = "decretogram".into();
        g.subtype = None;
        g.name = name.into();
        g.chronicle = "test_register".into();
        g
    }

    fn register() -> Vec<Gram> {
        vec![
            decision(
                "aanduiding_ingeschreven",
                "2024-01-10T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "orgaan": "raad", "gebied": "A"}),
            ),
            decision(
                "uitslag_vastgesteld",
                "2024-03-20T09:00:00+01:00",
                json!({"lijst": "VOORBEELD", "orgaan": "raad", "gebied": "A", "zetels": 4}),
            ),
            decision(
                "uitslag_vastgesteld",
                "2024-03-21T09:00:00+01:00",
                json!({"lijst": "VOORBEELD", "orgaan": "raad", "gebied": "B", "zetels": 2}),
            ),
            decision(
                "uitslag_vastgesteld",
                "2024-03-21T09:00:00+01:00",
                json!({"lijst": "ANDERS", "orgaan": "raad", "gebied": "B", "zetels": 7}),
            ),
            decision(
                "mededeling_gedaan",
                "2024-11-01T09:00:00+01:00",
                json!({"aanduiding": "VOORBEELD", "datum": "2024-11-01", "geblokkeerd_voor": ["staten"]}),
            ),
            decision(
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
            "{filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, collect: [gebied, zetels, samengevoegd]}",
        )
        .unwrap();
        let grams = register();
        let refs: Vec<&Gram> = grams.iter().collect();
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

    /// `bestaat` met `gevuld`: alleen een gram waarin dat veld een waarde
    /// heeft telt. Een filter vergelijkt op gelijkheid en kan dat niet.
    #[test]
    fn bestaat_met_gevuld_telt_alleen_een_gram_met_een_waarde() {
        let a = afl("{filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, exists: true, filled: samengevoegd}");
        assert_eq!(a.gelezen_paden(), vec!["samengevoegd", "lijst"]);
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let grams = register();
        let refs: Vec<&Gram> = grams.iter().collect();
        // De uitslagen van de fixture hebben geen samengevoegde aanduiding.
        assert_eq!(
            a.pas_toe_op_verzameling(inputs.as_object().unwrap(), &refs)
                .unwrap(),
            Some(json!(false))
        );
        let met = decision(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"lijst": "VOORBEELD", "samengevoegd": 2}),
        );
        let mut refs = refs;
        refs.push(&met);
        assert_eq!(
            a.pas_toe_op_verzameling(inputs.as_object().unwrap(), &refs)
                .unwrap(),
            Some(json!(true))
        );
    }

    /// Een lexostatus van de registerfixture, gereduceerd met deze inputs.
    fn register_lexostatus(name: &str, inputs: Value) -> Lexostatus {
        let c = parse(REGISTER, "register").unwrap();
        reduceer(
            c.lexostatus(name).unwrap(),
            inputs.as_object().unwrap(),
            &register(),
        )
        .unwrap()
        .unwrap()
    }

    fn registerstatus(aanduiding: &str) -> Lexostatus {
        register_lexostatus("registerstatus", json!({"aanduiding": aanduiding}))
    }

    fn registerstand(aanduiding: &str) -> Lexostatus {
        register_lexostatus(
            "register",
            json!({"aanduiding": aanduiding, "orgaan": "raad"}),
        )
    }

    #[test]
    fn filter_per_afleiding_bestaat_som_en_laatste() {
        let r = registerstand("VOORBEELD");
        assert_eq!(r.root, None, "zonder kies wordt geen gram gekozen");
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(true));
        assert_eq!(r.parameters["is_geschrapt"], json!(false));
        let l = registerstatus("VOORBEELD");
        // Alleen de uitslagen met deze aanduiding boven de lijst, over alle gebieden.
        assert_eq!(l.parameters["zetels_toegewezen"], json!(6));
        assert_eq!(l.parameters["datum_mededeling"], json!("2024-12-01"));
        // Het laatste gram telt: daar staat raad wel in de lijst.
        assert_eq!(l.parameters["geblokkeerd"], json!(true));
        assert!(l.not_derived.is_empty(), "{:?}", l.not_derived);
    }

    #[test]
    fn afwezigheid_in_de_eigen_kroniek_is_nee() {
        let r = registerstand("ONBEKEND");
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(false));
        let l = registerstatus("ONBEKEND");
        assert_eq!(l.parameters["zetels_toegewezen"], json!(0));
        assert_eq!(l.parameters["geblokkeerd"], json!(false));
        // Een datum is er niet: die blijft weg, er wordt niets aangevuld.
        assert!(!l.parameters.contains_key("datum_mededeling"));
        assert_eq!(
            l.not_derived,
            vec!["datum_mededeling", "jaar_van_mededeling"]
        );
    }

    #[test]
    fn som_zonder_getal_is_niet_af_te_leiden() {
        let a = afl("{filter: {name: uitslag_vastgesteld}, sum: zetels}");
        let g = decision(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": "vier"}),
        );
        assert_eq!(a.pas_toe_op_verzameling(&Map::new(), &[&g]).unwrap(), None);
        let h = decision(
            "uitslag_vastgesteld",
            "2024-03-20T09:00:00+01:00",
            json!({"zetels": 1.5}),
        );
        let i = decision(
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
        let g = decision(
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
        let a = afl("{filter: {name: x, orgaan: raad, aanduiding: $aanduiding}, exists: true}");
        assert!(!a.op_gekozen_gram());
        assert_eq!(a.gelezen_paden(), vec!["aanduiding", "orgaan"]);
        let a = afl("{filter: {name: x}, pick: latest, contains: {field: lijst, value: raad}}");
        assert!(matches!(a, Afleiding::LaatsteBevat { .. }));
        assert_eq!(a.gelezen_paden(), vec!["lijst"]);
        let a = afl("{filter: {name: x}, pick: latest, field: datum}");
        assert!(matches!(a, Afleiding::LaatsteVeld { .. }));
        assert!(afl("{field: datum}").op_gekozen_gram());
    }

    #[test]
    fn extra_velden_staan_apart_van_de_parameters() {
        let mut c = parse(CEL, "fixture").unwrap();
        let def = &mut c.lexostatus_definitions[0];
        def.reduction.extra_fields.insert(
            "aanduiding".into(),
            afl("{field: content.aanduiding}").into(),
        );
        let g = gram(
            ZAAK,
            "2025-03-01T09:00:00+01:00",
            json!({"content": {"aanduiding": "X"}}),
        );
        let l = leid_af(def, &g).unwrap();
        assert_eq!(l.extra_fields["aanduiding"], json!("X"));
        assert!(!l.parameters.contains_key("aanduiding"));
        assert!(def.levert("aanduiding") && def.levert("bevat_naam"));
    }

    #[test]
    fn afleiding_op_het_gram_zonder_kies_is_een_fout() {
        let mut c = parse(REGISTER, "register").unwrap();
        let def = &mut c.lexostatus_definitions[1];
        def.reduction
            .derivations
            .insert("x".into(), afl("{field: aanduiding}").into());
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let error = reduceer(def, inputs.as_object().unwrap(), &register()).unwrap_err();
        assert!(error.contains("kiest er geen"), "{error}");
    }

    #[test]
    fn register_fixture_valideert_tegen_het_schema() {
        let c = parse(REGISTER, "register").unwrap();
        assert_eq!(c.cell, "test_register");
        // Een afleiding draagt haar grondslag machineleesbaar.
        let a = &c.lexostatus_definitions[0].reduction.derivations["is_ingeschreven_in_register"];
        assert_eq!(a.legal_basis, ["testregeling_register#1 lid 1"]);
        assert!(matches!(a.derivation, Afleiding::Bestaat { .. }));
    }

    // --- Lijst-lexostatus: groepeer en zonder ---

    const WERKVOORRAAD: &str = "cell: c\nlexostatus_definitions:\n  - name: werkvoorraad\n    inputs: []\n    reduction:\n      chronicle: test_kroniek\n      filter: {type: submission, subtype: aanvraag}\n      group_by: root\n      without: {stage: BESLUIT}\n      pick: latest\n      derivations:\n        ontvangen_op: {moment: effective_at}\n        naam: {field: content.naam}\n";

    fn stage(case: &str, moment: &str, stage: &str) -> Gram {
        let mut g = gram(case, moment, json!({}));
        g.type_ = "decretogram".into();
        g.subtype = None;
        g.stage = Some(stage.into());
        g.name = "besluit".into();
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
            json!({"content": {"naam": "Geen"}}),
        );
        zonder_zaak.root = None;
        let grams = vec![
            // Zaak b is later ingediend maar komt eerst op kenmerk; de lijst
            // staat op moment.
            gram(
                b,
                "2025-03-05T09:00:00+01:00",
                json!({"content": {"naam": "B"}}),
            ),
            gram(
                a,
                "2025-03-01T09:00:00+01:00",
                json!({"content": {"naam": "A eerst"}}),
            ),
            gram(
                a,
                "2025-03-02T09:00:00+01:00",
                json!({"content": {"naam": "A herstel"}}),
            ),
            // Zaak d heeft een besluit en valt af; een andere stage niet.
            gram(
                d,
                "2025-03-03T09:00:00+01:00",
                json!({"content": {"naam": "D"}}),
            ),
            stage(d, "2025-03-04T09:00:00+01:00", "BESLUIT"),
            stage(b, "2025-03-06T09:00:00+01:00", "BEKENDMAKING"),
            zonder_zaak,
        ];
        let l = reduceer(def, &Map::new(), &grams).unwrap().unwrap();
        assert!(l.parameters.is_empty());
        let list = l.list.unwrap();
        let zaken: Vec<&str> = list.iter().map(|r| r.root.as_str()).collect();
        assert_eq!(zaken, [a, b]);
        assert_eq!(list[0].fields["naam"], json!("A herstel"));
        assert_eq!(list[0].fields["ontvangen_op"], json!("2025-03-02"));
        assert_eq!(
            list[0].effective_at.as_deref(),
            Some("2025-03-02T09:00:00+01:00")
        );
        // Een zaak met alleen een besluit en geen aanvraag telt niet.
        let alleen = vec![stage(a, "2025-03-04T09:00:00+01:00", "BEKENDMAKING")];
        let l = reduceer(def, &Map::new(), &alleen).unwrap().unwrap();
        assert!(l.list.unwrap().is_empty());
    }

    #[test]
    fn zonder_vraagt_groepeer() {
        let tekst = WERKVOORRAAD.replace("      group_by: root\n", "");
        let error = parse(&tekst, "w").unwrap_err();
        assert!(error.iter().any(|f| f.contains("group_by")), "{error:?}");
    }

    #[test]
    fn geen_gram_leest_afwezigheid() {
        let a = afl("{filter: {name: x}, pick: latest, field: datum, no_gram: null}");
        assert!(matches!(
            &a,
            Afleiding::LaatsteVeld {
                no_gram: Some(Value::Null),
                ..
            }
        ));
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[]).unwrap(),
            Some(Value::Null)
        );
        let g = decision(
            "x",
            "2024-03-20T09:00:00+01:00",
            json!({"datum": "2024-03-20"}),
        );
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[&g]).unwrap(),
            Some(json!("2024-03-20"))
        );
        // Een gram met een leeg veld is geen afwezigheid van het gram: weg.
        let leeg = decision("x", "2024-03-20T09:00:00+01:00", json!({"datum": null}));
        assert_eq!(
            a.pas_toe_op_verzameling(&Map::new(), &[&leeg]).unwrap(),
            None
        );
        let onwaar = afl("{filter: {name: x}, pick: latest, field: ja, no_gram: false}");
        assert_eq!(
            onwaar.pas_toe_op_verzameling(&Map::new(), &[]).unwrap(),
            Some(json!(false))
        );
        // Zonder geen_gram blijft de parameter weg.
        let without = afl("{filter: {name: x}, pick: latest, field: datum}");
        assert!(matches!(
            &without,
            Afleiding::LaatsteVeld { no_gram: None, .. }
        ));
        assert_eq!(
            without.pas_toe_op_verzameling(&Map::new(), &[]).unwrap(),
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

    // --- Tijd: twee tijden per gram en een peil (paper P:46, P:94) ---

    /// Een gram dat rechtens geldt op `op`, vastgelegd op `vastgelegd`.
    fn getijd(op: &str, recorded: &str, fields: Value) -> Gram {
        let mut g = gram("z", op, fields);
        g.recorded_at = recorded.into();
        g
    }

    fn kies_a() -> LexostatusDefinitie {
        serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {chronicle: test_kroniek, pick: latest, derivations: {a: {field: a}, sinds: {moment: effective_at}, bekend: {moment: recorded_at}}}}",
        )
        .unwrap()
    }

    fn peil(as_of: Option<&str>, known_at: Option<&str>) -> Peil {
        let lees = |t: Option<&str>| t.map(|t| crate::datum::Tijdpunt::lees("t", t).unwrap());
        Peil {
            as_of: lees(as_of),
            known_at: lees(known_at),
        }
    }

    /// Dezelfde kroniek op twee peilmomenten: twee lexostatussen. Wat na het
    /// peilmoment geldt, telt niet mee.
    #[test]
    fn dezelfde_kroniek_op_twee_peilmomenten() {
        let c = parse(REGISTER, "register").unwrap();
        let def = c.lexostatus("registerstatus").unwrap().clone();
        let register_def = c.lexostatus("register").unwrap().clone();
        let grams = register();
        let mut inputs = Map::new();
        inputs.insert("aanduiding".into(), json!("VOORBEELD"));
        let op = |p: &Peil| reduceer_op(&def, &inputs, &grams, p).unwrap().unwrap();
        let mut raad = inputs.clone();
        raad.insert("orgaan".into(), json!("raad"));
        let november = op(&peil(Some("2024-11-15"), None));
        let december = op(&peil(Some("2024-12-15"), None));
        assert_eq!(november.parameters["datum_mededeling"], json!("2024-11-01"));
        assert_eq!(november.parameters["geblokkeerd"], json!(false));
        assert_eq!(december.parameters["datum_mededeling"], json!("2024-12-01"));
        assert_eq!(december.parameters["geblokkeerd"], json!(true));
        assert_eq!(november.as_of.as_deref(), Some("2024-11-15"));
        // Voor de uitslag was er niets: geen zetels, niet ingeschreven.
        let januari = op(&peil(Some("2024-01-01"), None));
        assert_eq!(januari.parameters["zetels_toegewezen"], json!(0));
        let r = reduceer_op(
            &register_def,
            &raad,
            &grams,
            &peil(Some("2024-01-01"), None),
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(false));
        // Zonder peil telt alles.
        let nu = reduceer(&def, &inputs, &grams).unwrap().unwrap();
        assert_eq!(nu.parameters, december.parameters);
        assert_eq!(nu.as_of, None);
    }

    /// Een papieren aanvraag die op 5 maart binnenkwam en op 12 maart werd
    /// ingevoerd: rechtens geldt 5 maart, bekend is ze pas op 12 maart.
    #[test]
    fn een_laat_vastgelegd_feit_met_een_eerder_op_moment() {
        let def = kies_a();
        let grams = [
            getijd(
                "2025-03-10T09:00:00+01:00",
                "2025-03-10T09:00:00+01:00",
                json!({"a": "portaal"}),
            ),
            getijd(
                "2025-03-05T00:00:00+01:00",
                "2025-03-12T14:00:00+01:00",
                json!({"a": "papier"}),
            ),
        ];
        let op = |p: Peil| reduceer_op(&def, &Map::new(), &grams, &p).unwrap();
        // Nu: het laatste in de tijd is het gram van 10 maart, ook al is het
        // papieren gram later vastgelegd.
        let nu = op(Peil::default()).unwrap();
        assert_eq!(nu.parameters["a"], json!("portaal"));
        // Rechtens op 6 maart, met wat nu bekend is: de papieren aanvraag.
        let l = op(peil(Some("2025-03-06"), None)).unwrap();
        assert_eq!(l.parameters["a"], json!("papier"));
        assert_eq!(l.parameters["sinds"], json!("2025-03-05"));
        assert_eq!(l.parameters["bekend"], json!("2025-03-12"));
        assert_eq!(l.recorded_at.as_deref(), Some("2025-03-12T14:00:00+01:00"));
        // Zoals bekend op 11 maart: de papieren aanvraag lag er nog niet.
        let l = op(peil(None, Some("2025-03-11"))).unwrap();
        assert_eq!(l.parameters["a"], json!("portaal"));
        // Bitemporeel: rechtens op 6 maart, zoals bekend op 11 maart: niets.
        assert!(op(peil(Some("2025-03-06"), Some("2025-03-11"))).is_none());
    }

    /// `kies: laatste` kiest op op_moment; bij gelijk op_moment op
    /// vastgelegd_op; en als ook dat gelijk is, het later toegevoegde.
    #[test]
    fn kies_laatste_op_op_moment_dan_vastgelegd_op() {
        let def = kies_a();
        let pick = |grams: &[Gram]| {
            reduceer(&def, &Map::new(), grams)
                .unwrap()
                .unwrap()
                .parameters["a"]
                .clone()
        };
        let m = "2025-03-10T09:00:00+01:00";
        // Gelijk op_moment: het later vastgelegde, ongeacht de volgorde.
        let later = getijd(m, "2025-03-11T09:00:00+01:00", json!({"a": "later"}));
        let eerder = getijd(m, "2025-03-10T09:00:00+01:00", json!({"a": "eerder"}));
        assert_eq!(pick(&[later.clone(), eerder.clone()]), json!("later"));
        assert_eq!(pick(&[eerder.clone(), later.clone()]), json!("later"));
        // Het op_moment gaat voor: een eerder feit dat later is vastgelegd,
        // is niet het laatste.
        let laat_vastgelegd = getijd(
            "2025-03-09T09:00:00+01:00",
            "2025-03-20T09:00:00+01:00",
            json!({"a": "laat"}),
        );
        assert_eq!(pick(&[eerder.clone(), laat_vastgelegd]), json!("eerder"));
        // Alles gelijk: het later toegevoegde.
        let tweede = getijd(m, "2025-03-10T09:00:00+01:00", json!({"a": "tweede"}));
        assert_eq!(pick(&[eerder, tweede]), json!("tweede"));
    }

    #[test]
    fn een_ongeldig_peil_is_een_fout() {
        let mut q = Map::new();
        q.insert("as_of".into(), json!("morgen"));
        q.insert("aanduiding".into(), json!("X"));
        let f = Peil::uit_query(&mut q).unwrap_err();
        assert!(f.contains("ongeldig as_of 'morgen'"), "{f}");
        let mut q = Map::new();
        q.insert("known_at".into(), json!("2025-03-10"));
        q.insert("aanduiding".into(), json!("X"));
        let p = Peil::uit_query(&mut q).unwrap();
        assert!(p.as_of.is_none());
        assert_eq!(p.query(), vec![("known_at", "2025-03-10".to_string())]);
        // Wat overblijft, zijn de inputs.
        assert_eq!(q.keys().collect::<Vec<_>>(), ["aanduiding"]);
    }

    #[test]
    fn peilmoment_is_geen_input_van_een_lexostatus() {
        let f = parse(
            "cell: c\nlexostatus_definitions:\n  - name: l\n    inputs: [{name: as_of, type: date}]\n    reduction: {chronicle: k, derivations: {}}\n",
            "l",
        )
        .unwrap_err()
        .join("; ");
        assert!(f.contains("as_of"), "{f}");
    }
}
