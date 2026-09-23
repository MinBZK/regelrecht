//! Synthese: de toets van een cel voegt haar eigen lexostatus samen met
//! lexostatussen van andere cellen.
//!
//! Synthese gebeurt bij de afnemer, niet bij de bron. De bron reduceert haar
//! eigen kroniek; de afnemer vraagt die lexostatus op via een [`Transport`],
//! met een tijdslimiet van drie seconden, en neemt alleen de parameters over
//! die zij in `cel.yaml` expliciet van die bron verwacht. Per parameter houdt
//! ze bij waar hij vandaan kwam. Niets hiervan wordt vastgelegd: het is
//! informeren, geen feit van de afnemer. Is een bron onbereikbaar, dan wordt
//! er niets aangevuld.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::cel::Cel;
use crate::config::SyntheseBron;
use crate::reductie::Lexostatus;
use crate::regelingen;
use crate::transport::{haal_binnen, Transport, TransportFout};

/// Hoe lang een bron mag doen over een antwoord.
pub const TIJDSLIMIET: Duration = Duration::from_secs(3);

/// Een synthese-bron met het transport dat de runtime ervoor koos.
#[derive(Clone)]
pub struct Bron {
    pub definitie: SyntheseBron,
    pub transport: Arc<dyn Transport>,
}

/// Waar een parameter vandaan kwam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "bron", rename_all = "snake_case")]
pub enum Herkomst {
    /// De eigen reductie.
    Eigen { lexostatus: String },
    /// Een lexostatus van een andere cel.
    Cel {
        cel: String,
        lexostatus: String,
        transport: String,
    },
    /// Samengesteld per regel uit een tabelveld van een eigen lexostatus en
    /// de bronnen die per regel zijn bevraagd (zie [`crate::rijen`]).
    PerRegel { lexostatus: String, veld: String },
    /// Het besluitformulier: een oordeel van de behandelaar.
    Behandelaar,
    /// De stand bij besluit: een feit dat pas na het besluit ontstaat.
    StandBijBesluit,
}

/// Hoe de vraag aan een bron verliep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Bevraagd,
    /// Geen verbinding of geen antwoord binnen de tijdslimiet.
    Onbereikbaar,
    /// Wel een antwoord, maar geen lexostatus.
    Fout,
    /// Niet gevraagd: een invoer ontbrak in de eigen lexostatus.
    NietBevraagd,
}

/// De uitslag per bron.
#[derive(Debug, Clone, Serialize)]
pub struct BronUitslag {
    pub cel: String,
    pub lexostatus: String,
    pub transport: &'static str,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fout: Option<String>,
    pub invoer: Map<String, Value>,
    /// De verwachte parameters die de bron niet leverde.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub niet_geleverd: Vec<String>,
    /// De extra velden die deze bron doorgaf aan een latere bron.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub extra_velden: Map<String, Value>,
}

/// De samengevoegde parameters, met hun herkomst.
#[derive(Debug, Clone, Serialize)]
pub struct Samenvoeging {
    pub parameters: BTreeMap<String, Value>,
    pub herkomst: BTreeMap<String, Herkomst>,
    pub bronnen: Vec<BronUitslag>,
}

impl Samenvoeging {
    /// De parameters die een bron had moeten leveren en niet leverde: alle
    /// parameters van een bron die niet antwoordde, en wat een bron die wel
    /// antwoordde niet had. Zo'n feit ontbreekt niet omdat de aanvrager het nog
    /// moet vertellen, maar omdat de bron het niet gaf.
    pub fn niet_van_bronnen(&self, bronnen: &[Bron]) -> BTreeSet<String> {
        let mut uit = BTreeSet::new();
        for u in &self.bronnen {
            uit.extend(u.niet_geleverd.iter().cloned());
            if u.status != Status::Bevraagd {
                let def = bronnen
                    .iter()
                    .map(|b| &b.definitie)
                    .find(|d| d.cel == u.cel && d.lexostatus == u.lexostatus);
                if let Some(d) = def {
                    uit.extend(d.parameters.iter().cloned());
                }
            }
        }
        uit
    }

