//! Kanalen en rollen: hoe iemand bij een proces binnenkomt, en wat hij daar
//! mag (`kanalen` en `rollen` in `proces.yaml`).
//!
//! Een kanaal is een nagebootste login: een paar identificatievelden, elk
//! met een label en een vormcontrole (een patroon, en eventueel de
//! elfproef). Het kanaal zegt ook onder welk pad van `$intake` zijn velden
//! bij de cel aankomen en welk veld de eigenaar van een zaak aanwijst. Er is
//! geen register en geen gecertificeerde login: wie een geldig nummer invult,
//! is voor deze PoC ingelogd. Wie namens een organisatie mag handelen, zegt
//! een register via de synthese, niet de login.
//!
//! Een rol noemt haar kanaal en de routegroepen die ze mag gebruiken
//! ([`Routes`]). Welke routes er in een groep zitten, bepaalt de runtime; wie
//! ze mag gebruiken, de configuratie.

use std::collections::BTreeMap;

use regelrecht_engine::LawExecutionService;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::ProcesDefinitie;
use crate::stroom::{Binding, Event, Zaak};

/// Een kanaal (`kanalen.<id>` in `proces.yaml`).
#[derive(Debug, Clone, Deserialize)]
pub struct KanaalDefinitie {
    /// Hoe de frontend het kanaal noemt, zoals "Inloggen als medewerker".
    pub label: String,
    /// Uitleg onder het label op het inlogscherm; de frontend zegt er zelf
    /// bij dat de login nagebootst is.
    #[serde(default)]
    pub uitleg: Option<String>,
    /// De identificatievelden, in de volgorde van het inlogscherm.
    pub velden: Vec<Identificatieveld>,
    /// Het veld dat de eigenaar van een zaak aanwijst: een aanvrager die een
    /// zaak volgt, moet een gram in die zaak hebben met zijn waarde van dit
    /// veld.
    #[serde(default)]
    pub eigenaar: Option<String>,
    /// Het pad onder `$intake` waaronder de velden bij de cel aankomen; zonder:
    /// de id van het kanaal. Een veld `kvk` van kanaal `x` is dan
    /// `$intake.x.kvk`.
    #[serde(default)]
    pub intake: Option<String>,
}

/// Een identificatieveld van een kanaal.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Identificatieveld {
    pub naam: String,
    pub label: String,
    /// Een reguliere expressie waaraan de hele waarde (na trimmen) voldoet;
    /// zonder: niet leeg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub patroon: Option<String>,
    /// Een controle bovenop het patroon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub controle: Option<Controle>,
    /// De melding bij een waarde die niet voldoet; zonder een algemene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub melding: Option<String>,
    /// Alleen cijfers: de frontend toont een numeriek toetsenbord.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub numeriek: bool,
}

/// Een controle op een identificatieveld die een patroon niet kan uitdrukken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Controle {
    /// De elfproef zoals voor een burgerservicenummer: negen cijfers, de
    /// eerste acht gewogen 9 tot en met 2, de laatste met -1, en de som
    /// deelbaar door 11.
    Elfproef,
}

/// De routegroepen van een proces. Een rol noemt de groepen die ze mag
/// gebruiken.
#[derive(Debug, Clone, Copy, PartialEq, Eq, PartialOrd, Ord, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Routes {
    /// Het portaal: formulier, toets, aanbod en indienen, voor wie namens
    /// zichzelf of zijn organisatie aanvraagt.
    Portaal,
    /// De behandeling: werkvoorraad, zaak, proefbesluit en besluit.
    Behandeling,
    /// Het loket: een aanvraag die langs een andere weg binnenkwam invoeren
    /// namens de aanvrager, met de dag van ontvangst.
    Loket,
}

impl Routes {
    pub fn als_tekst(self) -> &'static str {
        match self {
            Routes::Portaal => "portaal",
            Routes::Behandeling => "behandeling",
            Routes::Loket => "loket",
        }
    }
}

/// Een rol (`rollen.<id>` in `proces.yaml`).
#[derive(Debug, Clone, Deserialize)]
pub struct RolDefinitie {
    /// Het kanaal waarlangs de rol inlogt.
    pub kanaal: String,
    /// De routegroepen die de rol mag gebruiken.
    pub routes: Vec<Routes>,
    /// Hoe de frontend de rol noemt; zonder: de id.
    #[serde(default)]
    pub label: Option<String>,
    /// Waarom deze rol dit mag, zoals een mandaatregeling
    /// (`<regeling>#<artikel>`); een besluit draagt het mee bij de
    /// handelende actor.
    #[serde(default)]
    pub grondslag: Option<String>,
}

impl RolDefinitie {
    pub fn mag(&self, r: Routes) -> bool {
        self.routes.contains(&r)
    }
}

