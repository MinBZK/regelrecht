//! Handelingen in een zaak: op proef, zonder vastleggen, en genomen, waarna
//! het proces de cel laat vastleggen.
//!
//! `behandeling.handelingen` in `proces.yaml` noemt per handeling een artikel
//! (een regeling en uitkomsten) en het event waarin de cel haar vastlegt. Wat
//! een handeling nodig heeft en van wie, staat daar niet: het volgt uit de
//! stage van het event (RFC-008) en uit de origin van de parameters (RFC-043).
//! Er zijn drie soorten ([`Handelingsoort`]), en het event zegt welke:
//!
//! - **Het besluit**: het event heeft een stage die de procedure van het
//!   artikel kent, en het is de eerste zo'n handeling op dat artikel. Het
//!   formulier zijn de oordelen (origin `OORDEEL`); wat een latere stage pas
//!   vraagt, is nog niet gebeurd ([`nog_niet`]).
//! - **Een vervolg**: een latere stage van hetzelfde besluit, zoals de
//!   bekendmaking. De engine voert die stage uit op de invoer van het
//!   vastgelegde besluit (RFC-008, `execute_stage`), met wat de stage vraagt
//!   (`requires`) als formulier; de haken die de wet op die stage laat vuren
//!   (zoals de bezwaartermijn, Awb 6:8) leveren hun uitkomsten mee.
//! - **Een feit** uit het verloop van de zaak: het event heeft geen stage.
//!   Het formulier zijn de velden van het event die geen uitkomst zijn; op
//!   proef laat de cel de lexostatussen van de zaak reduceren alsof het feit
//!   al vastlag, zodat de uitkomsten laten zien wat het feit doet.
//!
//! Een parameter komt uit precies een bron: een lexostatus van de zaak, een
//! andere synthese-bron, de synthese per regel, het formulier of de stand van
//! wat nog niet gebeurd is. Er wordt niets aangevuld. Staat het artikel van
//! de handeling in de grondslag van het event en is het een TOETS, dan telt
//! elke booleaanse uitkomst: onwaar is een conclusie van het proces
//! ([`toetsen`]). Zo zegt de wet dat een betaling boven het vastgestelde
//! bedrag niet overeenkomstig de vaststelling is (Awb 4:52), niet de
//! configuratie.
//!
//! Het proces concludeert voor het handelt, en weigert niets wat gebeurd is.
//! Zegt de proef inhoudelijk nee (een toets is onwaar, een haak geeft geen
//! waarde), dan doet het
//! proces de handeling niet uit zichzelf (`te_nemen` is onwaar). Meldt de
//! behandelaar dat het feit toch gebeurde (`gebeurd: true`), dan legt de cel
//! het vast, en tonen de lexostatussen de gevolgen: een betaling boven het
//! bedrag is onverschuldigd betaald, een bekendmaking die niet aan de wet
//! voldoet laat geen bezwaartermijn lopen. Alleen wat de vorm raakt, houdt
//! het vastleggen tegen: een formulier dat niet is ingevuld, een uitkomst
//! die de wet niet volledig kan uitrekenen (een waarde of een bron mist),
//! een vervolg zonder besluit, een moment in de toekomst of voor de zaak. Een besluit
//! neemt het proces zelf; dat wordt niet gemeld.
//!
//! Het proces leest de zaak niet: wat het over de zaak weet (welke stages er
//! liggen, wat het besluit vastlegde, hoeveel grammen), leidt de cel af in
//! haar lexostatus [`Zaakstand`].

use std::collections::{BTreeMap, BTreeSet};

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use serde_json::{Map, Value};

use regelrecht_engine::{ExecutionOutcome, LawExecutionService, StageState, Value as EngineValue};
use regelrecht_law_model::ProcedureDefinition;

use crate::cel::Cel;
use crate::celclient::{self, Besluitvelden, MetYaml, Vastlegverzoek};
use crate::config::{HandelingDefinitie, Handelingsoort, NogNiet, ProcesDefinitie};
use crate::datum::{self, Tijdpunt};
use crate::formulier::Veld;
use crate::gezag::{self, Bevoegdheid};
use crate::gram::{GeladenRegeling, Gram, HandelendeActor, Invoer, Receipt, StroomVerwijzing};
use crate::kanaal::Sessie;
use crate::proces::Proces;
use crate::reductie::{Besluitstand, Lexostatus, Peil, Zaakstand};
use crate::regelingen::{self, Benodigd};
use crate::rijen::{self, Rijen};
use crate::stroom::{Besluit, Binding, Event, Zaak};
use crate::synthese::{self, Bron, BronUitslag, Herkomst};
use crate::toets;
use crate::transport::{Transport, TransportFout};

/// Het antwoord op een handeling op proef.
#[derive(Debug, Clone, Serialize)]
pub struct Proefhandeling {
    pub handeling: String,
    #[serde(flatten)]
    pub soort: Handelingsoort,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub regeling: String,
    /// `<regeling>#<artikel>` van de uitkomsten.
    pub artikel: String,
    /// De dag waarop de engine de regeling leest en de cellen peilen.
    pub peildatum: String,
    /// Waar de peildatum vandaan komt: het `op_moment` van het event, met
    /// zijn grondslag, of vandaag.
    pub peildatum_uit: String,
    /// Of het proces de handeling uit zichzelf neemt: elke uitkomst heeft een
    /// waarde, elke toets is waar en elk feit is ingevuld.
    pub te_nemen: bool,
    /// Niet te nemen om de inhoud, niet om de vorm: meldt de behandelaar dat
    /// het feit toch gebeurde (`gebeurd: true`), dan legt de cel het vast.
    /// Nooit bij een besluit.
    pub te_melden: bool,
    /// De uitkomsten met een waarde, ook als de handeling niet te nemen is.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub uitkomsten: BTreeMap<String, Value>,
    /// De toetsen van het artikel (zie [`toetsen`]) met hun waarde.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub toetsen: BTreeMap<String, Value>,
    /// Wat de engine miste.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub mist: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reden: Option<String>,
    /// Wat naar de engine ging, en per parameter waar het vandaan kwam.
    pub parameters: BTreeMap<String, Value>,
    pub herkomst: BTreeMap<String, Herkomst>,
    pub bronnen: Vec<BronUitslag>,
    /// Parameters die de aanroeper van het artikel moet leveren, zonder
    /// waarde uit een bron.
    pub niet_geleverd: Vec<Benodigd>,
    /// De lexostatussen van de zaak, bij een feit met het concept erbij.
    pub lexostatussen: Vec<Lexostatus>,
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rijen: Vec<rijen::Uitslag>,
    /// Het vastgelegde besluit waarop de handeling handelt: bij een vervolg
    /// het besluit waarop de stage verdergaat, bij een feit het besluit dat
    /// het volgt (zoals de betaling die het uitvoert), bij een wijziging het
    /// besluit dat zij wijzigt.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub besluit: Option<BesluitVerwijzing>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// Een vastgelegd besluit in de zaak, zoals de cel het in de [`Zaakstand`]
/// noemt.
#[derive(Debug, Clone, Serialize)]
pub struct BesluitVerwijzing {
    pub besluitkenmerk: String,
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    pub op_moment: String,
    pub vastgelegd_op: String,
}

/// Een fout in de vraag of een stand die de handeling niet toelaat.
#[derive(Debug, Clone, PartialEq)]
pub enum Weigering {
    /// Het formulier noemt iets dat de handeling niet vraagt, of een waarde
    /// die de cel niet in een gram kan zetten.
    Ongeldig(String),
    /// Niet te nemen: iets mist, een toets is onwaar, of een feit is niet
    /// ingevuld. Er wordt geen gram vastgelegd.
    NietTeNemen(String),
    /// De cel weigert: de stage ligt al vast in de zaak, of de zaak veranderde
    /// sinds het proces haar las.
    Conflict(String),
    /// De wet wijst een ander bevoegd gezag aan dan de actor van het proces.
    Onbevoegd(String),
    /// De configuratie, of de cel: haar kroniek, een reductie of het
    /// vastleggen.
    Cel(String),
}

/// Wat een handeling nodig heeft van de runtime: de cel, de bronnen en de
/// synthese per regel van deze handeling, de geladen regelingen (voor het
/// receipt) en het moment van nu.
pub struct Omgeving<'a> {
    pub proces: &'a Proces,
    pub cel: &'a dyn Transport,
    pub bronnen: &'a [Bron],
    pub rijen: &'a [Rijen],
    pub regelingen: &'a [GeladenRegeling],
    pub nu: DateTime<FixedOffset>,
}

// --- Bij het laden ---------------------------------------------------------

/// Het artikel van een regeling met deze uitkomst, als `<regeling>#<artikel>`.
fn artikel_met(service: &LawExecutionService, regeling: &str, uitkomst: &str) -> Option<String> {
    service
        .resolver()
        .get_article_by_output(regeling, uitkomst, None)
        .map(|a| format!("{regeling}#{}", a.number))
}

/// De parameters die de aanroeper van het artikel van een handeling moet
/// leveren.
pub fn benodigd(
    service: &LawExecutionService,
    h: &HandelingDefinitie,
) -> Result<BTreeMap<String, Benodigd>, String> {
    let a = regelingen::artikel(service, &h.artikel)?;
    Ok(regelingen::benodigde_parameters(service, &h.regeling, a))
}

/// De procedure (RFC-008) van het rechtskarakter dat een artikel produceert.
pub fn procedure_van<'s>(
    service: &'s LawExecutionService,
    artikel: &str,
) -> Option<&'s ProcedureDefinition> {
    let a = regelingen::artikel(service, artikel).ok()?;
    let p = a.get_produces()?;
    service
        .resolver()
        .find_procedure(p.legal_character.as_deref()?, p.procedure_id.as_deref())
}

/// De haken die de wet laat vuren op een stage van een rechtskarakter
/// (RFC-007, RFC-008), als `<regeling>#<artikel>`, gesorteerd.
pub fn haken_op(service: &LawExecutionService, legal_character: &str, stage: &str) -> Vec<String> {
    let mut uit = Vec::new();
    for id in service.list_laws() {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        for a in &law.articles {
            let vuurt = a
                .machine_readable
                .as_ref()
                .and_then(|m| m.hooks.as_ref())
                .is_some_and(|hooks| {
                    hooks.iter().any(|h| {
                        h.applies_to.legal_character.as_deref() == Some(legal_character)
                            && h.applies_to.stage.as_deref().unwrap_or("BESLUIT") == stage
                    })
                });
            if vuurt {
                uit.push(format!("{id}#{}", a.number));
            }
        }
    }
    uit.sort();
    uit.dedup();
    uit
}

/// De uitkomsten van een artikel, in de volgorde van declaratie.
fn uitkomsten_van(service: &LawExecutionService, artikel: &str) -> Vec<String> {
    regelingen::artikel(service, artikel)
        .ok()
        .and_then(|a| a.get_execution_spec())
        .and_then(|e| e.output.as_ref())
        .map(|o| o.iter().map(|o| o.name.clone()).collect())
        .unwrap_or_default()
}

