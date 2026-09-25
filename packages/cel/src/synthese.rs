//! Synthese: een proces voegt de lexostatus van de zaak (bij de toets: de
//! proefreductie van het concept) samen met lexostatussen van cellen.
//!
//! Synthese gebeurt bij de afnemer, niet bij de bron. De bron reduceert haar
//! eigen kroniek; het proces vraagt die lexostatus op via een [`Transport`],
//! met een tijdslimiet van drie seconden, en neemt alleen de parameters over
//! die het in `proces.yaml` expliciet van die bron verwacht. Per parameter
//! houdt het bij waar hij vandaan kwam. Niets hiervan wordt vastgelegd: het
//! is informeren, geen feit. Is een bron onbereikbaar, dan wordt er niets
//! aangevuld.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;
use std::time::Duration;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::config::{BronInvoer, RijBron, SyntheseBron};
use crate::proces::Proces;
use crate::reductie::{Lexostatus, Peil};
use crate::regelingen;
use crate::transport::{haal_binnen, Transport, TransportFout};

/// Hoe lang een bron mag doen over een antwoord.
pub const TIJDSLIMIET: Duration = Duration::from_secs(3);

/// Een bron met het transport dat de runtime ervoor koos: een
/// synthese-bron ([`SyntheseBron`]) of een bron die per regel wordt bevraagd
/// ([`RijBron`], zie [`crate::rijen`]).
#[derive(Clone)]
pub struct Bron<D = SyntheseBron> {
    pub definitie: D,
    pub transport: Arc<dyn Transport>,
}

impl<D: Clone> Bron<D> {
    /// Dezelfde bron, langs een ander transport naar dezelfde cel (zoals een
    /// dat antwoorden onthoudt, zie [`crate::transport::Onthouden`]).
    pub fn langs(&self, transport: impl FnOnce(Arc<dyn Transport>) -> Arc<dyn Transport>) -> Self {
        Self {
            definitie: self.definitie.clone(),
            transport: transport(self.transport.clone()),
        }
    }
}

/// Welke lexostatus van welke cel een bron is.
pub trait Bronverwijzing {
    fn cel(&self) -> &str;
    fn lexostatus(&self) -> &str;
}

impl Bronverwijzing for SyntheseBron {
    fn cel(&self) -> &str {
        &self.cel
    }
    fn lexostatus(&self) -> &str {
        &self.lexostatus
    }
}

impl Bronverwijzing for RijBron {
    fn cel(&self) -> &str {
        &self.cel
    }
    fn lexostatus(&self) -> &str {
        &self.lexostatus
    }
}

impl<D: Bronverwijzing> Bron<D> {
    /// Vraag de lexostatus van de bron met deze invoer, binnen de
    /// tijdslimiet. Een antwoord dat geen lexostatus is, is een fout en geen
    /// lege lexostatus.
    pub async fn vraag(
        &self,
        invoer: &Map<String, Value>,
        peil: &Peil,
    ) -> Result<Lexostatus, TransportFout> {
        let d = &self.definitie;
        let v = haal_binnen(
            self.transport.as_ref(),
            &pad(d.cel(), d.lexostatus(), invoer, peil),
            TIJDSLIMIET,
        )
        .await?;
        serde_json::from_value(v).map_err(|e| TransportFout::Json(format!("geen lexostatus: {e}")))
    }
}

