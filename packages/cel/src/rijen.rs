//! Synthese per regel: een tabelveld van een lexostatus van de zaak wordt
//! een array-parameter, met per regel kolommen uit andere cellen.
//!
//! Een artikel kan een tabel als parameter vragen waarvan de indiener maar
//! een deel invult; de rest stelt de instantie zelf vast, per regel, uit
//! registers van andere cellen. Het proces bouwt die tabel niet in code op:
//! in `proces.yaml` staat welk tabelveld de regels levert, welke kolom onder welke
//! naam meegaat, en welke bron per regel met welke invoer wordt bevraagd.
//!
//! De regels worden tegelijk bevraagd, in de volgorde van het tabelveld
//! teruggegeven. Per regel gaat het proces langs de bronnen, in volgorde. De invoer van een
//! bron komt uit de regel zelf (`kolom`), uit een lexostatus van de zaak
//! (`lexostatus` en `veld`) of uit de samengevoegde parameters
//! (`parameter`); een bron die eerder aan de beurt was kan dus een kolom
//! leveren die een latere bron als invoer gebruikt. Ontbreekt een invoer, is
//! een bron onbereikbaar, of levert ze de waarde niet, dan blijft die kolom
//! weg: er wordt niets aangevuld. Welke kolommen dat waren, staat in `mist`.
//!
//! Het blok staat onder het besluit (`behandeling.besluit.rijen`) of onder de
//! toets (`portaal.toets.rijen`); beide voeren het uit met [`pas_toe`], vóór
//! de engine.

use std::collections::{BTreeMap, BTreeSet};

use futures_util::stream::{self, StreamExt};

use serde::Serialize;
use serde_json::{Map, Value};

use crate::cel::Cel;
use crate::config::{Omzetting, ProcesDefinitie, RijBron, RijInvoer, RijenDefinitie};
use crate::datum;
use crate::reductie::Lexostatus;
use crate::synthese::{Herkomst, Samenvoeging, Status};
use crate::transport::TransportFout;

/// Een bron die per regel wordt bevraagd, met het transport dat de runtime
/// ervoor koos.
pub type Bron = crate::synthese::Bron<RijBron>;

/// Een rijen-definitie met haar bronnen.
#[derive(Clone)]
pub struct Rijen {
    pub definitie: RijenDefinitie,
    pub bronnen: Vec<Bron>,
}

/// Hoe het bevragen van een bron over alle regels verliep.
#[derive(Debug, Clone, Serialize)]
pub struct BronUitslag {
    pub cel: String,
    pub lexostatus: String,
    pub transport: &'static str,
    /// Het aantal regels waarvoor de bron is bevraagd.
    pub bevraagd: usize,
    /// De ongunstigste status over de regels.
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fout: Option<String>,
}

/// Wat een rijen-definitie opleverde.
#[derive(Debug, Clone, Serialize)]
pub struct Uitslag {
    pub parameter: String,
    /// Waarom de tabel niet is opgebouwd: het tabelveld is geen lijst van
    /// regels. Dan gaat de parameter niet naar de engine; een regel
    /// weglaten zou de uitkomst stil veranderen.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub fout: Option<String>,
    /// De regels, elk met de kolommen die een bron leverde.
    pub regels: Vec<Value>,
    pub bronnen: Vec<BronUitslag>,
    /// Kolommen die niet in elke regel staan, zonder dubbelen.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mist: Vec<String>,
}

/// De waarde die een lexostatus onder een naam levert: een parameter of een
/// extra veld.
fn uit_lexostatus<'l>(
    lexostatussen: &'l [Lexostatus],
    naam: &str,
    veld: &str,
) -> Option<&'l Value> {
    lexostatussen.iter().find(|l| l.naam == naam)?.veld(veld)
}

/// Zet een waarde om voor ze als invoer meegaat.
fn zet_om(waarde: &Value, omzetting: Omzetting) -> Option<Value> {
    match omzetting {
        Omzetting::EersteDagVanHetJaar => {
            let jaar = waarde.as_i64().or_else(|| waarde.as_str()?.parse().ok())?;
            Some(Value::String(datum::eerste_dag_van_het_jaar(jaar)))
        }
    }
}

