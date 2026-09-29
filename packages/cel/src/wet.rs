//! De wet als bron van de gramvorm en de reductie (voorstel "lexostatus in
//! de wet", 28-09-2026).
//!
//! Een artikel kan in `produces.extensions.chronolex` (RFC-022 §3.2: een
//! namespace per integratie) twee dingen zeggen:
//!
//! - `vestigt`: de feiten die dit artikel doet ontstaan, als event van een
//!   kroniek: het type, de soort, de stage, naar welk gram het verwijst
//!   (`verwijst`, met een naam uit de wettekst: een besluit `op_aanvraag`,
//!   een betaling naar het `besluit`), de grondslag, waarom het `op_moment`
//!   rechtens telt en welke velden het gram draagt. Of het breidt een event uit dat een ander artikel
//!   vestigt (`breidt_uit`), met velden en grondslag erbij: zo vestigt de Awb
//!   de aanvraag en voegt een bijzondere wet haar inhoud toe.
//! - `leest`: hoe het artikel zijn eigen parameters uit de kroniek leest, in
//!   de woordenschat van de reductie ([`crate::reductie`]). Dat is een
//!   lexostatus "vanuit het gevraagde perspectief" (positionpaper): het
//!   perspectief is het artikel dat vraagt. Zij heet naar het artikel
//!   (`<regeling>#<artikel>`).
//!
//! Een stroom noemt per event welke artikelen het vestigen (`vestigt:`) en
//! houdt alleen de registratie: intake, de binding van elk veld, de bron van
//! `op_moment`. [`vestig`] vult de rest in uit de wet en controleert de velden
//! tegen de wet. [`lexostatussen`] maakt van elk lezend artikel waarvan de
//! feiten in de cel liggen een lexostatus-definitie, die daarna door dezelfde
//! controles en dezelfde reductie gaat als een definitie uit `cel.yaml`.

use std::collections::{BTreeMap, BTreeSet};

use regelrecht_engine::{Article, LawExecutionService};
use serde::Deserialize;
use serde_json::{Map, Value};

use crate::reductie::{
    Afgeleid, Filter, InputDefinitie, Kies, LexostatusDefinitie, Reductie, WetAanvulling,
};
use crate::schema::{self, Soort};
use crate::stroom::{Stroom, Verwijzing};

/// De namespace in `produces.extensions` (RFC-022 §3.2).
pub const NAMESPACE: &str = "chronolex";

/// Het blok `produces.extensions.chronolex` van een artikel.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chronolex {
    #[serde(default)]
    pub vestigt: Vec<Vestiging>,
    #[serde(default)]
    pub leest: Option<Leest>,
}

/// Een lezing, of meer: een artikel kan per lid anders lezen (lid 2 leest
/// het verzuimoordeel, lid 3 tot 5 de aanvraag). Elke lezing wordt een eigen
/// lexostatus.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Leest {
    Een(Lezing),
    Meer(Vec<Lezing>),
}

impl Leest {
    pub fn lezingen(&self) -> Vec<&Lezing> {
        match self {
            Leest::Een(l) => vec![l],
            Leest::Meer(v) => v.iter().collect(),
        }
    }
}

/// Een feit dat een artikel vestigt, of de uitbreiding van een feit dat een
/// ander artikel vestigt.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Vestiging {
    /// Het event dat dit artikel vestigt (de naam in de kroniek).
    #[serde(default)]
    pub event: Option<String>,
    /// Het event dat een ander artikel vestigt en dat dit artikel uitbreidt.
    #[serde(default)]
    pub breidt_uit: Option<String>,
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub soort: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    /// Naar welk gram een gram van dit event verwijst, per naam uit de
    /// wettekst (Wpp 107 "besluit op de aanvraag": `op_aanvraag`; Awb 3:41
    /// "bekendmaking van besluiten": `besluit`), en wat dat gram moet zijn.
    #[serde(default)]
    pub verwijst: BTreeMap<String, Verwijzing>,
    /// De grondslag die het gram draagt. Zonder: het artikel zelf bij een
    /// event, niets bij een uitbreiding.
    #[serde(default)]
    pub grondslag: Option<Vec<String>>,
    #[serde(default)]
    pub op_moment: Option<OpMomentWet>,
    #[serde(default)]
    pub velden: Option<Velden>,
    /// Naamsbrug: onder welke naam een ander artikel een veld van dit gram
    /// leest (`<naam bij de lezer>: <veld van het gram>`). Zo leest Awb 4:52
    /// "het vastgestelde bedrag" zonder de naam van de uitkomst van de
    /// bijzondere wet te kennen.
    #[serde(default)]
    pub als: BTreeMap<String, String>,
}

impl Vestiging {
    /// De verwijzingen zoals de stroom ze zou noemen, voor het document van
    /// de stroom (`GET /api/stroom`); `None` zonder verwijzing.
    fn verwijst_als_json(&self) -> Option<Value> {
        if self.verwijst.is_empty() {
            return None;
        }
        let mut uit = Map::new();
        for (naam, v) in &self.verwijst {
            let naar = match &v.naar {
                crate::stroom::Naar::Artikel(a) | crate::stroom::Naar::Event(a) => {
                    Value::String(a.clone())
                }
                crate::stroom::Naar::Stage(s) => serde_json::json!({"stage": s}),
            };
            uit.insert(
                naam.clone(),
                serde_json::json!({"naar": naar, "verplicht": v.verplicht}),
            );
        }
        Some(Value::Object(uit))
    }
}