/// Waar een parameter vandaan kwam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "bron", rename_all = "snake_case")]
pub enum Herkomst {
    /// Een lexostatus van de zaak, uit de cel waarin het proces vastlegt (bij
    /// de toets: de proefreductie van het concept).
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
    /// De stand bij besluit: een feit dat de procedure pas in een latere
    /// stage vraagt (RFC-008), en dat bij het besluit nog niet gebeurd is.
    StandBijBesluit { stage: String },
    /// Het tijdvak dat de aanvrager in het portaal koos.
    Keuze,
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

/// Het pad van een lexostatus op een runtime, met de invoer en het peil (zie
/// [`Peil`]) als query.
pub fn pad(cel: &str, lexostatus: &str, invoer: &Map<String, Value>, peil: &Peil) -> String {
    let mut paren: BTreeMap<&str, String> = invoer
        .iter()
        .map(|(k, v)| {
            let tekst = match v {
                Value::String(s) => s.clone(),
                ander => ander.to_string(),
            };
            (k.as_str(), tekst)
        })
        .collect();
    paren.extend(peil.query());
    let query = serde_urlencoded::to_string(&paren).unwrap_or_default();
    if query.is_empty() {
        format!("/cellen/{cel}/api/lexostatus/{lexostatus}")
    } else {
        format!("/cellen/{cel}/api/lexostatus/{lexostatus}?{query}")
    }
}

/// De invoer voor een bron: uit de eigen lexostatus (een parameter of een
/// extra veld), uit de extra velden die een eerdere bron doorgaf (`eerder`:
/// lexostatus van die bron naar haar extra velden), of een vaste waarde. Een
/// fout noemt wat ontbreekt.
fn invoer(
    bron: &SyntheseBron,
    eigen: &Lexostatus,
    eerder: &BTreeMap<String, Map<String, Value>>,
) -> Result<Map<String, Value>, String> {
    let mut uit = Map::new();
    for (naam, i) in &bron.invoer {
        let v = match i {
            BronInvoer::Waarde { waarde } => {
                uit.insert(naam.clone(), waarde.clone());
                continue;
            }
            BronInvoer::Veld(v) => v,
        };
        let waarde = if v.lexostatus == eigen.naam {
            eigen.veld(&v.veld)
        } else {
            eerder
                .get(&v.lexostatus)
                .and_then(|m| m.get(&v.veld))
                .filter(|w| !w.is_null())
        };
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

/// De lexostatussen van eerdere bronnen waarop een bron wacht: elke invoer
/// uit een veld dat niet uit de eigen lexostatus komt.
fn wacht_op<'b>(bron: &'b SyntheseBron, eigen: &Lexostatus) -> Vec<&'b str> {
    bron.invoer
        .values()
        .filter_map(BronInvoer::veld)
        .filter(|v| v.lexostatus != eigen.naam)
        .map(|v| v.lexostatus.as_str())
        .collect()
}

/// Voeg de eigen lexostatus samen met die van de bronnen, in rondes. Per
/// ronde worden de bronnen tegelijk bevraagd waarvan elke invoer er al is:
/// uit de eigen lexostatus, of een extra veld van een bron uit een eerdere
/// ronde (bijvoorbeeld een naam bij een registratienummer, en daarna de
/// aanduiding bij die naam). Een bron waarop niemand meer kan wachten, komt in
/// de laatste ronde en meldt wat ontbreekt.
///
/// Elke bron reduceert op `peil`: het moment waarop het proces de stand
/// vraagt (de peildatum van een besluit, het begin van een tijdvak).
pub async fn voeg_samen(eigen: &Lexostatus, bronnen: &[Bron], peil: &Peil) -> Samenvoeging {
    let mut eerder: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut gedaan: BTreeSet<String> = BTreeSet::new();
    let mut open: Vec<&Bron> = bronnen.iter().collect();
    let mut s = Samenvoeging {
        parameters: eigen.parameters.clone(),
        herkomst: eigen
            .parameters
            .keys()
            .map(|k| {
                (
                    k.clone(),
                    Herkomst::Eigen {
                        lexostatus: eigen.naam.clone(),
                    },
                )
            })
            .collect(),
        bronnen: Vec::new(),
    };
    while !open.is_empty() {
        let (klaar, wacht): (Vec<&Bron>, Vec<&Bron>) = open.into_iter().partition(|b| {
            wacht_op(&b.definitie, eigen)
                .iter()
                .all(|l| gedaan.contains(*l))
        });
        // Niemand is klaar: wat nog wacht, wacht op een bron die er niet is
        // of niets doorgaf. Vraag ze nu; de invoer meldt wat ontbreekt.
        let (ronde, rest) = if klaar.is_empty() {
            (wacht, Vec::new())
        } else {
            (klaar, wacht)
        };
        let t = vraag(eigen, &ronde, &eerder, peil).await;
        for (uitslag, bron) in t.bronnen.iter().zip(&ronde) {
            gedaan.insert(bron.definitie.lexostatus.clone());
            if !bron.definitie.extra_velden.is_empty() {
                eerder.insert(uitslag.lexostatus.clone(), uitslag.extra_velden.clone());
            }
        }
        for (k, v) in t.parameters {
            if !eigen.parameters.contains_key(&k) {
                s.parameters.insert(k, v);
            }
        }
        for (k, h) in t.herkomst {
            if !eigen.parameters.contains_key(&k) {
                s.herkomst.insert(k, h);
            }
        }
        s.bronnen.extend(t.bronnen);
        open = rest;
    }
    s
}

/// Bevraag deze bronnen tegelijk en voeg hun parameters samen met de eigen
/// lexostatus.
async fn vraag(
    eigen: &Lexostatus,
    bronnen: &[&Bron],
    eerder: &BTreeMap<String, Map<String, Value>>,
    peil: &Peil,
) -> Samenvoeging {
    let mut parameters = BTreeMap::new();
    let mut herkomst: BTreeMap<String, Herkomst> = BTreeMap::new();

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
        let antwoord = bron.vraag(&invoer, peil).await;
        uitslag.invoer = invoer;
        match antwoord {
            Ok(l) => {
                uitslag.status = Status::Bevraagd;
                for veld in &d.extra_velden {
                    if let Some(w) = l.extra_velden.get(veld) {
                        uitslag.extra_velden.insert(veld.clone(), w.clone());
                    }
                }
                // Wat de afnemer als parameter vraagt, mag de bron als
                // parameter of als extra veld leveren: welke feiten een
                // parameter zijn, zegt de wet van de afnemer, niet de bron.
                let mut geleverd = l.parameters;
                for (k, v) in l.extra_velden {
                    geleverd.entry(k).or_insert(v);
                }
                (uitslag, Some(geleverd))
            }
            Err(TransportFout::Onbereikbaar(r)) => {
                uitslag.status = Status::Onbereikbaar;
                uitslag.fout = Some(r);
                (uitslag, None)
            }
            Err(f @ (TransportFout::Antwoord { .. } | TransportFout::Json(_))) => {
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
            // De bron levert onder haar eigen naam; de afnemer vraagt het
            // onder de zijne.
            for (bij_bron, p) in bron.definitie.parameters.paren() {
                match geleverd.get(bij_bron) {
                    Some(w) => {
                        parameters.insert(p.to_string(), w.clone());
                        herkomst.insert(
                            p.to_string(),
                            Herkomst::Cel {
                                cel: uitslag.cel.clone(),
                                lexostatus: uitslag.lexostatus.clone(),
                                transport: uitslag.transport.to_string(),
                            },
                        );
                    }
                    None => uitslag.niet_geleverd.push(bij_bron.to_string()),
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
/// - een invoer uit een doorgevende bron komt van een eerdere bron in de lijst
///   die dat veld doorgeeft (die mag zelf ook op een eerdere bron wachten: de
///   synthese vraagt in rondes).
fn doorgeven(proces: &Proces) -> Vec<String> {
    let mut fouten = Vec::new();
    let bronnen: Vec<&SyntheseBron> = proces.definitie.andere_bronnen().collect();
    let eigen: Vec<&str> = proces
        .cel
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
        for (naam, v) in bron.invoer.iter().filter_map(|(n, i)| Some((n, i.veld()?))) {
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
            }
        }
    }
    fouten
}

/// De controles op de synthese van een proces bij het opstarten. Een fout
/// hier houdt de runtime tegen:
///
/// - synthese vraagt een portaal of een besluit, want alleen de toets en het
///   proefbesluit gebruiken haar;
/// - een bron is een andere cel dan die waarin het proces vastlegt, tenzij
///   het een bron van de zaak is (`zaak: true`);
/// - elke invoer komt uit een veld van de toets-lexostatus (met een portaal),
///   of van een eerdere bron die haar doorgeeft (zie [`doorgeven`]);
/// - elke parameter is een parameter van het artikel van de toets, van het
///   besluit of van het aanbod (`portaal.aanbod`), of van een artikel dat een
///   van die transitief aanroept;
/// - een parameter komt uit maar een bron: de eigen reductie of een bron.
///
/// Wat het besluit verder vraagt, staat in [`crate::handeling::controleer`].
/// De grondslag van de vertalingen in de synthese, bij het opstarten: elke
/// grondslag van een synthese-bron of van een bron per regel wijst een
/// geladen artikel aan, met het lid dat ze noemt. Met `herkomst: streng`
/// draagt elke bron die vertaalt (een naam bij de afnemer die anders is dan
/// bij de bron, of een vaste waarde in de invoer) een grondslag: de
/// vertaling is een lezing van de wet, net als een afleiding in een cel.
pub fn grondslagen(
    d: &crate::config::ProcesDefinitie,
    service: &regelrecht_engine::LawExecutionService,
) -> Vec<String> {
    let streng = d.herkomst == crate::config::Herkomstcontrole::Streng;
    let synthese = d.andere_bronnen().map(|b| {
        (
            format!("synthese-bron {}/{}", b.cel, b.lexostatus),
            &b.grondslag,
            b.vertaalt(),
        )
    });
    let rijen = d
        .portaal
        .iter()
        .flat_map(|p| p.toets.rijen.iter().map(|r| ("toets".to_string(), r)))
        .chain(
            d.behandeling
                .iter()
                .flat_map(|b| &b.handelingen)
                .flat_map(|h| {
                    h.rijen
                        .iter()
                        .map(move |r| (format!("handeling '{}'", h.naam), r))
                }),
        )
        .flat_map(|(waar, r)| {
            r.bronnen.iter().map(move |b| {
                (
                    format!(
                        "{waar}, rijen '{}', bron {}/{}",
                        r.parameter, b.cel, b.lexostatus
                    ),
                    &b.grondslag,
                    b.vertaalt(),
                )
            })
        });
    let mut fouten = BTreeSet::new();
    for (wie, grondslag, vertaalt) in synthese.chain(rijen) {
        for g in grondslag {
            if let Err(f) = regelingen::geldig(service, g) {
                fouten.insert(format!("{wie}: {f}"));
            }
        }
        if streng && grondslag.is_empty() && !vertaalt.is_empty() {
            fouten.insert(format!(
                "{wie}: vertaalt ({}) zonder grondslag; met herkomst: streng zegt de afnemer op welk artikel een vertaling rust",
                vertaalt.join("; ")
            ));
        }
    }
    fouten.into_iter().collect()
}

pub fn controleer(proces: &Proces) -> Vec<String> {
    let mut fouten = Vec::new();
    if proces.definitie.synthese.is_empty() {
        return fouten;
    }
    let bronnen: Vec<&SyntheseBron> = proces.definitie.andere_bronnen().collect();
    let cel = &proces.cel;
    let service = proces.service.as_ref();
    fouten.extend(doorgeven(proces));
    let doorgevers: Vec<&str> = bronnen
        .iter()
        .filter(|b| !b.extra_velden.is_empty())
        .map(|b| b.lexostatus.as_str())
        .collect();
    // Wat de handelingen vragen: de parameters van hun artikelen, samen.
    let handelingen = proces.handelingen();
    let onder_besluit: BTreeSet<String> = handelingen
        .iter()
        .filter_map(|h| {
            let a = regelingen::artikel(service, &h.artikel).ok()?;
            Some(regelingen::transitieve_parameters(service, &h.regeling, a))
        })
        .flatten()
        .collect();
    let Some(portaal) = proces.portaal() else {
        if handelingen.is_empty() {
            fouten.push(
                "synthese zonder portaal en zonder handelingen: alleen de toets van een portaal en de handelingen in een zaak gebruiken haar"
                    .into(),
            );
            return fouten;
        }
        for bron in &bronnen {
            let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
            if bron.cel == cel.id() {
                fouten.push(format!(
                    "{wie}: een bron uit de cel waarin het proces vastlegt, is een bron van de zaak (zaak: true)"
                ));
            }
            for p in bron
                .parameters
                .iter()
                .filter(|p| !onder_besluit.contains(*p))
            {
                fouten.push(format!(
                    "{wie}: '{p}' is geen parameter van een handeling of van een artikel dat zij aanroept"
                ));
            }
        }
        return fouten;
    };
    let eigen = cel.lexostatussen.lexostatus(&portaal.toets.lexostatus);
    let onder_toets = service
        .resolver()
        .get_article_by_output(&portaal.toets.regeling, &portaal.toets.uitkomst, None)
        .map(|a| regelingen::transitieve_parameters(service, &portaal.toets.regeling, a))
        .unwrap_or_default();
    // Wat het aanbod vraagt: de parameters van het aanbod-artikel.
    let onder_aanbod = portaal
        .aanbod
        .as_ref()
        .and_then(|a| {
            let art = service
                .resolver()
                .get_article_by_output(&a.regeling, &a.uitkomst, None)?;
            Some(regelingen::transitieve_parameters(
                service,
                &a.regeling,
                art,
            ))
        })
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
    for bron in &bronnen {
        let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
        if bron.cel == cel.id() {
            fouten.push(format!(
                "{wie}: een bron uit de cel waarin het proces vastlegt, is een bron van de zaak (zaak: true)"
            ));
        }
        for (naam, v) in bron.invoer.iter().filter_map(|(n, i)| Some((n, i.veld()?))) {
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
            if !onder_toets.contains(p) && !onder_besluit.contains(p) && !onder_aanbod.contains(p) {
                fouten.push(format!(
                    "{wie}: '{p}' is geen parameter van {}#{} (de toets, '{}'), een handeling of het aanbod, of van een artikel dat een van die aanroept",
                    portaal.toets.regeling,
                    service
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
pub async fn waarschuwingen(proces: &str, bronnen: &[Bron]) -> Vec<String> {
    let mut uit = Vec::new();
    for bron in bronnen {
        let d = &bron.definitie;
        let wie = format!(
            "proces '{proces}': synthese-bron {}/{} ({})",
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
        let mut geleverd = namen(lexo, "parameters");
        geleverd.extend(namen(lexo, "extra_velden"));
        for (p, _) in d.parameters.paren().filter(|(p, _)| !geleverd.contains(*p)) {
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
    use crate::transport::proef::Vast;
    use serde_json::json;

    fn bron(antwoord: Result<Value, TransportFout>) -> (Bron, Arc<Vast>) {
        let t = Arc::new(Vast::new(antwoord));
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
        Arc::new(Vast::new(Ok(antwoord)))
    }

    /// Een bron geeft een extra veld door aan een latere bron: eerst een
    /// nummer naar een naam, dan de naam naar een aantal. Het doorgegeven veld
    /// is geen parameter.
    #[tokio::test]
    async fn een_extra_veld_gaat_naar_de_volgende_bron() {
        let t1 = vaste(json!({"naam": "bron", "parameters": {}, "extra_velden": {"naam": "EEN"}}));
        let t2 = vaste(json!({"naam": "bron", "parameters": {"aantal": 3}}));
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
        let s = voeg_samen(&eigen, &[b1, b2], &Peil::default()).await;
        assert_eq!(
            t2.vragen()[0],
            "/cellen/register/api/lexostatus/op_naam?naam=EEN"
        );
        assert_eq!(s.parameters.get("aantal"), Some(&json!(3)));
        assert!(!s.parameters.contains_key("naam"));
        assert_eq!(s.bronnen.len(), 2);
    }

    /// Een keten van drie: een nummer naar een naam, de naam naar een
    /// aanduiding, de aanduiding naar een feit. De synthese vraagt in rondes;
    /// een bron wacht tot haar invoer er is. Een vaste waarde gaat mee, en de
    /// afnemer vraagt het feit onder zijn eigen naam.
    #[tokio::test]
    async fn een_keten_van_bronnen_in_rondes_met_vertaling() {
        let t1 = vaste(json!({"naam": "x", "parameters": {}, "extra_velden": {"naam": "EEN"}}));
        let t2 =
            vaste(json!({"naam": "x", "parameters": {}, "extra_velden": {"aanduiding": "LIJST"}}));
        let t3 = vaste(
            json!({"naam": "x", "parameters": {"is_ingeschreven_in_register": true},
                              "extra_velden": {"zetels_toegekend": 4}}),
        );
        let bron = |t: &Arc<Vast>, d: Value| Bron {
            definitie: serde_json::from_value(d).unwrap(),
            transport: t.clone(),
        };
        // In omgekeerde volgorde: de rondes volgen uit de invoer.
        let bronnen = [
            bron(
                &t3,
                json!({
                    "cel": "register", "lexostatus": "register",
                    "invoer": {"aanduiding": {"lexostatus": "op_naam", "veld": "aanduiding"}, "orgaan": {"waarde": "raad"}},
                    "parameters": {"is_ingeschreven_in_register": "is_ingeschreven_raad", "zetels_toegekend": "zetels_op_lijst"}
                }),
            ),
            bron(
                &t2,
                json!({
                    "cel": "register", "lexostatus": "op_naam",
                    "invoer": {"naam": {"lexostatus": "op_nummer", "veld": "naam"}},
                    "parameters": [], "extra_velden": ["aanduiding"]
                }),
            ),
            bron(
                &t1,
                json!({
                    "cel": "handelsregister", "lexostatus": "op_nummer",
                    "invoer": {"nummer": {"lexostatus": "eigen", "veld": "nummer"}},
                    "parameters": [], "extra_velden": ["naam"]
                }),
            ),
        ];
        let eigen: Lexostatus = serde_json::from_value(json!({
            "naam": "eigen", "parameters": {}, "extra_velden": {"nummer": "12345678"}
        }))
        .unwrap();
        let s = voeg_samen(&eigen, &bronnen, &Peil::default()).await;
        assert_eq!(
            t3.vragen()[0],
            "/cellen/register/api/lexostatus/register?aanduiding=LIJST&orgaan=raad"
        );
        assert_eq!(s.parameters.get("is_ingeschreven_raad"), Some(&json!(true)));
        assert!(!s.parameters.contains_key("is_ingeschreven_in_register"));
        // Een extra veld van de bron is voor de afnemer een parameter als zijn
        // wet het vraagt.
        assert_eq!(s.parameters.get("zetels_op_lijst"), Some(&json!(4)));
        assert_eq!(
            s.herkomst["is_ingeschreven_raad"],
            Herkomst::Cel {
                cel: "register".into(),
                lexostatus: "register".into(),
                transport: t3.soort().into()
            }
        );
        let volgorde: Vec<&str> = s.bronnen.iter().map(|b| b.lexostatus.as_str()).collect();
        assert_eq!(volgorde, ["op_nummer", "op_naam", "register"]);
    }

    /// Een bron die wacht op een bron die er niet is, wordt toch gevraagd, en
    /// meldt wat ontbreekt; er wordt niets aangevuld.
    #[tokio::test]
    async fn een_bron_die_op_niets_kan_wachten() {
        let t = vaste(json!({"naam": "x", "parameters": {"a": 1}}));
        let b = Bron {
            definitie: serde_json::from_value(json!({
                "cel": "register", "lexostatus": "l",
                "invoer": {"naam": {"lexostatus": "bestaat_niet", "veld": "naam"}},
                "parameters": ["a"]
            }))
            .unwrap(),
            transport: t.clone(),
        };
        let eigen = Lexostatus::leeg("eigen");
        let s = voeg_samen(&eigen, &[b], &Peil::default()).await;
        assert!(t.vragen().is_empty());
        assert_eq!(s.bronnen[0].status, Status::NietBevraagd);
        assert!(s.bronnen[0]
            .fout
            .as_deref()
            .unwrap()
            .contains("invoer 'naam' ontbreekt"));
        assert!(!s.parameters.contains_key("a"));
    }

    /// Geeft de eerste bron niets door, dan wordt de volgende niet bevraagd,
    /// en er wordt niets aangevuld.
    #[tokio::test]
    async fn zonder_doorgegeven_veld_geen_vraag() {
        let t1 = vaste(json!({"naam": "bron", "parameters": {}}));
        let t2 = vaste(json!({"naam": "bron", "parameters": {"aantal": 3}}));
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
        let s = voeg_samen(&eigen, &[b1, b2], &Peil::default()).await;
        assert!(t2.vragen().is_empty());
        assert!(!s.parameters.contains_key("aantal"));
        assert_eq!(s.bronnen[1].status, Status::NietBevraagd);
    }

    #[tokio::test]
    async fn samenvoegen_met_herkomst() {
        let (b, t) = bron(Ok(
            json!({"naam": "bron", "parameters": {"ingeschreven": true, "zetels": 6, "anders": 1}}),
        ));
        let s = voeg_samen(&eigen(Some("EEN & ANDER")), &[b], &Peil::default()).await;
        assert_eq!(
            t.vragen()[0],
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
        let s = voeg_samen(&eigen(Some("X")), &[b], &Peil::default()).await;
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
        let (b, t) = bron(Ok(json!({"naam": "bron", "parameters": {}})));
        let s = voeg_samen(&eigen(None), &[b], &Peil::default()).await;
        assert!(t.vragen().is_empty());
        assert_eq!(s.bronnen[0].status, Status::NietBevraagd);
        assert!(s.reden().unwrap().contains("invoer 'aanduiding' ontbreekt"));
    }

    #[tokio::test]
    async fn niet_geleverde_parameter_wordt_genoemd() {
        let (b, _) = bron(Ok(
            json!({"naam": "bron", "parameters": {"ingeschreven": false}}),
        ));
        let s = voeg_samen(&eigen(Some("X")), &[b], &Peil::default()).await;
        assert_eq!(s.bronnen[0].niet_geleverd, ["zetels"]);
        assert!(!s.parameters.contains_key("zetels"));
    }

    #[test]
    fn pad_zonder_invoer() {
        let nu = Peil::default();
        assert_eq!(
            pad("a", "b", &Map::new(), &nu),
            "/cellen/a/api/lexostatus/b"
        );
        let i = json!({"jaar": 2025}).as_object().unwrap().clone();
        assert_eq!(
            pad("a", "b", &i, &nu),
            "/cellen/a/api/lexostatus/b?jaar=2025"
        );
    }

    #[test]
    fn pad_met_peil() {
        let peil = Peil {
            peilmoment: Some(crate::datum::Tijdpunt::lees("p", "2027-01-01").unwrap()),
            bekend_op: Some(
                crate::datum::Tijdpunt::lees("b", "2026-09-25T10:00:00+02:00").unwrap(),
            ),
        };
        let i = json!({"jaar": 2025}).as_object().unwrap().clone();
        assert_eq!(
            pad("a", "b", &i, &peil),
            "/cellen/a/api/lexostatus/b?bekend_op=2026-09-25T10%3A00%3A00%2B02%3A00&jaar=2025&peilmoment=2027-01-01"
        );
    }
}