    /// Waarom de toets niet te beoordelen is als een bron niets leverde, in
    /// woorden; `None` als elke bron antwoordde.
    pub fn reden(&self) -> Option<String> {
        self.bronnen.iter().find_map(|b| {
            let fout = b.fout.as_deref().unwrap_or_default();
            match b.status {
                Status::Bevraagd => None,
                Status::Onbereikbaar => {
                    Some(format!("niet te beoordelen: bron {} onbereikbaar", b.cel))
                }
                Status::Fout => Some(format!(
                    "niet te beoordelen: bron {} gaf geen lexostatus ({fout})",
                    b.cel
                )),
                Status::NietBevraagd => Some(format!(
                    "niet te beoordelen: bron {} niet bevraagd ({fout})",
                    b.cel
                )),
            }
        })
    }
}

/// Het pad van een lexostatus op een runtime, met de invoer als query.
pub fn pad(cel: &str, lexostatus: &str, invoer: &Map<String, Value>) -> String {
    let paren: BTreeMap<&str, String> = invoer
        .iter()
        .map(|(k, v)| {
            let tekst = match v {
                Value::String(s) => s.clone(),
                ander => ander.to_string(),
            };
            (k.as_str(), tekst)
        })
        .collect();
    let query = serde_urlencoded::to_string(&paren).unwrap_or_default();
    if query.is_empty() {
        format!("/cellen/{cel}/api/lexostatus/{lexostatus}")
    } else {
        format!("/cellen/{cel}/api/lexostatus/{lexostatus}?{query}")
    }
}

/// De invoer voor een bron: uit de eigen lexostatus (een parameter of een
/// extra veld), of uit de extra velden die een eerdere bron doorgaf
/// (`eerder`: lexostatus van die bron naar haar extra velden). Een fout noemt
/// wat ontbreekt.
fn invoer(
    bron: &SyntheseBron,
    eigen: &Lexostatus,
    eerder: &BTreeMap<String, Map<String, Value>>,
) -> Result<Map<String, Value>, String> {
    let mut uit = Map::new();
    for (naam, v) in &bron.invoer {
        let waarde = if v.lexostatus == eigen.naam {
            eigen
                .parameters
                .get(&v.veld)
                .or_else(|| eigen.extra_velden.get(&v.veld))
        } else {
            eerder.get(&v.lexostatus).and_then(|m| m.get(&v.veld))
        }
        .filter(|w| !w.is_null());
        match waarde {
            Some(w) => {
                uit.insert(naam.clone(), w.clone());
            }
            None => {
                return Err(format!(
                    "invoer '{naam}' ontbreekt: {}.{} heeft geen waarde",
                    v.lexostatus, v.veld
                ))
            }
        }
    }
    Ok(uit)
}

/// Of een bron wacht op een eerdere bron: een invoer komt niet uit de eigen
/// lexostatus.
fn wacht_op_eerdere(bron: &SyntheseBron, eigen: &Lexostatus) -> bool {
    bron.invoer.values().any(|v| v.lexostatus != eigen.naam)
}

/// Voeg de eigen lexostatus samen met die van de bronnen. Eerst worden de
/// bronnen tegelijk bevraagd die alleen de eigen lexostatus nodig hebben,
/// daarna de bronnen die een extra veld van een eerdere bron als invoer
/// nemen (bijvoorbeeld een naam bij een registratienummer).
pub async fn voeg_samen(eigen: &Lexostatus, bronnen: &[Bron]) -> Samenvoeging {
    let (eerst, daarna): (Vec<&Bron>, Vec<&Bron>) = bronnen
        .iter()
        .partition(|b| !wacht_op_eerdere(&b.definitie, eigen));
    let mut eerder = BTreeMap::new();
    let mut s = vraag(eigen, &eerst, &eerder).await;
    for (uitslag, bron) in s.bronnen.iter().zip(&eerst) {
        if !bron.definitie.extra_velden.is_empty() {
            eerder.insert(uitslag.lexostatus.clone(), uitslag.extra_velden.clone());
        }
    }
    let t = vraag(eigen, &daarna, &eerder).await;
    for (k, v) in t.parameters {
        s.parameters.entry(k).or_insert(v);
    }
    s.herkomst.extend(
        t.herkomst
            .into_iter()
            .filter(|(k, _)| !eigen.parameters.contains_key(k)),
    );
    s.bronnen.extend(t.bronnen);
    s
}