/// Waarom het `op_moment` rechtens telt, en welk gegeven het is. De bron (wie
/// het opgeeft, `$external` of `$intake`) staat in de stroom: dat is
/// registratie.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct OpMomentWet {
    /// De parameter van dit artikel die het moment is (zoals de besluitdatum).
    #[serde(default)]
    pub parameter: Option<String>,
    /// Het veld van het gram dat het moment is (zoals de dag van betaling).
    #[serde(default)]
    pub veld: Option<String>,
    pub grondslag: Vec<String>,
}

/// De velden van een gram: een lijst veldpaden, of een trefwoord.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Velden {
    /// `uitkomsten`: de uitkomsten van dit artikel (een besluit draagt wat
    /// het besluit uitkomt). `stage`: wat de stage van de procedure vraagt
    /// (`requires`, RFC-008) en de uitkomsten van de haken op die stage
    /// (RFC-007), zoals de bezwaartermijn van Awb 6:8 bij de bekendmaking.
    Trefwoord(String),
    /// Veldpaden onder `fields` (`inhoud.subsidiejaar`); een pad dekt alles
    /// eronder.
    Lijst(Vec<String>),
    /// Veldpaden met hun type, zoals Awb 4:87 het bedrag van een betaling
    /// noemt: `{bedrag: {type: amount, unit: eurocent}}`.
    Getypeerd(BTreeMap<String, Veldtype>),
}

/// Het type van een veld van een gram, zoals het vestigende artikel het
/// noemt.
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Veldtype {
    #[serde(rename = "type")]
    pub type_: regelrecht_law_model::ParameterType,
    #[serde(default)]
    pub unit: Option<String>,
}

/// Hoe een artikel zijn parameters uit de kroniek leest.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Lezing {
    /// Het lid dat deze lezing draagt, als een artikel meer lezingen heeft;
    /// de lexostatus heet dan `<regeling>#<artikel> lid <n>`.
    #[serde(default)]
    pub lid: Option<Value>,
    /// Welk gram het artikel leest, als het er een kiest (met `kies`).
    #[serde(default)]
    pub uit: Option<Map<String, Value>>,
    #[serde(default)]
    pub kies: Option<Kies>,
    /// `dit`: het artikel leest de grammen van het besluit waarvoor het
    /// gevraagd wordt. De runtime geeft een bron van de groep alleen de
    /// wortel; zij leest dus per wortel (een wortel met een besluit geeft
    /// hetzelfde).
    #[serde(default)]
    pub besluit: Option<String>,
    /// Per parameter van dit artikel de afleiding, met optioneel `uit` (welke
    /// grammen) en `grondslag` (zonder: dit artikel).
    pub parameters: BTreeMap<String, Value>,
}

/// Een lexostatus uit de wet: welk artikel haar leest, en de typen van de
/// parameters zoals dat artikel ze declareert (voor de engine-route).
#[derive(Debug, Clone, Default, PartialEq)]
pub struct Wetlezing {
    pub artikel: String,
    pub typen: BTreeMap<String, String>,
    /// Het artikel leest per besluit (`besluit: dit`).
    pub besluit_dit: bool,
}

/// Een artikel met een chronolex-blok.
pub struct Wetartikel<'s> {
    /// `<regeling>#<artikel>`.
    pub verwijzing: String,
    pub artikel: &'s Article,
    pub chronolex: Chronolex,
}

/// Het chronolex-blok van een artikel, als het er een heeft.
fn blok(artikel: &Article) -> Option<&Value> {
    artikel
        .get_execution_spec()?
        .produces
        .as_ref()?
        .extensions
        .as_ref()?
        .get(NAMESPACE)
}

/// Elk artikel in het corpus met een chronolex-blok, per verwijzing. Een blok
/// dat niet te lezen is, is een fout met het artikel erbij.
pub fn artikelen(
    service: &LawExecutionService,
) -> Result<BTreeMap<String, Wetartikel<'_>>, Vec<String>> {
    let resolver = service.resolver();
    let mut uit = BTreeMap::new();
    let mut fouten = Vec::new();
    for id in resolver.list_laws() {
        let Some(law) = resolver.get_law(id) else {
            continue;
        };
        for artikel in &law.articles {
            let Some(b) = blok(artikel) else { continue };
            let verwijzing = format!("{id}#{}", artikel.number);
            match serde_json::from_value::<Chronolex>(b.clone()) {
                Ok(chronolex) => {
                    uit.insert(
                        verwijzing.clone(),
                        Wetartikel {
                            verwijzing,
                            artikel,
                            chronolex,
                        },
                    );
                }
                Err(e) => fouten.push(format!(
                    "{verwijzing}: produces.extensions.{NAMESPACE} is niet te lezen: {e}"
                )),
            }
        }
    }
    if fouten.is_empty() {
        Ok(uit)
    } else {
        Err(fouten)
    }
}

/// De uitkomsten van een artikel.
fn uitkomsten(artikel: &Article) -> Vec<String> {
    artikel
        .get_execution_spec()
        .and_then(|e| e.output.as_ref())
        .into_iter()
        .flatten()
        .map(|o| o.name.clone())
        .collect()
}

/// De parameters van een artikel met hun type (`date`, `number`, ...).
fn parametertypen(artikel: &Article) -> BTreeMap<String, String> {
    artikel
        .get_execution_spec()
        .and_then(|e| e.parameters.as_ref())
        .into_iter()
        .flatten()
        .map(|p| {
            let t = serde_json::to_value(p.param_type)
                .ok()
                .and_then(|v| v.as_str().map(str::to_string))
                .unwrap_or_default();
            (p.name.clone(), t)
        })
        .collect()
}