/// De toetsen van een handeling die een besluit uitvoert: de booleaanse
/// uitkomsten van een TOETS-artikel die de norm zijn van de grondslag van
/// het event. Alleen bij een executogram: dat is de levering of afhandeling
/// die een besluit uitvoert (positionpaper, P:54). Zegt de toets van precies
/// die bepaling nee, dan zou de levering niet op die grondslag gebeuren, en
/// het proces doet haar dan niet uit zichzelf: een betaling boven de
/// subsidievaststelling is geen betaling "overeenkomstig de
/// subsidievaststelling" (Awb 4:52 lid 1). Het is een conclusie van het
/// proces voor het handelt, geen weigering van de cel: gebeurde de levering
/// toch, dan legt de cel haar vast (zie [`neem`]). Een uitkomst is de norm
/// van een grondslag als haar `legal_basis` het artikel noemt, en het lid als
/// de grondslag er een noemt. Een vaststelling of een oordeel (zoals over
/// verzuim) is geen uitvoering: wat die op haar grondslag uitwerkt, is juist
/// wat zij vastlegt.
pub fn toetsen(service: &LawExecutionService, artikel: &str, event: &Event) -> Vec<String> {
    if event.type_ != "executogram" {
        return Vec::new();
    }
    let Ok(a) = regelingen::artikel(service, artikel) else {
        return Vec::new();
    };
    if a.get_produces().and_then(|p| p.legal_character.as_deref()) != Some("TOETS") {
        return Vec::new();
    }
    let leden: Vec<Option<String>> = event
        .grondslag
        .iter()
        .filter_map(|g| regelingen::ontleed(g).ok())
        .filter(|o| format!("{}#{}", o.regeling, o.artikel) == artikel)
        .map(|o| o.lid.map(str::to_string))
        .collect();
    if leden.is_empty() {
        return Vec::new();
    }
    a.get_execution_spec()
        .and_then(|e| e.output.as_ref())
        .map(|o| {
            o.iter()
                .filter(|o| {
                    serde_json::to_value(o.output_type).ok() == Some(Value::from("boolean"))
                })
                .filter(|o| {
                    let Some(lb) = &o.legal_basis else {
                        return false;
                    };
                    lb.article.as_deref().is_none_or(|x| x == a.number)
                        && leden.iter().any(|lid| match lid {
                            None => true,
                            Some(l) => lb.paragraph.as_deref() == Some(l.as_str()),
                        })
                })
                .map(|o| o.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Wat bij een besluit nog niet gebeurd is, uit de wet: de parameters die de
/// procedure van het rechtskarakter (RFC-008) pas vraagt in een stage na die
/// van het vastleg-event. Een boolean is onwaar, al het andere leeg. Wat een
/// lexostatus van de zaak al afleidt, staat hier niet: geen gram is dan "niet
/// gebeurd", en dat zegt de cel zelf (een parameter komt uit een bron).
pub fn nog_niet(
    service: &LawExecutionService,
    h: &HandelingDefinitie,
    procedure: &ProcedureDefinition,
    uit_de_zaak: &BTreeSet<String>,
) -> BTreeMap<String, NogNiet> {
    let mut uit = BTreeMap::new();
    let Some(stage) = &h.stage else {
        return uit;
    };
    let Some(i) = procedure.stages.iter().position(|s| &s.name == stage) else {
        return uit;
    };
    let Ok(benodigd) = benodigd(service, h) else {
        return uit;
    };
    for later in &procedure.stages[i + 1..] {
        for r in later.requires.iter().flatten() {
            let Some(p) = benodigd.get(&r.name) else {
                continue;
            };
            if uit_de_zaak.contains(&r.name) {
                continue;
            }
            let waarde = if p.soort.as_str() == Some("boolean") {
                Value::Bool(false)
            } else {
                Value::Null
            };
            uit.entry(r.name.clone()).or_insert(NogNiet {
                waarde,
                stage: later.name.clone(),
            });
        }
    }
    uit
}

/// Bereid de handelingen van een proces voor, bij het laden: de regeling (de
/// beschikking van het gezag waarvoor het proces handelt, `namens`, als de
/// handeling er geen noemt), het artikel,
/// de stage, de soort, de haken van een vervolg, de toetsen en wat bij een
/// besluit nog niet gebeurd is. Het formulier volgt later, uit de controle
/// op de herkomst (zie [`zet_formulier`]).
pub fn bereid_voor(
    d: &mut ProcesDefinitie,
    gezag: Option<&str>,
    service: &LawExecutionService,
    cel: &Cel,
) -> Vec<String> {
    let mut fouten = Vec::new();
    let uit_de_zaak: BTreeSet<String> = d
        .zaakbronnen()
        .filter_map(|b| cel.lexostatussen.lexostatus(&b.lexostatus))
        .flat_map(|l| l.reduction.afleidingen.keys().cloned())
        .collect();
    let Some(behandeling) = d.behandeling.as_mut() else {
        return fouten;
    };
    let mut namen = BTreeSet::new();
    for h in &mut behandeling.handelingen {
        let wie = format!("handeling '{}'", h.naam);
        if !namen.insert(h.naam.clone()) {
            fouten.push(format!("{wie}: de naam staat er meer dan een keer"));
        }
        let Some((_, event)) = cel.event(&h.vastleggen.stroom, &h.vastleggen.event) else {
            fouten.push(format!(
                "{wie}, vastleggen {}/{}: die stroom of dat event bestaat niet",
                h.vastleggen.stroom, h.vastleggen.event
            ));
            continue;
        };
        h.stage = event.stage.clone();
        h.besluitrol = event.besluit;
        // De regeling: genoemd, of de beschikking waarvoor het gezag van het
        // proces bevoegd is.
        let mut beschikking = None;
        if h.regeling.is_empty() {
            // Zonder gezag meldt de controle op `namens` het al.
            let Some(actor) = gezag else {
                continue;
            };
            let kandidaten = gezag::beschikkingen_van(service, actor);
            match kandidaten.as_slice() {
                [(r, a)] => {
                    h.regeling = r.clone();
                    beschikking = Some(format!("{r}#{a}"));
                }
                [] => {
                    fouten.push(format!(
                        "{wie}: geen regeling noemt '{actor}' als bevoegd gezag bij een BESCHIKKING; noem de regeling in de handeling"
                    ));
                    continue;
                }
                meer => {
                    let lijst: Vec<String> = meer.iter().map(|(r, a)| format!("{r}#{a}")).collect();
                    fouten.push(format!(
                        "{wie}: '{actor}' is bevoegd voor meer dan een beschikking ({}); kies er een met regeling",
                        lijst.join(", ")
                    ));
                    continue;
                }
            }
        }
        // Het artikel: dat van de eerste uitkomst, of de beschikking.
        let artikel = match h.uitkomsten.first() {
            Some(u) => artikel_met(service, &h.regeling, u),
            None => beschikking.clone(),
        };
        let Some(artikel) = artikel else {
            fouten.push(match h.uitkomsten.first() {
                Some(u) => format!("{wie}: regeling '{}' heeft geen uitkomst '{u}'", h.regeling),
                None => format!("{wie}: noem een uitkomst van regeling '{}'", h.regeling),
            });
            continue;
        };
        if let Some(b) = &beschikking {
            if b != &artikel {
                fouten.push(format!(
                    "{wie}: uitkomst '{}' komt niet uit {b}, de beschikking waarvoor '{}' bevoegd is",
                    h.uitkomsten[0],
                    gezag.unwrap_or_default()
                ));
            }
        }
        h.artikel = artikel;
        h.toetsen = toetsen(service, &h.artikel, event)
            .into_iter()
            .filter(|t| !h.uitkomsten.contains(t))
            .collect();
    }
    // De soort: per artikel met een procedure is de vroegste stage het
    // besluit, elke latere een vervolg.
    let lijst = &mut behandeling.handelingen;
    let mut vroegste: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for h in lijst.iter() {
        let (Some(stage), Some(p)) = (&h.stage, procedure_van(service, &h.artikel)) else {
            continue;
        };
        let Some(i) = p.stages.iter().position(|s| &s.name == stage) else {
            continue;
        };
        let e = vroegste
            .entry(h.artikel.clone())
            .or_insert((i, h.naam.clone()));
        if i < e.0 {
            *e = (i, h.naam.clone());
        }
    }
    for h in lijst.iter_mut() {
        let wie = format!("handeling '{}'", h.naam);
        let Some(stage) = h.stage.clone() else {
            h.soort = Handelingsoort::Feit;
            continue;
        };
        let Some(p) = procedure_van(service, &h.artikel) else {
            // Zonder procedure: een besluit zonder stand van wat later komt.
            h.soort = Handelingsoort::Besluit;
            continue;
        };
        if !p.stages.iter().any(|s| s.name == stage) {
            fouten.push(format!(
                "{wie}, vastleggen {}/{}: stage '{stage}' staat niet in procedure '{}' van {} ({})",
                h.vastleggen.stroom,
                h.vastleggen.event,
                p.id,
                h.artikel,
                p.stages
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            continue;
        }
        let besluit = vroegste.get(&h.artikel).map(|(_, n)| n.clone());
        if besluit.as_deref() == Some(h.naam.as_str()) {
            h.soort = Handelingsoort::Besluit;
            h.nog_niet = nog_niet(service, h, p, &uit_de_zaak);
        } else if let Some(besluit) = besluit {
            h.soort = Handelingsoort::Vervolg {
                besluit,
                procedure: p.id.clone(),
            };
            let lc = regelingen::artikel(service, &h.artikel)
                .ok()
                .and_then(|a| a.get_produces())
                .and_then(|p| p.legal_character.clone())
                .unwrap_or_default();
            h.haken = haken_op(service, &lc, &stage);
            for haak in &h.haken {
                for u in uitkomsten_van(service, haak) {
                    if !h.uitkomsten.contains(&u) {
                        h.uitkomsten.push(u);
                    }
                }
            }
            h.toetsen.clear();
        }
    }
    fouten
}

/// Het soort veld van een formulier bij een type uit de regeling. Een
/// `amount` is een bedrag in eurocent, zoals de corpora het met
/// `type_spec.unit: eurocent` declareren; de frontend vraagt het in euro.
pub fn veldsoort(soort: &Value) -> Option<String> {
    soort.as_str().map(|t| {
        match t {
            "boolean" => "janee",
            "date" => "datum",
            "amount" => "bedrag",
            "number" => "getal",
            _ => "tekst",
        }
        .to_string()
    })
}

/// Een naam leesbaar: `datum_betaling` wordt "Datum betaling".
fn leesbaar(naam: &str) -> String {
    let tekst = naam.replace('_', " ");
    let mut tekens = tekst.chars();
    match tekens.next() {
        Some(eerste) => eerste.to_uppercase().chain(tekens).collect(),
        None => tekst,
    }
}

/// Zet het formulier van elke handeling, na de controle op de herkomst: de
/// oordelen komen daaruit (zie [`crate::origin::oordelen`]). De feiten zijn
/// bij een vervolg wat de stage vraagt (`requires`), en anders de velden van
/// het event die geen uitkomst en geen oordeel zijn. Het type van een feit
/// komt uit de wet: uit de parameter die een afleiding van de zaak uit dat
/// veld maakt, of uit de stage; een veld dat alleen het `op_moment` bindt, is
/// een datum.
pub fn zet_formulier(d: &mut ProcesDefinitie, service: &LawExecutionService, cel: &Cel) {
    let Some(behandeling) = d.behandeling.as_mut() else {
        return;
    };
    let besluiten: BTreeMap<String, String> = behandeling
        .handelingen
        .iter()
        .map(|h| (h.naam.clone(), h.artikel.clone()))
        .collect();
    for h in &mut behandeling.handelingen {
        let Some((stroom, event)) = cel.event(&h.vastleggen.stroom, &h.vastleggen.event) else {
            continue;
        };
        h.feiten = match &h.soort {
            Handelingsoort::Vervolg { besluit, .. } => {
                let benodigd = besluiten
                    .get(besluit)
                    .and_then(|a| regelingen::artikel(service, a).ok())
                    .map(|a| regelingen::benodigde_parameters(service, &h.regeling, a))
                    .unwrap_or_default();
                let stage = procedure_van(service, &h.artikel)
                    .and_then(|p| p.stages.iter().find(|s| Some(&s.name) == h.stage.as_ref()));
                stage
                    .into_iter()
                    .flat_map(|s| s.requires.iter().flatten())
                    .map(|r| {
                        let b = benodigd.get(&r.name);
                        Veld {
                            naam: r.name.clone(),
                            label: b
                                .and_then(|b| b.omschrijving.as_deref())
                                .map(crate::origin::label_uit)
                                .unwrap_or_else(|| leesbaar(&r.name)),
                            soort: veldsoort(
                                &serde_json::to_value(r.req_type).unwrap_or(Value::Null),
                            ),
                            opties: None,
                            kolommen: None,
                            uitleg: None,
                            groep: None,
                            grondslag: Vec::new(),
                        }
                    })
                    .collect()
            }
            _ => {
                let oordelen: Vec<&str> = h.oordelen.iter().map(|o| o.parameter.as_str()).collect();
                event
                    .external_sleutels()
                    .into_iter()
                    .filter(|k| !h.uitkomsten.contains(k) && !oordelen.contains(&k.as_str()))
                    .map(|k| feitveld(service, cel, stroom, event, &k))
                    .collect()
            }
        };
    }
}

/// Het formulierveld van een feit: het `$external`-veld `sleutel` van het
/// event, met het type van de parameter die een lexostatus van de cel eruit
/// afleidt.
fn feitveld(
    service: &LawExecutionService,
    cel: &Cel,
    stroom: &crate::stroom::Stroom,
    event: &Event,
    sleutel: &str,
) -> Veld {
    let mut soort = None;
    let mut uitleg = None;
    // Welk veld van het gram bindt aan deze sleutel?
    let paden: Vec<String> = event
        .bladeren()
        .into_iter()
        .filter(|b| matches!(&b.binding, Binding::External(s) if s == sleutel))
        .map(|b| b.pad)
        .collect();
    'zoek: for def in &cel.lexostatussen.lexostatus_definitions {
        if def.reduction.kroniek != stroom.chronicle {
            continue;
        }
        for (naam, a) in def.alle_afleidingen() {
            if !a
                .gelezen_paden()
                .iter()
                .any(|p| paden.iter().any(|q| q == *p))
            {
                continue;
            }
            // De parameter met deze naam, in de grondslag van het event of
            // van de afleiding.
            for g in event.grondslag.iter().chain(a.grondslag.iter()) {
                let Ok(art) = regelingen::artikel(service, g) else {
                    continue;
                };
                if let Some(p) = art.get_parameters().iter().find(|p| &p.name == naam) {
                    soort = veldsoort(&serde_json::to_value(p.param_type).unwrap_or(Value::Null));
                    uitleg = p.description.clone();
                    break 'zoek;
                }
            }
        }
    }
    let op_moment = event
        .op_moment
        .as_ref()
        .is_some_and(|b| matches!(b.binding(), Binding::External(s) if s == sleutel));
    if soort.is_none() && op_moment {
        soort = Some("datum".into());
        uitleg = event.op_moment.as_ref().map(|b| {
            format!(
                "Het moment waarop het feit rechtens plaatsvond ({}).",
                b.grondslag.join(", ")
            )
        });
    }
    Veld {
        naam: sleutel.to_string(),
        label: leesbaar(sleutel),
        soort,
        opties: None,
        kolommen: None,
        uitleg,
        groep: None,
        grondslag: event.grondslag.clone(),
    }
}

/// De controles op `behandeling` van een proces bij het opstarten. Een fout
/// hier houdt de runtime tegen:
///
/// - een handeling die een rol noemt, noemt een rol die de behandeling mag;
/// - de werkvoorraad is een lijst-lexostatus van de cel;
/// - een bron van de zaak vraagt een behandeling;
/// - de uitkomsten van een handeling komen uit een en hetzelfde artikel (bij
///   een vervolg ook uit de haken van die stage);
/// - de lexostatussen van de zaak bestaan, zijn geen lijst en hebben als
///   enige input `zaakkenmerk`;
/// - elke parameter uit het formulier, de stand van wat nog niet gebeurd is
///   of een `rijen`-blok is een parameter die de aanroeper van het artikel
///   moet leveren, en komt uit maar een bron;
/// - het vastleg-event volgt een zaak; bij een besluit legt het elke uitkomst
///   vast, bij een vervolg wat de stage vraagt en de uitkomsten van de
///   haken; elk veld is een uitkomst of een veld van het formulier.
pub fn controleer(proces: &Proces) -> Vec<String> {
    let mut fouten = Vec::new();
    let d = &proces.definitie;
    let cel = &proces.cel;
    let Some(behandeling) = &d.behandeling else {
        for b in d.zaakbronnen() {
            fouten.push(format!(
                "synthese-bron {}/{}: een bron van de zaak (zaak: true) vraagt een behandeling; de toets leest het concept",
                b.cel, b.lexostatus
            ));
        }
        return fouten;
    };
    match cel
        .lexostatussen
        .lexostatus(&behandeling.werkvoorraad.lexostatus)
    {
        None => fouten.push(format!(
            "behandeling: werkvoorraad '{}' is geen lexostatus van de cel",
            behandeling.werkvoorraad.lexostatus
        )),
        Some(l) if !l.is_lijst() => fouten.push(format!(
            "behandeling: werkvoorraad '{}' is geen lijst (groepeer: zaakkenmerk)",
            l.name
        )),
        Some(_) => {}
    }

    // De lexostatussen van de zaak: een keer, voor alle handelingen.
    let zaak: Vec<&String> = d.zaakbronnen().map(|z| &z.lexostatus).collect();
    let mut uit_de_zaak: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for naam in zaak.iter().copied() {
        match cel.lexostatussen.lexostatus(naam) {
            None => fouten.push(format!("behandeling: lexostatus '{naam}' bestaat niet")),
            Some(l) => {
                if l.is_lijst() {
                    fouten.push(format!(
                        "behandeling: lexostatus '{naam}' is een lijst, en een lijst gaat nooit naar de engine"
                    ));
                }
                let inputs: Vec<&str> = l.inputs.iter().map(|i| i.name.as_str()).collect();
                if inputs != ["zaakkenmerk"] {
                    fouten.push(format!(
                        "behandeling: lexostatus '{naam}' heeft inputs [{}]; een handeling geeft alleen 'zaakkenmerk' mee",
                        inputs.join(", ")
                    ));
                }
                for p in l.reduction.afleidingen.keys() {
                    uit_de_zaak
                        .entry(p)
                        .or_default()
                        .push(format!("de eigen lexostatus '{naam}'"));
                }
            }
        }
    }
    // Een invoer uit een eerdere bron (een doorgegeven extra veld) komt niet
    // uit een eigen lexostatus; de synthese zelf controleert haar.
    let doorgegeven: Vec<&str> = d
        .andere_bronnen()
        .filter(|s| !s.extra_velden.is_empty())
        .map(|s| s.lexostatus.as_str())
        .collect();
    let mut invoer_uit: Vec<&str> = d
        .andere_bronnen()
        .flat_map(|s| s.invoer.values().filter_map(|v| v.veld()))
        .map(|v| v.lexostatus.as_str())
        .filter(|l| !doorgegeven.contains(l))
        .collect();
    invoer_uit.sort_unstable();
    invoer_uit.dedup();
    if invoer_uit.len() > 1 {
        fouten.push(format!(
            "behandeling: de invoer van de synthese komt uit meer dan een lexostatus ({}); een handeling geeft haar uit een",
            invoer_uit.join(", ")
        ));
    }
    for bron in d.andere_bronnen() {
        for (i, v) in bron.invoer.iter().filter_map(|(i, v)| Some((i, v.veld()?))) {
            if !zaak.contains(&&v.lexostatus) && !doorgegeven.contains(&v.lexostatus.as_str()) {
                fouten.push(format!(
                    "behandeling: synthese-bron {}/{}, invoer '{i}': komt uit lexostatus '{}', en die is geen lexostatus van de zaak (zaak: true)",
                    bron.cel, bron.lexostatus, v.lexostatus
                ));
            }
        }
    }

    for h in &behandeling.handelingen {
        fouten.extend(controleer_handeling(proces, h, &uit_de_zaak, &zaak));
    }
    fouten
}

fn controleer_handeling(
    proces: &Proces,
    h: &HandelingDefinitie,
    uit_de_zaak: &BTreeMap<&str, Vec<String>>,
    zaak: &[&String],
) -> Vec<String> {
    let mut fouten = Vec::new();
    let d = &proces.definitie;
    let cel = &proces.cel;
    let service = proces.service.as_ref();
    let wie = format!("handeling '{}'", h.naam);
    if let Some(r) = &h.rol {
        match d.rollen.get(r) {
            None => fouten.push(format!("{wie}: rol '{r}' staat niet onder rollen")),
            Some(rol) if !rol.mag(crate::kanaal::Routes::Behandeling) => fouten.push(format!(
                "{wie}: rol '{r}' mag de behandeling niet (routes: behandeling)"
            )),
            Some(_) => {}
        }
    }
    if h.artikel.is_empty() {
        // Het laden meldde al waarom.
        return fouten;
    }
    // De uitkomsten: van een artikel, bij een vervolg ook van de haken.
    let haakuitkomsten: BTreeSet<String> = h
        .haken
        .iter()
        .flat_map(|a| uitkomsten_van(service, a))
        .collect();
    let mut artikelen = BTreeSet::new();
    for u in &h.uitkomsten {
        if haakuitkomsten.contains(u) {
            continue;
        }
        match artikel_met(service, &h.regeling, u) {
            None => fouten.push(format!(
                "{wie}: regeling '{}' heeft geen uitkomst '{u}'",
                h.regeling
            )),
            Some(a) => {
                artikelen.insert(a);
            }
        }
    }
    if artikelen.len() > 1 {
        fouten.push(format!(
            "{wie}: de uitkomsten komen uit meer dan een artikel ({}); een handeling is een artikel",
            artikelen.into_iter().collect::<Vec<_>>().join(", ")
        ));
    }

    // Het vastleg-event.
    let vl = format!(
        "{wie}, vastleggen {}/{}",
        h.vastleggen.stroom, h.vastleggen.event
    );
    if h.vastleggen.cel != cel.id() {
        // De cel van het proces meldt `proces::de_cel`.
    } else if let Some((_, event)) = cel.event(&h.vastleggen.stroom, &h.vastleggen.event) {
        if event.zaak != Zaak::Volgt {
            fouten.push(format!(
                "{vl}: het event heeft zaak: {}, en een handeling volgt de zaak van de aanvraag",
                event.zaak.als_tekst()
            ));
        }
        fouten.extend(controleer_besluit(proces, h, &vl));
        let sleutels = event.external_sleutels();
        let oordelen: Vec<&str> = h.oordelen.iter().map(|o| o.parameter.as_str()).collect();
        let feiten: Vec<&str> = h.feiten.iter().map(|f| f.naam.as_str()).collect();
        match &h.soort {
            Handelingsoort::Besluit => {
                let ontbreekt: Vec<&str> = h
                    .uitkomsten
                    .iter()
                    .filter(|u| !sleutels.contains(u))
                    .map(String::as_str)
                    .collect();
                if !ontbreekt.is_empty() {
                    fouten.push(format!(
                        "{vl}: het event legt de uitkomsten [{}] van het besluit niet vast",
                        ontbreekt.join(", ")
                    ));
                }
                if !feiten.is_empty() {
                    fouten.push(format!(
                        "{vl}: het event legt [{}] vast, en dat is geen uitkomst en geen oordeel van het besluit",
                        feiten.join(", ")
                    ));
                }
            }
            Handelingsoort::Vervolg { .. } => {
                let ontbreekt: Vec<&str> = feiten
                    .iter()
                    .copied()
                    .chain(haakuitkomsten.iter().map(String::as_str))
                    .filter(|k| !sleutels.iter().any(|s| s == k))
                    .collect();
                if !ontbreekt.is_empty() {
                    fouten.push(format!(
                        "{vl}: het event legt [{}] niet vast; een vervolg legt vast wat de stage vraagt en wat de haken uitrekenen (RFC-008, RFC-022 par. 3.3)",
                        ontbreekt.join(", ")
                    ));
                }
                let over: Vec<&str> = sleutels
                    .iter()
                    .map(String::as_str)
                    .filter(|k| !h.uitkomsten.iter().any(|u| u == k) && !feiten.contains(k))
                    .collect();
                if !over.is_empty() {
                    fouten.push(format!(
                        "{vl}: [{}] is geen uitkomst en niets wat de stage vraagt",
                        over.join(", ")
                    ));
                }
            }
            Handelingsoort::Feit => {}
        }
        let _ = oordelen;
    }

    if matches!(h.soort, Handelingsoort::Vervolg { .. }) {
        // Een vervolg leest het vastgelegde besluit, geen bronnen.
        if !h.rijen.is_empty() {
            fouten.push(format!(
                "{wie}: een vervolg rekent op de invoer van het besluit, zonder synthese per regel"
            ));
        }
        return fouten;
    }

    // Een parameter komt uit maar een bron.
    let mut per: BTreeMap<&str, Vec<String>> = uit_de_zaak.clone();
    for bron in d.andere_bronnen() {
        for p in &bron.parameters {
            per.entry(p)
                .or_default()
                .push(format!("synthese-bron {}/{}", bron.cel, bron.lexostatus));
        }
    }
    for o in &h.oordelen {
        per.entry(&o.parameter)
            .or_default()
            .push("het formulier".into());
    }
    for p in h.nog_niet.keys() {
        per.entry(p)
            .or_default()
            .push("de stand van wat nog niet gebeurd is".into());
    }
    let zaak: Vec<&str> = zaak.iter().map(|z| z.as_str()).collect();
    for r in &h.rijen {
        per.entry(&r.parameter)
            .or_default()
            .push(format!("de synthese per regel uit '{}'", r.tabel.veld));
        fouten.extend(rijen::controleer(
            &wie,
            r,
            &zaak,
            "geen lexostatus van de zaak (zaak: true)",
            d,
            cel,
        ));
    }
    let benodigd = match benodigd(service, h) {
        Ok(b) => b,
        Err(f) => {
            fouten.push(format!("{wie}: {f}"));
            return fouten;
        }
    };
    for (p, van) in &per {
        if van.len() > 1 && benodigd.contains_key(*p) {
            fouten.push(format!(
                "{wie}: parameter '{p}' komt uit meer dan een bron: {}",
                van.join(", ")
            ));
        }
    }
    let namen = h
        .oordelen
        .iter()
        .map(|o| ("formulier", o.parameter.as_str()))
        .chain(h.nog_niet.keys().map(|p| ("nog niet gebeurd", p.as_str())))
        .chain(h.rijen.iter().map(|r| ("rijen", r.parameter.as_str())));
    for (waar, p) in namen {
        if !benodigd.contains_key(p) {
            fouten.push(format!(
                "{wie}, {waar}: '{p}' is geen parameter van {} of van een artikel dat het zonder eigen parameters aanroept",
                h.artikel
            ));
        }
    }
    fouten
}

/// Of het vastleg-event past bij de soort handeling, en de handeling bij het
/// besluit dat zij noemt. Een zaak kan meer besluiten hebben; welk gram bij
/// welk besluit hoort, zegt de stroom (`besluit`), en welke handeling bij
/// welk besluit, `besluit` in de handeling (bij een vervolg: het artikel).
///
/// - een besluit legt vast in een event dat een besluit opent of wijzigt;
///   een wijziging noemt het besluit dat zij wijzigt, een nieuw besluit niet;
/// - een vervolg legt vast in een event dat een besluit volgt;
/// - een feit dat een besluit volgt, noemt het besluit; een ander feit niet;
/// - `besluit` noemt de handeling van een besluit in dit proces.
fn controleer_besluit(proces: &Proces, h: &HandelingDefinitie, vl: &str) -> Vec<String> {
    let mut fouten = Vec::new();
    let rol = h.besluitrol.map_or("geen", Besluit::als_tekst);
    let genoemd = h.besluit.as_deref();
    let besluit_handeling = |naam: &str| {
        proces
            .definitie
            .behandeling
            .as_ref()
            .and_then(|b| b.handeling(naam))
            .filter(|b| b.soort == Handelingsoort::Besluit && b.naam != h.naam)
    };
    match (&h.soort, h.besluitrol) {
        (Handelingsoort::Besluit, Some(Besluit::Opent)) => {
            if let Some(b) = genoemd {
                fouten.push(format!(
                    "{vl}: het event opent een besluit, en een nieuw besluit noemt geen ander (besluit: {b}); een besluit dat een ander wijzigt, legt vast in een event met besluit: wijzigt"
                ));
            }
        }
        (Handelingsoort::Besluit, Some(Besluit::Wijzigt))
        | (Handelingsoort::Feit, Some(Besluit::Volgt)) => match genoemd {
            None => fouten.push(format!(
                "{vl}: het event {rol} een besluit; noem met besluit de handeling van dat besluit"
            )),
            Some(b) if besluit_handeling(b).is_none() => fouten.push(format!(
                "handeling '{}': besluit '{b}' is geen andere handeling van een besluit in dit proces",
                h.naam
            )),
            Some(_) => {}
        },
        (Handelingsoort::Besluit, _) => fouten.push(format!(
            "{vl}: een besluit legt vast in een event met besluit: opent (of wijzigt, als het een ander besluit wijzigt), niet besluit: {rol}"
        )),
        (Handelingsoort::Vervolg { besluit, .. }, Some(Besluit::Volgt)) => {
            if genoemd.is_some_and(|b| b != besluit) {
                fouten.push(format!(
                    "handeling '{}': een vervolg op het besluit van '{besluit}' (hetzelfde artikel) noemt besluit '{}'",
                    h.naam,
                    genoemd.unwrap_or_default()
                ));
            }
        }
        (Handelingsoort::Vervolg { .. }, _) => fouten.push(format!(
            "{vl}: een vervolg is een latere stage van een besluit, en legt vast in een event met besluit: volgt, niet besluit: {rol}"
        )),
        (Handelingsoort::Feit, _) => {
            if let Some(b) = genoemd {
                fouten.push(format!(
                    "{vl}: het event volgt geen besluit (besluit: {rol}), en de handeling noemt besluit '{b}'"
                ));
            }
        }
    }
    fouten
}

/// De synthese-bronnen die een handeling vraagt: die een parameter van haar
/// artikel leveren, en de bronnen die hun (of de synthese per regel) een
/// invoer doorgeven. Welke bron een handeling nodig heeft, volgt zo uit wat
/// het artikel vraagt; een betaling vraagt de registers van een aanvraag
/// niet.
pub fn bronnen_voor(proces: &Proces, h: &HandelingDefinitie) -> Vec<usize> {
    let d = &proces.definitie;
    if matches!(h.soort, Handelingsoort::Vervolg { .. }) {
        return Vec::new();
    }
    let benodigd: BTreeSet<String> = benodigd(&proces.service, h)
        .map(|b| b.into_keys().collect())
        .unwrap_or_default();
    let bronnen: Vec<&crate::config::SyntheseBron> = d.andere_bronnen().collect();
    let mut nodig: BTreeSet<usize> = bronnen
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parameters.iter().any(|p| benodigd.contains(p)))
        .map(|(i, _)| i)
        .collect();
    // Lexostatussen waaruit een invoer komt: van de gekozen bronnen en van
    // de synthese per regel.
    let mut gevraagd: BTreeSet<String> = h
        .rijen
        .iter()
        .flat_map(|r| {
            std::iter::once(r.tabel.lexostatus.clone()).chain(r.bronnen.iter().flat_map(|b| {
                b.invoer.values().filter_map(|i| match i {
                    crate::config::RijInvoer::Eigen { lexostatus, .. } => Some(lexostatus.clone()),
                    _ => None,
                })
            }))
        })
        .collect();
    loop {
        for i in &nodig {
            for v in bronnen[*i].invoer.values().filter_map(|v| v.veld()) {
                gevraagd.insert(v.lexostatus.clone());
            }
        }
        let erbij: BTreeSet<usize> = bronnen
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                !nodig.contains(i) && !b.extra_velden.is_empty() && gevraagd.contains(&b.lexostatus)
            })
            .map(|(i, _)| i)
            .collect();
        if erbij.is_empty() {
            break;
        }
        nodig.extend(erbij);
    }
    nodig.into_iter().collect()
}

// --- In een zaak -----------------------------------------------------------

/// De besluiten in de zaak van de handeling `besluit`, in de volgorde waarin
/// de cel ze vastlegde, met de besluiten die ze wijzigen (van een handeling
/// met `besluit: wijzigt` die `besluit` noemt). Het laatste is het besluit
/// zoals het nu geldt.
fn keten<'z>(proces: &Proces, besluit: &str, zaak: &'z Zaakstand) -> Vec<&'z Besluitstand> {
    let Some(b) = &proces.definitie.behandeling else {
        return Vec::new();
    };
    let events: Vec<&HandelingDefinitie> = b
        .handelingen
        .iter()
        .filter(|h| {
            h.naam == besluit
                || (h.besluitrol == Some(Besluit::Wijzigt) && h.besluit.as_deref() == Some(besluit))
        })
        .collect();
    zaak.besluiten
        .iter()
        .filter(|s| {
            events
                .iter()
                .any(|h| s.van(&h.vastleggen.stroom, &h.vastleggen.event))
        })
        .collect()
}

