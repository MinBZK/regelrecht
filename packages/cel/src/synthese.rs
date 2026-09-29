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
use crate::reductie::{Lexostatus, Peil, Reductieroute};
use crate::regelingen;
use crate::transport::{haal_binnen, Transport, TransportFout};
use regelrecht_engine::LawExecutionService;

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
    fn cell(&self) -> &str;
    fn lexostatus(&self) -> &str;
}

impl Bronverwijzing for SyntheseBron {
    fn cell(&self) -> &str {
        &self.cell
    }
    fn lexostatus(&self) -> &str {
        &self.lexostatus
    }
}

impl Bronverwijzing for RijBron {
    fn cell(&self) -> &str {
        &self.cell
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
        input: &Map<String, Value>,
        peil: &Peil,
    ) -> Result<Lexostatus, TransportFout> {
        let d = &self.definitie;
        let v = haal_binnen(
            self.transport.as_ref(),
            &path(d.cell(), d.lexostatus(), input, peil),
            TIJDSLIMIET,
        )
        .await?;
        serde_json::from_value(v).map_err(|e| TransportFout::Json(format!("geen lexostatus: {e}")))
    }
}

/// Een synthese-bron die geen cel is maar het eigen beleid van de afnemer
/// (notitie bron en gram-id): het proces vraagt haar als elke bron, en zij
/// antwoordt met een engine-run van het artikel in plaats van een reductie.
/// Zo staat de keten "KvK-nummer, dan de naam, dan de aanduiding" in het
/// beleid van de afnemer (met een gewone `source` naar het beleid van de
/// beheerder van elk register) en niet in `proces.yaml`. De uitkomsten staan
/// in de lexostatus als extra velden: invoer voor een latere bron.
pub struct Beleidsbron {
    service: Arc<LawExecutionService>,
    regulation: String,
    outputs: Vec<String>,
    name: String,
}

impl Beleidsbron {
    pub fn nieuw(service: Arc<LawExecutionService>, d: &SyntheseBron) -> Self {
        Self {
            service,
            regulation: d.regulation.clone().unwrap_or_default(),
            outputs: d.extra_fields.clone(),
            name: d.lexostatus.clone(),
        }
    }

    /// Een parameter uit de query met het type dat de regeling hem geeft:
    /// de query kent alleen tekst (zie [`pad`]), de engine rekent met een
    /// getal, een bedrag of een waarheidswaarde. `null` is geen waarde.
    fn value(&self, name: &str, tekst: String) -> Value {
        use regelrecht_engine::ParameterType as T;
        if tekst == "null" {
            return Value::Null;
        }
        let soort = self
            .service
            .resolver()
            .get_law(&self.regulation)
            .and_then(|law| {
                law.articles
                    .iter()
                    .filter_map(|a| a.get_execution_spec())
                    .flat_map(|e| e.parameters.iter().flatten())
                    .find(|p| p.name == name)
                    .map(|p| p.param_type)
            });
        match soort {
            Some(T::Number | T::Amount | T::Boolean | T::Array | T::Object) => {
                serde_json::from_str(&tekst).unwrap_or(Value::String(tekst))
            }
            _ => Value::String(tekst),
        }
    }

    fn antwoord(&self, path: &str) -> Result<Value, TransportFout> {
        let query = path.split_once('?').map_or("", |(_, q)| q);
        let paren: Vec<(String, String)> = serde_urlencoded::from_str(query)
            .map_err(|e| TransportFout::Json(format!("de vraag is niet te lezen: {e}")))?;
        let mut parameters: BTreeMap<String, Value> = BTreeMap::new();
        let mut date = crate::datum::reference_date(&chrono::Local::now().fixed_offset());
        for (k, v) in paren {
            match k.as_str() {
                // Een datum of een moment: de engine leest de regeling op die dag.
                "as_of" => {
                    date = match crate::datum::datum_van(&v) {
                        Some(d) => d.format("%Y-%m-%d").to_string(),
                        None => crate::datum::peildatum_van(&v).map_err(TransportFout::Json)?,
                    }
                }
                "known_at" => {}
                _ => {
                    let w = self.value(&k, v);
                    parameters.insert(k, w);
                }
            }
        }
        let outputs: Vec<&str> = self.outputs.iter().map(String::as_str).collect();
        let e = crate::toets::evalueer(
            &self.service,
            &self.regulation,
            &outputs,
            &parameters,
            &date,
        );
        if let Some(f) = &e.error {
            return Err(TransportFout::Antwoord {
                status: 400,
                error: format!("{}: {f}", self.name),
            });
        }
        // Een uitkomst zonder waarde valt weg (de synthese mist dan dat veld),
        // maar niet stil.
        if !e.missing.is_empty() {
            tracing::warn!(source = %self.name, missing = %e.missing.join(", "), "beleidsbron: uitkomsten zonder waarde");
        }
        let l = Lexostatus {
            extra_fields: e
                .waarden
                .into_iter()
                .filter(|(_, v)| !v.is_null())
                .collect(),
            ..Lexostatus::leeg(&self.name)
        };
        serde_json::to_value(l).map_err(|e| TransportFout::Json(e.to_string()))
    }
}