/// Een ingelogde gebruiker: in welke rol, langs welk kanaal, met welke
/// waarden van de identificatievelden.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct Sessie {
    pub rol: String,
    pub kanaal: String,
    pub velden: BTreeMap<String, String>,
}

impl KanaalDefinitie {
    /// Het pad onder `$intake` van de velden van dit kanaal.
    pub fn intake_prefix<'a>(&'a self, id: &'a str) -> &'a str {
        self.intake.as_deref().unwrap_or(id)
    }

    /// De paden onder `$intake` die dit kanaal levert.
    pub fn intake_paden(&self, id: &str) -> Vec<String> {
        let p = self.intake_prefix(id);
        self.velden
            .iter()
            .map(|v| format!("{p}.{}", v.naam))
            .collect()
    }

    /// Het pad onder `$intake` van het eigenaarveld, als het kanaal er een
    /// noemt.
    pub fn eigenaar_pad(&self, id: &str) -> Option<String> {
        self.eigenaar
            .as_ref()
            .map(|e| format!("{}.{e}", self.intake_prefix(id)))
    }

    /// Controleer de invoer van een login: elk veld is er, als tekst, en
    /// voldoet aan zijn vorm. Andere sleutels tellen niet.
    pub fn valideer(
        &self,
        invoer: &Map<String, Value>,
    ) -> Result<BTreeMap<String, String>, String> {
        let mut uit = BTreeMap::new();
        for v in &self.velden {
            let waarde = match invoer.get(&v.naam) {
                Some(Value::String(s)) => s.trim().to_string(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            };
            if waarde.is_empty() {
                return Err(format!("{} ontbreekt", v.label));
            }
            if !v.voldoet(&waarde) {
                return Err(v
                    .melding
                    .clone()
                    .unwrap_or_else(|| format!("{} is ongeldig", v.label)));
            }
            uit.insert(v.naam.clone(), waarde);
        }
        Ok(uit)
    }

    /// Controleer het kanaal zelf, bij het opstarten: velden met een unieke
    /// naam, patronen die te lezen zijn, en een eigenaar die een veld is.
    pub fn controleer(&self, id: &str) -> Vec<String> {
        let mut fouten = Vec::new();
        let mut namen: Vec<&str> = Vec::new();
        for v in &self.velden {
            if namen.contains(&v.naam.as_str()) {
                fouten.push(format!(
                    "kanaal '{id}': veld '{}' staat er twee keer",
                    v.naam
                ));
            }
            namen.push(&v.naam);
            if let Some(p) = &v.patroon {
                if let Err(e) = Regex::new(p) {
                    fouten.push(format!(
                        "kanaal '{id}': veld '{}' heeft een ongeldig patroon '{p}': {e}",
                        v.naam
                    ));
                }
            }
        }
        if let Some(e) = &self.eigenaar {
            if !namen.contains(&e.as_str()) {
                fouten.push(format!(
                    "kanaal '{id}': eigenaar '{e}' is geen veld van het kanaal ({})",
                    namen.join(", ")
                ));
            }
        }
        fouten
    }
}

impl Identificatieveld {
    fn voldoet(&self, waarde: &str) -> bool {
        let patroon = match &self.patroon {
            // Het hele veld, niet een stuk ervan.
            Some(p) => Regex::new(&format!("^(?:{p})$")).is_ok_and(|r| r.is_match(waarde)),
            None => true,
        };
        patroon
            && match self.controle {
                Some(Controle::Elfproef) => elfproef(waarde),
                None => true,
            }
    }
}

/// De elfproef van een burgerservicenummer: negen cijfers, gewogen 9, 8, ...,
/// 2 en -1, en de som deelbaar door 11.
pub fn elfproef(nummer: &str) -> bool {
    let cijfers: Vec<i64> = nummer
        .chars()
        .filter_map(|c| c.to_digit(10).map(i64::from))
        .collect();
    if cijfers.len() != 9 || nummer.len() != 9 {
        return false;
    }
    let som: i64 = cijfers[..8]
        .iter()
        .zip((2..=9).rev())
        .map(|(c, w)| c * w)
        .sum::<i64>()
        - cijfers[8];
    som % 11 == 0
}

/// Wat een kanaal de cel meegeeft onder `$intake`: `kanaal` en, voor elk
/// kanaal in `kanalen`, zijn velden: met de waarden van de ingelogde
/// gebruiker voor zijn eigen kanaal, en leeg (`null`) voor de andere. Zo
/// levert elk kanaal van een portaal elk pad dat het event bindt, en blijft
/// wat een ander kanaal zou leveren leeg.
pub fn intake<'a>(
    kanaal: &str,
    kanalen: impl IntoIterator<Item = (&'a str, &'a KanaalDefinitie)>,
    gebruiker: Option<(&str, &BTreeMap<String, String>)>,
) -> Value {
    let mut uit = Map::new();
    uit.insert("kanaal".into(), Value::String(kanaal.to_string()));
    for (id, k) in kanalen {
        let eigen = gebruiker.filter(|(g, _)| *g == id).map(|(_, v)| v);
        let velden: Map<String, Value> = k
            .velden
            .iter()
            .map(|v| {
                let w = eigen
                    .and_then(|e| e.get(&v.naam))
                    .map_or(Value::Null, |w| Value::String(w.clone()));
                (v.naam.clone(), w)
            })
            .collect();
        zet_pad(&mut uit, k.intake_prefix(id), Value::Object(velden));
    }
    Value::Object(uit)
}