/// De invoer voor een bron bij een regel. Een fout noemt wat ontbreekt.
fn invoer(
    bron: &RijBron,
    regel: &Map<String, Value>,
    lexostatussen: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
) -> Result<Map<String, Value>, String> {
    let mut uit = Map::new();
    for (naam, verwijzing) in &bron.invoer {
        let (waarde, wat) = match verwijzing {
            RijInvoer::Kolom { kolom, .. } => (
                regel.get(kolom).filter(|w| !w.is_null()),
                format!("kolom '{kolom}'"),
            ),
            RijInvoer::Eigen {
                lexostatus, veld, ..
            } => (
                uit_lexostatus(lexostatussen, lexostatus, veld),
                format!("lexostatus '{lexostatus}', veld '{veld}'"),
            ),
            RijInvoer::Parameter { parameter, .. } => (
                parameters.get(parameter).filter(|w| !w.is_null()),
                format!("parameter '{parameter}'"),
            ),
        };
        let Some(waarde) = waarde else {
            return Err(format!(
                "invoer '{naam}' ontbreekt: {wat} heeft geen waarde"
            ));
        };
        let waarde = match verwijzing.omzetting() {
            Some(o) => zet_om(waarde, o)
                .ok_or_else(|| format!("invoer '{naam}': {wat} is niet om te zetten ({waarde})"))?,
            None => waarde.clone(),
        };
        uit.insert(naam.clone(), waarde);
    }
    Ok(uit)
}

/// De ongunstigste van twee statussen: bevraagd is het best, daarna fout,
/// niet bevraagd en onbereikbaar.
fn slechtste(a: Status, b: Status) -> Status {
    fn rang(s: Status) -> u8 {
        match s {
            Status::Bevraagd => 0,
            Status::Fout => 1,
            Status::NietBevraagd => 2,
            Status::Onbereikbaar => 3,
        }
    }
    if rang(b) > rang(a) {
        b
    } else {
        a
    }
}

/// Hoeveel regels tegelijk hun bronnen bevragen.
const GELIJKTIJDIG: usize = 16;

/// Hoe het bevragen van een bron bij een regel verliep.
enum Bevraging {
    Bevraagd,
    /// Niet gevraagd: een invoer ontbrak.
    NietBevraagd(String),
    /// Gevraagd, zonder lexostatus.
    Mislukt(Status, String),
}

/// Wat een regel opleverde: de regel, de kolommen die ontbreken, en per bron
/// (in de volgorde van de definitie) hoe het bevragen verliep.
struct Regeluitslag {
    regel: Map<String, Value>,
    mist: BTreeSet<String>,
    bronnen: Vec<Bevraging>,
}

/// Stel een regel samen: de kolommen uit de tabel, en daarna die van elke
/// bron, na elkaar.
async fn stel_regel_samen(
    rijen: &Rijen,
    bron: &Map<String, Value>,
    lexostatussen: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
) -> Regeluitslag {
    let mut uit = Regeluitslag {
        regel: Map::new(),
        mist: BTreeSet::new(),
        bronnen: Vec::new(),
    };
    for (kolom, naam) in &rijen.definitie.kolommen {
        if let Some(w) = bron.get(kolom) {
            uit.regel.insert(naam.clone(), w.clone());
        } else {
            uit.mist.insert(naam.clone());
        }
    }
    for b in &rijen.bronnen {
        let invoer = match invoer(&b.definitie, &uit.regel, lexostatussen, parameters) {
            Ok(i) => i,
            Err(f) => {
                uit.bronnen.push(Bevraging::NietBevraagd(f));
                uit.mist.extend(b.definitie.kolommen.values().cloned());
                continue;
            }
        };
        let geleverd = match b.vraag(&invoer).await {
            Ok(l) => {
                uit.bronnen.push(Bevraging::Bevraagd);
                let mut samen = l.parameters;
                samen.extend(l.extra_velden);
                samen
            }
            Err(f) => {
                let status = match f {
                    TransportFout::Onbereikbaar(_) => Status::Onbereikbaar,
                    TransportFout::Antwoord { .. } | TransportFout::Json(_) => Status::Fout,
                };
                uit.bronnen.push(Bevraging::Mislukt(status, f.to_string()));
                BTreeMap::new()
            }
        };
        for (van, naar) in &b.definitie.kolommen {
            match geleverd.get(van).filter(|w| !w.is_null()) {
                Some(w) => {
                    uit.regel.insert(naar.clone(), w.clone());
                }
                None => {
                    uit.mist.insert(naar.clone());
                }
            }
        }
    }
    uit
}