impl Transport for Beleidsbron {
    fn soort(&self) -> &'static str {
        "policy"
    }
    fn haal<'a>(&'a self, path: &'a str) -> crate::transport::Antwoord<'a> {
        let uit = self.antwoord(path);
        Box::pin(async move { uit })
    }
    fn stuur<'a>(&'a self, path: &'a str, _body: &'a Value) -> crate::transport::Antwoord<'a> {
        Box::pin(async move {
            Err(TransportFout::Json(format!(
                "{path}: een beleidsbron legt niets vast"
            )))
        })
    }
}

/// Waar een parameter vandaan kwam.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "source", rename_all = "snake_case")]
pub enum Herkomst {
    /// Een lexostatus van de zaak, uit de cel waarin het proces vastlegt (bij
    /// de toets: de proefreductie van het concept).
    Own { lexostatus: String },
    /// Een lexostatus van een andere cel.
    Cell {
        cell: String,
        lexostatus: String,
        transport: String,
    },
    /// Samengesteld per regel uit een tabelveld van een eigen lexostatus en
    /// de bronnen die per regel zijn bevraagd (zie [`crate::rijen`]).
    PerRow { lexostatus: String, field: String },
    /// Het besluitformulier: een oordeel van de behandelaar.
    Handler,
    /// De stand bij besluit: een feit dat de procedure pas in een latere
    /// stage vraagt (RFC-008), en dat bij het besluit nog niet gebeurd is.
    StateAtDecision { stage: String },
    /// Het tijdvak dat de aanvrager in het portaal koos.
    Choice,
    /// Het id van het besluit waarop de handeling handelt (de
    /// `besluitparameter` van de handeling; notitie bron en gram-id).
    Decision,
}

/// Hoe de vraag aan een bron verliep.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Queried,
    /// Geen verbinding of geen antwoord binnen de tijdslimiet.
    Unreachable,
    /// Wel een antwoord, maar geen lexostatus.
    Error,
    /// Niet gevraagd: een invoer ontbrak in de eigen lexostatus.
    NotQueried,
}

/// De uitslag per bron.
#[derive(Debug, Clone, Serialize)]
pub struct BronUitslag {
    pub cell: String,
    pub lexostatus: String,
    pub transport: &'static str,
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    pub input: Map<String, Value>,
    /// De verwachte parameters die de bron niet leverde.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub not_delivered: Vec<String>,
    /// De extra velden die deze bron doorgaf aan een latere bron.
    #[serde(skip_serializing_if = "Map::is_empty")]
    pub extra_fields: Map<String, Value>,
    /// Langs welke route de bron reduceerde, als zij dat zegt (een runtime
    /// met de engine-route, experiment A).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduction: Option<Reductieroute>,
}

/// De samengevoegde parameters, met hun herkomst.
#[derive(Debug, Clone, Serialize)]
pub struct Samenvoeging {
    pub parameters: BTreeMap<String, Value>,
    pub provenance: BTreeMap<String, Herkomst>,
    pub sources: Vec<BronUitslag>,
}

impl Samenvoeging {
    /// Waarom de toets niet te beoordelen is als een bron niets leverde, in
    /// woorden; `None` als elke bron antwoordde.
    pub fn reason(&self) -> Option<String> {
        self.sources.iter().find_map(|b| {
            let error = b.error.as_deref().unwrap_or_default();
            match b.status {
                Status::Queried => None,
                Status::Unreachable => {
                    Some(format!("niet te beoordelen: bron {} onbereikbaar", b.cell))
                }
                Status::Error => Some(format!(
                    "niet te beoordelen: bron {} gaf geen lexostatus ({error})",
                    b.cell
                )),
                Status::NotQueried => Some(format!(
                    "niet te beoordelen: bron {} niet bevraagd ({error})",
                    b.cell
                )),
            }
        })
    }
}

