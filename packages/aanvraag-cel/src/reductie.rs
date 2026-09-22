//! Reductie tot lexostatus: de celconfiguratie laden, en een kroniek
//! reduceren tot de parameters van een artikel.
//!
//! Een lexostatus-definitie kiest met `filter` en `kies` een gram uit de
//! kroniek en leidt met `afleidingen` per parameter een waarde af. De
//! afleidingswoordenschat is klein en vast: `veld`, `gevuld`, `gelijk`,
//! `tabel` met `elke_regel` of `een_regel` (en `alleen_waar`), en `moment`.
//! Veldpaden zijn relatief aan `fields` van het gram, met punten ertussen.

use std::collections::BTreeMap;
use std::path::Path;

use chrono::DateTime;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::schema::{self, Soort};
use crate::stroom::Gram;

/// De celconfiguratie (`schema/chronolex/v0.1.0/lexostatus.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct CelConfig {
    pub cel: String,
    pub lexostatus_definitions: Vec<LexostatusDefinitie>,
    #[serde(default)]
    pub portaal: Option<Portaal>,
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

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Reductie {
    pub kroniek: String,
    pub filter: BTreeMap<String, String>,
    pub kies: Kies,
    pub afleidingen: BTreeMap<String, Afleiding>,
}

/// Welk gram telt als het filter er meer vindt.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Kies {
    /// Het gram met het laatste `op_moment`; een herstel is een nieuw gram.
    Laatste,
}

/// Een afleiding: hoe een parameter uit het gram volgt.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Afleiding {
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

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Moment {
    OpMoment,
}

/// Het portaalblok: welk event een indiening wordt en welke uitkomst de
/// toets vraagt.
#[derive(Debug, Clone, Deserialize)]
pub struct Portaal {
    pub stroom: String,
    pub event: String,
    pub toets: Toets,
    #[serde(default)]
    pub formulier: Option<FormulierVerwijzing>,
}

#[derive(Debug, Clone, Deserialize)]
pub struct Toets {
    pub lexostatus: String,
    pub regeling: String,
    pub uitkomst: String,
}

#[derive(Debug, Clone, Deserialize)]
pub struct FormulierVerwijzing {
    pub pad: String,
    pub scherm: String,
}