/// Bouw de array-parameter op. Voor elke regel van het tabelveld gaan de
/// kolommen uit de configuratie mee, en daarna de kolommen die de bronnen
/// per regel leveren. Ontbreekt er iets, dan blijft die kolom weg.
pub async fn stel_samen(
    rijen: &Rijen,
    lexostatussen: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
) -> Option<Uitslag> {
    let d = &rijen.definitie;
    let waar = format!(
        "lexostatus '{}', veld '{}'",
        d.tabel.lexostatus, d.tabel.veld
    );
    let niet_op_te_bouwen = |fout: String| Uitslag {
        parameter: d.parameter.clone(),
        fout: Some(fout),
        regels: Vec::new(),
        bronnen: Vec::new(),
        mist: Vec::new(),
    };
    let tabel = uit_lexostatus(lexostatussen, &d.tabel.lexostatus, &d.tabel.veld)?;
    let Some(tabel) = tabel.as_array() else {
        return Some(niet_op_te_bouwen(format!(
            "{waar} is geen lijst van regels"
        )));
    };
    let mut rijregels = Vec::new();
    for (i, r) in tabel.iter().enumerate() {
        match r.as_object() {
            Some(r) => rijregels.push(r),
            None => {
                return Some(niet_op_te_bouwen(format!(
                    "{waar}: regel {i} is geen object met kolommen"
                )))
            }
        }
    }

    let mut uitslagen: Vec<BronUitslag> = rijen
        .bronnen
        .iter()
        .map(|b| BronUitslag {
            cel: b.definitie.cel.clone(),
            lexostatus: b.definitie.lexostatus.clone(),
            transport: b.transport.soort(),
            bevraagd: 0,
            status: Status::Bevraagd,
            fout: None,
        })
        .collect();
    let mut regels = Vec::new();
    let mut mist: BTreeSet<String> = BTreeSet::new();

    // De regels tegelijk (hooguit GELIJKTIJDIG), in de volgorde van de
    // tabel; binnen een regel de bronnen na elkaar, want een bron kan een
    // kolom van een eerdere als invoer nemen.
    // Eerst de futures in een lijst: een stream over een closure met
    // verwijzingen maakt de future van een route niet Send.
    let mut vragen = Vec::with_capacity(rijregels.len());
    for r in rijregels {
        vragen.push(stel_regel_samen(rijen, r, lexostatussen, parameters));
    }
    let per_regel: Vec<Regeluitslag> = stream::iter(vragen).buffered(GELIJKTIJDIG).collect().await;
    for r in per_regel {
        for (uitslag, bevraging) in uitslagen.iter_mut().zip(r.bronnen) {
            match bevraging {
                Bevraging::Bevraagd => uitslag.bevraagd += 1,
                Bevraging::NietBevraagd(f) => {
                    uitslag.status = slechtste(uitslag.status, Status::NietBevraagd);
                    uitslag.fout.get_or_insert(f);
                }
                Bevraging::Mislukt(status, f) => {
                    uitslag.bevraagd += 1;
                    uitslag.status = slechtste(uitslag.status, status);
                    uitslag.fout.get_or_insert(f);
                }
            }
        }
        mist.extend(r.mist);
        regels.push(Value::Object(r.regel));
    }
    Some(Uitslag {
        parameter: d.parameter.clone(),
        fout: None,
        regels,
        bronnen: uitslagen,
        mist: mist.into_iter().collect(),
    })
}

/// Voer de rijen-definities uit op een samenvoeging, vóór de engine: elke
/// array-parameter komt erbij, met herkomst per regel. `eigen` zijn de
/// lexostatussen van de zaak (bij de toets: de proefreductie van het
/// concept); een tabel kan ook een extra veld zijn dat een synthese-bron
/// doorgaf, en dat staat dan naast de eigen lexostatussen onder de naam van
/// die bron. Gedeeld door de toets en het besluit.
pub async fn pas_toe(
    rijen: &[Rijen],
    eigen: &[Lexostatus],
    samen: &mut Samenvoeging,
) -> Vec<Uitslag> {
    let mut met_bronnen = eigen.to_vec();
    for u in samen.bronnen.iter().filter(|u| !u.extra_velden.is_empty()) {
        met_bronnen.push(Lexostatus {
            extra_velden: u
                .extra_velden
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            ..Lexostatus::leeg(&u.lexostatus)
        });
    }
    let mut uitslagen = Vec::new();
    for r in rijen {
        let Some(uitslag) = stel_samen(r, &met_bronnen, &samen.parameters).await else {
            continue;
        };
        if uitslag.fout.is_some() {
            uitslagen.push(uitslag);
            continue;
        }
        samen.parameters.insert(
            uitslag.parameter.clone(),
            Value::Array(uitslag.regels.clone()),
        );
        samen.herkomst.insert(
            uitslag.parameter.clone(),
            Herkomst::PerRegel {
                lexostatus: r.definitie.tabel.lexostatus.clone(),
                veld: r.definitie.tabel.veld.clone(),
            },
        );
        uitslagen.push(uitslag);
    }
    uitslagen
}