/// Het pad van een lexostatus op een runtime, met de invoer en het peil (zie
/// [`Peil`]) als query.
pub fn path(cell: &str, lexostatus: &str, input: &Map<String, Value>, peil: &Peil) -> String {
    let mut paren: BTreeMap<&str, String> = input
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
    // Een lexostatus uit de wet heet naar haar artikel (`<regeling>#<artikel>`).
    let lexostatus = crate::celclient::url_segment(lexostatus);
    if query.is_empty() {
        format!("/cells/{cell}/api/lexostatus/{lexostatus}")
    } else {
        format!("/cells/{cell}/api/lexostatus/{lexostatus}?{query}")
    }
}

/// De invoer voor een bron: uit de eigen lexostatus (een parameter of een
/// extra veld), uit de extra velden die een eerdere bron doorgaf (`eerder`:
/// lexostatus van die bron naar haar extra velden), of een vaste waarde. Een
/// fout noemt wat ontbreekt.
fn input(
    source: &SyntheseBron,
    eigen: &Lexostatus,
    eerder: &BTreeMap<String, Map<String, Value>>,
) -> Result<Map<String, Value>, String> {
    let mut uit = Map::new();
    for (name, i) in &source.input {
        let v = match i {
            BronInvoer::Waarde { value } => {
                uit.insert(name.clone(), value.clone());
                continue;
            }
            BronInvoer::Veld(v) => v,
        };
        let value = if v.lexostatus == eigen.name {
            eigen.field(&v.field)
        } else {
            eerder
                .get(&v.lexostatus)
                .and_then(|m| m.get(&v.field))
                .filter(|w| !w.is_null())
        };
        match value {
            Some(w) => {
                uit.insert(name.clone(), w.clone());
            }
            None => {
                return Err(format!(
                    "invoer '{name}' ontbreekt: {}.{} heeft geen waarde",
                    v.lexostatus, v.field
                ))
            }
        }
    }
    Ok(uit)
}

/// De lexostatussen van eerdere bronnen waarop een bron wacht: elke invoer
/// uit een veld dat niet uit de eigen lexostatus komt.
fn wacht_op<'b>(source: &'b SyntheseBron, eigen: &Lexostatus) -> Vec<&'b str> {
    source
        .input
        .values()
        .filter_map(BronInvoer::field)
        .filter(|v| v.lexostatus != eigen.name)
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
pub async fn voeg_samen(eigen: &Lexostatus, sources: &[Bron], peil: &Peil) -> Samenvoeging {
    let mut eerder: BTreeMap<String, Map<String, Value>> = BTreeMap::new();
    let mut gedaan: BTreeSet<String> = BTreeSet::new();
    let mut open: Vec<&Bron> = sources.iter().collect();
    let mut s = Samenvoeging {
        parameters: eigen.parameters.clone(),
        provenance: eigen
            .parameters
            .keys()
            .map(|k| {
                (
                    k.clone(),
                    Herkomst::Own {
                        lexostatus: eigen.name.clone(),
                    },
                )
            })
            .collect(),
        sources: Vec::new(),
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
        for (result, source) in t.sources.iter().zip(&ronde) {
            gedaan.insert(source.definitie.lexostatus.clone());
            if !source.definitie.extra_fields.is_empty() {
                eerder.insert(result.lexostatus.clone(), result.extra_fields.clone());
            }
        }
        for (k, v) in t.parameters {
            if !eigen.parameters.contains_key(&k) {
                s.parameters.insert(k, v);
            }
        }
        for (k, h) in t.provenance {
            if !eigen.parameters.contains_key(&k) {
                s.provenance.insert(k, h);
            }
        }
        s.sources.extend(t.sources);
        open = rest;
    }
    s
}