/// De besluiten in de zaak die de handeling `besluit` zelf vastlegde.
fn eigen<'z>(proces: &Proces, besluit: &str, zaak: &'z Zaakstand) -> Vec<&'z Besluitstand> {
    let Some(b) = proces
        .definitie
        .behandeling
        .as_ref()
        .and_then(|b| b.handeling(besluit))
    else {
        return Vec::new();
    };
    zaak.besluiten
        .iter()
        .filter(|s| s.van(&b.vastleggen.stroom, &b.vastleggen.event))
        .collect()
}

/// Het besluit in de zaak waarop een handeling handelt, uit de
/// [`Zaakstand`]: bij een vervolg het laatste besluit van de handeling van
/// het besluit (de stage gaat daarop verder), bij een feit dat een besluit
/// volgt en bij een wijziging het laatste besluit van de handeling die zij
/// noemen, een wijziging ervan meegerekend. `Ok(None)`: de handeling hoort
/// bij geen besluit, of opent er zelf een. `Err`: het besluit ligt er nog
/// niet.
pub fn doel<'z>(
    proces: &Proces,
    h: &HandelingDefinitie,
    zaak: &'z Zaakstand,
) -> Result<Option<&'z Besluitstand>, String> {
    let (lijst, van) = match (&h.soort, h.besluitrol) {
        (Handelingsoort::Vervolg { besluit, .. }, _) => (eigen(proces, besluit, zaak), besluit),
        (_, Some(Besluit::Volgt | Besluit::Wijzigt)) => {
            let Some(b) = h.besluit.as_ref() else {
                return Ok(None);
            };
            (keten(proces, b, zaak), b)
        }
        _ => return Ok(None),
    };
    match lijst.last() {
        Some(b) => Ok(Some(b)),
        None => {
            let label = proces
                .definitie
                .behandeling
                .as_ref()
                .and_then(|b| b.handeling(van))
                .map_or(van.as_str(), |b| b.label());
            Err(format!("wacht op het besluit ({label})"))
        }
    }
}