/// Wat een stage vraagt: `requires` van die stage in een procedure (RFC-008)
/// en de uitkomsten van de artikelen met een haak op die stage (RFC-007).
fn stagevelden(service: &LawExecutionService, stage: &str) -> Vec<String> {
    let resolver = service.resolver();
    let mut uit: Vec<String> = Vec::new();
    let mut voeg = |n: String| {
        if !uit.contains(&n) {
            uit.push(n);
        }
    };
    for id in resolver.list_laws() {
        let Some(law) = resolver.get_law(id) else {
            continue;
        };
        for p in law.procedure.iter().flatten() {
            for s in p.stages.iter().filter(|s| s.name == stage) {
                for r in s.requires.iter().flatten() {
                    voeg(r.name.clone());
                }
            }
        }
    }
    for id in resolver.list_laws() {
        let Some(law) = resolver.get_law(id) else {
            continue;
        };
        for artikel in &law.articles {
            let haakt = artikel
                .machine_readable
                .as_ref()
                .and_then(|m| m.hooks.as_ref())
                .is_some_and(|h| {
                    h.iter()
                        .any(|h| h.applies_to.stage.as_deref() == Some(stage))
                });
            if haakt {
                for o in uitkomsten(artikel) {
                    voeg(o);
                }
            }
        }
    }
    uit
}

/// De veldpaden die een vestiging declareert.
fn veldpaden(
    service: &LawExecutionService,
    wa: &Wetartikel<'_>,
    v: &Vestiging,
    stage: Option<&str>,
) -> Result<Vec<String>, String> {
    Ok(match &v.velden {
        None => Vec::new(),
        Some(Velden::Lijst(l)) => l.clone(),
        Some(Velden::Getypeerd(m)) => m.keys().cloned().collect(),
        Some(Velden::Trefwoord(t)) if t == "uitkomsten" => uitkomsten(wa.artikel),
        Some(Velden::Trefwoord(t)) if t == "stage" => match stage {
            Some(s) => stagevelden(service, s),
            None => {
                return Err(format!(
                    "{}: velden: stage, maar het event heeft geen stage",
                    wa.verwijzing
                ))
            }
        },
        Some(Velden::Trefwoord(t)) => {
            return Err(format!(
                "{}: velden '{t}' is geen lijst en geen trefwoord (uitkomsten, stage)",
                wa.verwijzing
            ))
        }
    })
}

/// Vul de events met `vestigt` in uit de wet, en controleer ze. Een fout
/// noemt de stroom, het event en het artikel.
pub fn vestig(strommen: &mut [Stroom], service: &LawExecutionService) -> Vec<String> {
    if !strommen
        .iter()
        .any(|s| s.events.iter().any(|e| !e.vestigt.is_empty()))
    {
        return Vec::new();
    }
    let wet = match artikelen(service) {
        Ok(w) => w,
        Err(f) => return f,
    };
    let mut fouten = Vec::new();
    for stroom in strommen.iter_mut() {
        for i in 0..stroom.events.len() {
            if stroom.events[i].vestigt.is_empty() {
                continue;
            }
            let waar = format!("stroom '{}', event '{}'", stroom.id, stroom.events[i].name);
            match vestig_event(stroom, i, &wet, service) {
                Ok(()) => {}
                Err(f) => fouten.extend(f.into_iter().map(|f| format!("{waar}: {f}"))),
            }
        }
        if fouten.is_empty() {
            // Het ingevulde document hoort het schema te halen zoals een
            // stroom zonder `vestigt`.
            let mut kopie = stroom.document.clone();
            if let Some(events) = kopie.get_mut("events").and_then(Value::as_array_mut) {
                for e in events {
                    if let Some(o) = e.as_object_mut() {
                        o.remove("vestigt");
                    }
                }
            }
            if let Err(f) = schema::valideer(Soort::Stroom, &kopie) {
                fouten.extend(
                    f.into_iter()
                        .map(|f| format!("stroom '{}', ingevuld uit de wet: {f}", stroom.id)),
                );
            }
        }
    }
    fouten
}

