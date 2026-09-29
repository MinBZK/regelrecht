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
use std::sync::OnceLock;

use regelrecht_engine::LawExecutionService;
use regex::Regex;
use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::ProcesDefinitie;
use crate::gram::zet_pad;
use crate::stroom::{Binding, Event, Zaak};

/// Een kanaal (`kanalen.<id>` in `proces.yaml`).
#[derive(Debug, Clone, Deserialize)]
pub struct KanaalDefinitie {
    /// Hoe de frontend het kanaal noemt, zoals "Inloggen als medewerker".
    pub label: String,
    /// Uitleg onder het label op het inlogscherm; de frontend zegt er zelf
    /// bij dat de login nagebootst is.
    #[serde(default)]
    pub explanation: Option<String>,
    /// De identificatievelden, in de volgorde van het inlogscherm.
    pub fields: Vec<Identificatieveld>,
    /// Het veld dat de eigenaar van een zaak aanwijst: een aanvrager die een
    /// zaak volgt, moet een gram in die zaak hebben met zijn waarde van dit
    /// veld.
    #[serde(default)]
    pub owner: Option<String>,
    /// Het pad onder `$intake` waaronder de velden bij de cel aankomen; zonder:
    /// de id van het kanaal. Een veld `kvk` van kanaal `x` is dan
    /// `$intake.x.kvk`.
    #[serde(default)]
    pub intake: Option<String>,
    /// Waarop het kanaal en zijn eigenaar rusten, zoals de regel die zegt met
    /// welk middel en namens wie iemand inlogt (`<regeling>#<artikel>`).
    #[serde(default)]
    pub legal_basis: Vec<String>,
}

/// Een identificatieveld van een kanaal.
#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Identificatieveld {
    pub name: String,
    pub label: String,
    /// Een reguliere expressie waaraan de hele waarde (na trimmen) voldoet;
    /// zonder: niet leeg.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pattern: Option<String>,
    /// Een controle bovenop het patroon.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub check: Option<Controle>,
    /// De melding bij een waarde die niet voldoet; zonder een algemene.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub message: Option<String>,
    /// Alleen cijfers: de frontend toont een numeriek toetsenbord.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub numeric: bool,
    /// Waarop het veld rust: de regel die het gegeven en zijn vorm kent,
    /// zoals het nummer dat een register toekent.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legal_basis: Vec<String>,
    /// Het patroon, gecompileerd: bij het laden (zie
    /// [`KanaalDefinitie::controleer`]), niet bij elke login.
    #[serde(skip)]
    regex: OnceLock<Regex>,
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
    Portal,
    /// De behandeling: werkvoorraad, zaak, proefbesluit en besluit.
    Handling,
    /// Het loket: een aanvraag die langs een andere weg binnenkwam invoeren
    /// namens de aanvrager, met de dag van ontvangst.
    Counter,
}

impl Routes {
    pub fn als_tekst(self) -> &'static str {
        match self {
            Routes::Portal => "portal",
            Routes::Handling => "handling",
            Routes::Counter => "counter",
        }
    }
}

/// Een rol (`rollen.<id>` in `proces.yaml`).
#[derive(Debug, Clone, Deserialize)]
pub struct RolDefinitie {
    /// Het kanaal waarlangs de rol inlogt.
    pub channel: String,
    /// De routegroepen die de rol mag gebruiken.
    pub routes: Vec<Routes>,
    /// Hoe de frontend de rol noemt; zonder: de id.
    #[serde(default)]
    pub label: Option<String>,
    /// Waarom deze rol dit mag, zoals een mandaatregeling
    /// (`<regeling>#<artikel>`); een besluit draagt het mee bij de
    /// handelende actor.
    #[serde(default)]
    pub legal_basis: Option<String>,
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
    pub role: String,
    pub channel: String,
    pub fields: BTreeMap<String, String>,
}

impl KanaalDefinitie {
    /// Het pad onder `$intake` van de velden van dit kanaal.
    pub fn intake_prefix<'a>(&'a self, id: &'a str) -> &'a str {
        self.intake.as_deref().unwrap_or(id)
    }

    /// De paden onder `$intake` die dit kanaal levert.
    pub fn intake_paden(&self, id: &str) -> Vec<String> {
        let p = self.intake_prefix(id);
        self.fields
            .iter()
            .map(|v| format!("{p}.{}", v.name))
            .collect()
    }

    /// Het pad onder `$intake` van het eigenaarveld, als het kanaal er een
    /// noemt.
    pub fn eigenaar_pad(&self, id: &str) -> Option<String> {
        self.owner
            .as_ref()
            .map(|e| format!("{}.{e}", self.intake_prefix(id)))
    }