fn verwijzing(b: &Besluitstand) -> Option<BesluitVerwijzing> {
    let (stage, gram) = b.genomen()?;
    Some(BesluitVerwijzing {
        besluitkenmerk: b.besluitkenmerk.clone(),
        name: gram.event.clone(),
        stage: Some(stage.clone()),
        op_moment: gram.op_moment.clone(),
        vastgelegd_op: gram.vastgelegd_op.clone(),
    })
}

/// Een lexostatus van de zaak, gevraagd aan de cel. Heeft de cel er geen
/// gram voor (404), dan levert ze niets: een lege lexostatus. Met een
/// concept reduceert de cel op proef, alsof het concept al vastlag.
async fn zaaklexostatus(
    om: &Omgeving<'_>,
    bron: &crate::config::SyntheseBron,
    zaakkenmerk: &str,
    peil: &Peil,
    concept: Option<&Vastlegverzoek>,
) -> Result<Lexostatus, Weigering> {
    let def = om
        .proces
        .cel
        .lexostatussen
        .lexostatus(&bron.lexostatus)
        .ok_or_else(|| Weigering::Cel(format!("lexostatus '{}' bestaat niet", bron.lexostatus)))?;
    let mut inputs = Map::new();
    inputs.insert("zaakkenmerk".into(), Value::String(zaakkenmerk.to_string()));
    let antwoord = match concept {
        None => om
            .cel
            .haal(&synthese::pad(&bron.cel, &bron.lexostatus, &inputs, peil))
            .await
            .and_then(|v| {
                serde_json::from_value::<Lexostatus>(v)
                    .map_err(|e| TransportFout::Json(e.to_string()))
            }),
        Some(c) => {
            for (k, v) in peil.query() {
                inputs.insert(k.into(), Value::String(v));
            }
            celclient::proef(om.cel, &bron.cel, &bron.lexostatus, c, &inputs)
                .await
                .map(|p| p.lexostatus)
        }
    };
    match antwoord {
        Ok(l) => Ok(l),
        // Kiest de definitie een gram en is er geen, dan levert zij niets.
        Err(TransportFout::Antwoord { status: 404, .. }) => Ok(Lexostatus {
            niet_afgeleid: def.reduction.afleidingen.keys().cloned().collect(),
            ..Lexostatus::leeg(&def.name)
        }),
        // Het concept past niet in een gram: een fout in het formulier.
        Err(TransportFout::Antwoord { status: 400, fout }) => Err(Weigering::Ongeldig(fout)),
        Err(TransportFout::Antwoord { status: 409, fout }) => Err(Weigering::Conflict(fout)),
        Err(f) => Err(Weigering::Cel(format!(
            "cel '{}', lexostatus '{}': {f}",
            bron.cel, bron.lexostatus
        ))),
    }
}