fn vestig_event(
    stroom: &mut Stroom,
    i: usize,
    wet: &BTreeMap<String, Wetartikel<'_>>,
    service: &LawExecutionService,
) -> Result<(), Vec<String>> {
    let naam = stroom.events[i].name.clone();
    let mut fouten = Vec::new();
    // Per artikel in de volgorde van de stroom: de vestiging of uitbreiding.
    let mut delen: Vec<(&Wetartikel<'_>, &Vestiging, bool)> = Vec::new();
    for r in &stroom.events[i].vestigt {
        let Some(wa) = wet.get(r) else {
            fouten.push(format!(
                "artikel '{r}' is niet geladen of heeft geen produces.extensions.{NAMESPACE}"
            ));
            continue;
        };
        let mut gevonden = false;
        for v in &wa.chronolex.vestigt {
            if v.event.as_deref() == Some(naam.as_str()) {
                delen.push((wa, v, true));
                gevonden = true;
            } else if v.breidt_uit.as_deref() == Some(naam.as_str()) {
                delen.push((wa, v, false));
                gevonden = true;
            }
        }
        if !gevonden {
            fouten.push(format!(
                "artikel '{r}' vestigt '{naam}' niet en breidt het niet uit"
            ));
        }
    }
    let basis: Vec<&(&Wetartikel<'_>, &Vestiging, bool)> = delen.iter().filter(|d| d.2).collect();
    let basis = match basis[..] {
        [b] => b,
        [] => {
            fouten.push(format!(
                "geen van de artikelen in vestigt vestigt '{naam}' zelf (event: {naam})"
            ));
            return Err(fouten);
        }
        _ => {
            fouten.push(format!("meer dan een artikel vestigt '{naam}'"));
            return Err(fouten);
        }
    };
    for (wa, v, is_basis) in &delen {
        if v.event.is_some() && v.breidt_uit.is_some() {
            fouten.push(format!("{}: event en breidt_uit tegelijk", wa.verwijzing));
        }
        if !is_basis
            && (v.type_.is_some()
                || v.soort.is_some()
                || v.stage.is_some()
                || !v.verwijst.is_empty())
        {
            fouten.push(format!(
                "{}: een uitbreiding zet geen type, soort, stage of verwijzing; dat doet het artikel dat '{naam}' vestigt",
                wa.verwijzing
            ));
        }
        if let Some(p) = v.op_moment.as_ref().and_then(|o| o.parameter.as_ref()) {
            if !parametertypen(wa.artikel).contains_key(p) {
                fouten.push(format!(
                    "{}: op_moment.parameter '{p}' is geen parameter van dit artikel",
                    wa.verwijzing
                ));
            }
        }
    }
    let (bwa, bv, _) = basis;
    let Some(type_) = bv.type_.clone() else {
        fouten.push(format!("{}: vestigt '{naam}' zonder type", bwa.verwijzing));
        return Err(fouten);
    };
    let mut grondslag: Vec<String> = Vec::new();
    let mut moment_grondslag: Vec<String> = Vec::new();
    let mut paden: Vec<(String, String)> = Vec::new();
    let mut als = BTreeMap::new();
    for (wa, v, is_basis) in &delen {
        let g = match &v.grondslag {
            Some(g) => g.clone(),
            None if *is_basis => vec![wa.verwijzing.clone()],
            None => Vec::new(),
        };
        for x in g {
            if !grondslag.contains(&x) {
                grondslag.push(x);
            }
        }
        for x in v.op_moment.iter().flat_map(|o| o.grondslag.iter()) {
            if !moment_grondslag.contains(x) {
                moment_grondslag.push(x.clone());
            }
        }
        match veldpaden(service, wa, v, bv.stage.as_deref()) {
            Ok(p) => paden.extend(p.into_iter().map(|p| (p, wa.verwijzing.clone()))),
            Err(f) => fouten.push(f),
        }
        als.extend(v.als.clone());
    }
    for x in grondslag.iter().chain(&moment_grondslag) {
        if let Err(f) = crate::regelingen::geldig(service, x) {
            fouten.push(f);
        }
    }

    let event = &mut stroom.events[i];
    if !event.verwijst.is_empty() {
        fouten.push(
            "de stroom noemt verwijst, maar het event heeft vestigt: de verwijzingen komen uit de wet"
                .into(),
        );
    }
    // De velden: elk blad van de stroom valt onder een pad van de wet, en
    // elk pad van de wet heeft een blad in de stroom.
    let bladeren: Vec<String> = event.bladeren().into_iter().map(|b| b.pad).collect();
    let onder = |blad: &str, pad: &str| blad == pad || blad.starts_with(&format!("{pad}."));
    for b in &bladeren {
        if !paden.iter().any(|(p, _)| onder(b, p)) {
            fouten.push(format!(
                "veld '{b}' staat in de stroom, maar geen artikel in vestigt declareert het"
            ));
        }
    }
    for (p, r) in &paden {
        if !bladeren.iter().any(|b| onder(b, p)) {
            fouten.push(format!(
                "{r} declareert veld '{p}', maar de stroom bindt het niet"
            ));
        }
    }
    match (&mut event.op_moment, moment_grondslag.is_empty()) {
        (Some(o), false) => o.grondslag = moment_grondslag.clone(),
        (Some(_), true) => fouten.push(
            "de stroom bindt op_moment, maar de wet zegt niet waarom dat moment rechtens telt (op_moment.grondslag)"
                .into(),
        ),
        (None, false) => fouten.push(
            "de wet noemt een op_moment, maar de stroom bindt het niet (op_moment.bron)".into(),
        ),
        (None, true) => {}
    }
    if !fouten.is_empty() {
        return Err(fouten);
    }
    event.type_ = type_;
    event.soort = bv.soort.clone();
    event.stage = bv.stage.clone();
    event.verwijst = bv.verwijst.clone();
    event.veldtypen = delen
        .iter()
        .filter_map(|(_, v, _)| match &v.velden {
            Some(Velden::Getypeerd(m)) => Some(m.clone()),
            _ => None,
        })
        .flatten()
        .collect();
    event.grondslag = grondslag;
    event.als = als;

    // Het document (`GET /api/stroom`) toont het event zoals het geldt.
    if let Some(e) = stroom
        .document
        .get_mut("events")
        .and_then(|e| e.get_mut(i))
        .and_then(Value::as_object_mut)
    {
        let event = &stroom.events[i];
        e.insert("type".into(), Value::String(event.type_.clone()));
        if let Some(s) = &event.soort {
            e.insert("soort".into(), Value::String(s.clone()));
        }
        if let Some(s) = &event.stage {
            e.insert("stage".into(), Value::String(s.clone()));
        }
        if let Some(v) = bv.verwijst_als_json() {
            e.insert("verwijst".into(), v);
        }
        e.insert("grondslag".into(), serde_json::json!(event.grondslag));
        if let Some(o) = e.get_mut("op_moment").and_then(Value::as_object_mut) {
            o.insert("grondslag".into(), serde_json::json!(moment_grondslag));
        }
    }
    Ok(())
}

/// Een event van de cel: de stroom en het event.
struct Celevent<'a> {
    chronicle: &'a str,
    event: &'a crate::stroom::Event,
}

fn celevents(strommen: &[Stroom]) -> Vec<Celevent<'_>> {
    strommen
        .iter()
        .flat_map(|s| {
            s.events.iter().map(|e| Celevent {
                chronicle: &s.chronicle,
                event: e,
            })
        })
        .collect()
}