/// Bevraag deze bronnen tegelijk en voeg hun parameters samen met de eigen
/// lexostatus.
async fn vraag(
    eigen: &Lexostatus,
    bronnen: &[&Bron],
    eerder: &BTreeMap<String, Map<String, Value>>,
) -> Samenvoeging {
    let mut parameters = eigen.parameters.clone();
    let mut herkomst: BTreeMap<String, Herkomst> = parameters
        .keys()
        .map(|k| {
            (
                k.clone(),
                Herkomst::Eigen {
                    lexostatus: eigen.naam.clone(),
                },
            )
        })
        .collect();

    let vragen = bronnen.iter().map(|bron| async move {
        let d = &bron.definitie;
        let mut uitslag = BronUitslag {
            cel: d.cel.clone(),
            lexostatus: d.lexostatus.clone(),
            transport: bron.transport.soort(),
            status: Status::NietBevraagd,
            fout: None,
            invoer: Map::new(),
            niet_geleverd: Vec::new(),
            extra_velden: Map::new(),
        };
        let invoer = match invoer(d, eigen, eerder) {
            Ok(i) => i,
            Err(f) => {
                uitslag.fout = Some(f);
                return (uitslag, None);
            }
        };
        uitslag.invoer = invoer.clone();
        let antwoord = haal_binnen(
            bron.transport.as_ref(),
            &pad(&d.cel, &d.lexostatus, &invoer),
            TIJDSLIMIET,
        )
        .await;
        match antwoord {
            Ok(v) => {
                uitslag.status = Status::Bevraagd;
                if let Some(extra) = v.get("extra_velden").and_then(Value::as_object) {
                    for veld in &d.extra_velden {
                        if let Some(w) = extra.get(veld) {
                            uitslag.extra_velden.insert(veld.clone(), w.clone());
                        }
                    }
                }
                (
                    uitslag,
                    v.get("parameters").and_then(Value::as_object).cloned(),
                )
            }
            Err(TransportFout::Onbereikbaar(r)) => {
                uitslag.status = Status::Onbereikbaar;
                uitslag.fout = Some(r);
                (uitslag, None)
            }
            Err(f @ TransportFout::Antwoord { .. }) => {
                uitslag.status = Status::Fout;
                uitslag.fout = Some(f.to_string());
                (uitslag, None)
            }
        }
    });
    let antwoorden = futures_util::future::join_all(vragen).await;

    let mut uitslagen = Vec::new();
    for ((mut uitslag, geleverd), bron) in antwoorden.into_iter().zip(bronnen.iter()) {
        if let Some(geleverd) = geleverd {
            for p in &bron.definitie.parameters {
                match geleverd.get(p) {
                    Some(w) => {
                        parameters.insert(p.clone(), w.clone());
                        herkomst.insert(
                            p.clone(),
                            Herkomst::Cel {
                                cel: uitslag.cel.clone(),
                                lexostatus: uitslag.lexostatus.clone(),
                                transport: uitslag.transport.to_string(),
                            },
                        );
                    }
                    None => uitslag.niet_geleverd.push(p.clone()),
                }
            }
        }
        uitslagen.push(uitslag);
    }
    Samenvoeging {
        parameters,
        herkomst,
        bronnen: uitslagen,
    }
}

/// De controles op bronnen die een extra veld doorgeven, met en zonder
/// portaal:
///
/// - een bron levert een parameter of geeft een extra veld door;
/// - een doorgevende bron heet anders dan elke eigen lexostatus en dan elke
///   andere doorgevende bron, zodat een invoer maar een ding kan aanwijzen;
/// - een invoer uit een doorgevende bron komt van een eerdere bron in de lijst,
///   die dat veld doorgeeft en zelf niet op een andere bron wacht (de synthese
///   kent twee rondes).
fn doorgeven(cel: &Cel) -> Vec<String> {
    let mut fouten = Vec::new();
    let bronnen = &cel.definitie.synthese;
    let eigen: Vec<&str> = cel
        .lexostatussen
        .lexostatus_definitions
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    for (i, bron) in bronnen.iter().enumerate() {
        let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
        if bron.parameters.is_empty() && bron.extra_velden.is_empty() {
            fouten.push(format!(
                "{wie}: levert geen parameter en geeft geen extra veld door"
            ));
        }
        if !bron.extra_velden.is_empty() {
            if eigen.contains(&bron.lexostatus.as_str()) {
                fouten.push(format!(
                    "{wie}: geeft extra velden door, maar heet als een eigen lexostatus; een invoer zou dan twee dingen kunnen aanwijzen"
                ));
            }
            if bronnen[..i]
                .iter()
                .any(|b| !b.extra_velden.is_empty() && b.lexostatus == bron.lexostatus)
            {
                fouten.push(format!(
                    "{wie}: een andere bron die extra velden doorgeeft heet ook zo"
                ));
            }
        }
        for (naam, v) in &bron.invoer {
            let doorgever = bronnen
                .iter()
                .enumerate()
                .find(|(_, b)| !b.extra_velden.is_empty() && b.lexostatus == v.lexostatus);
            let Some((j, e)) = doorgever else { continue };
            if j >= i {
                fouten.push(format!(
                    "{wie}, invoer '{naam}': bron {}/{} staat niet eerder in de lijst",
                    e.cel, e.lexostatus
                ));
            } else if !e.extra_velden.contains(&v.veld) {
                fouten.push(format!(
                    "{wie}, invoer '{naam}': bron {}/{} geeft geen extra veld '{}' door",
                    e.cel, e.lexostatus, v.veld
                ));
            } else if e
                .invoer
                .values()
                .any(|w| !eigen.contains(&w.lexostatus.as_str()))
            {
                fouten.push(format!(
                    "{wie}, invoer '{naam}': bron {}/{} wacht zelf op een eerdere bron; de synthese kent twee rondes",
                    e.cel, e.lexostatus
                ));
            }
        }
    }
    fouten
}