/// Bevraag deze bronnen tegelijk en voeg hun parameters samen met de eigen
/// lexostatus.
async fn vraag(
    eigen: &Lexostatus,
    sources: &[&Bron],
    eerder: &BTreeMap<String, Map<String, Value>>,
    peil: &Peil,
) -> Samenvoeging {
    let mut parameters = BTreeMap::new();
    let mut provenance: BTreeMap<String, Herkomst> = BTreeMap::new();

    let vragen = sources.iter().map(|source| async move {
        let d = &source.definitie;
        let mut result = BronUitslag {
            cell: d.cell.clone(),
            lexostatus: d.lexostatus.clone(),
            transport: source.transport.soort(),
            status: Status::NotQueried,
            error: None,
            input: Map::new(),
            not_delivered: Vec::new(),
            extra_fields: Map::new(),
            reduction: None,
        };
        let input = match input(d, eigen, eerder) {
            Ok(i) => i,
            Err(f) => {
                result.error = Some(f);
                return (result, None);
            }
        };
        let antwoord = source.vraag(&input, peil).await;
        result.input = input;
        match antwoord {
            Ok(l) => {
                result.status = Status::Queried;
                result.reduction = l.reduction.clone();
                for field in &d.extra_fields {
                    if let Some(w) = l.extra_fields.get(field) {
                        result.extra_fields.insert(field.clone(), w.clone());
                    }
                }
                // Wat de afnemer als parameter vraagt, mag de bron als
                // parameter of als extra veld leveren: welke feiten een
                // parameter zijn, zegt de wet van de afnemer, niet de bron.
                let mut geleverd = l.parameters;
                for (k, v) in l.extra_fields {
                    geleverd.entry(k).or_insert(v);
                }
                (result, Some(geleverd))
            }
            Err(TransportFout::Unreachable(r)) => {
                result.status = Status::Unreachable;
                result.error = Some(r);
                (result, None)
            }
            Err(f @ (TransportFout::Antwoord { .. } | TransportFout::Json(_))) => {
                result.status = Status::Error;
                result.error = Some(f.to_string());
                (result, None)
            }
        }
    });
    let antwoorden = futures_util::future::join_all(vragen).await;

    let mut uitslagen = Vec::new();
    for ((mut result, geleverd), source) in antwoorden.into_iter().zip(sources.iter()) {
        if let Some(geleverd) = geleverd {
            // De bron levert onder haar eigen naam; de afnemer vraagt het
            // onder de zijne.
            for (bij_bron, p) in source.definitie.parameters.paren() {
                match geleverd.get(bij_bron) {
                    Some(w) => {
                        parameters.insert(p.to_string(), w.clone());
                        provenance.insert(
                            p.to_string(),
                            Herkomst::Cell {
                                cell: result.cell.clone(),
                                lexostatus: result.lexostatus.clone(),
                                transport: result.transport.to_string(),
                            },
                        );
                    }
                    None => result.not_delivered.push(bij_bron.to_string()),
                }
            }
        }
        uitslagen.push(result);
    }
    Samenvoeging {
        parameters,
        provenance,
        sources: uitslagen,
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
    let sources: Vec<&SyntheseBron> = proces.definitie.andere_bronnen().collect();
    let eigen: Vec<&str> = proces
        .cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .map(|d| d.name.as_str())
        .collect();
    for (i, source) in sources.iter().enumerate() {
        let wie = format!("synthese-bron {}/{}", source.cell, source.lexostatus);
        if source.parameters.is_empty() && source.extra_fields.is_empty() {
            fouten.push(format!(
                "{wie}: levert geen parameter en geeft geen extra veld door"
            ));
        }
        if !source.extra_fields.is_empty() {
            if eigen.contains(&source.lexostatus.as_str()) {
                fouten.push(format!(
                    "{wie}: geeft extra velden door, maar heet als een eigen lexostatus; een invoer zou dan twee dingen kunnen aanwijzen"
                ));
            }
            if sources[..i]
                .iter()
                .any(|b| !b.extra_fields.is_empty() && b.lexostatus == source.lexostatus)
            {
                fouten.push(format!(
                    "{wie}: een andere bron die extra velden doorgeeft heet ook zo"
                ));
            }
        }
        for (name, v) in source
            .input
            .iter()
            .filter_map(|(n, i)| Some((n, i.field()?)))
        {
            let doorgever = sources
                .iter()
                .enumerate()
                .find(|(_, b)| !b.extra_fields.is_empty() && b.lexostatus == v.lexostatus);
            let Some((j, e)) = doorgever else { continue };
            if j >= i {
                fouten.push(format!(
                    "{wie}, invoer '{name}': bron {}/{} staat niet eerder in de lijst",
                    e.cell, e.lexostatus
                ));
            } else if !e.extra_fields.contains(&v.field) {
                fouten.push(format!(
                    "{wie}, invoer '{name}': bron {}/{} geeft geen extra veld '{}' door",
                    e.cell, e.lexostatus, v.field
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
    let streng = d.origin_check == crate::config::Herkomstcontrole::Strict;
    let synthesis = d.andere_bronnen().map(|b| {
        (
            format!("synthese-bron {}/{}", b.cell, b.lexostatus),
            &b.legal_basis,
            b.vertaalt(),
        )
    });
    let rows = d
        .portal
        .iter()
        .flat_map(|p| {
            p.assessment
                .rows
                .iter()
                .map(|r| ("assessment".to_string(), r))
        })
        .chain(d.handling.iter().flat_map(|b| &b.actions).flat_map(|h| {
            h.rows
                .iter()
                .map(move |r| (format!("handeling '{}'", h.name), r))
        }))
        .flat_map(|(waar, r)| {
            r.sources.iter().map(move |b| {
                (
                    format!(
                        "{waar}, rijen '{}', bron {}/{}",
                        r.parameter, b.cell, b.lexostatus
                    ),
                    &b.legal_basis,
                    b.vertaalt(),
                )
            })
        });
    let mut fouten = BTreeSet::new();
    for (wie, legal_basis, vertaalt) in synthesis.chain(rows) {
        for g in legal_basis {
            if let Err(f) = regelingen::geldig(service, g) {
                fouten.insert(format!("{wie}: {f}"));
            }
        }
        if streng && legal_basis.is_empty() && !vertaalt.is_empty() {
            fouten.insert(format!(
                "{wie}: vertaalt ({}) zonder grondslag; met origin_check: strict zegt de afnemer op welk artikel een vertaling rust",
                vertaalt.join("; ")
            ));
        }
    }
    fouten.into_iter().collect()
}

pub fn controleer(proces: &Proces) -> Vec<String> {
    let mut fouten = Vec::new();
    if proces.definitie.synthesis.is_empty() {
        return fouten;
    }
    let sources: Vec<&SyntheseBron> = proces.definitie.andere_bronnen().collect();
    let cell = &proces.cell;
    let service = proces.service.as_ref();
    fouten.extend(doorgeven(proces));
    let doorgevers: Vec<&str> = sources
        .iter()
        .filter(|b| !b.extra_fields.is_empty())
        .map(|b| b.lexostatus.as_str())
        .collect();
    // Wat de handelingen vragen: de parameters van hun artikelen, samen.
    let actions = proces.actions();
    let onder_besluit: BTreeSet<String> = actions
        .iter()
        .filter_map(|h| {
            let a = regelingen::article(service, &h.article).ok()?;
            Some(regelingen::transitieve_parameters(
                service,
                &h.regulation,
                a,
            ))
        })
        .flatten()
        .collect();
    let Some(portal) = proces.portal() else {
        if actions.is_empty() {
            fouten.push(
                "synthese zonder portaal en zonder handelingen: alleen de toets van een portaal en de handelingen in een zaak gebruiken haar"
                    .into(),
            );
            return fouten;
        }
        for source in &sources {
            let wie = format!("synthese-bron {}/{}", source.cell, source.lexostatus);
            if source.cell == cell.id() {
                fouten.push(format!(
                    "{wie}: een bron uit de cel waarin het proces vastlegt, is een bron van de zaak (zaak: true)"
                ));
            }
            for p in source
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
    let eigen = cell.lexostatuses.lexostatus(&portal.assessment.lexostatus);
    let onder_toets = service
        .resolver()
        .get_article_by_output(
            &portal.assessment.regulation,
            &portal.assessment.output,
            None,
        )
        .map(|a| regelingen::transitieve_parameters(service, &portal.assessment.regulation, a))
        .unwrap_or_default();
    // Wat het aanbod vraagt: de parameters van het aanbod-artikel.
    let onder_aanbod = portal
        .offer
        .as_ref()
        .and_then(|a| {
            let art = service
                .resolver()
                .get_article_by_output(&a.regulation, &a.output, None)?;
            Some(regelingen::transitieve_parameters(
                service,
                &a.regulation,
                art,
            ))
        })
        .unwrap_or_default();
    // parameter -> bronnen die hem leveren
    let mut per: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    if let Some(def) = eigen {
        for p in def.reduction.derivations.keys() {
            per.entry(p)
                .or_default()
                .push(format!("de eigen lexostatus '{}'", def.name));
        }
    }
    for source in &sources {
        let wie = format!("synthese-bron {}/{}", source.cell, source.lexostatus);
        if source.cell == cell.id() {
            fouten.push(format!(
                "{wie}: een bron uit de cel waarin het proces vastlegt, is een bron van de zaak (zaak: true)"
            ));
        }
        for (name, v) in source
            .input
            .iter()
            .filter_map(|(n, i)| Some((n, i.field()?)))
        {
            if doorgevers.contains(&v.lexostatus.as_str()) {
                // Doorgegeven door een eerdere bron: zie `doorgeven`.
            } else if v.lexostatus != portal.assessment.lexostatus {
                fouten.push(format!(
                    "{wie}, invoer '{name}': komt uit lexostatus '{}', maar de toets reduceert '{}'",
                    v.lexostatus, portal.assessment.lexostatus
                ));
            } else if !eigen.is_some_and(|d| d.levert(&v.field)) {
                fouten.push(format!(
                    "{wie}, invoer '{name}': lexostatus '{}' levert geen '{}' (geen afleiding en geen extra veld)",
                    v.lexostatus, v.field
                ));
            }
        }
        for p in &source.parameters {
            if !onder_toets.contains(p) && !onder_besluit.contains(p) && !onder_aanbod.contains(p) {
                fouten.push(format!(
                    "{wie}: '{p}' is geen parameter van {}#{} (de toets, '{}'), een handeling of het aanbod, of van een artikel dat een van die aanroept",
                    portal.assessment.regulation,
                    service
                        .resolver()
                        .get_article_by_output(&portal.assessment.regulation, &portal.assessment.output, None)
                        .map(|a| a.number.clone())
                        .unwrap_or_default(),
                    portal.assessment.output
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
fn lexostatus_van<'v>(cells: &'v Value, cell: &str, lexostatus: &str) -> Result<&'v Value, String> {
    let c = cells
        .as_array()
        .and_then(|a| {
            a.iter()
                .find(|c| c.get("id").and_then(Value::as_str) == Some(cell))
        })
        .ok_or_else(|| format!("de runtime van de bron heeft geen cel '{cell}'"))?;
    c.get("lexostatuses")
        .and_then(Value::as_array)
        .and_then(|a| {
            a.iter()
                .find(|l| l.get("name").and_then(Value::as_str) == Some(lexostatus))
        })
        .ok_or_else(|| format!("cel '{cell}' biedt geen lexostatus '{lexostatus}' aan"))
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
pub async fn warnings(proces: &str, sources: &[Bron]) -> Vec<String> {
    let mut uit = Vec::new();
    for source in sources {
        let d = &source.definitie;
        let wie = format!(
            "proces '{proces}': synthese-bron {}/{} ({})",
            d.cell,
            d.lexostatus,
            source.transport.soort()
        );
        let cells = match haal_binnen(source.transport.as_ref(), "/api/cells", TIJDSLIMIET).await {
            Ok(c) => c,
            Err(f) => {
                uit.push(format!("{wie}: nu niet te controleren, {f}"));
                continue;
            }
        };
        let lexo = match lexostatus_van(&cells, &d.cell, &d.lexostatus) {
            Ok(l) => l,
            Err(f) => {
                uit.push(format!("{wie}: {f}"));
                continue;
            }
        };
        if lexo.get("list") == Some(&Value::Bool(true)) {
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
        for i in inputs.iter().filter(|i| !d.input.contains_key(*i)) {
            uit.push(format!(
                "{wie}: de bron vraagt input '{i}', en de synthese geeft die niet"
            ));
        }
        for i in d.input.keys().filter(|i| !inputs.contains(*i)) {
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

    fn source(antwoord: Result<Value, TransportFout>) -> (Bron, Arc<Vast>) {
        let t = Arc::new(Vast::new(antwoord));
        let definitie: SyntheseBron = serde_json::from_value(json!({
            "cell": "register", "lexostatus": "status",
            "input": {"aanduiding": {"lexostatus": "eigen", "field": "aanduiding"}},
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
            "name": "eigen",
            "parameters": {"bevat_aanduiding": aanduiding.is_some()},
            "extra_fields": aanduiding.map(|a| json!({"aanduiding": a})).unwrap_or(json!({})),
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
        let t1 = vaste(json!({"name": "bron", "parameters": {}, "extra_fields": {"naam": "EEN"}}));
        let t2 = vaste(json!({"name": "bron", "parameters": {"aantal": 3}}));
        let b1 = Bron {
            definitie: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_nummer",
                "input": {"nummer": {"lexostatus": "eigen", "field": "nummer"}},
                "parameters": [], "extra_fields": ["naam"]
            }))
            .unwrap(),
            transport: t1.clone(),
        };
        let b2 = Bron {
            definitie: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_naam",
                "input": {"naam": {"lexostatus": "op_nummer", "field": "naam"}},
                "parameters": ["aantal"]
            }))
            .unwrap(),
            transport: t2.clone(),
        };
        let eigen: Lexostatus = serde_json::from_value(json!({
            "name": "eigen", "parameters": {}, "extra_fields": {"nummer": "12345678"}
        }))
        .unwrap();
        // De wachtende bron komt in de tweede ronde; de opstartcontrole eist
        // dat de doorgevende bron eerder in de lijst staat.
        let s = voeg_samen(&eigen, &[b1, b2], &Peil::default()).await;
        assert_eq!(
            t2.vragen()[0],
            "/cells/register/api/lexostatus/op_naam?naam=EEN"
        );
        assert_eq!(s.parameters.get("aantal"), Some(&json!(3)));
        assert!(!s.parameters.contains_key("naam"));
        assert_eq!(s.sources.len(), 2);
    }

    /// Een keten van drie: een nummer naar een naam, de naam naar een
    /// aanduiding, de aanduiding naar een feit. De synthese vraagt in rondes;
    /// een bron wacht tot haar invoer er is. Een vaste waarde gaat mee, en de
    /// afnemer vraagt het feit onder zijn eigen naam.
    #[tokio::test]
    async fn een_keten_van_bronnen_in_rondes_met_vertaling() {
        let t1 = vaste(json!({"name": "x", "parameters": {}, "extra_fields": {"naam": "EEN"}}));
        let t2 =
            vaste(json!({"name": "x", "parameters": {}, "extra_fields": {"aanduiding": "LIJST"}}));
        let t3 = vaste(
            json!({"name": "x", "parameters": {"is_ingeschreven_in_register": true},
                              "extra_fields": {"zetels_toegekend": 4}}),
        );
        let source = |t: &Arc<Vast>, d: Value| Bron {
            definitie: serde_json::from_value(d).unwrap(),
            transport: t.clone(),
        };
        // In omgekeerde volgorde: de rondes volgen uit de invoer.
        let sources = [
            source(
                &t3,
                json!({
                    "cell": "register", "lexostatus": "register",
                    "input": {"aanduiding": {"lexostatus": "op_naam", "field": "aanduiding"}, "orgaan": {"value": "raad"}},
                    "parameters": {"is_ingeschreven_in_register": "is_ingeschreven_raad", "zetels_toegekend": "zetels_op_lijst"}
                }),
            ),
            source(
                &t2,
                json!({
                    "cell": "register", "lexostatus": "op_naam",
                    "input": {"naam": {"lexostatus": "op_nummer", "field": "naam"}},
                    "parameters": [], "extra_fields": ["aanduiding"]
                }),
            ),
            source(
                &t1,
                json!({
                    "cell": "handelsregister", "lexostatus": "op_nummer",
                    "input": {"nummer": {"lexostatus": "eigen", "field": "nummer"}},
                    "parameters": [], "extra_fields": ["naam"]
                }),
            ),
        ];
        let eigen: Lexostatus = serde_json::from_value(json!({
            "name": "eigen", "parameters": {}, "extra_fields": {"nummer": "12345678"}
        }))
        .unwrap();
        let s = voeg_samen(&eigen, &sources, &Peil::default()).await;
        assert_eq!(
            t3.vragen()[0],
            "/cells/register/api/lexostatus/register?aanduiding=LIJST&orgaan=raad"
        );
        assert_eq!(s.parameters.get("is_ingeschreven_raad"), Some(&json!(true)));
        assert!(!s.parameters.contains_key("is_ingeschreven_in_register"));
        // Een extra veld van de bron is voor de afnemer een parameter als zijn
        // wet het vraagt.
        assert_eq!(s.parameters.get("zetels_op_lijst"), Some(&json!(4)));
        assert_eq!(
            s.provenance["is_ingeschreven_raad"],
            Herkomst::Cell {
                cell: "register".into(),
                lexostatus: "register".into(),
                transport: t3.soort().into()
            }
        );
        let volgorde: Vec<&str> = s.sources.iter().map(|b| b.lexostatus.as_str()).collect();
        assert_eq!(volgorde, ["op_nummer", "op_naam", "register"]);
    }

    /// Een bron die wacht op een bron die er niet is, wordt toch gevraagd, en
    /// meldt wat ontbreekt; er wordt niets aangevuld.
    #[tokio::test]
    async fn een_bron_die_op_niets_kan_wachten() {
        let t = vaste(json!({"name": "x", "parameters": {"a": 1}}));
        let b = Bron {
            definitie: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "l",
                "input": {"naam": {"lexostatus": "bestaat_niet", "field": "naam"}},
                "parameters": ["a"]
            }))
            .unwrap(),
            transport: t.clone(),
        };
        let eigen = Lexostatus::leeg("eigen");
        let s = voeg_samen(&eigen, &[b], &Peil::default()).await;
        assert!(t.vragen().is_empty());
        assert_eq!(s.sources[0].status, Status::NotQueried);
        assert!(s.sources[0]
            .error
            .as_deref()
            .unwrap()
            .contains("invoer 'naam' ontbreekt"));
        assert!(!s.parameters.contains_key("a"));
    }

    /// Geeft de eerste bron niets door, dan wordt de volgende niet bevraagd,
    /// en er wordt niets aangevuld.
    #[tokio::test]
    async fn zonder_doorgegeven_veld_geen_vraag() {
        let t1 = vaste(json!({"name": "bron", "parameters": {}}));
        let t2 = vaste(json!({"name": "bron", "parameters": {"aantal": 3}}));
        let b1 = Bron {
            definitie: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_nummer",
                "input": {"nummer": {"lexostatus": "eigen", "field": "nummer"}},
                "parameters": [], "extra_fields": ["naam"]
            }))
            .unwrap(),
            transport: t1,
        };
        let b2 = Bron {
            definitie: serde_json::from_value(json!({
                "cell": "register", "lexostatus": "op_naam",
                "input": {"naam": {"lexostatus": "op_nummer", "field": "naam"}},
                "parameters": ["aantal"]
            }))
            .unwrap(),
            transport: t2.clone(),
        };
        let eigen: Lexostatus = serde_json::from_value(json!({
            "name": "eigen", "parameters": {}, "extra_fields": {"nummer": "12345678"}
        }))
        .unwrap();
        let s = voeg_samen(&eigen, &[b1, b2], &Peil::default()).await;
        assert!(t2.vragen().is_empty());
        assert!(!s.parameters.contains_key("aantal"));
        assert_eq!(s.sources[1].status, Status::NotQueried);
    }

    #[tokio::test]
    async fn samenvoegen_met_herkomst() {
        let (b, t) = source(Ok(
            json!({"name": "bron", "parameters": {"ingeschreven": true, "zetels": 6, "anders": 1}}),
        ));
        let s = voeg_samen(&eigen(Some("EEN & ANDER")), &[b], &Peil::default()).await;
        assert_eq!(
            t.vragen()[0],
            "/cells/register/api/lexostatus/status?aanduiding=EEN+%26+ANDER"
        );
        assert_eq!(s.parameters["zetels"], json!(6));
        // Alleen de verwachte parameters, geen wildcard.
        assert!(!s.parameters.contains_key("anders"));
        // De aanduiding is een extra veld en gaat niet naar de engine.
        assert!(!s.parameters.contains_key("aanduiding"));
        assert_eq!(
            s.provenance["bevat_aanduiding"],
            Herkomst::Own {
                lexostatus: "eigen".into()
            }
        );
        assert_eq!(
            serde_json::to_value(&s.provenance["ingeschreven"]).unwrap(),
            json!({"source": "cell", "cell": "register", "lexostatus": "status", "transport": "internal"})
        );
        assert_eq!(s.sources[0].status, Status::Queried);
        assert_eq!(s.reason(), None);
    }

    #[tokio::test]
    async fn onbereikbare_bron_vult_niets_aan() {
        let (b, _) = source(Err(TransportFout::Unreachable("weg".into())));
        let s = voeg_samen(&eigen(Some("X")), &[b], &Peil::default()).await;
        assert_eq!(
            s.parameters.keys().collect::<Vec<_>>(),
            ["bevat_aanduiding"]
        );
        assert_eq!(s.sources[0].status, Status::Unreachable);
        assert_eq!(
            s.reason().as_deref(),
            Some("niet te beoordelen: bron register onbereikbaar")
        );
    }

    #[tokio::test]
    async fn ontbrekende_invoer_vraagt_de_bron_niet() {
        let (b, t) = source(Ok(json!({"name": "bron", "parameters": {}})));
        let s = voeg_samen(&eigen(None), &[b], &Peil::default()).await;
        assert!(t.vragen().is_empty());
        assert_eq!(s.sources[0].status, Status::NotQueried);
        assert!(s
            .reason()
            .unwrap()
            .contains("invoer 'aanduiding' ontbreekt"));
    }

    #[tokio::test]
    async fn niet_geleverde_parameter_wordt_genoemd() {
        let (b, _) = source(Ok(
            json!({"name": "bron", "parameters": {"ingeschreven": false}}),
        ));
        let s = voeg_samen(&eigen(Some("X")), &[b], &Peil::default()).await;
        assert_eq!(s.sources[0].not_delivered, ["zetels"]);
        assert!(!s.parameters.contains_key("zetels"));
    }

    #[test]
    fn pad_zonder_invoer() {
        let nu = Peil::default();
        assert_eq!(
            path("a", "b", &Map::new(), &nu),
            "/cells/a/api/lexostatus/b"
        );
        let i = json!({"jaar": 2025}).as_object().unwrap().clone();
        assert_eq!(
            path("a", "b", &i, &nu),
            "/cells/a/api/lexostatus/b?jaar=2025"
        );
    }

    #[test]
    fn pad_met_peil() {
        let peil = Peil {
            as_of: Some(crate::datum::Tijdpunt::lees("p", "2027-01-01").unwrap()),
            known_at: Some(crate::datum::Tijdpunt::lees("b", "2026-09-25T10:00:00+02:00").unwrap()),
        };
        let i = json!({"jaar": 2025}).as_object().unwrap().clone();
        assert_eq!(
            path("a", "b", &i, &peil),
            "/cells/a/api/lexostatus/b?as_of=2027-01-01&jaar=2025&known_at=2026-09-25T10%3A00%3A00%2B02%3A00"
        );
    }
}