/// De peildatum van een handeling: de dag van het `op_moment` dat het event
/// aan een veld van het formulier bindt (de dag van het besluit, de
/// bekendmaking, de betaling), met de grondslag die de stroom daarvoor
/// geeft; anders vandaag. Een besluit leest de wet en de cellen zo op de dag
/// waarop het genomen wordt, ook als de behandelaar het later vastlegt.
/// Het derde deel is een bezwaar tegen dat moment: het ligt na vandaag (wat
/// nog moet gebeuren, is geen feit), of op een dag voor het laatste feit van
/// de zaak (een zaak loopt vooruit in de tijd). De cel weigert zo'n gram ook;
/// het proces zegt het vooraf.
fn peildatum(
    event: &Event,
    formulier: &Map<String, Value>,
    nu: &DateTime<FixedOffset>,
    zaak: &Zaakstand,
) -> Result<(String, String, Option<String>), Weigering> {
    if let Some(b) = &event.op_moment {
        if let Binding::External(pad) = b.binding() {
            if let Some(Value::String(t)) = formulier.get(&pad) {
                let tp = Tijdpunt::lees(&pad, t).map_err(Weigering::Ongeldig)?;
                let moment = tp.als_moment(*nu.offset());
                let dag = datum::peildatum(&moment);
                let laatste = zaak
                    .laatste_op_moment
                    .as_deref()
                    .map(datum::peildatum_van)
                    .transpose()
                    .map_err(Weigering::Cel)?;
                let bezwaar = if moment > *nu {
                    Some(format!(
                        "{pad} {dag} ligt na vandaag: wat nog moet gebeuren, is geen feit"
                    ))
                } else {
                    laatste.filter(|l| dag < *l).map(|l| {
                        format!(
                            "{pad} {dag} ligt voor de zaak: het laatste feit erin geldt op {l}; een zaak loopt vooruit in de tijd"
                        )
                    })
                };
                return Ok((
                    dag,
                    format!("{pad} (op_moment, {})", b.grondslag.join(", ")),
                    bezwaar,
                ));
            }
        }
    }
    Ok((datum::peildatum(nu), "vandaag".to_string(), None))
}

/// Waarom een handeling niet uit zichzelf genomen wordt.
enum Bezwaar {
    /// De vorm: het formulier, de volgorde van de zaak, het moment. Dan
    /// wordt er ook niets gemeld.
    Vorm(String),
    /// De inhoud: de wet zegt nee, of kan niet zeggen wat het feit doet.
    /// Een gebeurd feit legt de cel dan toch vast.
    Inhoud(String),
}