/// De controles op de synthese van een cel bij het opstarten. Een fout hier
/// houdt de cel tegen:
///
/// - synthese vraagt een portaal of een besluit, want alleen de toets en het
///   proefbesluit gebruiken haar;
/// - een bron is een andere cel;
/// - elke invoer komt uit een veld van de toets-lexostatus (met een portaal),
///   of van een eerdere bron die haar doorgeeft (zie [`doorgeven`]);
/// - elke parameter is een parameter van het artikel van de toets of van het
///   besluit, of van een artikel dat een van beide transitief aanroept;
/// - een parameter komt uit maar een bron: de eigen reductie of een bron.
///
/// Wat het besluit verder vraagt, staat in [`crate::besluit::controleer`].
pub fn controleer(cel: &Cel) -> Vec<String> {
    let mut fouten = Vec::new();
    let bronnen = &cel.definitie.synthese;
    if bronnen.is_empty() {
        return fouten;
    }
    fouten.extend(doorgeven(cel));
    let doorgevers: Vec<&str> = bronnen
        .iter()
        .filter(|b| !b.extra_velden.is_empty())
        .map(|b| b.lexostatus.as_str())
        .collect();
    let besluit = cel.definitie.behandeling.as_ref().map(|b| &b.besluit);
    let onder_besluit = besluit
        .and_then(|b| {
            let u = b.uitkomsten.first()?;
            let a = cel
                .service
                .resolver()
                .get_article_by_output(&b.regeling, u, None)?;
            Some(regelingen::transitieve_parameters(
                &cel.service,
                &b.regeling,
                a,
            ))
        })
        .unwrap_or_default();
    let Some(portaal) = cel.portaal() else {
        if besluit.is_none() {
            fouten.push(
                "synthese zonder portaal en zonder besluit: alleen de toets van een portaal en het proefbesluit gebruiken haar"
                    .into(),
            );
            return fouten;
        }
        for bron in bronnen {
            let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
            if bron.cel == cel.id() {
                fouten.push(format!(
                    "{wie}: een bron is een andere cel, niet de cel zelf"
                ));
            }
            for p in bron
                .parameters
                .iter()
                .filter(|p| !onder_besluit.contains(*p))
            {
                fouten.push(format!(
                    "{wie}: '{p}' is geen parameter van het besluit of van een artikel dat het aanroept"
                ));
            }
        }
        return fouten;
    };
    let eigen = cel.lexostatussen.lexostatus(&portaal.toets.lexostatus);
    let onder_toets = cel
        .service
        .resolver()
        .get_article_by_output(&portaal.toets.regeling, &portaal.toets.uitkomst, None)
        .map(|a| regelingen::transitieve_parameters(&cel.service, &portaal.toets.regeling, a))
        .unwrap_or_default();
    // parameter -> bronnen die hem leveren
    let mut per: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    if let Some(def) = eigen {
        for p in def.reduction.afleidingen.keys() {
            per.entry(p)
                .or_default()
                .push(format!("de eigen lexostatus '{}'", def.name));
        }
    }
    for bron in bronnen {
        let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
        if bron.cel == cel.id() {
            fouten.push(format!(
                "{wie}: een bron is een andere cel, niet de cel zelf"
            ));
        }
        for (naam, v) in &bron.invoer {
            if doorgevers.contains(&v.lexostatus.as_str()) {
                // Doorgegeven door een eerdere bron: zie `doorgeven`.
            } else if v.lexostatus != portaal.toets.lexostatus {
                fouten.push(format!(
                    "{wie}, invoer '{naam}': komt uit lexostatus '{}', maar de toets reduceert '{}'",
                    v.lexostatus, portaal.toets.lexostatus
                ));
            } else if !eigen.is_some_and(|d| d.levert(&v.veld)) {
                fouten.push(format!(
                    "{wie}, invoer '{naam}': lexostatus '{}' levert geen '{}' (geen afleiding en geen extra veld)",
                    v.lexostatus, v.veld
                ));
            }
        }
        for p in &bron.parameters {
            if !onder_toets.contains(p) && !onder_besluit.contains(p) {
                fouten.push(format!(
                    "{wie}: '{p}' is geen parameter van {}#{} ('{}') of van een artikel dat het aanroept",
                    portaal.toets.regeling,
                    cel.service
                        .resolver()
                        .get_article_by_output(&portaal.toets.regeling, &portaal.toets.uitkomst, None)
                        .map(|a| a.number.clone())
                        .unwrap_or_default(),
                    portaal.toets.uitkomst
                ));
            }
            per.entry(p).or_default().push(wie.clone());
        }
    }
    for (p, wie) in per {
        if wie.len() > 1 {
            fouten.push(format!(
                "parameter '{p}' komt uit meer dan een bron: {}",
                wie.join(", ")
            ));
        }
    }
    fouten
}