/// Controleer een rijen-definitie bij het laden: de tabel komt uit een van de
/// `eigen` lexostatussen (die haar levert) of uit een extra veld dat een
/// synthese-bron doorgeeft, elke kolom komt uit maar een plek, een bron is
/// een andere cel, en elke invoer van een bron wordt gevuld. `wie` is de
/// uitvoering (toets of besluit), `anders` zegt wat een andere lexostatus
/// dan niet is.
pub fn controleer(
    wie: &str,
    r: &RijenDefinitie,
    eigen: &[&str],
    anders: &str,
    d: &ProcesDefinitie,
    cel: &Cel,
) -> Vec<String> {
    let mut fouten = Vec::new();
    let wie = format!("{wie}, rijen '{}'", r.parameter);
    // Een tabel of invoer mag ook uit een extra veld komen dat een
    // synthese-bron doorgeeft (bijvoorbeeld de regels van een uitslag uit een
    // register).
    let doorgegeven_veld = |lexostatus: &str, veld: &str| {
        d.andere_bronnen()
            .any(|s| s.lexostatus == lexostatus && s.extra_velden.iter().any(|e| e == veld))
    };
    if doorgegeven_veld(&r.tabel.lexostatus, &r.tabel.veld) {
        // Uit een bron: de synthese controleert de bron zelf.
    } else if !eigen.contains(&r.tabel.lexostatus.as_str()) {
        fouten.push(format!(
            "{wie}: de tabel komt uit lexostatus '{}', en die is {anders}",
            r.tabel.lexostatus
        ));
    } else if !cel
        .lexostatussen
        .lexostatus(&r.tabel.lexostatus)
        .is_some_and(|l| l.levert(&r.tabel.veld))
    {
        fouten.push(format!(
            "{wie}: lexostatus '{}' levert geen '{}' (geen afleiding en geen extra veld)",
            r.tabel.lexostatus, r.tabel.veld
        ));
    }
    // Elke kolomnaam komt uit maar een plek: de tabel of een bron.
    let mut kolommen: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for naam in r.kolommen.values() {
        kolommen.entry(naam).or_default().push("de tabel".into());
    }
    for bron in &r.bronnen {
        let bronwie = format!("{wie}, bron {}/{}", bron.cel, bron.lexostatus);
        if bron.cel == cel.id() {
            fouten.push(format!(
                "{bronwie}: een bron is een andere cel, niet de cel zelf"
            ));
        }
        for naam in bron.kolommen.values() {
            kolommen
                .entry(naam)
                .or_default()
                .push(format!("bron {}/{}", bron.cel, bron.lexostatus));
        }
        for (i, v) in &bron.invoer {
            match v {
                RijInvoer::Kolom { kolom, .. } if !kolommen.contains_key(kolom.as_str()) => {
                    fouten.push(format!(
                        "{bronwie}, invoer '{i}': kolom '{kolom}' wordt door niets ervoor gevuld"
                    ));
                }
                RijInvoer::Eigen {
                    lexostatus, veld, ..
                } if !doorgegeven_veld(lexostatus, veld)
                    && !cel
                        .lexostatussen
                        .lexostatus(lexostatus)
                        .is_some_and(|l| l.levert(veld)) =>
                {
                    fouten.push(format!(
                        "{bronwie}, invoer '{i}': lexostatus '{lexostatus}' levert geen '{veld}'"
                    ));
                }
                _ => {}
            }
        }
    }
    for (naam, waar) in &kolommen {
        if waar.len() > 1 {
            fouten.push(format!(
                "{wie}: kolom '{naam}' komt uit meer dan een plek: {}",
                waar.join(", ")
            ));
        }
    }
    fouten
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::proef::Vast;
    use serde_json::json;
    use std::sync::Arc;

    fn eigen() -> Vec<Lexostatus> {
        vec![serde_json::from_value(json!({
            "naam": "aanvraag",
            "parameters": {},
            "extra_velden": {
                "aanduiding": "EEN LIJST",
                "organen": [
                    {"orgaan": "raad", "gebied": "A", "zetels": 10, "aantal": null},
                    {"orgaan": "raad", "gebied": "B", "zetels": 3, "aantal": 2},
                ],
            },
        }))
        .unwrap()]
    }

    fn definitie() -> RijenDefinitie {
        serde_json::from_value(json!({
            "parameter": "tabel",
            "tabel": {"lexostatus": "aanvraag", "veld": "organen"},
            "kolommen": {"orgaan": "orgaan", "gebied": "gebiedscode", "zetels": "zetels", "aantal": "samenstellende"},
        }))
        .unwrap()
    }

    fn bron(antwoord: Result<Value, TransportFout>, invoer: Value) -> (Bron, Arc<Vast>) {
        let t = Arc::new(Vast::new(antwoord));
        let definitie: RijBron = serde_json::from_value(json!({
            "cel": "register",
            "lexostatus": "per_gebied",
            "invoer": invoer,
            "kolommen": {"bedrag": "tarief"},
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

    #[tokio::test]
    async fn elke_regel_krijgt_haar_kolommen() {
        let (b, t) = bron(
            Ok(json!({"naam": "per_gebied", "parameters": {}, "extra_velden": {"bedrag": 5}})),
            json!({"gebied": {"kolom": "gebiedscode"}}),
        );
        let rijen = Rijen {
            definitie: definitie(),
            bronnen: vec![b],
        };
        let u = stel_samen(&rijen, &eigen(), &BTreeMap::new())
            .await
            .unwrap();
        assert_eq!(
            t.vragen().as_slice(),
            [
                "/cellen/register/api/lexostatus/per_gebied?gebied=A",
                "/cellen/register/api/lexostatus/per_gebied?gebied=B"
            ]
        );
        assert_eq!(
            u.regels[0],
            json!({"orgaan": "raad", "gebiedscode": "A", "zetels": 10, "samenstellende": null, "tarief": 5})
        );
        assert_eq!(u.regels[1]["samenstellende"], json!(2));
        assert!(u.mist.is_empty(), "{:?}", u.mist);
        assert_eq!(u.bronnen[0].bevraagd, 2);
        assert_eq!(u.bronnen[0].status, Status::Bevraagd);
    }

    #[tokio::test]
    async fn een_onbereikbare_bron_vult_niets_aan() {
        let (b, _) = bron(
            Err(TransportFout::Onbereikbaar("weg".into())),
            json!({"gebied": {"kolom": "gebiedscode"}}),
        );
        let rijen = Rijen {
            definitie: definitie(),
            bronnen: vec![b],
        };
        let u = stel_samen(&rijen, &eigen(), &BTreeMap::new())
            .await
            .unwrap();
        assert!(!u.regels[0].as_object().unwrap().contains_key("tarief"));
        assert_eq!(u.mist, ["tarief"]);
        assert_eq!(u.bronnen[0].status, Status::Onbereikbaar);
    }

    #[tokio::test]
    async fn invoer_uit_een_parameter_met_omzetting() {
        let (b, t) = bron(
            Ok(json!({"naam": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({
                "gebied": {"kolom": "gebiedscode"},
                "peildatum": {"parameter": "jaar", "als": "eerste_dag_van_het_jaar"},
                "naam": {"lexostatus": "aanvraag", "veld": "aanduiding"},
            }),
        );
        let rijen = Rijen {
            definitie: definitie(),
            bronnen: vec![b],
        };
        let mut parameters = BTreeMap::new();
        parameters.insert("jaar".to_string(), json!(2026));
        let u = stel_samen(&rijen, &eigen(), &parameters).await.unwrap();
        assert_eq!(
            t.vragen()[0],
            "/cellen/register/api/lexostatus/per_gebied?gebied=A&naam=EEN+LIJST&peildatum=2026-01-01"
        );
        assert_eq!(u.regels[0]["tarief"], json!(7));
    }

    #[tokio::test]
    async fn zonder_invoer_wordt_de_bron_niet_bevraagd() {
        let (b, t) = bron(
            Ok(json!({"naam": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({"peildatum": {"parameter": "ontbreekt"}}),
        );
        let rijen = Rijen {
            definitie: definitie(),
            bronnen: vec![b],
        };
        let u = stel_samen(&rijen, &eigen(), &BTreeMap::new())
            .await
            .unwrap();
        assert!(t.vragen().is_empty());
        assert_eq!(u.bronnen[0].status, Status::NietBevraagd);
        assert!(u.bronnen[0]
            .fout
            .as_ref()
            .unwrap()
            .contains("invoer 'peildatum' ontbreekt"));
        assert_eq!(u.mist, ["tarief"]);
    }

    #[tokio::test]
    async fn een_regel_die_geen_object_is_bouwt_geen_tabel_op() {
        let mut l = eigen();
        l[0].extra_velden.insert(
            "organen".into(),
            json!([{"orgaan": "raad", "gebied": "A", "zetels": 1}, "raad B"]),
        );
        let rijen = Rijen {
            definitie: definitie(),
            bronnen: Vec::new(),
        };
        let u = stel_samen(&rijen, &l, &BTreeMap::new()).await.unwrap();
        assert!(u.regels.is_empty());
        assert!(
            u.fout
                .as_deref()
                .unwrap()
                .contains("regel 1 is geen object"),
            "{u:?}"
        );
        // Dan gaat de parameter niet naar de engine.
        let mut samen = Samenvoeging {
            parameters: BTreeMap::new(),
            herkomst: BTreeMap::new(),
            bronnen: Vec::new(),
        };
        let uit = pas_toe(std::slice::from_ref(&rijen), &l, &mut samen).await;
        assert!(uit[0].fout.is_some());
        assert!(!samen.parameters.contains_key("tabel"));
    }

    /// Een bron die even wacht, telt hoeveel vragen er tegelijk lopen, en
    /// het gebied uit de vraag als bedrag teruggeeft.
    struct Traag {
        bezig: std::sync::atomic::AtomicUsize,
        hoogste: std::sync::atomic::AtomicUsize,
    }

    impl crate::transport::Transport for Traag {
        fn soort(&self) -> &'static str {
            "intern"
        }
        fn haal<'a>(&'a self, pad: &'a str) -> crate::transport::Antwoord<'a> {
            use std::sync::atomic::Ordering::SeqCst;
            Box::pin(async move {
                let nu = self.bezig.fetch_add(1, SeqCst) + 1;
                self.hoogste.fetch_max(nu, SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                self.bezig.fetch_sub(1, SeqCst);
                let gebied = pad.rsplit_once("gebied=").unwrap().1.to_string();
                Ok(
                    json!({"naam": "per_gebied", "parameters": {}, "extra_velden": {"bedrag": gebied}}),
                )
            })
        }
        fn stuur<'a>(&'a self, pad: &'a str, _body: &'a Value) -> crate::transport::Antwoord<'a> {
            self.haal(pad)
        }
    }

    #[tokio::test]
    async fn de_regels_worden_tegelijk_bevraagd_in_hun_eigen_volgorde() {
        let organen: Vec<Value> = (0..8)
            .map(|i| json!({"orgaan": "raad", "gebied": format!("G{i}"), "zetels": i}))
            .collect();
        let mut l = eigen();
        l[0].extra_velden
            .insert("organen".into(), Value::Array(organen));
        let t = Arc::new(Traag {
            bezig: 0.into(),
            hoogste: 0.into(),
        });
        let (mut b, _) = bron(Ok(Value::Null), json!({"gebied": {"kolom": "gebiedscode"}}));
        b.transport = t.clone();
        let rijen = Rijen {
            definitie: definitie(),
            bronnen: vec![b],
        };
        let u = stel_samen(&rijen, &l, &BTreeMap::new()).await.unwrap();
        assert!(t.hoogste.load(std::sync::atomic::Ordering::SeqCst) > 1);
        // Elke regel houdt haar eigen antwoord, in de volgorde van de tabel.
        let tarieven: Vec<&str> = u
            .regels
            .iter()
            .map(|r| r["tarief"].as_str().unwrap())
            .collect();
        assert_eq!(tarieven, ["G0", "G1", "G2", "G3", "G4", "G5", "G6", "G7"]);
        assert_eq!(u.bronnen[0].bevraagd, 8);
        assert_eq!(u.bronnen[0].status, Status::Bevraagd);
    }

    #[tokio::test]
    async fn zonder_tabel_geen_parameter() {
        let mut d = definitie();
        d.tabel.veld = "bestaat_niet".into();
        let rijen = Rijen {
            definitie: d,
            bronnen: Vec::new(),
        };
        assert!(stel_samen(&rijen, &eigen(), &BTreeMap::new())
            .await
            .is_none());
    }
}