/// Reken een handeling in een zaak uit, zonder iets vast te leggen. `zaak`
/// is de stand van de zaak zoals de cel haar afleidt.
pub async fn proef(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    zaakkenmerk: &str,
    zaak: &Zaakstand,
    formulier: &Map<String, Value>,
) -> Result<Proefhandeling, Weigering> {
    let proces = om.proces;
    for naam in formulier.keys() {
        if !h.oordelen.iter().any(|o| &o.parameter == naam)
            && !h.feiten.iter().any(|f| &f.naam == naam)
        {
            return Err(Weigering::Ongeldig(format!(
                "'{naam}' is geen veld van het formulier van handeling '{}'",
                h.naam
            )));
        }
    }
    let (_, event) = proces
        .cel
        .event(&h.vastleggen.stroom, &h.vastleggen.event)
        .ok_or_else(|| Weigering::Cel(format!("handeling '{}': geen vastleg-event", h.naam)))?;
    let (peildatum, peildatum_uit, tijd) = peildatum(event, formulier, &om.nu, zaak)?;
    let mut p = Proefhandeling {
        handeling: h.naam.clone(),
        soort: h.soort.clone(),
        stage: h.stage.clone(),
        regeling: h.regeling.clone(),
        artikel: h.artikel.clone(),
        peildatum: peildatum.clone(),
        peildatum_uit,
        te_nemen: false,
        te_melden: false,
        uitkomsten: BTreeMap::new(),
        toetsen: BTreeMap::new(),
        mist: Vec::new(),
        reden: None,
        parameters: BTreeMap::new(),
        herkomst: BTreeMap::new(),
        bronnen: Vec::new(),
        niet_geleverd: Vec::new(),
        lexostatussen: Vec::new(),
        rijen: Vec::new(),
        besluit: None,
        trace_text: None,
    };
    let ontbrekend: Vec<String> = h
        .feiten
        .iter()
        .filter(|f| formulier.get(&f.naam).is_none_or(Value::is_null))
        .map(|f| f.naam.clone())
        .collect();
    // Het besluit waarop de handeling handelt. Ligt het er nog niet, dan is
    // er niets uit te rekenen: dat is de vorm (de volgorde van de zaak).
    let doel = match doel(proces, h, zaak) {
        Ok(b) => b,
        Err(r) => {
            p.reden = Some(format!("niet te nemen: {r}"));
            return Ok(p);
        }
    };
    p.besluit = doel.and_then(verwijzing);
    if let Some(r) = al_genomen(proces, h, zaak) {
        p.reden = Some(format!("niet te nemen: {r}"));
        return Ok(p);
    }
    let uitkomst = match (&h.soort, doel) {
        (Handelingsoort::Vervolg { besluit, .. }, Some(b)) => {
            vervolg(om, h, besluit, b, formulier, &mut p)?
        }
        (Handelingsoort::Vervolg { .. }, None) => {
            return Err(Weigering::Cel(format!(
                "handeling '{}': geen besluit",
                h.naam
            )))
        }
        // Een onvolledige uitkomst (een waarde mist, een bron antwoordde niet)
        // is geen conclusie over de inhoud: dan ligt er niets vast, ook niet
        // gemeld, want de invoer en het receipt zouden niet kloppen.
        _ => op_de_zaak(om, h, event, zaakkenmerk, formulier, &mut p)
            .await?
            .map(Bezwaar::Vorm),
    };
    // Een besluit waarvan de wet een uitkomst leeg laat, neemt het proces
    // niet: de wet besluit dan niets (zoals een wijziging zonder grond voor
    // een wijziging). Net als een haak die geen waarde geeft bij een vervolg.
    let leeg: Vec<&str> = h
        .uitkomsten
        .iter()
        .filter(|u| p.uitkomsten.get(*u) == Some(&Value::Null))
        .map(String::as_str)
        .collect();
    let uitkomst = match uitkomst {
        None if h.soort == Handelingsoort::Besluit && !leeg.is_empty() => {
            Some(Bezwaar::Inhoud(format!(
                "niet te nemen: {} geeft geen waarde voor {}",
                h.artikel,
                leeg.join(", ")
            )))
        }
        u => u,
    };
    let onwaar: Vec<&String> = p
        .toetsen
        .iter()
        .filter(|(_, w)| **w == Value::Bool(false))
        .map(|(n, _)| n)
        .collect();
    let toets = (!onwaar.is_empty()).then(|| {
        format!(
            "niet te nemen: {} zegt nee ({})",
            h.artikel,
            onwaar
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    });
    let vorm = tijd
        .map(|t| format!("niet te nemen: {t}"))
        .or((!ontbrekend.is_empty())
            .then(|| format!("niet te nemen: vul in: {}", ontbrekend.join(", "))))
        .or(match &uitkomst {
            Some(Bezwaar::Vorm(r)) => Some(r.clone()),
            _ => None,
        });
    let inhoud = match uitkomst {
        Some(Bezwaar::Inhoud(r)) => Some(r),
        _ => None,
    }
    .or(toets);
    p.te_nemen = vorm.is_none() && inhoud.is_none();
    p.te_melden = vorm.is_none() && inhoud.is_some() && h.soort != Handelingsoort::Besluit;
    p.reden = vorm.or(inhoud);
    Ok(p)
}

/// Een besluit of een feit: de lexostatussen van de zaak (bij een feit met
/// het concept erbij), de synthese, de synthese per regel, het formulier en
/// wat nog niet gebeurd is, en dan de engine. Geeft de reden terug als de
/// uitkomsten niet volledig zijn.
async fn op_de_zaak(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    event: &Event,
    zaakkenmerk: &str,
    formulier: &Map<String, Value>,
    p: &mut Proefhandeling,
) -> Result<Option<String>, Weigering> {
    let proces = om.proces;
    let service = proces.service.as_ref();
    let peil = Peil::op(Tijdpunt::lees("peildatum", &p.peildatum).map_err(Weigering::Cel)?);

    // 1. De lexostatussen van de zaak. Een feit telt op proef mee: de cel
    // reduceert alsof het concept al vastlag.
    let concept = (!h.feiten.is_empty()).then(|| Vastlegverzoek {
        actor: proces.definitie.actor.clone(),
        stroom: h.vastleggen.stroom.clone(),
        event: h.vastleggen.event.clone(),
        intake: Value::Null,
        external: event_velden(event, &h.uitkomsten, formulier, &BTreeMap::new()),
        zaakkenmerk: Some(zaakkenmerk.to_string()),
        besluitkenmerk: p.besluit.as_ref().map(|b| b.besluitkenmerk.clone()),
        besluit: None,
        zaak_grammen: None,
    });
    let mut eigen = Vec::new();
    for bron in proces.definitie.zaakbronnen() {
        eigen.push(zaaklexostatus(om, bron, zaakkenmerk, &peil, concept.as_ref()).await?);
    }

    // 2. Synthese, met de invoer uit de lexostatus van de zaak die haar
    // levert.
    let hoofd = om
        .bronnen
        .iter()
        .flat_map(|s| s.definitie.invoer.values().filter_map(|v| v.veld()))
        .find_map(|v| eigen.iter().position(|l| l.naam == v.lexostatus))
        .unwrap_or(0);
    let mut samen = match eigen.get(hoofd) {
        Some(l) => synthese::voeg_samen(l, om.bronnen, &peil).await,
        None => synthese::voeg_samen(&Lexostatus::leeg(""), om.bronnen, &peil).await,
    };
    for (i, l) in eigen.iter().enumerate() {
        if i == hoofd {
            continue;
        }
        for (naam, w) in &l.parameters {
            samen.parameters.insert(naam.clone(), w.clone());
            samen.herkomst.insert(
                naam.clone(),
                Herkomst::Eigen {
                    lexostatus: l.naam.clone(),
                },
            );
        }
    }

    // 3. Synthese per regel.
    let wet = rijen::Omgeving {
        service,
        datum: &p.peildatum,
        peil: &peil,
    };
    p.rijen = rijen::pas_toe(om.rijen, &eigen, &mut samen, wet).await;

    // 4. De oordelen van de behandelaar; een leeg veld gaat niet mee.
    for o in &h.oordelen {
        if let Some(w) = formulier.get(&o.parameter).filter(|w| !w.is_null()) {
            samen.parameters.insert(o.parameter.clone(), w.clone());
            samen
                .herkomst
                .insert(o.parameter.clone(), Herkomst::Behandelaar);
        }
    }

    // 5. Wat een latere stage pas vraagt, is nog niet gebeurd.
    for (naam, n) in &h.nog_niet {
        samen.parameters.insert(naam.clone(), n.waarde.clone());
        samen.herkomst.insert(
            naam.clone(),
            Herkomst::StandBijBesluit {
                stage: n.stage.clone(),
            },
        );
    }

    // 6. De engine: de uitkomsten en de toetsen, in een run.
    let gevraagd: Vec<&str> = h
        .uitkomsten
        .iter()
        .chain(h.toetsen.iter())
        .map(String::as_str)
        .collect();
    let e = toets::evalueer_met_trace(
        service,
        &h.regeling,
        &gevraagd,
        &samen.parameters,
        &p.peildatum,
    );
    let volledig = e.volledig(&gevraagd);
    let mut reden = (!volledig).then(|| e.reden("niet te nemen"));
    if !volledig {
        if let Some(r) = samen.reden() {
            reden = Some(r.replacen("niet te beoordelen", "niet te nemen", 1));
        }
    }
    for (naam, w) in e.waarden {
        if h.toetsen.contains(&naam) {
            p.toetsen.insert(naam, w);
        } else {
            p.uitkomsten.insert(naam, w);
        }
    }
    p.trace_text = e.trace_text;
    p.mist = e.mist;
    p.niet_geleverd = benodigd(service, h)
        .map_err(Weigering::Cel)?
        .into_values()
        .filter(|b| !samen.parameters.contains_key(&b.naam))
        .collect();
    p.parameters = samen.parameters;
    p.herkomst = samen.herkomst;
    p.bronnen = samen.bronnen;
    p.lexostatussen = eigen;
    Ok(reden)
}

/// Een vervolg: de engine voert de stage van de handeling uit op de invoer
/// en de uitkomsten van het vastgelegde besluit (RFC-008: het besluit is de
/// toestand, de orkestratie bewaart haar en levert wat de stage vraagt). Die
/// toestand leidt de cel af (de stage van het besluit in de [`Zaakstand`]);
/// het proces leest geen gram. Wat de stage vraagt, komt uit het formulier.
/// De haken van die stage vuren (RFC-007), zoals de aanvang en het einde van
/// de bezwaartermijn.
fn vervolg(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    besluit: &str,
    stand: &Besluitstand,
    formulier: &Map<String, Value>,
    p: &mut Proefhandeling,
) -> Result<Option<Bezwaar>, Weigering> {
    let proces = om.proces;
    let service = proces.service.as_ref();
    let b = proces
        .definitie
        .behandeling
        .as_ref()
        .and_then(|b| b.handeling(besluit))
        .ok_or_else(|| Weigering::Cel(format!("geen handeling '{besluit}'")))?;
    let Some((_, gram)) = stand.genomen() else {
        return Err(Weigering::Cel(format!(
            "besluit {} heeft geen stage van het besluit",
            stand.besluitkenmerk
        )));
    };
    if let Some(s) = h.stage.as_ref().filter(|s| stand.stages.contains_key(*s)) {
        return Ok(Some(Bezwaar::Vorm(format!(
            "niet te nemen: stage {s} ligt al in besluit {}",
            stand.besluitkenmerk
        ))));
    }
    let Handelingsoort::Vervolg { procedure, .. } = &h.soort else {
        return Err(Weigering::Cel("geen vervolg".into()));
    };
    let stage = h.stage.clone().unwrap_or_default();
    let state = StageState {
        procedure_id: procedure.clone(),
        contextual_law: gram
            .regulation
            .clone()
            .unwrap_or_else(|| h.regeling.clone()),
        current_stage: stage.clone(),
        accumulated_outputs: gram
            .velden
            .iter()
            .map(|(k, v)| (k.clone(), EngineValue::from(v)))
            .collect(),
        parameters: gram
            .invoer
            .iter()
            .map(|(k, w)| (k.clone(), EngineValue::from(w)))
            .collect(),
    };
    let mut invoer = BTreeMap::new();
    for f in &h.feiten {
        if let Some(w) = formulier.get(&f.naam).filter(|w| !w.is_null()) {
            invoer.insert(f.naam.clone(), EngineValue::from(w));
            p.parameters.insert(f.naam.clone(), w.clone());
            p.herkomst.insert(f.naam.clone(), Herkomst::Behandelaar);
        }
    }
    let uitkomst = b
        .uitkomsten
        .first()
        .ok_or_else(|| Weigering::Cel(format!("handeling '{besluit}' noemt geen uitkomst")))?;
    let outputs =
        match service.execute_stage(&h.regeling, uitkomst, Some(state), invoer, &p.peildatum) {
            Ok(ExecutionOutcome::Complete(r)) => r.outputs,
            Ok(ExecutionOutcome::Yielded {
                state,
                outputs,
                pending_inputs,
            }) => {
                if state.current_stage == stage {
                    p.mist = pending_inputs;
                    return Ok(Some(Bezwaar::Vorm(format!(
                        "niet te nemen: stage {stage} vraagt {}",
                        p.mist.join(", ")
                    ))));
                }
                outputs
            }
            Err(e) => return Ok(Some(Bezwaar::Vorm(format!("niet te nemen: {e}")))),
        };
    for u in &h.uitkomsten {
        match outputs.get(u) {
            Some(w) if w.contains_unknown() => {
                for f in w.missing_facts() {
                    if !p.mist.contains(&f.name) {
                        p.mist.push(f.name.clone());
                    }
                }
            }
            Some(w) => {
                if let Ok(v) = serde_json::to_value(w) {
                    p.uitkomsten.insert(u.clone(), v);
                }
            }
            None => {}
        }
    }
    let leeg: Vec<&str> = h
        .uitkomsten
        .iter()
        .filter(|u| p.uitkomsten.get(*u).is_none_or(Value::is_null))
        .map(String::as_str)
        .collect();
    if !p.mist.is_empty() {
        return Ok(Some(Bezwaar::Vorm(format!(
            "niet te nemen: mist {}",
            p.mist.join(", ")
        ))));
    }
    if !leeg.is_empty() {
        // Een haak die geen waarde geeft, zegt dat de stage niet op de
        // voorgeschreven wijze plaatsvond (zoals een bekendmaking die niet
        // aan Awb 3:41 voldoet: de bezwaartermijn vangt dan niet aan). Een
        // conclusie over de inhoud: gebeurde het toch, dan ligt het vast, met
        // een lege termijn.
        return Ok(Some(Bezwaar::Inhoud(format!(
            "niet te nemen: geen waarde voor {} ({})",
            leeg.join(", "),
            h.haken.join(", ")
        ))));
    }
    Ok(None)
}

/// De velden van het gram: per `$external`-sleutel van het event een
/// uitkomst, of de waarde uit het formulier.
fn event_velden(
    event: &Event,
    uitkomsten_namen: &[String],
    formulier: &Map<String, Value>,
    uitkomsten: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    event
        .external_sleutels()
        .into_iter()
        .map(|k| {
            let w = if uitkomsten_namen.contains(&k) {
                uitkomsten.get(&k).cloned().unwrap_or(Value::Null)
            } else {
                formulier.get(&k).cloned().unwrap_or(Value::Null)
            };
            (k, w)
        })
        .collect()
}

/// Een genomen handeling: het vastgelegde gram, de proef waaruit het
/// volgde, en wat er bij het vastleggen op te merken viel.
#[derive(Debug, Clone, Serialize)]
pub struct Genomen {
    pub gram: Gram,
    pub yaml: String,
    pub proef: Proefhandeling,
    pub waarschuwingen: Vec<String>,
}

/// Neem een handeling in een zaak en laat de cel haar vastleggen.
///
/// De proef moet te nemen zijn: dan handelt het proces uit zichzelf. Is zij
/// dat om de inhoud niet (`te_melden`), dan legt de cel het feit alleen vast
/// als de behandelaar meldt dat het gebeurde (`gebeurd`): een executogram
/// legt een daadwerkelijke levering vast (paper P:54), en wat gebeurd is,
/// weigert het proces niet om wat het ervan vindt. De reden gaat mee als
/// waarschuwing, en de lexostatussen van de zaak tonen de gevolgen. Een
/// besluit wordt niet gemeld. Bij een besluit en een vervolg toetst het
/// proces het bevoegd gezag van de wet aan het gezag waarvoor het handelt
/// (`namens`), of een mandaat van dat gezag (zie [`crate::gezag`]): anders
/// weigert het; noemt de wet geen gezag, dan legt het vast met een
/// waarschuwing. Het gram draagt wie handelde (`handelende_actor`): de rol,
/// het kanaal en de identiteit van de ingelogde gebruiker, en bij een besluit
/// of vervolg namens welk gezag. De cel bouwt het gram uit haar stroom, en
/// weigert (409) als de stage al in de zaak ligt of als de zaak veranderde
/// sinds het proces haar las: wat het proces uitrekende, gold voor de zaak
/// zoals die toen was.
pub async fn neem(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    zaakkenmerk: &str,
    zaak: &Zaakstand,
    formulier: &Map<String, Value>,
    gebeurd: bool,
    handelend: &Sessie,
) -> Result<Genomen, Weigering> {
    let proces = om.proces;
    let service = proces.service.as_ref();
    let actor = &proces.definitie.actor;
    if gebeurd && h.soort == Handelingsoort::Besluit {
        return Err(Weigering::Ongeldig(format!(
            "handeling '{}' is een besluit: dat neemt het proces zelf, het wordt niet als gebeurd gemeld",
            h.naam
        )));
    }
    let proef = proef(om, h, zaakkenmerk, zaak, formulier).await?;
    let mut waarschuwingen = Vec::new();
    if !proef.te_nemen {
        let reden = proef
            .reden
            .clone()
            .unwrap_or_else(|| "niet te nemen".to_string());
        if !(gebeurd && proef.te_melden) {
            return Err(Weigering::NietTeNemen(if proef.te_melden {
                format!("{reden}; is het toch gebeurd, meld het dan als gebeurd (gebeurd: true)")
            } else {
                reden
            }));
        }
        waarschuwingen.push(format!(
            "gemeld als gebeurd, tegen de conclusie van het proces in: {reden}"
        ));
    }
    let (stroom, event) = proces
        .cel
        .event(&h.vastleggen.stroom, &h.vastleggen.event)
        .ok_or_else(|| Weigering::Cel(format!("handeling '{}': geen vastleg-event", h.naam)))?;

    let eigen = proces.gezag.as_deref();
    let mut gezag = None;
    let (mut namens, mut mandaat) = (None, None);
    if !matches!(h.soort, Handelingsoort::Feit) {
        let nummer = regelingen::ontleed(&h.artikel)
            .map_err(Weigering::Cel)?
            .artikel;
        gezag = gezag::gezag_van(service, &h.regeling, nummer);
        match &gezag {
            Some(g) => match gezag::toets(eigen, &proces.definitie.mandaten, g) {
                Ok(Bevoegdheid::Eigen) => namens = Some(g.clone()),
                Ok(Bevoegdheid::Mandaat(m)) => {
                    namens = Some(g.clone());
                    mandaat = Some(m.grondslag.clone());
                }
                Err(reden) => {
                    return Err(Weigering::Onbevoegd(format!("{}: {reden}", h.artikel)));
                }
            },
            None => {
                waarschuwingen.push(format!(
                    "regeling '{}' noemt geen bevoegd gezag bij {}; vastgelegd zonder competent_authority",
                    h.regeling, h.artikel
                ));
                namens = eigen.map(str::to_string);
            }
        }
    }
    let handelende_actor = HandelendeActor {
        rol: handelend.rol.clone(),
        kanaal: handelend.kanaal.clone(),
        identiteit: handelend.velden.clone(),
        grondslag: proces
            .definitie
            .rollen
            .get(&handelend.rol)
            .and_then(|r| r.grondslag.clone()),
        namens,
        mandaat,
    };

    let external = event_velden(event, &h.uitkomsten, formulier, &proef.uitkomsten);
    // Elke parameter die meedeed gaat mee, met zijn herkomst.
    let mut inputs: BTreeMap<String, Invoer> = BTreeMap::new();
    for (naam, waarde) in &proef.parameters {
        let herkomst = proef
            .herkomst
            .get(naam)
            .ok_or_else(|| Weigering::Cel(format!("parameter '{naam}' heeft geen herkomst")))?;
        inputs.insert(
            naam.clone(),
            Invoer {
                waarde: waarde.clone(),
                herkomst: herkomst.clone(),
            },
        );
    }
    let mut stromen: Vec<StroomVerwijzing> = proces
        .cel
        .strommen
        .iter()
        .map(|s| StroomVerwijzing {
            id: s.id.clone(),
            sha256: s.sha256.clone(),
        })
        .collect();
    stromen.sort_by(|a, b| a.id.cmp(&b.id));
    // Het rechtskarakter en de regeling horen bij een besluit (een
    // decretogram); invoer en receipt bij elke handeling die de engine
    // uitrekende.
    let decretogram = event.type_ == "decretogram";
    let artikel = regelingen::artikel(service, &h.artikel).map_err(Weigering::Cel)?;
    let produces = artikel
        .get_execution_spec()
        .and_then(|e| e.produces.as_ref())
        .filter(|_| decretogram);
    let verzoek = Vastlegverzoek {
        actor: actor.clone(),
        stroom: stroom.id.clone(),
        event: event.name.clone(),
        intake: Value::Null,
        external,
        zaakkenmerk: Some(zaakkenmerk.to_string()),
        // Een nieuw besluit krijgt zijn kenmerk van de cel; een gram dat een
        // besluit volgt of wijzigt, noemt dat besluit.
        besluitkenmerk: match h.besluitrol {
            Some(Besluit::Volgt | Besluit::Wijzigt) => {
                proef.besluit.as_ref().map(|b| b.besluitkenmerk.clone())
            }
            _ => None,
        },
        besluit: Some(Besluitvelden {
            legal_character: produces.and_then(|p| p.legal_character.clone()),
            decision_type: produces.and_then(|p| p.decision_type.clone()),
            regulation: decretogram.then(|| h.regeling.clone()),
            regulation_valid_from: decretogram
                .then(|| service.resolver().get_law(&h.regeling))
                .flatten()
                .and_then(|l| {
                    l.valid_from
                        .clone()
                        .or_else(|| Some(l.publication_date.clone()))
                }),
            competent_authority: gezag.filter(|_| decretogram),
            handelende_actor: Some(handelende_actor),
            inputs,
            receipt: Some(Receipt::nieuw(om.regelingen.to_vec(), stromen)),
        }),
        zaak_grammen: Some(zaak.grammen),
    };
    let MetYaml { gram, yaml } = celclient::leg_vast(om.cel, &h.vastleggen.cel, &verzoek)
        .await
        .map_err(|f| match f {
            // De vorm (de stage, de zaak, het moment) toetst de cel, onder haar slot.
            TransportFout::Antwoord { status: 409, fout } => Weigering::Conflict(fout),
            TransportFout::Antwoord { status: 400, fout } => Weigering::Ongeldig(fout),
            f => Weigering::Cel(format!("de handeling is niet vastgelegd: {f}")),
        })?;
    Ok(Genomen {
        gram,
        yaml,
        proef,
        waarschuwingen,
    })
}

/// Of een handeling in een zaak kan, naar de besluiten en hun stages: een
/// besluit zolang de zaak geen besluit van die handeling heeft (een ander
/// besluit over dezelfde aanvraag vraagt een eigen grondslag: een
/// wijziging), een wijziging als er een besluit is om te wijzigen, een
/// vervolg als het besluit er ligt en zijn stage nog niet, een feit dat een
/// besluit volgt als dat besluit er ligt, en elk ander feit altijd (wat het
/// doet, zegt de proef).
#[derive(Debug, Clone, Serialize)]
pub struct Stand {
    pub beschikbaar: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reden: Option<String>,
    /// De grammen die de handeling in deze zaak al vastlegde.
    pub vastgelegd: usize,
    /// Het besluit waarop de handeling nu zou handelen (zie [`doel`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub besluit: Option<String>,
}

/// De stand van een handeling in een zaak, uit de [`Zaakstand`] die de cel
/// afleidt: welke besluiten er liggen, welke stages elk doorliep en hoe vaak
/// het event van de handeling.
pub fn stand(proces: &Proces, h: &HandelingDefinitie, zaak: &Zaakstand) -> Stand {
    let vastgelegd = zaak.aantal(&h.vastleggen.stroom, &h.vastleggen.event);
    let mut besluit = None;
    let reden = match doel(proces, h, zaak) {
        Err(r) => Some(r),
        Ok(b) => {
            besluit = b.map(|b| b.besluitkenmerk.clone());
            match (&h.soort, b) {
                (Handelingsoort::Besluit, _) => al_genomen(proces, h, zaak),
                (Handelingsoort::Vervolg { .. }, Some(b)) => h
                    .stage
                    .as_ref()
                    .filter(|s| b.stages.contains_key(*s))
                    .map(|s| format!("stage {s} ligt al in besluit {}", b.besluitkenmerk)),
                _ => None,
            }
        }
    };
    Stand {
        beschikbaar: reden.is_none(),
        reden,
        vastgelegd,
        besluit,
    }
}

/// Of een besluit dat geen ander wijzigt, al in de zaak ligt: de cel legt
/// geen tweede besluit van hetzelfde event in een zaak vast. Een ander
/// besluit over dezelfde aanvraag is een wijziging, met een eigen grondslag.
fn al_genomen(proces: &Proces, h: &HandelingDefinitie, zaak: &Zaakstand) -> Option<String> {
    if h.besluitrol != Some(Besluit::Opent) {
        return None;
    }
    eigen(proces, &h.naam, zaak).first().map(|b| {
        format!(
            "besluit {} ligt al in de zaak; een ander besluit hierover vraagt een eigen grondslag (een wijziging)",
            b.besluitkenmerk
        )
    })
}

/// De procedure van een besluit (RFC-008): de stages, met per stage of er
/// een gram van ligt en welke handeling het vastlegt.
#[derive(Debug, Clone, Serialize)]
pub struct ProcedureStand {
    pub id: String,
    pub stages: Vec<StageStand>,
}

#[derive(Debug, Clone, Serialize)]
pub struct StageStand {
    pub name: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    pub vastgelegd: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub handeling: Option<String>,
}

/// De rechtsbescherming na de laatste stage van een besluit, afgeleid uit de
/// procedure en de wet (RFC-022 par. 3.3): de volgende stage, als geen
/// handeling haar vastlegt (zoals BEZWAAR, die na de bekendmaking vanzelf
/// loopt), met de uitkomsten van de haken die de wet op de laatste stage liet
/// vuren (zoals het einde van de bezwaartermijn, Awb 6:7 en 6:8). Niets
/// hiervan staat per regel in de configuratie.
#[derive(Debug, Clone, Serialize)]
pub struct Rechtsbescherming {
    pub procedure: String,
    /// De stage waarna de route loopt.
    pub na: String,
    /// De stage die nu loopt.
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// De haken die de route uitrekenden, als `<regeling>#<artikel>`.
    pub grondslag: Vec<String>,
    /// Hun uitkomsten, zoals het gram van de laatste stage ze vastlegde.
    pub uitkomsten: BTreeMap<String, Value>,
}

/// Een besluit in de zaak, voor het zaakscherm: welke handeling het nam, de
/// procedure met zijn stages, de rechtsbescherming die daaruit volgt, en de
/// handelingen die nu op dit besluit handelen (zijn vervolg, de feiten die
/// het volgen, een wijziging).
#[derive(Debug, Clone, Serialize)]
pub struct BesluitInZaak {
    pub besluitkenmerk: String,
    /// De handeling die het besluit vastlegde, en haar artikel.
    pub handeling: String,
    pub label: String,
    pub artikel: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub op_moment: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub wijzigt: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedure: Option<ProcedureStand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub rechtsbescherming: Option<Rechtsbescherming>,
    /// De handelingen die nu op dit besluit handelen.
    pub handelingen: Vec<String>,
}

/// De besluiten in een zaak, elk met zijn procedure en rechtsbescherming.
/// Welke besluiten er liggen, welke stages elk doorliep en wat hun grammen
/// vastlegden, zegt de [`Zaakstand`] van de cel; de procedure en de haken
/// komen uit de wet. Een stage die bij geen besluit hoort (de aanvraag),
/// telt voor elk besluit van de zaak.
pub fn besluiten_in_zaak(proces: &Proces, zaak: &Zaakstand) -> Vec<BesluitInZaak> {
    let Some(behandeling) = &proces.definitie.behandeling else {
        return Vec::new();
    };
    zaak.besluiten
        .iter()
        .filter_map(|b| {
            let h = behandeling.handelingen.iter().find(|h| {
                h.soort == Handelingsoort::Besluit
                    && b.van(&h.vastleggen.stroom, &h.vastleggen.event)
            })?;
            let (procedure, rechtsbescherming) = procedure_en_route(proces, h, b, zaak);
            let handelingen = behandeling
                .handelingen
                .iter()
                .filter(|x| x.naam != h.naam)
                .filter(|x| {
                    doel(proces, x, zaak)
                        .ok()
                        .flatten()
                        .is_some_and(|d| d.besluitkenmerk == b.besluitkenmerk)
                })
                .map(|x| x.naam.clone())
                .collect();
            Some(BesluitInZaak {
                besluitkenmerk: b.besluitkenmerk.clone(),
                handeling: h.naam.clone(),
                label: h.label().to_string(),
                artikel: h.artikel.clone(),
                event: b.event.clone(),
                op_moment: b.genomen().map(|(_, g)| g.op_moment.clone()),
                wijzigt: b.wijzigt.clone(),
                procedure,
                rechtsbescherming,
                handelingen,
            })
        })
        .collect()
}

/// De procedure en de rechtsbescherming van een besluit in de zaak.
fn procedure_en_route(
    proces: &Proces,
    besluit: &HandelingDefinitie,
    stand: &Besluitstand,
    zaak: &Zaakstand,
) -> (Option<ProcedureStand>, Option<Rechtsbescherming>) {
    let service = proces.service.as_ref();
    let Some(behandeling) = &proces.definitie.behandeling else {
        return (None, None);
    };
    let Some(p) = procedure_van(service, &besluit.artikel) else {
        return (None, None);
    };
    let door = |stage: &str| {
        behandeling
            .handelingen
            .iter()
            .find(|h| h.stage.as_deref() == Some(stage) && h.artikel == besluit.artikel)
    };
    let gram = |stage: &str| stand.stages.get(stage).or_else(|| zaak.stages.get(stage));
    let stages: Vec<StageStand> = p
        .stages
        .iter()
        .map(|s| StageStand {
            name: s.name.clone(),
            description: s.description.clone(),
            vastgelegd: gram(&s.name).is_some(),
            handeling: door(&s.name).map(|h| h.naam.clone()),
        })
        .collect();
    let route = stages.iter().rposition(|s| s.vastgelegd).and_then(|i| {
        let na = &p.stages[i];
        let volgende = p.stages.get(i + 1)?;
        if door(&volgende.name).is_some() {
            return None;
        }
        let h = door(&na.name)?;
        if h.haken.is_empty() {
            return None;
        }
        let gram = gram(&na.name)?;
        let uitkomsten = h
            .haken
            .iter()
            .flat_map(|a| uitkomsten_van(service, a))
            .filter_map(|u| gram.velden.get(&u).map(|w| (u, w.clone())))
            .collect();
        Some(Rechtsbescherming {
            procedure: p.id.clone(),
            na: na.name.clone(),
            stage: volgende.name.clone(),
            description: volgende.description.clone(),
            grondslag: h.haken.clone(),
            uitkomsten,
        })
    });
    (
        Some(ProcedureStand {
            id: p.id.clone(),
            stages,
        }),
        route,
    )
}

/// De procedure van de zaak voor er een besluit ligt: de procedure van het
/// eerste besluit dat het proces kent, met de stages die bij geen besluit
/// horen (zoals de aanvraag).
pub fn procedure_van_de_zaak(proces: &Proces, zaak: &Zaakstand) -> Option<ProcedureStand> {
    let b = proces
        .definitie
        .behandeling
        .as_ref()?
        .handelingen
        .iter()
        .find(|h| h.soort == Handelingsoort::Besluit && h.besluitrol == Some(Besluit::Opent))?;
    let p = procedure_van(proces.service.as_ref(), &b.artikel)?;
    Some(ProcedureStand {
        id: p.id.clone(),
        stages: p
            .stages
            .iter()
            .map(|s| StageStand {
                name: s.name.clone(),
                description: s.description.clone(),
                vastgelegd: zaak.stages.contains_key(&s.name),
                handeling: None,
            })
            .collect(),
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use regelrecht_engine::LawExecutionService;

    /// Een fictieve regeling met twee artikelen: een beschikking van "De
    /// Instantie van Voorbeeld" en een toets van hetzelfde gezag.
    const REGELING: &str = r#"
$id: testregeling_bevoegd
regulatory_layer: WET
publication_date: '2025-01-01'
competent_authority:
  name: De Instantie van Voorbeeld
articles:
  - number: '1'
    text: Toets
    machine_readable:
      execution:
        produces: {legal_character: TOETS, decision_type: GEEN_BESLUIT}
        parameters: [{name: x, type: number, required: false}]
        output:
          - {name: toets, type: boolean, legal_basis: {article: '1', paragraph: '1'}}
          - {name: zonder_grondslag, type: boolean}
        actions:
          - {output: toets, value: {operation: GREATER_THAN, subject: $x, value: 0}}
          - {output: zonder_grondslag, value: true}
  - number: '2'
    text: Besluit
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: $x}]
"#;

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(REGELING).unwrap();
        s
    }

    /// Een toets telt alleen als het artikel in de grondslag van het event
    /// staat, en alleen een booleaanse uitkomst met de norm van dat lid als
    /// `legal_basis`.
    #[test]
    fn toetsen_uit_de_grondslag_van_het_event() {
        let s = service();
        let event: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\ngrondslag: ['testregeling_bevoegd#1 lid 1']\ntype: executogram\nzaak: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert_eq!(toetsen(&s, "testregeling_bevoegd#1", &event), ["toets"]);
        // Een oordeel of vaststelling voert geen besluit uit: geen toets.
        let mut oordeel = event.clone();
        oordeel.type_ = "handeling".into();
        assert!(toetsen(&s, "testregeling_bevoegd#1", &oordeel).is_empty());
        assert!(toetsen(&s, "testregeling_bevoegd#2", &event).is_empty());
        let ander: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\ngrondslag: ['testregeling_bevoegd#2']\ntype: executogram\nzaak: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert!(toetsen(&s, "testregeling_bevoegd#1", &ander).is_empty());
        let ander_lid: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\ngrondslag: ['testregeling_bevoegd#1 lid 2']\ntype: executogram\nzaak: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert!(toetsen(&s, "testregeling_bevoegd#1", &ander_lid).is_empty());
    }
}