/// Zet een waarde op een pad met punten, en maak de tussenliggende objecten.
pub fn zet_pad(doel: &mut Map<String, Value>, pad: &str, waarde: Value) {
    let mut delen = pad.split('.').peekable();
    let mut hier = doel;
    while let Some(deel) = delen.next() {
        if delen.peek().is_none() {
            hier.insert(deel.to_string(), waarde);
            return;
        }
        let volgend = hier
            .entry(deel.to_string())
            .or_insert_with(|| Value::Object(Map::new()));
        if !volgend.is_object() {
            *volgend = Value::Object(Map::new());
        }
        let Value::Object(m) = volgend else {
            return;
        };
        hier = m;
    }
}

/// De paden onder `$intake` die het portaal levert: `kanaal` en de velden
/// van elk kanaal van een rol met routes `portaal` of `loket`. Het loket
/// identificeert de aanvrager met de velden van een portaalkanaal.
pub fn portaal_intake_paden(d: &ProcesDefinitie) -> Vec<String> {
    let mut uit = vec!["kanaal".to_string()];
    for (id, k) in d.kanalen_met(Routes::Portaal) {
        uit.extend(k.intake_paden(id));
    }
    uit
}

/// Het pad onder `$intake` waaraan het event zijn `op_moment` bindt, als het
/// dat doet: daar zet het loket de dag van ontvangst.
pub fn ontvangstpad(event: &Event) -> Option<String> {
    match event.op_moment.as_ref()?.binding() {
        Binding::Intake(pad) => Some(pad),
        _ => None,
    }
}