/// Wat een runtime over haar cellen zegt (`GET /api/cellen`), voor zover de
/// controle het nodig heeft.
fn lexostatus_van<'v>(cellen: &'v Value, cel: &str, lexostatus: &str) -> Result<&'v Value, String> {
    let c = cellen
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|c| c.get("id").and_then(Value::as_str) == Some(cel))
        })
        .ok_or_else(|| format!("de runtime van de bron heeft geen cel '{cel}'"))?;
    c.get("lexostatussen")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .find(|l| l.get("name").and_then(Value::as_str) == Some(lexostatus))
        })
        .ok_or_else(|| format!("cel '{cel}' biedt geen lexostatus '{lexostatus}' aan"))
}

fn namen(v: &Value, sleutel: &str) -> BTreeSet<String> {
    v.get(sleutel)
        .and_then(Value::as_array)
        .into_iter()
        .flatten()
        .filter_map(|x| {
            x.as_str()
                .or_else(|| x.get("name").and_then(Value::as_str))
                .map(str::to_string)
        })
        .collect()
}

/// De controle op de bronnen zelf, bij het opstarten: is de bron bereikbaar,
/// biedt ze de lexostatus aan met deze parameters, en passen de inputs? Elk
/// probleem is een waarschuwing, geen weigering: de bron mag later komen.
pub async fn waarschuwingen(cel: &str, bronnen: &[Bron]) -> Vec<String> {
    let mut uit = Vec::new();
    for bron in bronnen {
        let d = &bron.definitie;
        let wie = format!(
            "cel '{cel}': synthese-bron {}/{} ({})",
            d.cel,
            d.lexostatus,
            bron.transport.soort()
        );
        let cellen = match haal_binnen(bron.transport.as_ref(), "/api/cellen", TIJDSLIMIET).await {
            Ok(c) => c,
            Err(f) => {
                uit.push(format!("{wie}: nu niet te controleren, {f}"));
                continue;
            }
        };
        let lexo = match lexostatus_van(&cellen, &d.cel, &d.lexostatus) {
            Ok(l) => l,
            Err(f) => {
                uit.push(format!("{wie}: {f}"));
                continue;
            }
        };
        if lexo.get("lijst") == Some(&Value::Bool(true)) {
            uit.push(format!(
                "{wie}: de bron is een lijst (groepeer) en levert geen parameters"
            ));
        }
        let geleverd = namen(lexo, "parameters");
        for p in d.parameters.iter().filter(|p| !geleverd.contains(*p)) {
            uit.push(format!("{wie}: de bron levert geen parameter '{p}'"));
        }
        let inputs = namen(lexo, "inputs");
        for i in inputs.iter().filter(|i| !d.invoer.contains_key(*i)) {
            uit.push(format!(
                "{wie}: de bron vraagt input '{i}', en de synthese geeft die niet"
            ));
        }
        for i in d.invoer.keys().filter(|i| !inputs.contains(*i)) {
            uit.push(format!("{wie}: invoer '{i}' is geen input van de bron"));
        }
    }
    uit
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::Antwoord;
    use serde_json::json;
    use std::sync::Mutex;

    /// Een transport dat een vast antwoord geeft en de vragen onthoudt.
    struct Vast {
        antwoord: Result<Value, TransportFout>,
        vragen: Mutex<Vec<String>>,
    }

    impl Transport for Vast {
        fn soort(&self) -> &'static str {
            "intern"
        }
        fn haal<'a>(&'a self, pad: &'a str) -> Antwoord<'a> {
            self.vragen.lock().unwrap().push(pad.to_string());
            let a = self.antwoord.clone();
            Box::pin(async move { a })
        }
    }

    fn bron(antwoord: Result<Value, TransportFout>) -> (Bron, Arc<Vast>) {
        let t = Arc::new(Vast {
            antwoord,
            vragen: Mutex::new(Vec::new()),
        });
        let definitie: SyntheseBron = serde_json::from_value(json!({
            "cel": "register", "lexostatus": "status",
            "invoer": {"aanduiding": {"lexostatus": "eigen", "veld": "aanduiding"}},
            "parameters": ["ingeschreven", "zetels"]
        }))
        .unwrap();
        (
            Bron {
                definitie,
                transport: t.clone(),
            },
            t,
        )
    }

    fn eigen(aanduiding: Option<&str>) -> Lexostatus {
        serde_json::from_value(json!({
            "naam": "eigen",
            "parameters": {"bevat_aanduiding": aanduiding.is_some()},
            "extra_velden": aanduiding.map(|a| json!({"aanduiding": a})).unwrap_or(json!({})),
        }))
        .unwrap()
    }

    fn vaste(antwoord: Value) -> Arc<Vast> {
        Arc::new(Vast {
            antwoord: Ok(antwoord),
            vragen: Mutex::new(Vec::new()),
        })
    }

    /// Een bron geeft een extra veld door aan een latere bron: eerst een
    /// nummer naar een naam, dan de naam naar een aantal. Het doorgegeven veld
    /// is geen parameter.
    #[tokio::test]
    async fn een_extra_veld_gaat_naar_de_volgende_bron() {
        let t1 = vaste(json!({"parameters": {}, "extra_velden": {"naam": "EEN"}}));
        let t2 = vaste(json!({"parameters": {"aantal": 3}}));
        let b1 = Bron {
            definitie: serde_json::from_value(json!({
                "cel": "register", "lexostatus": "op_nummer",
                "invoer": {"nummer": {"lexostatus": "eigen", "veld": "nummer"}},
                "parameters": [], "extra_velden": ["naam"]
            }))
            .unwrap(),
            transport: t1.clone(),
        };
        let b2 = Bron {
            definitie: serde_json::from_value(json!({
                "cel": "register", "lexostatus": "op_naam",
                "invoer": {"naam": {"lexostatus": "op_nummer", "veld": "naam"}},
                "parameters": ["aantal"]
            }))
            .unwrap(),
            transport: t2.clone(),
        };
        let eigen: Lexostatus = serde_json::from_value(json!({
            "naam": "eigen", "parameters": {}, "extra_velden": {"nummer": "12345678"}
        }))
        .unwrap();
        // De wachtende bron komt in de tweede ronde; de opstartcontrole eist
        // dat de doorgevende bron eerder in de lijst staat.
        let s = voeg_samen(&eigen, &[b1, b2]).await;
        assert_eq!(
            t2.vragen.lock().unwrap()[0],
            "/cellen/register/api/lexostatus/op_naam?naam=EEN"
        );
        assert_eq!(s.parameters.get("aantal"), Some(&json!(3)));
        assert!(!s.parameters.contains_key("naam"));
        assert_eq!(s.bronnen.len(), 2);
    }

    /// Geeft de eerste bron niets door, dan wordt de volgende niet bevraagd,
    /// en er wordt niets aangevuld.
    #[tokio::test]
    async fn zonder_doorgegeven_veld_geen_vraag() {
        let t1 = vaste(json!({"parameters": {}}));
        let t2 = vaste(json!({"parameters": {"aantal": 3}}));
        let b1 = Bron {
            definitie: serde_json::from_value(json!({
                "cel": "register", "lexostatus": "op_nummer",
                "invoer": {"nummer": {"lexostatus": "eigen", "veld": "nummer"}},
                "parameters": [], "extra_velden": ["naam"]
            }))
            .unwrap(),
            transport: t1,
        };
        let b2 = Bron {
            definitie: serde_json::from_value(json!({
                "cel": "register", "lexostatus": "op_naam",
                "invoer": {"naam": {"lexostatus": "op_nummer", "veld": "naam"}},
                "parameters": ["aantal"]
            }))
            .unwrap(),
            transport: t2.clone(),
        };
        let eigen: Lexostatus = serde_json::from_value(json!({
            "naam": "eigen", "parameters": {}, "extra_velden": {"nummer": "12345678"}
        }))
        .unwrap();
        let s = voeg_samen(&eigen, &[b1, b2]).await;
        assert!(t2.vragen.lock().unwrap().is_empty());
        assert!(!s.parameters.contains_key("aantal"));
        assert_eq!(s.bronnen[1].status, Status::NietBevraagd);
    }

    #[tokio::test]
    async fn samenvoegen_met_herkomst() {
        let (b, t) = bron(Ok(
            json!({"parameters": {"ingeschreven": true, "zetels": 6, "anders": 1}}),
        ));
        let s = voeg_samen(&eigen(Some("EEN & ANDER")), &[b]).await;
        assert_eq!(
            t.vragen.lock().unwrap()[0],
            "/cellen/register/api/lexostatus/status?aanduiding=EEN+%26+ANDER"
        );
        assert_eq!(s.parameters["zetels"], json!(6));
        // Alleen de verwachte parameters, geen wildcard.
        assert!(!s.parameters.contains_key("anders"));
        // De aanduiding is een extra veld en gaat niet naar de engine.
        assert!(!s.parameters.contains_key("aanduiding"));
        assert_eq!(
            s.herkomst["bevat_aanduiding"],
            Herkomst::Eigen {
                lexostatus: "eigen".into()
            }
        );
        assert_eq!(
            serde_json::to_value(&s.herkomst["ingeschreven"]).unwrap(),
            json!({"bron": "cel", "cel": "register", "lexostatus": "status", "transport": "intern"})
        );
        assert_eq!(s.bronnen[0].status, Status::Bevraagd);
        assert_eq!(s.reden(), None);
    }

    #[tokio::test]
    async fn onbereikbare_bron_vult_niets_aan() {
        let (b, _) = bron(Err(TransportFout::Onbereikbaar("weg".into())));
        let s = voeg_samen(&eigen(Some("X")), &[b]).await;
        assert_eq!(
            s.parameters.keys().collect::<Vec<_>>(),
            ["bevat_aanduiding"]
        );
        assert_eq!(s.bronnen[0].status, Status::Onbereikbaar);
        assert_eq!(
            s.reden().as_deref(),
            Some("niet te beoordelen: bron register onbereikbaar")
        );
    }

    #[tokio::test]
    async fn ontbrekende_invoer_vraagt_de_bron_niet() {
        let (b, t) = bron(Ok(json!({"parameters": {}})));
        let s = voeg_samen(&eigen(None), &[b]).await;
        assert!(t.vragen.lock().unwrap().is_empty());
        assert_eq!(s.bronnen[0].status, Status::NietBevraagd);
        assert!(s.reden().unwrap().contains("invoer 'aanduiding' ontbreekt"));
    }

    #[tokio::test]
    async fn niet_geleverde_parameter_wordt_genoemd() {
        let (b, _) = bron(Ok(json!({"parameters": {"ingeschreven": false}})));
        let s = voeg_samen(&eigen(Some("X")), &[b]).await;
        assert_eq!(s.bronnen[0].niet_geleverd, ["zetels"]);
        assert!(!s.parameters.contains_key("zetels"));
    }

    #[test]
    fn pad_zonder_invoer() {
        assert_eq!(pad("a", "b", &Map::new()), "/cellen/a/api/lexostatus/b");
        let i = json!({"jaar": 2025}).as_object().unwrap().clone();
        assert_eq!(pad("a", "b", &i), "/cellen/a/api/lexostatus/b?jaar=2025");
    }
}