/// Lees een celconfiguratie uit tekst en valideer haar tegen het schema.
pub fn parse(tekst: &str, bron: &str) -> Result<CelConfig, Vec<String>> {
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

/// Laad de celconfiguratie uit een bestand.
pub fn laad(pad: &Path) -> Result<CelConfig, Vec<String>> {
    let bron = pad.display().to_string();
    let tekst = std::fs::read_to_string(pad).map_err(|e| vec![format!("{bron}: {e}")])?;
    parse(&tekst, &bron)
}

impl CelConfig {
    pub fn lexostatus(&self, naam: &str) -> Option<&LexostatusDefinitie> {
        self.lexostatus_definitions.iter().find(|d| d.name == naam)
    }
}

impl Afleiding {
    /// De veldpaden die deze afleiding leest.
    pub fn gelezen_paden(&self) -> Vec<&str> {
        match self {
            Afleiding::Veld { veld } => vec![veld],
            Afleiding::Gevuld { gevuld } => vec![gevuld],
            Afleiding::Gelijk { gelijk } => vec![&gelijk.veld],
            Afleiding::ElkeRegel { tabel, .. } | Afleiding::EenRegel { tabel, .. } => vec![tabel],
            Afleiding::Moment { .. } => vec![],
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

    /// Pas de afleiding toe op een gram. `None`: het gram zegt er niets over
    /// en de parameter blijft weg; er wordt niets aangevuld.
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
        }
    }
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
#[derive(Debug, Clone, Serialize)]
pub struct Lexostatus {
    pub naam: String,
    /// Het gram waaruit is afgeleid.
    pub zaakkenmerk: String,
    pub op_moment: String,
    pub parameters: BTreeMap<String, Value>,
    /// Parameters met een afleiding waarover het gram niets zegt.
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

/// Pas alle afleidingen van een definitie toe op een gram.
pub fn leid_af(definitie: &LexostatusDefinitie, gram: &Gram) -> Lexostatus {
    let mut parameters = BTreeMap::new();
    let mut niet_afgeleid = Vec::new();
    for (naam, afleiding) in &definitie.reduction.afleidingen {
        match afleiding.pas_toe(gram) {
            Some(w) => {
                parameters.insert(naam.clone(), w);
            }
            None => niet_afgeleid.push(naam.clone()),
        }
    }
    Lexostatus {
        naam: definitie.name.clone(),
        zaakkenmerk: gram.zaakkenmerk.clone(),
        op_moment: gram.op_moment.clone(),
        parameters,
        niet_afgeleid,
    }
}

/// Of een gram door het filter komt. Een waarde `$x` komt uit de inputs.
fn past(
    filter: &BTreeMap<String, String>,
    inputs: &Map<String, Value>,
    gram: &Gram,
) -> Result<bool, String> {
    for (sleutel, verwacht) in filter {
        let verwacht = match verwacht.strip_prefix('$') {
            Some(input) => inputs
                .get(input)
                .and_then(Value::as_str)
                .ok_or_else(|| format!("input '{input}' ontbreekt"))?,
            None => verwacht.as_str(),
        };
        let waarde = match sleutel.as_str() {
            "name" => Some(gram.name.as_str()),
            "type" => Some(gram.type_.as_str()),
            "soort" => gram.soort.as_deref(),
            "zaakkenmerk" => Some(gram.zaakkenmerk.as_str()),
            "recording_actor" => Some(gram.recording_actor.as_str()),
            "chronicle" => Some(gram.chronicle.as_str()),
            ander => return Err(format!("onbekende filtersleutel '{ander}'")),
        };
        if waarde != Some(verwacht) {
            return Ok(false);
        }
    }
    Ok(true)
}

/// Reduceer de grammen van een kroniek tot een lexostatus. `None`: geen gram
/// komt door het filter.
pub fn reduceer(
    definitie: &LexostatusDefinitie,
    inputs: &Map<String, Value>,
    grammen: &[Gram],
) -> Result<Option<Lexostatus>, String> {
    let mut gekozen: Option<(DateTime<chrono::FixedOffset>, &Gram)> = None;
    for gram in grammen {
        if gram.chronicle != definitie.reduction.kroniek
            || !past(&definitie.reduction.filter, inputs, gram)?
        {
            continue;
        }
        let moment = DateTime::parse_from_rfc3339(&gram.op_moment)
            .map_err(|e| format!("gram met ongeldig op_moment '{}': {e}", gram.op_moment))?;
        match definitie.reduction.kies {
            // `>=`: bij gelijk moment wint het later toegevoegde gram.
            Kies::Laatste => {
                if gekozen.as_ref().is_none_or(|(m, _)| moment >= *m) {
                    gekozen = Some((moment, gram));
                }
            }
        }
    }
    Ok(gekozen.map(|(_, gram)| leid_af(definitie, gram)))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::stroom::StroomVerwijzing;
    use serde_json::json;

    const CEL: &str = include_str!("../tests/fixtures/cel/lexostatussen.yaml");

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
            zaakkenmerk: zaak.into(),
            stroom: StroomVerwijzing {
                id: "test_aanvragen".into(),
                sha256: "0".repeat(64),
            },
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
        assert_eq!(c.portaal.unwrap().toets.uitkomst, "aanvraag_volledig");
    }

    #[test]
    fn ongeldige_afleiding_faalt_op_het_schema() {
        let tekst = CEL.replace("{veld: inhoud.aanvraagjaar}", "{som: inhoud.aanvraagjaar}");
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
        assert_eq!(l.op_moment, "2025-03-02T09:00:00+01:00");
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
        let l = leid_af(def, &g);
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
}
