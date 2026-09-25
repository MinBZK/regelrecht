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
            Afleiding::Veld { veld } => gram.veld(veld).filter(|w| gevuld(w)).cloned(),
            Afleiding::JaarVan { jaar_van } => jaar_uit(jaar_van, gram.veld(jaar_van))?,
            Afleiding::PeriodeVan {
                periode_van,
                periode,
            } => periode_uit(periode_van, gram.veld(periode_van), *periode)?,
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
            Afleiding::Moment {
                moment: Moment::VastgelegdOp,
            } => Some(Value::String(datum::peildatum(&gram.vastgelegd()?))),
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
            Afleiding::Bestaat { gevuld: None, .. } => Some(Value::Bool(!door.is_empty())),
            Afleiding::Bestaat {
                gevuld: Some(veld), ..
            } => Some(Value::Bool(
                door.iter().any(|g| g.veld(veld).is_some_and(gevuld)),
            )),
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
            Afleiding::LaatsteMoment {
                moment, geen_gram, ..
            } => match laatste(&door)? {
                Some(g) => Some(Value::String(datum::peildatum(&match moment {
                    Moment::OpMoment => g.moment()?,
                    Moment::VastgelegdOp => g.vastgelegd()?,
                }))),
                None => geen_gram.clone(),
            },
            Afleiding::LaatsteJaarVan {
                jaar_van,
                geen_gram,
                ..
            } => match laatste(&door)? {
                Some(g) => jaar_uit(jaar_van, g.veld(jaar_van))?,
                None => geen_gram.clone(),
            },
            Afleiding::LaatstePeriodeVan {
                periode_van,
                periode,
                geen_gram,
                ..
            } => match laatste(&door)? {
                Some(g) => periode_uit(periode_van, g.veld(periode_van), *periode)?,
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

/// Het laatste gram in de tijd: het laatste `op_moment`, bij gelijk moment
/// het laatste `vastgelegd_op`, en daarna het later toegevoegde.
fn laatste<'g>(grammen: &[&'g Gram]) -> Result<Option<&'g Gram>, String> {
    let mut gekozen: Option<&Gram> = None;
    for gram in grammen {
        // Ook een enkel gram wordt gelezen: een ongeldig moment is een fout.
        gram.moment()?;
        gram.vastgelegd()?;
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
fn datum_in(pad: &str, waarde: Option<&Value>) -> Result<Option<chrono::NaiveDate>, String> {
    match waarde {
        None | Some(Value::Null) => Ok(None),
        Some(Value::String(t)) => datum::datum_van(t)
            .map(Some)
            .ok_or_else(|| format!("veld '{pad}': '{t}' is geen datum")),
        Some(w) => Err(format!("veld '{pad}': {w} is geen datum")),
    }
}

/// Het jaartal van de datum in het veld `pad` (zie `datum_in`).
pub fn jaar_uit(pad: &str, waarde: Option<&Value>) -> Result<Option<Value>, String> {
    Ok(datum_in(pad, waarde)?.map(|d| Value::from(i64::from(d.year()))))
}

/// De eerste dag van de periode waarin de datum in het veld `pad` valt (zie
/// `datum_in`), als `JJJJ-MM-DD`. Een afleiding zonder periode is een
/// fout: het laden zet haar uit de regeling, en lukte dat niet, dan is er
/// geen periode om te kiezen.
pub fn periode_uit(
    pad: &str,
    waarde: Option<&Value>,
    periode: Option<Periode>,
) -> Result<Option<Value>, String> {
    let periode = periode
        .ok_or_else(|| format!("periode_van '{pad}' zonder periode (jaar, kwartaal of maand)"))?;
    Ok(datum_in(pad, waarde)?
        .and_then(|d| periode.begin(d))
        .map(|b| Value::String(b.format("%Y-%m-%d").to_string())))
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
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub vastgelegd_op: Option<String>,
    /// Het peil waarop gereduceerd is (zie [`Peil`]); weggelaten zonder
    /// peil.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub peilmoment: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bekend_op: Option<String>,
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
            vastgelegd_op: None,
            peilmoment: None,
            bekend_op: None,
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
        let gelijk = match gram.kenmerk(sleutel) {
            Some(waarde) => waarde == Some(verwacht),
            None => match gram.veld(sleutel) {
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
struct Opbrengst<'g> {
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
) -> Result<Option<Opbrengst<'g>>, String> {
    let r = &definitie.reduction;
    let gekozen = match r.kies {
        Some(Kies::Laatste) => match laatste(door)? {
            Some(g) => Some(g),
            None => return Ok(None),
        },
        None => None,
    };
    let mut uit = Opbrengst {
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

/// Reduceer de grammen van een kroniek tot een lexostatus, zonder peil: elk
/// gram telt. Zie [`reduceer_op`].
pub fn reduceer<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: impl IntoIterator<Item = &'g Gram>,
) -> Result<Option<Lexostatus>, String> {
    reduceer_op(definitie, inputs, grammen, &Peil::default())
}

/// Reduceer de grammen van een kroniek tot een lexostatus, op een peil: alleen
/// de grammen die bij het peil tellen doen mee. `None`: de definitie kiest
/// een gram (`kies`) en geen gram komt door het filter. Een lijst-lexostatus
/// (`groepeer`) geeft altijd een lexostatus, met een lege lijst als geen zaak
/// past.
pub fn reduceer_op<'g>(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: impl IntoIterator<Item = &'g Gram>,
    peil: &Peil,
) -> Result<Option<Lexostatus>, String> {
    let r = &definitie.reduction;
    let mut in_kroniek = Vec::new();
    for g in grammen {
        if g.chronicle == r.kroniek && peil.laat_door(g)? {
            in_kroniek.push(g);
        }
    }
    let gepeild = |l: Lexostatus| Lexostatus {
        peilmoment: peil.peilmoment.map(|t| t.to_string()),
        bekend_op: peil.bekend_op.map(|t| t.to_string()),
        ..l
    };
    if r.groepeer.is_some() {
        return reduceer_lijst(definitie, inputs, in_kroniek).map(|l| Some(gepeild(l)));
    }
    let door = door_filter(&r.filter, inputs, in_kroniek)?;
    let Some(a) = leid_af_uit(definitie, inputs, &door)? else {
        return Ok(None);
    };
    Ok(Some(gepeild(Lexostatus {
        zaakkenmerk: a.gekozen.and_then(|g| g.zaakkenmerk.clone()),
        op_moment: a.gekozen.map(|g| g.op_moment.clone()),
        vastgelegd_op: a.gekozen.map(|g| g.vastgelegd_op.clone()),
        parameters: a.parameters,
        extra_velden: a.extra_velden,
        niet_afgeleid: a.niet_afgeleid,
        ..Lexostatus::leeg(&definitie.name)
    })))
}

/// Een lijst-lexostatus: groepeer per zaakkenmerk, houd de zaken met een gram
/// door `filter` en zonder gram door `zonder`, en leid per zaak af. De regels
/// staan op het moment van het gekozen gram (zonder `kies`: het eerste gram
/// van de zaak), de oudste eerst.
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
    use crate::gram::StroomVerwijzing;
    use crate::stroom::Zaak;
    use serde_json::json;

    const CEL: &str = include_str!("../../tests/fixtures/cellen/instantie/lexostatussen.yaml");

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
            handelende_actor: None,
            op_moment: moment.into(),
            op_moment_grondslag: None,
            vastgelegd_op: moment.into(),
            zaak: Zaak::Opent,
            zaakkenmerk: Some(zaak.into()),
            besluit: None,
            besluitkenmerk: None,
            wijzigt: None,
            stroom: StroomVerwijzing {
                id: "test_aanvragen".into(),
                sha256: "0".repeat(64),
            },
            herkomst: None,
            fields: fields.as_object().unwrap().clone(),
            inputs: BTreeMap::new(),
            receipt: None,
            tijden: Default::default(),
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
        assert_eq!(een("{jaar_van: a.ontbreekt}", f.clone()), None);
        // Een waarde die geen datum is, is een fout, geen ontbrekende waarde.
        let fout = afl("{jaar_van: a.tekst}")
            .pas_toe(&gram("z", "2025-03-12T10:14:03+01:00", f))
            .unwrap_err();
        assert!(fout.contains("'geen datum' is geen datum"), "{fout}");
        assert_eq!(afl("{jaar_van: a.datum}").gelezen_paden(), vec!["a.datum"]);
    }

    /// Een periode heet naar haar eerste dag: een maand, een kwartaal, een
    /// jaar. Een waarde die geen datum is, en een afleiding zonder periode
    /// (niet uit de regeling gezet), zijn een fout.
    #[test]
    fn afleiding_periode_van() {
        let f = json!({"a": {"datum": "2026-08-18", "moment": "2026-11-30T23:30:00+01:00", "tekst": "geen datum"}});
        let per =
            |p: &str, veld: &str| een(&format!("{{periode_van: {veld}, periode: {p}}}"), f.clone());
        assert_eq!(per("maand", "a.datum"), Some(json!("2026-08-01")));
        assert_eq!(per("kwartaal", "a.datum"), Some(json!("2026-07-01")));
        assert_eq!(per("jaar", "a.datum"), Some(json!("2026-01-01")));
        assert_eq!(per("maand", "a.moment"), Some(json!("2026-11-01")));
        assert_eq!(per("kwartaal", "a.moment"), Some(json!("2026-10-01")));
        let fout = |yaml: &str| {
            afl(yaml)
                .pas_toe(&gram("z", "2025-03-12T10:14:03+01:00", f.clone()))
                .unwrap_err()
        };
        assert!(fout("{periode_van: a.tekst, periode: maand}").contains("geen datum"));
        assert!(fout("{periode_van: a.datum}").contains("zonder periode"));
        assert_eq!(
            afl("{periode_van: a.datum}").gelezen_paden(),
            vec!["a.datum"]
        );
        let a = afl("{filter: {name: x}, kies: laatste, periode_van: datum, periode: maand, geen_gram: null}");
        assert!(matches!(a, Afleiding::LaatstePeriodeVan { .. }));
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

    /// Een filter kan per besluit in de zaak selecteren: op het besluit
    /// (opent, volgt, wijzigt) en op het besluitkenmerk, ook uit een input.
    #[test]
    fn filter_op_besluit_en_besluitkenmerk() {
        let mut filter = Filter::new();
        filter.insert("besluit".into(), "volgt".into());
        filter.insert("besluitkenmerk".into(), "$besluitkenmerk".into());
        let k1 = format!("{ZAAK}/1");
        let inputs = json!({"besluitkenmerk": k1});
        let inputs = inputs.as_object().unwrap();
        let mut g = gram(ZAAK, "2025-03-01T09:00:00+01:00", json!({}));
        assert!(!past(&filter, inputs, &g).unwrap(), "zonder besluit");
        g.besluit = Some(crate::stroom::Besluit::Volgt);
        g.besluitkenmerk = Some(k1.clone());
        assert!(past(&filter, inputs, &g).unwrap());
        g.besluitkenmerk = Some(format!("{ZAAK}/2"));
        assert!(!past(&filter, inputs, &g).unwrap(), "een ander besluit");
        g.besluitkenmerk = Some(k1);
        g.besluit = Some(crate::stroom::Besluit::Opent);
        assert!(!past(&filter, inputs, &g).unwrap(), "het besluit zelf");
    }

    /// Elke sleutel van het gram zelf heeft een waarde in `Gram::kenmerk`;
    /// een andere sleutel is een veldpad.
    #[test]
    fn elke_gramsleutel_is_een_kenmerk() {
        let g = gram(ZAAK, "2025-03-01T09:00:00+01:00", json!({}));
        for k in GRAM_SLEUTELS {
            assert!(g.kenmerk(k).is_some(), "{k}");
        }
        assert!(g.kenmerk("inhoud.naam").is_none());
    }

    const REGISTER: &str = include_str!("../../tests/fixtures/cellen/register/lexostatussen.yaml");

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

    /// `bestaat` met `gevuld`: alleen een gram waarin dat veld een waarde
    /// heeft telt. Een filter vergelijkt op gelijkheid en kan dat niet.
    #[test]
    fn bestaat_met_gevuld_telt_alleen_een_gram_met_een_waarde() {
        let a = afl("{filter: {name: uitslag_vastgesteld, lijst: $aanduiding}, bestaat: true, gevuld: samengevoegd}");
        assert_eq!(a.gelezen_paden(), vec!["samengevoegd", "lijst"]);
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let grammen = register();
        let refs: Vec<&Gram> = grammen.iter().collect();
        // De uitslagen van de fixture hebben geen samengevoegde aanduiding.
        assert_eq!(
            a.pas_toe_op_verzameling(inputs.as_object().unwrap(), &refs)
                .unwrap(),
            Some(json!(false))
        );
        let met = besluit(
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
    fn register_lexostatus(naam: &str, inputs: Value) -> Lexostatus {
        let c = parse(REGISTER, "register").unwrap();
        reduceer(
            c.lexostatus(naam).unwrap(),
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
        assert_eq!(r.zaakkenmerk, None, "zonder kies wordt geen gram gekozen");
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(true));
        assert_eq!(r.parameters["is_geschrapt"], json!(false));
        let l = registerstatus("VOORBEELD");
        // Alleen de uitslagen met deze aanduiding boven de lijst, over alle gebieden.
        assert_eq!(l.parameters["zetels_toegewezen"], json!(6));
        assert_eq!(l.parameters["datum_mededeling"], json!("2024-12-01"));
        // Het laatste gram telt: daar staat raad wel in de lijst.
        assert_eq!(l.parameters["geblokkeerd"], json!(true));
        assert!(l.niet_afgeleid.is_empty(), "{:?}", l.niet_afgeleid);
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
            l.niet_afgeleid,
            vec!["datum_mededeling", "jaar_van_mededeling"]
        );
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
            .insert("aanduiding".into(), afl("{veld: inhoud.aanduiding}").into());
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
        let def = &mut c.lexostatus_definitions[1];
        def.reduction
            .afleidingen
            .insert("x".into(), afl("{veld: aanduiding}").into());
        let inputs = json!({"aanduiding": "VOORBEELD"});
        let fout = reduceer(def, inputs.as_object().unwrap(), &register()).unwrap_err();
        assert!(fout.contains("kiest er geen"), "{fout}");
    }

    #[test]
    fn register_fixture_valideert_tegen_het_schema() {
        let c = parse(REGISTER, "register").unwrap();
        assert_eq!(c.cel, "test_register");
        // Een afleiding draagt haar grondslag machineleesbaar.
        let a = &c.lexostatus_definitions[0].reduction.afleidingen["is_ingeschreven_in_register"];
        assert_eq!(a.grondslag, ["testregeling_register#1 lid 1"]);
        assert!(matches!(a.afleiding, Afleiding::Bestaat { .. }));
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

    // --- Tijd: twee tijden per gram en een peil (paper P:46, P:94) ---

    /// Een gram dat rechtens geldt op `op`, vastgelegd op `vastgelegd`.
    fn getijd(op: &str, vastgelegd: &str, fields: Value) -> Gram {
        let mut g = gram("z", op, fields);
        g.vastgelegd_op = vastgelegd.into();
        g
    }

    fn kies_a() -> LexostatusDefinitie {
        serde_yaml_ng::from_str(
            "{name: l, inputs: [], reduction: {kroniek: test_kroniek, kies: laatste, afleidingen: {a: {veld: a}, sinds: {moment: op_moment}, bekend: {moment: vastgelegd_op}}}}",
        )
        .unwrap()
    }

    fn peil(peilmoment: Option<&str>, bekend_op: Option<&str>) -> Peil {
        let lees = |t: Option<&str>| t.map(|t| crate::datum::Tijdpunt::lees("t", t).unwrap());
        Peil {
            peilmoment: lees(peilmoment),
            bekend_op: lees(bekend_op),
        }
    }

    /// Dezelfde kroniek op twee peilmomenten: twee lexostatussen. Wat na het
    /// peilmoment geldt, telt niet mee.
    #[test]
    fn dezelfde_kroniek_op_twee_peilmomenten() {
        let c = parse(REGISTER, "register").unwrap();
        let def = c.lexostatus("registerstatus").unwrap().clone();
        let register_def = c.lexostatus("register").unwrap().clone();
        let grammen = register();
        let mut inputs = Map::new();
        inputs.insert("aanduiding".into(), json!("VOORBEELD"));
        let op = |p: &Peil| reduceer_op(&def, &inputs, &grammen, p).unwrap().unwrap();
        let mut raad = inputs.clone();
        raad.insert("orgaan".into(), json!("raad"));
        let november = op(&peil(Some("2024-11-15"), None));
        let december = op(&peil(Some("2024-12-15"), None));
        assert_eq!(november.parameters["datum_mededeling"], json!("2024-11-01"));
        assert_eq!(november.parameters["geblokkeerd"], json!(false));
        assert_eq!(december.parameters["datum_mededeling"], json!("2024-12-01"));
        assert_eq!(december.parameters["geblokkeerd"], json!(true));
        assert_eq!(november.peilmoment.as_deref(), Some("2024-11-15"));
        // Voor de uitslag was er niets: geen zetels, niet ingeschreven.
        let januari = op(&peil(Some("2024-01-01"), None));
        assert_eq!(januari.parameters["zetels_toegewezen"], json!(0));
        let r = reduceer_op(
            &register_def,
            &raad,
            &grammen,
            &peil(Some("2024-01-01"), None),
        )
        .unwrap()
        .unwrap();
        assert_eq!(r.parameters["is_ingeschreven_in_register"], json!(false));
        // Zonder peil telt alles.
        let nu = reduceer(&def, &inputs, &grammen).unwrap().unwrap();
        assert_eq!(nu.parameters, december.parameters);
        assert_eq!(nu.peilmoment, None);
    }

    /// Een papieren aanvraag die op 5 maart binnenkwam en op 12 maart werd
    /// ingevoerd: rechtens geldt 5 maart, bekend is ze pas op 12 maart.
    #[test]
    fn een_laat_vastgelegd_feit_met_een_eerder_op_moment() {
        let def = kies_a();
        let grammen = [
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
        let op = |p: Peil| reduceer_op(&def, &Map::new(), &grammen, &p).unwrap();
        // Nu: het laatste in de tijd is het gram van 10 maart, ook al is het
        // papieren gram later vastgelegd.
        let nu = op(Peil::default()).unwrap();
        assert_eq!(nu.parameters["a"], json!("portaal"));
        // Rechtens op 6 maart, met wat nu bekend is: de papieren aanvraag.
        let l = op(peil(Some("2025-03-06"), None)).unwrap();
        assert_eq!(l.parameters["a"], json!("papier"));
        assert_eq!(l.parameters["sinds"], json!("2025-03-05"));
        assert_eq!(l.parameters["bekend"], json!("2025-03-12"));
        assert_eq!(
            l.vastgelegd_op.as_deref(),
            Some("2025-03-12T14:00:00+01:00")
        );
        // Zoals bekend op 11 maart: de papieren aanvraag lag er nog niet.
        let l = op(peil(None, Some("2025-03-11"))).unwrap();
        assert_eq!(l.parameters["a"], json!("portaal"));
        // Bitemporeel: rechtens op 6 maart, zoals bekend op 11 maart: niets.
        assert!(op(peil(Some("2025-03-06"), Some("2025-03-11"))).is_none());
    }

    /// Een gram van voor `vastgelegd_op` (uit een oudere kroniek) krijgt
    /// zijn op_moment, en telt daarna gewoon mee.
    #[test]
    fn een_oud_gram_zonder_vastgelegd_op() {
        let mut oud =
            serde_json::to_value(gram("z", "2025-03-05T09:00:00+01:00", json!({"a": 1}))).unwrap();
        oud.as_object_mut().unwrap().remove("vastgelegd_op");
        let mut g: Gram = serde_json::from_value(oud).unwrap();
        assert_eq!(g.vastgelegd_op, "");
        assert!(g.vul_vastgelegd_op());
        assert!(!g.vul_vastgelegd_op());
        assert_eq!(g.vastgelegd_op, g.op_moment);
        let l = reduceer_op(
            &kies_a(),
            &Map::new(),
            [&g],
            &peil(None, Some("2025-03-05")),
        )
        .unwrap()
        .unwrap();
        assert_eq!(l.parameters["bekend"], json!("2025-03-05"));
    }

    /// `kies: laatste` kiest op op_moment; bij gelijk op_moment op
    /// vastgelegd_op; en als ook dat gelijk is, het later toegevoegde.
    #[test]
    fn kies_laatste_op_op_moment_dan_vastgelegd_op() {
        let def = kies_a();
        let kies = |grammen: &[Gram]| {
            reduceer(&def, &Map::new(), grammen)
                .unwrap()
                .unwrap()
                .parameters["a"]
                .clone()
        };
        let m = "2025-03-10T09:00:00+01:00";
        // Gelijk op_moment: het later vastgelegde, ongeacht de volgorde.
        let later = getijd(m, "2025-03-11T09:00:00+01:00", json!({"a": "later"}));
        let eerder = getijd(m, "2025-03-10T09:00:00+01:00", json!({"a": "eerder"}));
        assert_eq!(kies(&[later.clone(), eerder.clone()]), json!("later"));
        assert_eq!(kies(&[eerder.clone(), later.clone()]), json!("later"));
        // Het op_moment gaat voor: een eerder feit dat later is vastgelegd,
        // is niet het laatste.
        let laat_vastgelegd = getijd(
            "2025-03-09T09:00:00+01:00",
            "2025-03-20T09:00:00+01:00",
            json!({"a": "laat"}),
        );
        assert_eq!(kies(&[eerder.clone(), laat_vastgelegd]), json!("eerder"));
        // Alles gelijk: het later toegevoegde.
        let tweede = getijd(m, "2025-03-10T09:00:00+01:00", json!({"a": "tweede"}));
        assert_eq!(kies(&[eerder, tweede]), json!("tweede"));
    }

    #[test]
    fn een_ongeldig_peil_is_een_fout() {
        let mut q = Map::new();
        q.insert("peilmoment".into(), json!("morgen"));
        q.insert("aanduiding".into(), json!("X"));
        let f = Peil::uit_query(&mut q).unwrap_err();
        assert!(f.contains("ongeldig peilmoment 'morgen'"), "{f}");
        let mut q = Map::new();
        q.insert("bekend_op".into(), json!("2025-03-10"));
        q.insert("aanduiding".into(), json!("X"));
        let p = Peil::uit_query(&mut q).unwrap();
        assert!(p.peilmoment.is_none());
        assert_eq!(p.query(), vec![("bekend_op", "2025-03-10".to_string())]);
        // Wat overblijft, zijn de inputs.
        assert_eq!(q.keys().collect::<Vec<_>>(), ["aanduiding"]);
    }

    #[test]
    fn peilmoment_is_geen_input_van_een_lexostatus() {
        let f = parse(
            "cel: c\nlexostatus_definitions:\n  - name: l\n    inputs: [{name: peilmoment, type: date}]\n    reduction: {kroniek: k, afleidingen: {}}\n",
            "l",
        )
        .unwrap_err()
        .join("; ");
        assert!(f.contains("peilmoment"), "{f}");
    }
}