    /// Controleer de invoer van een login: elk veld is er, als tekst, en
    /// voldoet aan zijn vorm. Andere sleutels tellen niet.
    pub fn valideer(&self, input: &Map<String, Value>) -> Result<BTreeMap<String, String>, String> {
        let mut uit = BTreeMap::new();
        for v in &self.fields {
            let value = match input.get(&v.name) {
                Some(Value::String(s)) => s.trim().to_string(),
                Some(Value::Number(n)) => n.to_string(),
                _ => String::new(),
            };
            if value.is_empty() {
                return Err(format!("{} ontbreekt", v.label));
            }
            if !v.voldoet(&value) {
                return Err(v
                    .message
                    .clone()
                    .unwrap_or_else(|| format!("{} is ongeldig", v.label)));
            }
            uit.insert(v.name.clone(), value);
        }
        Ok(uit)
    }

    /// Controleer het kanaal zelf, bij het opstarten: velden met een unieke
    /// naam, patronen die te lezen zijn, en een eigenaar die een veld is.
    pub fn controleer(&self, id: &str) -> Vec<String> {
        let mut fouten = Vec::new();
        let mut namen: Vec<&str> = Vec::new();
        for v in &self.fields {
            if namen.contains(&v.name.as_str()) {
                fouten.push(format!(
                    "kanaal '{id}': veld '{}' staat er twee keer",
                    v.name
                ));
            }
            namen.push(&v.name);
            if let Some(p) = &v.pattern {
                if let Some(Err(e)) = v.regex() {
                    fouten.push(format!(
                        "kanaal '{id}': veld '{}' heeft een ongeldig patroon '{p}': {e}",
                        v.name
                    ));
                }
            }
        }
        if let Some(e) = &self.owner {
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
    /// Het patroon als reguliere expressie voor het hele veld (niet een stuk
    /// ervan), een keer gecompileerd. `None` zonder patroon.
    fn regex(&self) -> Option<Result<&Regex, regex::Error>> {
        let p = self.pattern.as_ref()?;
        if let Some(r) = self.regex.get() {
            return Some(Ok(r));
        }
        Some(Regex::new(&format!("^(?:{p})$")).map(|r| self.regex.get_or_init(|| r)))
    }

    fn voldoet(&self, value: &str) -> bool {
        let pattern = match self.regex() {
            Some(r) => r.is_ok_and(|r| r.is_match(value)),
            None => true,
        };
        pattern
            && match self.check {
                Some(Controle::Elfproef) => elfproef(value),
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
    let sum: i64 = cijfers[..8]
        .iter()
        .zip((2..=9).rev())
        .map(|(c, w)| c * w)
        .sum::<i64>()
        - cijfers[8];
    sum % 11 == 0
}

/// Wat een kanaal de cel meegeeft onder `$intake`: `kanaal` en, voor elk
/// kanaal in `kanalen`, zijn velden: met de waarden van de ingelogde
/// gebruiker voor zijn eigen kanaal, en leeg (`null`) voor de andere. Zo
/// levert elk kanaal van een portaal elk pad dat het event bindt, en blijft
/// wat een ander kanaal zou leveren leeg.
pub fn intake<'a>(
    channel: &str,
    channels: impl IntoIterator<Item = (&'a str, &'a KanaalDefinitie)>,
    gebruiker: Option<(&str, &BTreeMap<String, String>)>,
) -> Value {
    let mut uit = Map::new();
    uit.insert("channel".into(), Value::String(channel.to_string()));
    for (id, k) in channels {
        let eigen = gebruiker.filter(|(g, _)| *g == id).map(|(_, v)| v);
        let fields: Map<String, Value> = k
            .fields
            .iter()
            .map(|v| {
                let w = eigen
                    .and_then(|e| e.get(&v.name))
                    .map_or(Value::Null, |w| Value::String(w.clone()));
                (v.name.clone(), w)
            })
            .collect();
        zet_pad(&mut uit, k.intake_prefix(id), Value::Object(fields));
    }
    Value::Object(uit)
}

/// De paden onder `$intake` die het portaal levert: `kanaal` en de velden
/// van elk kanaal van een rol met routes `portaal` of `loket`. Het loket
/// identificeert de aanvrager met de velden van een portaalkanaal.
pub fn portaal_intake_paden(d: &ProcesDefinitie) -> Vec<String> {
    let mut uit = vec!["channel".to_string()];
    for (id, k) in d.kanalen_met(Routes::Portal) {
        uit.extend(k.intake_paden(id));
    }
    uit
}

/// Het pad onder `$intake` waaraan het event zijn `op_moment` bindt, als het
/// dat doet: daar zet het loket de dag van ontvangst.
pub fn ontvangstpad(event: &Event) -> Option<String> {
    match event.effective_at.as_ref()?.binding() {
        Binding::Intake(path) => Some(path),
        _ => None,
    }
}

/// De controles op `kanalen` en `rollen` van een proces bij het opstarten:
///
/// - elk kanaal is in orde ([`KanaalDefinitie::controleer`]), en elke
///   grondslag van een kanaal of van een veld wijst een geladen artikel aan,
///   met het lid dat ze noemt;
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
    for (id, k) in &d.channels {
        fouten.extend(k.controleer(id));
        let fields = k.fields.iter().flat_map(|v| {
            v.legal_basis
                .iter()
                .map(move |g| (format!("kanaal '{id}', veld '{}'", v.name), g))
        });
        for (waar, g) in k
            .legal_basis
            .iter()
            .map(|g| (format!("kanaal '{id}'"), g))
            .chain(fields)
        {
            if let Err(f) = crate::regelingen::geldig(service, g) {
                fouten.push(format!("{waar}: {f}"));
            }
        }
    }
    for (id, role) in &d.roles {
        if !d.channels.contains_key(&role.channel) {
            fouten.push(format!(
                "rol '{id}': kanaal '{}' staat niet onder kanalen",
                role.channel
            ));
        }
        if let Some(g) = &role.legal_basis {
            if let Err(f) = crate::regelingen::geldig(service, g) {
                fouten.push(format!("rol '{id}': {f}"));
            }
        }
    }
    let heeft = |r: Routes| d.rollen_met(r).next().is_some();
    for (r, blok, is_er) in [
        (Routes::Portal, "portaal", d.portal.is_some()),
        (Routes::Handling, "behandeling", d.handling.is_some()),
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
    if heeft(Routes::Counter) {
        match (d.portal.is_some(), portaal_event) {
            (false, _) => fouten.push(
                "een rol met routes loket en geen portal: het loket voert een aanvraag in in het event van het portaal".into(),
            ),
            (true, Some(e)) if ontvangstpad(e).is_none() => fouten.push(format!(
                "loket: event '{}' bindt op_moment niet aan $intake; het loket geeft de dag van ontvangst op (Awb 4:13)",
                e.name
            )),
            _ => {}
        }
        if !heeft(Routes::Portal) {
            fouten.push(
                "loket: geen portaalkanaal om de aanvrager mee aan te duiden; geef een rol routes: [portal]".into(),
            );
        }
    }
    if portaal_event.is_some_and(|e| e.case == Zaak::Follows) {
        for (id, k) in d.kanalen_met(Routes::Portal) {
            if k.owner.is_none() {
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

    fn channel(yaml: &str) -> KanaalDefinitie {
        serde_yaml_ng::from_str(yaml).unwrap()
    }

    fn organisatie() -> KanaalDefinitie {
        channel(
            "label: Organisatie\nfields:\n  - {name: nummer, label: Organisatienummer, pattern: '[0-9]{8}', message: een organisatienummer heeft acht cijfers}\n  - {name: persoon, label: Naam}\nowner: nummer\n",
        )
    }

    fn burger() -> KanaalDefinitie {
        channel(
            "label: Burger\nfields:\n  - {name: nummer, label: Burgernummer, pattern: '[0-9]{9}', check: elfproef}\nowner: nummer\nintake: burger\n",
        )
    }

    fn input(v: Value) -> Map<String, Value> {
        v.as_object().unwrap().clone()
    }

    #[test]
    fn een_login_voldoet_aan_de_velden_van_het_kanaal() {
        let k = organisatie();
        let ok = k
            .valideer(&input(
                json!({"nummer": " 12345678 ", "persoon": "A. Tester", "machtiging": 1}),
            ))
            .unwrap();
        assert_eq!(ok["nummer"], "12345678");
        assert_eq!(ok["persoon"], "A. Tester");
        assert_eq!(
            k.valideer(&input(json!({"nummer": "1234567", "persoon": "A"}))),
            Err("een organisatienummer heeft acht cijfers".into())
        );
        // Het patroon geldt voor de hele waarde.
        assert!(k
            .valideer(&input(json!({"nummer": "123456789", "persoon": "A"})))
            .is_err());
        assert_eq!(
            k.valideer(&input(json!({"nummer": "12345678", "persoon": "  "}))),
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
        assert!(k.valideer(&input(json!({"nummer": "111222333"}))).is_ok());
        assert_eq!(
            k.valideer(&input(json!({"nummer": "111222334"}))),
            Err("Burgernummer is ongeldig".into())
        );
    }

    #[test]
    fn de_intake_levert_elk_pad_van_elk_kanaal() {
        let (o, b) = (organisatie(), burger());
        let fields: BTreeMap<String, String> =
            [("nummer".to_string(), "111222333".to_string())].into();
        let i = intake(
            "portaal",
            [("organisatie", &o), ("burgerlogin", &b)],
            Some(("burgerlogin", &fields)),
        );
        assert_eq!(
            i,
            json!({"channel": "portaal", "organisatie": {"nummer": null, "persoon": null}, "burger": {"nummer": "111222333"}})
        );
        assert_eq!(
            o.intake_paden("organisatie"),
            ["organisatie.nummer", "organisatie.persoon"]
        );
        assert_eq!(b.eigenaar_pad("burgerlogin").unwrap(), "burger.nummer");
    }

    #[test]
    fn een_kanaal_wordt_bij_het_opstarten_gecontroleerd() {
        let k = channel(
            "label: X\nfields:\n  - {name: a, label: A, pattern: '[0-9'}\n  - {name: a, label: B}\nowner: c\n",
        );
        let f = k.controleer("x");
        assert_eq!(f.len(), 3, "{f:?}");
        assert!(f[0].contains("ongeldig patroon"));
        assert!(f[1].contains("twee keer"));
        assert!(f[2].contains("eigenaar 'c'"));
        assert!(organisatie().controleer("o").is_empty());
    }
}