/// Of een filtersleutel met waarde een event van de cel kan raken. Alleen de
/// vaste kenmerken tellen; een veld of een input legt niets vast.
fn raakt(e: &crate::stroom::Event, filter: &Filter) -> bool {
    filter.iter().all(|(k, v)| {
        if v.starts_with('$') {
            return true;
        }
        match k.as_str() {
            "name" => e.name == *v,
            "type" => e.type_ == *v,
            "soort" => e.soort.as_deref() == Some(v),
            "stage" => e.stage.as_deref() == Some(v),
            _ => true,
        }
    })
}

/// Vertaal `uit` naar een filter van de reductie. `None`: de events staan
/// niet in deze cel. `event: <naam>` en `gevestigd_door: <artikel>` worden
/// `name`; `besluit: dit` legt niets vast (zie [`Lezing::besluit`]); de rest
/// (zoals `stage` of een veld) blijft staan.
fn filter_uit(
    uit: &Map<String, Value>,
    events: &[Celevent<'_>],
    waar: &str,
) -> Result<Option<Filter>, String> {
    let mut f = Filter::new();
    for (k, v) in uit {
        let tekst = match v {
            Value::String(s) => s.clone(),
            Value::Bool(_) | Value::Number(_) => v.to_string(),
            _ => return Err(format!("{waar}: uit.{k} is geen tekst, getal of ja/nee")),
        };
        match k.as_str() {
            "event" => {
                if !events.iter().any(|e| e.event.name == tekst) {
                    return Ok(None);
                }
                f.insert("name".into(), tekst);
            }
            "gevestigd_door" => {
                let namen: Vec<&str> = events
                    .iter()
                    .filter(|e| e.event.vestigt.contains(&tekst))
                    .map(|e| e.event.name.as_str())
                    .collect();
                match namen[..] {
                    [] => return Ok(None),
                    [n] => {
                        f.insert("name".into(), n.to_string());
                    }
                    _ => {
                        return Err(format!(
                            "{waar}: {tekst} vestigt meer dan een event in deze cel ({}); noem het event (uit.event)",
                            namen.join(", ")
                        ))
                    }
                }
            }
            "besluit" if tekst == "dit" => {}
            "besluit" => return Err(format!("{waar}: uit.besluit kent alleen 'dit'")),
            "stage" => {
                if !events
                    .iter()
                    .any(|e| e.event.stage.as_deref() == Some(&tekst))
                {
                    return Ok(None);
                }
                f.insert(k.clone(), tekst);
            }
            _ => {
                f.insert(k.clone(), tekst);
            }
        }
    }
    Ok(Some(f))
}

/// De lexostatussen die de wet in deze cel leest: een per artikel met
/// `leest` waarvan elk gelezen feit in een stroom van de cel ligt. Een artikel
/// waarvan geen feit hier ligt, hoort bij een andere cel; een artikel waarvan
/// maar een deel hier ligt, is een fout. `aanvullingen` (uit
/// `lexostatussen.yaml`) zetten extra velden bij een lexostatus uit de wet:
/// wat de cel meegeeft voor de synthese, geen parameter.
pub fn lexostatussen(
    strommen: &[Stroom],
    service: &LawExecutionService,
    aanvullingen: &[WetAanvulling],
) -> Result<Vec<LexostatusDefinitie>, Vec<String>> {
    let events = celevents(strommen);
    let heeft_leest = || {
        service.resolver().list_laws().into_iter().any(|id| {
            service.resolver().get_law(id).is_some_and(|l| {
                l.articles
                    .iter()
                    .any(|a| blok(a).is_some_and(|b| b.get("leest").is_some()))
            })
        })
    };
    if !events.iter().any(|e| e.event.zaak.heeft_kenmerk()) || !heeft_leest() {
        return match aanvullingen.first() {
            Some(a) => Err(vec![format!(
                "wet: '{}' is geen lexostatus uit de wet in deze cel",
                a.artikel
            )]),
            None => Ok(Vec::new()),
        };
    }
    let wet = artikelen(service)?;
    // Het type van een parameter die een lezing levert: uit het lezende
    // artikel, en anders uit het artikel in het corpus dat haar declareert.
    // Een lezing in beleid (notitie bron en gram-id) levert parameters van een
    // wetsartikel dat ze zelf niet declareert.
    let mut alle_typen: BTreeMap<String, String> = BTreeMap::new();
    for wa in wet.values() {
        for (n, t) in parametertypen(wa.artikel) {
            alle_typen.entry(n).or_insert(t);
        }
    }
    for id in service.resolver().list_laws() {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        for a in &law.articles {
            for (n, t) in parametertypen(a) {
                alle_typen.entry(n).or_insert(t);
            }
        }
    }
    let mut uit = Vec::new();
    let mut fouten = Vec::new();
    let mut geleverd: BTreeMap<String, String> = BTreeMap::new();
    for wa in wet.values() {
        let Some(leest) = &wa.chronolex.leest else {
            continue;
        };
        for lezing in leest.lezingen() {
            match definitie(wa, lezing, &events, aanvullingen, &alle_typen) {
                Ok(None) => {}
                Ok(Some(d)) => {
                    for p in d.reduction.afleidingen.keys() {
                        if let Some(ander) = geleverd.insert(p.clone(), d.name.clone()) {
                            fouten.push(format!(
                                "parameter '{p}' komt uit twee lexostatussen uit de wet: {ander} en {}",
                                d.name
                            ));
                        }
                    }
                    if uit.iter().any(|u: &LexostatusDefinitie| u.name == d.name) {
                        fouten.push(format!(
                            "{}: twee lezingen met dezelfde naam; geef elke lezing een eigen lid",
                            d.name
                        ));
                    }
                    uit.push(d);
                }
                Err(f) => fouten.extend(f),
            }
        }
    }
    for a in aanvullingen {
        if !uit.iter().any(|d| d.name == a.artikel) {
            fouten.push(format!(
                "wet: '{}' is geen lexostatus uit de wet in deze cel",
                a.artikel
            ));
        }
    }
    if fouten.is_empty() {
        Ok(uit)
    } else {
        Err(fouten)
    }
}

fn definitie(
    wa: &Wetartikel<'_>,
    lezing: &Lezing,
    events: &[Celevent<'_>],
    aanvullingen: &[WetAanvulling],
    alle_typen: &BTreeMap<String, String>,
) -> Result<Option<LexostatusDefinitie>, Vec<String>> {
    // De naam: het artikel, of het lid als de lezing er een noemt.
    let lexonaam = match &lezing.lid {
        None => wa.verwijzing.clone(),
        Some(Value::String(l)) => format!("{} lid {l}", wa.verwijzing),
        Some(l) => format!("{} lid {l}", wa.verwijzing),
    };
    let waar = format!("{lexonaam} (leest)");
    let mut fouten = Vec::new();
    let mut typen = parametertypen(wa.artikel);
    for p in lezing.parameters.keys() {
        if let (false, Some(t)) = (typen.contains_key(p), alle_typen.get(p)) {
            typen.insert(p.clone(), t.clone());
        }
    }
    // Per filter: waar het in de cel landt. Geen: niet deze cel.
    let mut hier = 0usize;
    let mut elders = 0usize;
    let mut top = Filter::new();
    top.insert("wortel".into(), "$wortel".into());
    if let Some(u) = &lezing.uit {
        match filter_uit(u, events, &waar) {
            Ok(Some(f)) => {
                hier += 1;
                top.extend(f);
            }
            Ok(None) => elders += 1,
            Err(f) => fouten.push(f),
        }
    }
    let mut afleidingen = BTreeMap::new();
    for (naam, v) in &lezing.parameters {
        let waar = format!("{waar}, parameter '{naam}'");
        if !typen.contains_key(naam) {
            fouten.push(format!("{waar}: geen parameter van dit artikel"));
        }
        let Some(o) = v.as_object() else {
            fouten.push(format!("{waar}: een afleiding is een object"));
            continue;
        };
        let mut o = o.clone();
        let mut eigen: Option<Filter> = None;
        if let Some(u) = o.remove("uit") {
            let Some(u) = u.as_object() else {
                fouten.push(format!("{waar}: uit is een object"));
                continue;
            };
            match filter_uit(u, events, &waar) {
                Ok(Some(f)) => {
                    hier += 1;
                    eigen = Some(f);
                }
                Ok(None) => {
                    elders += 1;
                    continue;
                }
                Err(f) => {
                    fouten.push(f);
                    continue;
                }
            }
        }
        // De naamsbrug: leest de afleiding een veld dat de gelezen events
        // onder een andere naam dragen, dan dat veld.
        let gelezen = eigen.as_ref().unwrap_or(&top);
        if let Some(Value::String(veld)) = o.get("veld").cloned() {
            let doelen: BTreeSet<Option<&String>> = events
                .iter()
                .filter(|e| raakt(e.event, gelezen))
                .map(|e| e.event.als.get(&veld))
                .collect();
            if let [Some(ander)] = doelen.into_iter().collect::<Vec<_>>()[..] {
                o.insert("veld".into(), Value::String(ander.clone()));
            }
        }
        if let Some(f) = eigen {
            let mut filter = Map::new();
            for (k, w) in f {
                filter.insert(k, Value::String(w));
            }
            o.insert("filter".into(), Value::Object(filter));
        }
        if !o.contains_key("grondslag") {
            o.insert("grondslag".into(), serde_json::json!([lexonaam]));
        }
        match serde_json::from_value::<Afgeleid>(Value::Object(o)) {
            Ok(a) => {
                afleidingen.insert(naam.clone(), a);
            }
            Err(e) => fouten.push(format!("{waar}: geen afleiding: {e}")),
        }
    }
    if hier == 0 {
        // Geen feit van dit artikel in deze cel: het hoort bij een andere.
        return if fouten.is_empty() {
            Ok(None)
        } else {
            Err(fouten)
        };
    }
    if elders > 0 {
        fouten.push(format!(
            "{waar}: een deel van de gelezen feiten ligt in deze cel, een deel niet; een lexostatus reduceert een cel"
        ));
    }
    // De kroniek: die van de events die de filters raken.
    let mut kronieken: BTreeSet<&str> = BTreeSet::new();
    let filters: Vec<Filter> = std::iter::once(top.clone())
        .chain(
            afleidingen
                .values()
                .filter_map(|a: &Afgeleid| a.filter().cloned()),
        )
        .collect();
    for f in &filters {
        let mut volledig = top.clone();
        volledig.extend(f.clone());
        for e in events.iter().filter(|e| raakt(e.event, &volledig)) {
            kronieken.insert(e.chronicle);
        }
    }
    let kroniek = match kronieken.into_iter().collect::<Vec<_>>()[..] {
        [k] => k.to_string(),
        [] => {
            fouten.push(format!(
                "{waar}: geen event in deze cel past bij wat het artikel leest"
            ));
            String::new()
        }
        ref meer => {
            fouten.push(format!(
                "{waar}: de gelezen events staan in meer dan een kroniek ({})",
                meer.join(", ")
            ));
            String::new()
        }
    };
    if lezing.besluit.as_deref().is_some_and(|b| b != "dit") {
        fouten.push(format!("{waar}: besluit kent alleen 'dit'"));
    }
    if !fouten.is_empty() {
        return Err(fouten);
    }
    let extra_velden = aanvullingen
        .iter()
        .filter(|a| a.artikel == lexonaam)
        .flat_map(|a| a.extra_velden.clone())
        .collect();
    Ok(Some(LexostatusDefinitie {
        name: lexonaam.clone(),
        inputs: vec![InputDefinitie {
            name: "wortel".into(),
            soort: "string".into(),
        }],
        reduction: Reductie {
            kroniek,
            filter: top,
            groepeer: None,
            zonder: Filter::new(),
            kies: lezing.kies,
            afleidingen,
            extra_velden,
        },
        wet: Some(Wetlezing {
            artikel: lexonaam,
            typen,
            besluit_dit: lezing.besluit.as_deref() == Some("dit"),
        }),
    }))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::{testgram, Gram};
    use crate::lexostatus_engine::{self, CelRoute, Wijze};
    use crate::reductie::{self, Afleiding, Peil};
    use serde_json::json;
    use std::sync::Arc;

    /// Een fictieve wet: art. 1 besluit (een decretogram met zijn uitkomst
    /// als veld, en een naamsbrug), art. 2 vestigt de betaling en leest het
    /// vastgestelde bedrag en de som van de betalingen.
    const WET: &str = r#"
$id: testwet_lezing
regulatory_layer: WET
publication_date: '2025-01-01'
valid_from: '2025-01-01'
url: https://example.com/testwet_lezing
articles:
  - number: '1'
    text: De instantie besluit op de aanvraag.
    url: https://example.com/testwet_lezing/1
    machine_readable:
      execution:
        produces:
          legal_character: BESCHIKKING
          decision_type: TOEKENNING
          extensions:
            chronolex:
              vestigt:
                - event: besloten
                  type: decretogram
                  stage: BESLUIT
                  op_moment: {parameter: besluitdatum, grondslag: ['testwet_lezing#1']}
                  velden: uitkomsten
                  als: {vastgesteld_bedrag: bedrag_art1}
        parameters:
          - {name: besluitdatum, type: date}
        output:
          - {name: bedrag_art1, type: number}
        actions:
          - {output: bedrag_art1, value: 100}
  - number: '2'
    text: Het bedrag wordt overeenkomstig het besluit betaald.
    url: https://example.com/testwet_lezing/2
    machine_readable:
      execution:
        produces:
          legal_character: TOETS
          decision_type: GEEN_BESLUIT
          extensions:
            chronolex:
              vestigt:
                - event: betaald
                  type: executogram
                  soort: betaling
                  verwijst: {besluit: {naar: {stage: BESLUIT}, verplicht: true}}
                  velden: [bedrag]
              leest:
                besluit: dit
                parameters:
                  vastgesteld_bedrag: {uit: {stage: BESLUIT}, kies: laatste, veld: vastgesteld_bedrag}
                  betaald_bedrag: {uit: {gevestigd_door: 'testwet_lezing#2'}, som: bedrag}
        parameters:
          - {name: vastgesteld_bedrag, type: number}
          - {name: betaald_bedrag, type: number}
        output:
          - {name: nog_te_betalen, type: number}
        actions:
          - output: nog_te_betalen
            value: {operation: SUBTRACT, values: [$vastgesteld_bedrag, $betaald_bedrag]}
"#;

    fn stroom(velden_besluit: &str) -> Stroom {
        let tekst = format!(
            r#"
$id: test_verloop
recording_actor: test_instantie
chronicle: test_kroniek
events:
  - name: besloten
    vestigt: ['testwet_lezing#1']
    intake: behandelaar
    op_moment: {{bron: $external.besluitdatum}}
    fields: {{{velden_besluit}}}
  - name: betaald
    vestigt: ['testwet_lezing#2']
    intake: behandelaar
    fields: {{bedrag: $external.bedrag}}
"#
        );
        crate::stroom::parse(&tekst, "test").unwrap()
    }

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(WET).unwrap();
        s
    }

    fn gram(naam: &str, stage: Option<&str>, fields: Value, moment: &str) -> Gram {
        let mut g = testgram("Z1");
        g.name = naam.into();
        g.stage = stage.map(str::to_string);
        g.chronicle = "test_kroniek".into();
        g.fields = fields.as_object().cloned().unwrap();
        g.op_moment = moment.into();
        g.vastgelegd_op = moment.into();
        g
    }

    #[test]
    fn de_wet_vult_het_event_in() {
        let s = service();
        let mut strommen = vec![stroom("bedrag_art1: $external.bedrag_art1")];
        assert_eq!(vestig(&mut strommen, &s), Vec::<String>::new());
        crate::stroom::leid_rollen_af(&mut strommen);
        let e = &strommen[0].events[0];
        assert_eq!(e.type_, "decretogram");
        assert_eq!(e.stage.as_deref(), Some("BESLUIT"));
        assert_eq!(e.besluit, Some(crate::stroom::Besluit::Opent));
        assert_eq!(
            strommen[0].events[1].verwijst["besluit"].naar,
            crate::stroom::Naar::Stage("BESLUIT".into())
        );
        assert_eq!(
            strommen[0].events[1].besluit,
            Some(crate::stroom::Besluit::Volgt)
        );
        assert_eq!(e.grondslag, ["testwet_lezing#1"]);
        assert_eq!(
            e.op_moment.as_ref().unwrap().grondslag,
            ["testwet_lezing#1"]
        );
        assert_eq!(e.als["vastgesteld_bedrag"], "bedrag_art1");
        // Het document van de stroom toont het event zoals het geldt.
        assert_eq!(strommen[0].document["events"][0]["type"], "decretogram");
    }

    #[test]
    fn een_veld_dat_de_wet_niet_noemt_is_een_fout() {
        let s = service();
        let mut strommen = vec![stroom(
            "bedrag_art1: $external.bedrag_art1, notitie: $external.notitie",
        )];
        let f = vestig(&mut strommen, &s);
        assert!(f.iter().any(|f| f.contains("veld 'notitie'")), "{f:?}");
        // En andersom: een uitkomst die de stroom niet bindt.
        let mut strommen = vec![stroom("notitie: $external.notitie")];
        let f = vestig(&mut strommen, &s);
        assert!(f.iter().any(|f| f.contains("'bedrag_art1'")), "{f:?}");
    }

    #[test]
    fn het_lezende_artikel_wordt_een_lexostatus_die_de_engine_ook_zo_leest() {
        let s = service();
        let mut strommen = vec![stroom("bedrag_art1: $external.bedrag_art1")];
        assert!(vestig(&mut strommen, &s).is_empty());
        crate::stroom::leid_rollen_af(&mut strommen);
        let defs = lexostatussen(&strommen, &s, &[]).unwrap();
        assert_eq!(defs.len(), 1);
        let d = &defs[0];
        assert_eq!(d.name, "testwet_lezing#2");
        assert_eq!(d.reduction.kroniek, "test_kroniek");
        // De naamsbrug van art. 1: art. 2 leest zijn eigen naam, het gram
        // draagt die van art. 1.
        match &d.reduction.afleidingen["vastgesteld_bedrag"].afleiding {
            Afleiding::LaatsteVeld { veld, filter, .. } => {
                assert_eq!(veld, "bedrag_art1");
                assert_eq!(filter["stage"], "BESLUIT");
            }
            a => panic!("{a:?}"),
        }
        match &d.reduction.afleidingen["betaald_bedrag"].afleiding {
            Afleiding::Som { filter, .. } => assert_eq!(filter["name"], "betaald"),
            a => panic!("{a:?}"),
        }
        assert!(d.wet.as_ref().unwrap().besluit_dit);
        // Zonder eigen grondslag rust een afleiding op het lezende artikel.
        assert_eq!(
            d.reduction.afleidingen["betaald_bedrag"].grondslag,
            ["testwet_lezing#2"]
        );

        let grammen = vec![
            gram(
                "besloten",
                Some("BESLUIT"),
                json!({"bedrag_art1": 100}),
                "2025-03-01T10:00:00+01:00",
            ),
            gram(
                "betaald",
                None,
                json!({"bedrag": 30}),
                "2025-03-02T10:00:00+01:00",
            ),
            gram(
                "betaald",
                None,
                json!({"bedrag": 20}),
                "2025-03-03T10:00:00+01:00",
            ),
        ];
        let inputs = json!({"wortel": "Z1"}).as_object().cloned().unwrap();
        let l = reductie::reduceer(d, &inputs, &grammen).unwrap().unwrap();
        assert_eq!(l.parameters["vastgesteld_bedrag"], json!(100));
        assert_eq!(l.parameters["betaald_bedrag"], json!(50));

        // De engine-route: de regeling uit de lezing, vergeleken met de DSL
        // (een verschil is een fout).
        let mut lexo = LawExecutionService::new();
        let tekst =
            crate::engine_regeling::regeling(d, "lexostatus_test", &BTreeMap::new()).unwrap();
        let id = lexo.load_law(&tekst).unwrap();
        let route = CelRoute {
            service: Arc::new(lexo),
            wijzen: BTreeMap::from([(
                d.name.clone(),
                Wijze::Engine {
                    regeling: id,
                    artikel: Some(d.name.clone()),
                },
            )]),
            vergelijk: true,
        };
        for g in [&grammen[..1], &grammen[..], &[]] {
            let r = lexostatus_engine::reduceer_lexostatus(
                &route,
                d,
                &inputs,
                g,
                &Peil::default(),
                "2025-06-01",
                false,
            );
            let l = r.unwrap().unwrap();
            assert_eq!(
                l.reductie.unwrap().regeling.as_deref(),
                Some("testwet_lezing#2")
            );
        }
    }

    #[test]
    fn een_lezing_per_lid_is_een_eigen_lexostatus() {
        let wet = WET.replace(
            "              leest:\n                besluit: dit\n                parameters:\n                  vastgesteld_bedrag: {uit: {stage: BESLUIT}, kies: laatste, veld: vastgesteld_bedrag}\n                  betaald_bedrag: {uit: {gevestigd_door: 'testwet_lezing#2'}, som: bedrag}\n",
            "              leest:\n                - parameters:\n                    vastgesteld_bedrag: {uit: {stage: BESLUIT}, kies: laatste, veld: vastgesteld_bedrag}\n                - lid: 2\n                  parameters:\n                    betaald_bedrag: {uit: {gevestigd_door: 'testwet_lezing#2'}, som: bedrag}\n",
        );
        assert_ne!(wet, WET, "de vervanging raakte niets");
        let mut s = LawExecutionService::new();
        s.load_law(&wet).unwrap();
        let mut strommen = vec![stroom("bedrag_art1: $external.bedrag_art1")];
        assert!(vestig(&mut strommen, &s).is_empty());
        crate::stroom::leid_rollen_af(&mut strommen);
        let namen: Vec<String> = lexostatussen(&strommen, &s, &[])
            .unwrap()
            .into_iter()
            .map(|d| d.name)
            .collect();
        assert_eq!(namen, ["testwet_lezing#2", "testwet_lezing#2 lid 2"]);
    }

    #[test]
    fn een_aanvulling_op_een_onbekend_artikel_is_een_fout() {
        let s = service();
        let mut strommen = vec![stroom("bedrag_art1: $external.bedrag_art1")];
        assert!(vestig(&mut strommen, &s).is_empty());
        crate::stroom::leid_rollen_af(&mut strommen);
        let a: WetAanvulling = serde_json::from_value(json!({
            "artikel": "testwet_lezing#1",
            "extra_velden": {"x": {"veld": "bedrag_art1"}}
        }))
        .unwrap();
        let f = lexostatussen(&strommen, &s, &[a]).unwrap_err();
        assert!(f[0].contains("testwet_lezing#1"), "{f:?}");
    }
}