/// De controles op `kanalen` en `rollen` van een proces bij het opstarten:
///
/// - elk kanaal is in orde ([`KanaalDefinitie::controleer`]);
/// - elke rol noemt een kanaal dat bestaat, en een grondslag die een geladen
///   artikel aanwijst;
/// - een portaal vraagt een rol met routes `portaal`, en zo'n rol een
///   portaal; een behandeling en routes `behandeling` net zo;
/// - routes `loket` vragen een portaal waarvan het event zijn `op_moment`
///   aan `$intake` bindt: daar komt de dag van ontvangst;
/// - volgt het portaal-event een zaak, dan noemt elk portaalkanaal een
///   eigenaar: wie een zaak volgt, moet haar kennen.
pub fn controleer_proces(
    d: &ProcesDefinitie,
    portaal_event: Option<&Event>,
    service: &LawExecutionService,
) -> Vec<String> {
    let mut fouten = Vec::new();
    for (id, k) in &d.kanalen {
        fouten.extend(k.controleer(id));
    }
    for (id, rol) in &d.rollen {
        if !d.kanalen.contains_key(&rol.kanaal) {
            fouten.push(format!(
                "rol '{id}': kanaal '{}' staat niet onder kanalen",
                rol.kanaal
            ));
        }
        if let Some(g) = &rol.grondslag {
            if let Err(f) = crate::regelingen::geldig(service, g) {
                fouten.push(format!("rol '{id}': {f}"));
            }
        }
    }
    let heeft = |r: Routes| d.rollen_met(r).next().is_some();
    for (r, blok, is_er) in [
        (Routes::Portaal, "portaal", d.portaal.is_some()),
        (Routes::Behandeling, "behandeling", d.behandeling.is_some()),
    ] {
        match (is_er, heeft(r)) {
            (true, false) => fouten.push(format!(
                "{blok} zonder rol die het mag: geef een rol routes: [{}]",
                r.als_tekst()
            )),
            (false, true) => fouten.push(format!(
                "een rol met routes {} en geen {blok}: die rol heeft niets te doen",
                r.als_tekst()
            )),
            _ => {}
        }
    }
    if heeft(Routes::Loket) {
        match (d.portaal.is_some(), portaal_event) {
            (false, _) => fouten.push(
                "een rol met routes loket en geen portaal: het loket voert een aanvraag in in het event van het portaal".into(),
            ),
            (true, Some(e)) if ontvangstpad(e).is_none() => fouten.push(format!(
                "loket: event '{}' bindt op_moment niet aan $intake; het loket geeft de dag van ontvangst op (Awb 4:13)",
                e.name
            )),
            _ => {}
        }
        if !heeft(Routes::Portaal) {
            fouten.push(
                "loket: geen portaalkanaal om de aanvrager mee aan te duiden; geef een rol routes: [portaal]".into(),
            );
        }
    }
    if portaal_event.is_some_and(|e| e.zaak == Zaak::Volgt) {
        for (id, k) in d.kanalen_met(Routes::Portaal) {
            if k.eigenaar.is_none() {
                fouten.push(format!(
                    "kanaal '{id}': het portaal volgt een zaak, en het kanaal noemt geen eigenaar"
                ));
            }
        }
    }
    fouten
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use serde_json::json;

    fn kanaal(yaml: &str) -> KanaalDefinitie {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    fn organisatie() -> KanaalDefinitie {
        kanaal(
            "label: Organisatie\nvelden:\n  - {naam: nummer, label: Organisatienummer, patroon: '[0-9]{8}', melding: een organisatienummer heeft acht cijfers}\n  - {naam: persoon, label: Naam}\neigenaar: nummer\n",
        )
    }

    fn burger() -> KanaalDefinitie {
        kanaal(
            "label: Burger\nvelden:\n  - {naam: nummer, label: Burgernummer, patroon: '[0-9]{9}', controle: elfproef}\neigenaar: nummer\nintake: burger\n",
        )
    }

    fn invoer(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn een_login_voldoet_aan_de_velden_van_het_kanaal() {
        let k = organisatie();
        let ok = k
            .valideer(&invoer(
                json!({"nummer": " 12345678 ", "persoon": "A. Tester", "machtiging": 1}),
            ))
            .unwrap();
        assert_eq!(ok["nummer"], "12345678");
        assert_eq!(ok["persoon"], "A. Tester");
        assert_eq!(
            k.valideer(&invoer(json!({"nummer": "1234567", "persoon": "A"}))),
            Err("een organisatienummer heeft acht cijfers".into())
        );
        // Het patroon geldt voor de hele waarde.
        assert!(k
            .valideer(&invoer(json!({"nummer": "123456789", "persoon": "A"})))
            .is_err());
        assert_eq!(
            k.valideer(&invoer(json!({"nummer": "12345678", "persoon": "  "}))),
            Err("Naam ontbreekt".into())
        );
    }

    #[test]
    fn de_elfproef() {
        assert!(elfproef("111222333"));
        assert!(elfproef("123456782"));
        assert!(!elfproef("123456789"));
        assert!(!elfproef("12345678"));
        assert!(!elfproef("12345678a"));
        let k = burger();
        assert!(k.valideer(&invoer(json!({"nummer": "111222333"}))).is_ok());
        assert_eq!(
            k.valideer(&invoer(json!({"nummer": "111222334"}))),
            Err("Burgernummer is ongeldig".into())
        );
    }

    #[test]
    fn de_intake_levert_elk_pad_van_elk_kanaal() {
        let (o, b) = (organisatie(), burger());
        let velden: BTreeMap<String, String> =
            [("nummer".to_string(), "111222333".to_string())].into();
        let i = intake(
            "portaal",
            [("organisatie", &o), ("burgerlogin", &b)],
            Some(("burgerlogin", &velden)),
        );
        assert_eq!(
            i,
            json!({"kanaal": "portaal", "organisatie": {"nummer": null, "persoon": null}, "burger": {"nummer": "111222333"}})
        );
        assert_eq!(
            o.intake_paden("organisatie"),
            ["organisatie.nummer", "organisatie.persoon"]
        );
        assert_eq!(b.eigenaar_pad("burgerlogin").unwrap(), "burger.nummer");
    }

    #[test]
    fn een_kanaal_wordt_bij_het_opstarten_gecontroleerd() {
        let k = kanaal(
            "label: X\nvelden:\n  - {naam: a, label: A, patroon: '[0-9'}\n  - {naam: a, label: B}\neigenaar: c\n",
        );
        let f = k.controleer("x");
        assert_eq!(f.len(), 3, "{f:?}");
        assert!(f[0].contains("ongeldig patroon"));
        assert!(f[1].contains("twee keer"));
        assert!(f[2].contains("eigenaar 'c'"));
        assert!(organisatie().controleer("o").is_empty());
    }

    #[test]
    fn zet_pad_maakt_de_objecten() {
        let mut m = Map::new();
        zet_pad(&mut m, "a.b.c", json!(1));
        zet_pad(&mut m, "a.d", json!(2));
        assert_eq!(Value::Object(m), json!({"a": {"b": {"c": 1}, "d": 2}}));
    }
}
