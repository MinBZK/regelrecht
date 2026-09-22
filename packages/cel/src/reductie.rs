//! Reductie tot lexostatus: de lexostatus-definities laden, en een kroniek
//! reduceren tot de parameters van een artikel.
//!
//! Een lexostatus-definitie beperkt de kroniek met `filter` en kiest met
//! `kies` zo nodig een gram. Per parameter leidt een afleiding een waarde af,
//! op een van twee manieren:
//!
//! - **op het gekozen gram**: `veld`, `gevuld`, `gelijk`, `tabel` met
//!   `elke_regel` of `een_regel` (en `alleen_waar`), en `moment`;
//! - **over een verzameling grammen**, met een eigen `filter`: `bestaat`,
//!   `som`, en `kies: laatste` met `veld` of `bevat`.
//!
//! Geen gram betekent binnen de eigen kroniek "nee" (`bestaat`, `bevat`) of
//! nul (`som`): de cel spreekt alleen over haar eigen kroniek. Een waarde die
//! er niet is (`veld` op een leeg veld, `kies` zonder gram) blijft weg; er
//! wordt niets aangevuld.
//!
//! `extra_velden` leidt waarden af die geen parameter zijn, zoals de invoer
//! van een synthese-bron. Ze staan apart in de lexostatus en gaan nooit naar
//! de engine. Veldpaden zijn relatief aan `fields` van het gram, met punten.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::schema::{self, Soort};
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
    "zaakkenmerk",
    "recording_actor",
    "chronicle",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Reductie {
    pub kroniek: String,
    #[serde(default, skip_serializing_if = "Filter::is_empty")]
    pub filter: Filter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub kies: Option<Kies>,
    pub afleidingen: BTreeMap<String, Afleiding>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_velden: BTreeMap<String, Afleiding>,
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
    LaatsteVeld {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        kies: Kies,
        veld: String,
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
    Veld {
        veld: String,
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
    let yaml: serde_yaml_ng::Value = serde_yaml_ng::from_str(tekst)
        .map_err(|e| vec![format!("{bron}: geen geldige YAML: {e}")])?;
    let document: Value = serde_json::to_value(&yaml).map_err(|e| vec![format!("{bron}: {e}")])?;
    schema::valideer(Soort::Lexostatus, &document).map_err(|f| {
        f.into_iter()
            .map(|f| format!("{bron}: {f}"))
            .collect::<Vec<_>>()
    })?;
    serde_json::from_value(document).map_err(|e| vec![format!("{bron}: {e}")])
}

/// Laad de lexostatus-definities uit een bestand.
pub fn laad(pad: &Path) -> Result<Lexostatussen, Vec<String>> {
    let bron = pad.display().to_string();
    let tekst = std::fs::read_to_string(pad).map_err(|e| vec![format!("{bron}: {e}")])?;
    parse(&tekst, &bron)
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
            | Afleiding::LaatsteBevat { filter, .. }
            | Afleiding::Bestaat { filter, .. }
            | Afleiding::Som { filter, .. } => Some(filter),
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
            Afleiding::Gevuld { gevuld } => vec![gevuld.as_str()],
            Afleiding::Gelijk { gelijk } => vec![gelijk.veld.as_str()],
            Afleiding::ElkeRegel { tabel, .. } | Afleiding::EenRegel { tabel, .. } => {
                vec![tabel.as_str()]
            }
            Afleiding::Som { som, .. } => vec![som.as_str()],
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
    /// [`Afleiding::pas_toe_op_verzameling`].
    pub fn pas_toe(&self, gram: &Gram) -> Option<Value> {
        match self {
            Afleiding::Veld { veld } => gram.veld(veld).filter(|w| gevuld(w)).cloned(),
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
                let rijen = rijen(gram, tabel);
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
                rijen(gram, tabel)
                    .iter()
                    .any(|r| r.get(een_regel) == Some(&Value::Bool(true))),
            )),
            Afleiding::Moment {
                moment: Moment::OpMoment,
            } => DateTime::parse_from_rfc3339(&gram.op_moment)
                .ok()
                .map(|m| Value::String(m.date_naive().format("%Y-%m-%d").to_string())),
            Afleiding::LaatsteVeld { .. }
            | Afleiding::LaatsteBevat { .. }
            | Afleiding::Bestaat { .. }
            | Afleiding::Som { .. } => None,
        }
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
            Afleiding::LaatsteVeld { veld, .. } => laatste(&door)?
                .and_then(|g| g.veld(veld))
                .filter(|w| gevuld(w))
                .cloned(),
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
        let moment = DateTime::parse_from_rfc3339(&gram.op_moment)
            .map_err(|e| format!("gram met ongeldig op_moment '{}': {e}", gram.op_moment))?;
        if gekozen.as_ref().is_none_or(|(m, _)| moment >= *m) {
            gekozen = Some((moment, gram));
        }
    }
    Ok(gekozen.map(|(_, g)| g))
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

fn rijen<'g>(gram: &'g Gram, tabel: &str) -> Vec<&'g Map<String, Value>> {
    gram.veld(tabel)
        .and_then(Value::as_array)
        .map(|a| a.iter().filter_map(Value::as_object).collect())
        .unwrap_or_default()
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

/// Reduceer de grammen van een kroniek tot een lexostatus. `None`: de
/// definitie kiest een gram (`kies`) en geen gram komt door het filter.
pub fn reduceer(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: &[Gram],
) -> Result<Option<Lexostatus>, String> {
    let r = &definitie.reduction;
    let mut door: Vec<&Gram> = Vec::new();
    for gram in grammen.iter().filter(|g| g.chronicle == r.kroniek) {
        if past(&r.filter, inputs, gram)? {
            door.push(gram);
        }
    }
    let gekozen = match r.kies {
        Some(Kies::Laatste) => match laatste(&door)? {
            Some(g) => Some(g),
            None => return Ok(None),
        },
        None => None,
    };
    let mut uit = Lexostatus {
        naam: definitie.name.clone(),
        zaakkenmerk: gekozen.and_then(|g| g.zaakkenmerk.clone()),
        op_moment: gekozen.map(|g| g.op_moment.clone()),
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
            afleiding.pas_toe(gram)
        } else {
            afleiding.pas_toe_op_verzameling(inputs, &door)?
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
            name: "aanvraag_ontvangen".into(),
            chronicle: "test_kroniek".into(),
            recording_actor: "test_instantie".into(),
            grondslag: vec!["testregeling_aanvraag#1".into()],
            op_moment: moment.into(),
            zaak: Zaak::Opent,
            zaakkenmerk: Some(zaak.into()),
            stroom: StroomVerwijzing {
                id: "test_aanvragen".into(),
                sha256: "0".repeat(64),
            },
            herkomst: None,
            fields: fields.as_object().unwrap().clone(),
        }
    }

    fn afl(yaml: &str) -> Afleiding {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    fn een(yaml: &str, fields: Value) -> Option<Value> {
        afl(yaml).pas_toe(&gram("z", "2025-03-12T10:14:03+01:00", fields))
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
    fn afleiding_moment() {
        // De datum in de eigen tijdzone van het moment.
        let g = gram("z", "2025-03-12T00:30:00+01:00", json!({}));
        assert_eq!(
            afl("{moment: op_moment}").pas_toe(&g),
            Some(json!("2025-03-12"))
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
        assert_eq!(l.niet_afgeleid, vec!["datum_mededeling"]);
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
            vec!["testregeling_afnemer#1", "testregeling_afnemer#2"]
        );
    }
}
