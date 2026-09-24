//! Wie een parameter levert, volgens de wet: `origin` op een parameter en
//! `origins` in uitvoeringsbeleid (RFC-043).
//!
//! De wet geeft per parameter de herkomst met een grondslag. Uitvoeringsbeleid
//! van de actor van het proces kan die overschrijven. Bij het opstarten gaat
//! het proces na dat elke parameter die de aanroeper van een uitgevoerde
//! uitkomst (toets, aanbod, besluit) moet leveren, een leverancier heeft die
//! bij zijn herkomst past:
//!
//! | herkomst | leverancier |
//! |---|---|
//! | `BELANGHEBBENDE` | een afleiding van een eigen lexostatus (de reductie van de indiening), of de keuze van het tijdvak in het portaal |
//! | `DOSSIER` | een afleiding van een eigen lexostatus, of de stand bij besluit |
//! | `OORDEEL` | het besluitformulier, alleen bij het besluit |
//! | `REGISTER` | een synthese-bron die de parameter levert, waarvan de lexostatus in `levert_aan` een artikel noemt dat hem vraagt, en die een kroniek bijhoudt met een grondslag in `register` |
//! | `KANAAL` | een afleiding die alleen `$intake` leest |
//!
//! Zonder leverancier weigert de runtime te starten, behalve bij een
//! parameter met `required: false`: dan rekent de engine zonder, en is het
//! een waarschuwing. Een parameter zonder `origin` is een waarschuwing, en
//! een `BELANGHEBBENDE`-parameter zonder `required: false` ook (RFC-036).

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use regelrecht_engine::{Article, LawExecutionService, RegulatoryLayer};
use regelrecht_law_model::{Origin, OriginValue, Parameter};

use crate::besluit;
use crate::cel::Cel;
use crate::config::{Oordeel, ProcesDefinitie, RijenDefinitie};
use crate::regelingen::{self, Benodigd};
use crate::stroom::Binding;

/// De grondslag van de gevraagde beschikking (Awb 4:2 lid 1): wat de
/// aanvrager vraagt, zoals het tijdvak. Het aanbod mag erop leunen.
pub const GEVRAAGDE_BESCHIKKING: &str = "algemene_wet_bestuursrecht#4:2 lid 1";

/// De herkomst die voor een parameter geldt, en waar ze staat.
#[derive(Debug, Clone, PartialEq)]
pub struct Geldend {
    pub origin: Origin,
    /// Het artikel van het beleid dat haar overschrijft; `None` als ze uit
    /// de wet komt.
    pub beleid: Option<String>,
}

impl Geldend {
    /// Voor een melding: `REGISTER, register kieswet, grondslag x#1`.
    pub fn beschrijving(&self) -> String {
        let mut s = self.origin.waarde.as_str().to_string();
        if let Some(r) = &self.origin.register {
            s.push_str(&format!(", register {r}"));
        }
        s.push_str(&format!(", grondslag {}", self.origin.grondslag));
        if let Some(b) = &self.beleid {
            s.push_str(&format!(", uit {b}"));
        }
        s
    }

    /// Of de grondslag die van de gevraagde beschikking is (Awb 4:2 lid 1).
    pub fn is_gevraagde_beschikking(&self) -> bool {
        self.origin.waarde == OriginValue::Belanghebbende
            && zelfde_grondslag(&self.origin.grondslag, GEVRAAGDE_BESCHIKKING)
    }
}

/// Twee grondslagen wijzen hetzelfde artikel en lid aan.
fn zelfde_grondslag(a: &str, b: &str) -> bool {
    match (regelingen::ontleed(a), regelingen::ontleed(b)) {
        (Ok(a), Ok(b)) => a == b,
        _ => false,
    }
}

/// De overschrijvingen door het uitvoeringsbeleid van een actor, per
/// (regeling, parameter).
#[derive(Debug, Default)]
pub struct Overschrijvingen(BTreeMap<(String, String), Geldend>);

/// Lees `origins` uit elk geladen uitvoeringsbeleid waarvan het bevoegd
/// gezag (van het artikel, anders van de regeling) de actor is. Twee
/// artikelen die dezelfde parameter een andere herkomst geven, zijn een fout.
pub fn overschrijvingen(
    service: &LawExecutionService,
    actor: &str,
) -> Result<Overschrijvingen, Vec<String>> {
    let mut uit: BTreeMap<(String, String), Geldend> = BTreeMap::new();
    let mut fouten = Vec::new();
    let mut ids: Vec<&str> = service.list_laws();
    ids.sort_unstable();
    for id in ids {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        if law.regulatory_layer != RegulatoryLayer::Uitvoeringsbeleid {
            continue;
        }
        for a in &law.articles {
            let Some(origins) = a.machine_readable.as_ref().and_then(|m| m.origins.as_ref()) else {
                continue;
            };
            let van_actor = besluit::gezag_van(service, id, &a.number)
                .is_some_and(|g| besluit::genormaliseerd(&g) == besluit::genormaliseerd(actor));
            if !van_actor {
                continue;
            }
            let artikel = format!("{id}#{}", a.number);
            for o in origins {
                if let Err(f) = regelingen::ontleed(&o.origin.grondslag) {
                    fouten.push(format!("origins in {artikel}: {f}"));
                    continue;
                }
                if !declareert(service, &o.regulation, &o.parameter) {
                    fouten.push(format!(
                        "origins in {artikel}: regeling '{}' heeft geen parameter '{}'",
                        o.regulation, o.parameter
                    ));
                    continue;
                }
                let nieuw = Geldend {
                    origin: o.origin.clone(),
                    beleid: Some(artikel.clone()),
                };
                let sleutel = (o.regulation.clone(), o.parameter.clone());
                match uit.get(&sleutel) {
                    Some(eerder) if eerder.origin != nieuw.origin => fouten.push(format!(
                        "origins: '{}' van {} krijgt twee herkomsten: {} en {}",
                        o.parameter,
                        o.regulation,
                        eerder.beschrijving(),
                        nieuw.beschrijving()
                    )),
                    Some(_) => {}
                    None => {
                        uit.insert(sleutel, nieuw);
                    }
                }
            }
        }
    }
    if fouten.is_empty() {
        Ok(Overschrijvingen(uit))
    } else {
        Err(fouten)
    }
}

/// Of een geladen regeling ergens een parameter met deze naam declareert.
fn declareert(service: &LawExecutionService, regeling: &str, parameter: &str) -> bool {
    service.resolver().get_law(regeling).is_some_and(|l| {
        l.articles
            .iter()
            .any(|a| a.get_parameters().iter().any(|p| p.name == parameter))
    })
}

/// De parameter achter een [`Benodigd`].
pub fn parameter<'s>(service: &'s LawExecutionService, b: &Benodigd) -> Option<&'s Parameter> {
    regelingen::artikel(service, &b.artikel)
        .ok()?
        .get_parameters()
        .iter()
        .find(|p| p.name == b.naam)
}

impl Overschrijvingen {
    /// De geldende herkomst van een parameter van een regeling: die uit het
    /// beleid, anders die uit de wet.
    pub fn geldend(&self, regeling: &str, p: &Parameter) -> Option<Geldend> {
        if let Some(g) = self.0.get(&(regeling.to_string(), p.name.clone())) {
            return Some(g.clone());
        }
        p.origin.clone().map(|origin| Geldend {
            origin,
            beleid: None,
        })
    }
}

/// Een uitkomst die het proces uitvoert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Uitvoering {
    Toets,
    Aanbod,
    Besluit,
}

impl Uitvoering {
    fn naam(self) -> &'static str {
        match self {
            Uitvoering::Toets => "toets",
            Uitvoering::Aanbod => "aanbod",
            Uitvoering::Besluit => "besluit",
        }
    }
}

/// Een synthese-bron die een parameter levert.
#[derive(Debug, Clone)]
struct BronLevering {
    cel: String,
    lexostatus: String,
    /// Zonder url: de cel draait in deze runtime en is na te gaan.
    intern: bool,
}

/// Wat het proces kan leveren, per soort leverancier.
#[derive(Debug, Default)]
struct Leveranciers {
    /// Afleidingen van de toets-lexostatus en de lexostatussen van de zaak,
    /// en de synthese per regel uit een tabel van de zaak.
    eigen: BTreeSet<String>,
    /// Afleidingen die alleen `$intake` lezen.
    kanaal: BTreeSet<String>,
    /// De stand bij besluit.
    stand: BTreeSet<String>,
    /// Of het portaal tijdvakken aanbiedt (`aanbod.keuzes`).
    keuzes: bool,
    /// Synthese-bronnen, ook die per regel.
    bronnen: BTreeMap<String, Vec<BronLevering>>,
}

impl Leveranciers {
    fn van(d: &ProcesDefinitie, cel: &Cel) -> Self {
        let mut l = Leveranciers::default();
        let mut eigen_lexostatussen: Vec<&str> =
            d.zaakbronnen().map(|b| b.lexostatus.as_str()).collect();
        if let Some(p) = &d.portaal {
            l.keuzes = p.aanbod.as_ref().is_some_and(|a| a.keuzes.is_some());
            eigen_lexostatussen.push(&p.toets.lexostatus);
            if let (Some((_, event)), Some(def)) = (
                cel.event(&p.stroom, &p.event),
                cel.lexostatussen.lexostatus(&p.toets.lexostatus),
            ) {
                let intake: BTreeSet<String> = event
                    .bladeren()
                    .into_iter()
                    .filter(|b| matches!(b.binding, Binding::Intake(_)))
                    .map(|b| b.pad)
                    .collect();
                for (naam, afleiding) in &def.reduction.afleidingen {
                    let paden = afleiding.gelezen_paden();
                    if !paden.is_empty() && paden.iter().all(|pad| intake.contains(*pad)) {
                        l.kanaal.insert(naam.clone());
                    }
                }
            }
        }
        for naam in &eigen_lexostatussen {
            if let Some(def) = cel.lexostatussen.lexostatus(naam) {
                l.eigen.extend(def.reduction.afleidingen.keys().cloned());
            }
        }
        for b in d.andere_bronnen() {
            for p in &b.parameters {
                l.bronnen.entry(p.clone()).or_default().push(BronLevering {
                    cel: b.cel.clone(),
                    lexostatus: b.lexostatus.clone(),
                    intern: b.url.is_none(),
                });
            }
        }
        if let Some(b) = d.behandeling.as_ref().map(|b| &b.besluit) {
            l.stand.extend(b.stand_bij_besluit.keys().cloned());
            for r in &b.rijen {
                l.rijen(d, r);
            }
        }
        l
    }

    /// De synthese per regel: uit een tabel van de zaak is het een eigen
    /// levering, uit een extra veld van een bron een levering van die bron.
    fn rijen(&mut self, d: &ProcesDefinitie, r: &RijenDefinitie) {
        let bron = d
            .andere_bronnen()
            .find(|b| b.lexostatus == r.tabel.lexostatus && b.extra_velden.contains(&r.tabel.veld));
        match bron {
            Some(b) => self
                .bronnen
                .entry(r.parameter.clone())
                .or_default()
                .push(BronLevering {
                    cel: b.cel.clone(),
                    lexostatus: b.lexostatus.clone(),
                    intern: b.url.is_none(),
                }),
            None => {
                self.eigen.insert(r.parameter.clone());
            }
        }
    }

    /// Wie deze parameter nu levert, in woorden, voor een melding.
    fn nu(&self, naam: &str) -> Option<String> {
        if let Some(b) = self.bronnen.get(naam).and_then(|v| v.first()) {
            return Some(format!("synthese-bron {}/{}", b.cel, b.lexostatus));
        }
        if self.kanaal.contains(naam) {
            return Some("de login ($intake)".into());
        }
        if self.eigen.contains(naam) {
            return Some("een eigen lexostatus".into());
        }
        if self.stand.contains(naam) {
            return Some("de stand bij besluit".into());
        }
        None
    }
}

/// Wat de controle oplevert.
#[derive(Debug, Default)]
pub struct Controle {
    pub fouten: Vec<String>,
    pub waarschuwingen: Vec<String>,
    /// Per uitvoering de parameters met hun geldende herkomst, in de volgorde
    /// van declaratie.
    pub parameters: BTreeMap<&'static str, Vec<(Benodigd, Option<Geldend>)>>,
    /// De parameter van het aanbod-artikel die het tijdvak is: origin
    /// BELANGHEBBENDE met grondslag Awb 4:2 lid 1.
    pub tijdvak: Option<String>,
}

/// De parameters die de aanroeper van een uitkomst moet leveren, in de
/// volgorde waarin het artikel (en daarna elk aangeroepen artikel) ze
/// declareert.
fn in_volgorde(service: &LawExecutionService, regeling: &str, artikel: &Article) -> Vec<Benodigd> {
    let benodigd = regelingen::benodigde_parameters(service, regeling, artikel);
    let mut uit: Vec<Benodigd> = Vec::new();
    for p in artikel.get_parameters() {
        if let Some(b) = benodigd.get(&p.name) {
            uit.push(b.clone());
        }
    }
    for b in benodigd.values() {
        if !uit.iter().any(|u| u.naam == b.naam) {
            uit.push(b.clone());
        }
    }
    uit
}

/// Controleer de herkomst van elke parameter van de uitkomsten die het
/// proces uitvoert. `cellen` zijn de cellen van de runtime, voor de
/// register-bronnen.
pub fn controleer(
    d: &ProcesDefinitie,
    cel: &Cel,
    cellen: &BTreeMap<String, Arc<Cel>>,
    service: &LawExecutionService,
) -> Controle {
    let mut c = Controle::default();
    let overschrijvingen = match overschrijvingen(service, &d.actor) {
        Ok(o) => o,
        Err(f) => {
            c.fouten.extend(f);
            return c;
        }
    };
    let leveranciers = Leveranciers::van(d, cel);
    let mut uitvoeringen: Vec<(Uitvoering, &str, &str)> = Vec::new();
    if let Some(p) = &d.portaal {
        uitvoeringen.push((Uitvoering::Toets, &p.toets.regeling, &p.toets.uitkomst));
        if let Some(a) = &p.aanbod {
            uitvoeringen.push((Uitvoering::Aanbod, &a.regeling, &a.uitkomst));
        }
    }
    if let Some(b) = d.behandeling.as_ref().map(|b| &b.besluit) {
        if let Some(u) = b.uitkomsten.first() {
            uitvoeringen.push((Uitvoering::Besluit, &b.regeling, u));
        }
    }
    // Een melding per parameter van een artikel, ook als twee uitvoeringen
    // hem vragen.
    let mut gemeld: BTreeSet<(String, String, &'static str)> = BTreeSet::new();
    for (uitvoering, regeling, uitkomst) in uitvoeringen {
        // Een uitkomst die niet bestaat, meldt de controle op portaal of
        // besluit.
        let Some(artikel) = service
            .resolver()
            .get_article_by_output(regeling, uitkomst, None)
        else {
            continue;
        };
        let mut lijst = Vec::new();
        for b in in_volgorde(service, regeling, artikel) {
            let Some(p) = parameter(service, &b) else {
                continue;
            };
            let law = b
                .artikel
                .split_once('#')
                .map(|(r, _)| r)
                .unwrap_or(regeling);
            let g = overschrijvingen.geldend(law, p);
            let mut eenmaal = |soort: &'static str, tekst: String, fout: bool| {
                if gemeld.insert((b.artikel.clone(), b.naam.clone(), soort)) {
                    if fout {
                        c.fouten.push(tekst);
                    } else {
                        c.waarschuwingen.push(tekst);
                    }
                }
            };
            if uitvoering == Uitvoering::Aanbod && !vooraf_bekend(g.as_ref()) {
                let herkomst = g
                    .as_ref()
                    .map(Geldend::beschrijving)
                    .unwrap_or_else(|| "geen origin".into());
                eenmaal(
                    "aanbod-regel",
                    format!(
                        "aanbod: voorwaarde leunt op '{}' ({herkomst}), dat vooraf niet bekend is",
                        b.naam
                    ),
                    true,
                );
            }
            match &g {
                None => eenmaal(
                    "zonder",
                    format!(
                        "herkomst: parameter '{}' van {} heeft geen origin; wie hem levert is niet na te gaan",
                        b.naam, b.artikel
                    ),
                    false,
                ),
                Some(g) => {
                    if let Err(f) = regelingen::ontleed(&g.origin.grondslag) {
                        eenmaal(
                            "grondslag",
                            format!("herkomst: parameter '{}' van {}: {f}", b.naam, b.artikel),
                            true,
                        );
                    }
                    if g.origin.waarde == OriginValue::Belanghebbende && p.required != Some(false)
                    {
                        eenmaal(
                            "required",
                            format!(
                                "herkomst: parameter '{}' van {} komt van de belanghebbende, maar heeft geen required: false (RFC-036)",
                                b.naam, b.artikel
                            ),
                            false,
                        );
                    }
                    if let Some(probleem) =
                        geen_leverancier(uitvoering, &b, g, &leveranciers, cellen, service)
                    {
                        let tekst = format!(
                            "{}: geen leverancier voor '{}' van {} ({}){probleem}",
                            uitvoering.naam(),
                            b.naam,
                            b.artikel,
                            g.beschrijving()
                        );
                        if p.required == Some(false) {
                            eenmaal(
                                "leverancier",
                                format!("{tekst}; required: false, dus de engine rekent zonder"),
                                false,
                            );
                        } else {
                            eenmaal("leverancier", tekst, true);
                        }
                    }
                }
            }
            lijst.push((b, g));
        }
        c.parameters.insert(uitvoering.naam(), lijst);
    }
    tijdvak(d, &mut c);
    c
}

/// Het tijdvak van het aanbod: hoogstens een parameter met de grondslag van
/// de gevraagde beschikking, en die vraagt `aanbod.keuzes`; keuzes zonder
/// zo'n parameter zijn ook een fout.
fn tijdvak(d: &ProcesDefinitie, c: &mut Controle) {
    let Some(aanbod) = d.portaal.as_ref().and_then(|p| p.aanbod.as_ref()) else {
        return;
    };
    let namen: Vec<String> = c
        .parameters
        .get(Uitvoering::Aanbod.naam())
        .into_iter()
        .flatten()
        .filter(|(_, g)| g.as_ref().is_some_and(Geldend::is_gevraagde_beschikking))
        .map(|(b, _)| b.naam.clone())
        .collect();
    match (namen.as_slice(), aanbod.keuzes.is_some()) {
        ([], false) => {}
        ([], true) => c.fouten.push(format!(
            "aanbod: keuzes, maar {} vraagt geen tijdvak (een parameter met origin BELANGHEBBENDE en grondslag {GEVRAAGDE_BESCHIKKING})",
            aanbod.regeling
        )),
        ([naam], true) => c.tijdvak = Some(naam.clone()),
        ([naam], false) => c.fouten.push(format!(
            "aanbod: het tijdvak '{naam}' ({GEVRAAGDE_BESCHIKKING}) vraagt aanbod.keuzes: welke tijdvakken het portaal aanbiedt"
        )),
        (meer, _) => c.fouten.push(format!(
            "aanbod: meer dan een tijdvak ({}); het portaal biedt er een aan",
            meer.join(", ")
        )),
    }
}

/// Het besluitformulier: de parameters van het besluit met origin
/// `OORDEEL`, in de volgorde van declaratie. Het label is de omschrijving van
/// de parameter, of het deel na "Naam:" als dat er staat, en anders de naam;
/// de groep is het artikel van de grondslag.
pub fn oordelen(c: &Controle, service: &LawExecutionService) -> Vec<Oordeel> {
    c.parameters
        .get(Uitvoering::Besluit.naam())
        .into_iter()
        .flatten()
        .filter_map(|(b, g)| {
            let g = g
                .as_ref()
                .filter(|g| g.origin.waarde == OriginValue::Oordeel)?;
            let p = parameter(service, b)?;
            Some(Oordeel {
                parameter: b.naam.clone(),
                label: label(p),
                groep: groep(service, &g.origin.grondslag),
                uitleg: None,
            })
        })
        .collect()
}

/// Het label van een parameter: het deel van de omschrijving na "Naam:",
/// anders de hele omschrijving, anders de naam.
fn label(p: &Parameter) -> String {
    let tekst = p.description.as_deref().map(str::trim).unwrap_or_default();
    let tekst = match tekst.rsplit_once("Naam:") {
        Some((_, naam)) => naam.trim(),
        None => tekst,
    };
    let tekst = tekst.strip_suffix('.').unwrap_or(tekst).trim();
    if tekst.is_empty() {
        p.name.clone()
    } else {
        tekst.split_whitespace().collect::<Vec<_>>().join(" ")
    }
}

/// De groep van een oordeel: de regeling en het artikel van zijn grondslag.
fn groep(service: &LawExecutionService, grondslag: &str) -> Option<String> {
    let g = regelingen::ontleed(grondslag).ok()?;
    let naam = service
        .resolver()
        .get_law(g.regeling)
        .and_then(|l| l.name.clone())
        .unwrap_or_else(|| leesbaar(g.regeling));
    Some(format!("{naam}, artikel {}", g.artikel))
}

/// Een `$id` als naam, voor een regeling zonder `name`:
/// `een_regeling` wordt "Een regeling".
fn leesbaar(id: &str) -> String {
    let tekst = id.replace('_', " ");
    let mut tekens = tekst.chars();
    match tekens.next() {
        Some(eerste) => eerste.to_uppercase().chain(tekens).collect(),
        None => tekst,
    }
}

/// Of het aanbod op een parameter mag leunen: wat vooraf vaststaat, is wie
/// inlogt (`KANAAL`), wat een register weet (`REGISTER`) en wat de aanvrager
/// vraagt (`BELANGHEBBENDE` met grondslag Awb 4:2 lid 1, zoals het tijdvak).
/// Of een aanvraag volledig is, weet je pas na het invullen.
fn vooraf_bekend(g: Option<&Geldend>) -> bool {
    g.is_some_and(|g| {
        matches!(g.origin.waarde, OriginValue::Kanaal | OriginValue::Register)
            || g.is_gevraagde_beschikking()
    })
}

/// Waarom een parameter geen leverancier heeft die bij zijn herkomst past, of
/// `None` als hij er een heeft. De tekst begint met `: ` of is leeg.
fn geen_leverancier(
    uitvoering: Uitvoering,
    b: &Benodigd,
    g: &Geldend,
    l: &Leveranciers,
    cellen: &BTreeMap<String, Arc<Cel>>,
    service: &LawExecutionService,
) -> Option<String> {
    let naam = b.naam.as_str();
    let nu = || {
        l.nu(naam)
            .map(|n| format!("; nu komt hij uit {n}"))
            .unwrap_or_default()
    };
    let past = match g.origin.waarde {
        OriginValue::Kanaal => l.kanaal.contains(naam),
        OriginValue::Belanghebbende => {
            l.eigen.contains(naam)
                || (uitvoering == Uitvoering::Aanbod && g.is_gevraagde_beschikking() && l.keuzes)
        }
        OriginValue::Dossier => {
            l.eigen.contains(naam) || (uitvoering == Uitvoering::Besluit && l.stand.contains(naam))
        }
        OriginValue::Oordeel => uitvoering == Uitvoering::Besluit,
        OriginValue::Register => {
            let Some(bronnen) = l.bronnen.get(naam) else {
                return Some(nu());
            };
            let mut redenen = Vec::new();
            for bron in bronnen {
                match register_bron(bron, naam, g, cellen, service) {
                    Ok(()) => return None,
                    Err(r) => redenen.push(r),
                }
            }
            return Some(format!(": {}", redenen.join("; ")));
        }
    };
    (!past).then(nu)
}

/// Of een synthese-bron een register-parameter mag leveren: haar
/// lexostatus noemt in `levert_aan` een artikel dat hem vraagt, en houdt een
/// kroniek bij met een grondslag in het register van de herkomst. Een bron
/// met een url is niet na te gaan en telt.
fn register_bron(
    bron: &BronLevering,
    naam: &str,
    g: &Geldend,
    cellen: &BTreeMap<String, Arc<Cel>>,
    service: &LawExecutionService,
) -> Result<(), String> {
    let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
    if !bron.intern {
        return Ok(());
    }
    let Some(cel) = cellen.get(&bron.cel) else {
        // Een interne bron die niet draait, meldt de runtime als waarschuwing.
        return Ok(());
    };
    let Some(def) = cel.lexostatussen.lexostatus(&bron.lexostatus) else {
        return Err(format!("{wie} bestaat niet"));
    };
    let vraagt = def.levert_aan.iter().any(|a| {
        regelingen::artikel(service, a)
            .is_ok_and(|a| a.get_parameters().iter().any(|p| p.name == naam))
    });
    if !vraagt {
        return Err(format!(
            "{wie} noemt in levert_aan geen artikel dat '{naam}' vraagt"
        ));
    }
    if let Some(register) = &g.origin.register {
        let houdt_bij = cel
            .strommen
            .iter()
            .filter(|s| s.chronicle == def.reduction.kroniek)
            .flat_map(|s| s.events.iter())
            .flat_map(|e| e.grondslag.iter())
            .any(|gr| regelingen::ontleed(gr).is_ok_and(|gr| gr.regeling == register));
        if !houdt_bij {
            return Err(format!(
                "{wie} houdt geen kroniek bij met een grondslag in '{register}'"
            ));
        }
    }
    Ok(())
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use std::path::{Path, PathBuf};

    fn fixtures() -> PathBuf {
        Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
    }

    /// De fixture-regelingen, een aanpassing op de regeling van de afnemer,
    /// en extra regelingen.
    fn service(pas_aan: impl Fn(String) -> String, extra: &[&str]) -> Arc<LawExecutionService> {
        let mut s = LawExecutionService::new();
        for e in walkdir::WalkDir::new(fixtures().join("regulation")) {
            let e = e.unwrap();
            if e.file_type().is_file() {
                let mut tekst = std::fs::read_to_string(e.path()).unwrap();
                if tekst.contains("$id: testregeling_afnemer") {
                    tekst = pas_aan(tekst);
                }
                s.load_law(&tekst).unwrap();
            }
        }
        for t in extra {
            s.load_law(t).unwrap();
        }
        Arc::new(s)
    }

    fn cellen(s: &Arc<LawExecutionService>) -> BTreeMap<String, Arc<Cel>> {
        crate::config::celmappen(&fixtures().join("cellen"))
            .unwrap()
            .iter()
            .map(|m| {
                let c = Cel::laad(m, s.clone()).unwrap();
                (c.id().to_string(), Arc::new(c))
            })
            .collect()
    }

    fn proces(naam: &str, pas_aan: impl Fn(String) -> String) -> ProcesDefinitie {
        let pad = fixtures().join("processes").join(naam).join("proces.yaml");
        let tekst = pas_aan(std::fs::read_to_string(&pad).unwrap());
        ProcesDefinitie::parse(&tekst, "t").unwrap()
    }

    /// De controle op het proces van de afnemer.
    fn afnemer(
        regeling: impl Fn(String) -> String,
        pas_aan: impl Fn(String) -> String,
        extra: &[&str],
    ) -> Controle {
        let s = service(regeling, extra);
        let c = cellen(&s);
        let d = proces("afnemer", pas_aan);
        controleer(&d, &c["test_afnemer"], &c, &s)
    }

    fn zo(t: String) -> String {
        t
    }

    #[test]
    fn de_fixture_van_de_afnemer_heeft_voor_alles_een_leverancier() {
        let c = afnemer(zo, zo, &[]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert!(c.waarschuwingen.is_empty(), "{:?}", c.waarschuwingen);
        let besluit: Vec<(&str, OriginValue)> = c.parameters["besluit"]
            .iter()
            .map(|(b, g)| (b.naam.as_str(), g.as_ref().unwrap().origin.waarde))
            .collect();
        assert!(besluit.contains(&("feiten_vergaard", OriginValue::Oordeel)));
        assert!(besluit.contains(&("jaar", OriginValue::Register)));
    }

    /// Een verplichte parameter zonder leverancier houdt de runtime tegen, met
    /// parameter, herkomst en grondslag in de melding.
    #[test]
    fn een_ontbrekende_leverancier_is_een_fout() {
        let c = afnemer(
            zo,
            |t| t.replace("      datum_bekendmaking: null\n", ""),
            &[],
        );
        assert_eq!(
            c.fouten,
            ["besluit: geen leverancier voor 'datum_bekendmaking' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2)"]
        );
    }

    /// Met required: false rekent de engine zonder: een waarschuwing.
    #[test]
    fn zonder_leverancier_en_niet_verplicht_is_een_waarschuwing() {
        let c = afnemer(zo, |t| t.replace("      bekendgemaakt: false\n", ""), &[]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert_eq!(
            c.waarschuwingen,
            ["besluit: geen leverancier voor 'bekendgemaakt' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2); required: false, dus de engine rekent zonder"]
        );
    }

    /// Een leverancier van de verkeerde soort telt niet, en de melding zegt
    /// waar de parameter nu vandaan komt.
    #[test]
    fn een_leverancier_die_niet_bij_de_herkomst_past() {
        let c = afnemer(
            |t| {
                t.replace(
                    "origin: {waarde: REGISTER, register: testregeling_register, grondslag: testregeling_register#3}\n          - name: gebiedstabel",
                    "origin: {waarde: DOSSIER, grondslag: 'testregeling_afnemer#3'}\n          - name: gebiedstabel",
                )
            },
            zo,
            &[],
        );
        assert_eq!(
            c.fouten,
            ["besluit: geen leverancier voor 'jaar' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3); nu komt hij uit synthese-bron test_register/registerstatus"]
        );
    }

    #[test]
    fn een_register_moet_bij_de_bron_passen() {
        let c = afnemer(
            |t| {
                t.replace(
                    "origin: {waarde: REGISTER, register: testregeling_register, grondslag: testregeling_register#3}\n          - name: gebiedstabel",
                    "origin: {waarde: REGISTER, register: een_ander_register, grondslag: testregeling_register#3}\n          - name: gebiedstabel",
                )
            },
            zo,
            &[],
        );
        assert_eq!(
            c.fouten,
            ["besluit: geen leverancier voor 'jaar' van testregeling_afnemer#3 (REGISTER, register een_ander_register, grondslag testregeling_register#3): synthese-bron test_register/registerstatus houdt geen kroniek bij met een grondslag in 'een_ander_register'"]
        );
    }

    /// Zonder origin, en een belanghebbende-parameter zonder required: false:
    /// waarschuwingen, geen fouten.
    #[test]
    fn waarschuwingen_over_de_herkomst() {
        let s = service(zo, &[]);
        let c = cellen(&s);
        let d = proces("instantie", zo);
        let uit = controleer(&d, &c["test_instantie"], &c, &s);
        assert!(uit.fouten.is_empty(), "{:?}", uit.fouten);
        assert!(uit.waarschuwingen.contains(
            &"herkomst: parameter 'bevat_naam' van testregeling_aanvraag#1 komt van de belanghebbende, maar heeft geen required: false (RFC-036)".to_string()
        ), "{:?}", uit.waarschuwingen);
        // bevat_aantal_aanduidingen heeft required: false.
        assert!(!uit
            .waarschuwingen
            .iter()
            .any(|w| w.contains("'bevat_aantal_aanduidingen'")));

        let s = service(
            |t| {
                t.replace("            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen", "          - name: opgeschorte_dagen")
            },
            &[],
        );
        let c = cellen(&s);
        let uit = controleer(&proces("afnemer", zo), &c["test_afnemer"], &c, &s);
        assert_eq!(
            uit.waarschuwingen,
            ["herkomst: parameter 'datum_uitnodiging_aanvulling' van testregeling_afnemer#3 heeft geen origin; wie hem levert is niet na te gaan"]
        );
    }

    /// Het betalingsvoorbeeld: een regeling die het vastgestelde en het
    /// betaalde bedrag uit het dossier vraagt, uitgevoerd door een proces
    /// zonder lexostatus die het betaalde bedrag levert.
    const BETALING: &str = r#"
$id: testregeling_betaling
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: |-
      1. Het bedrag wordt overeenkomstig de vaststelling betaald.
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters:
          - name: vastgesteld_bedrag
            type: number
            origin: {waarde: DOSSIER, grondslag: 'testregeling_betaling#1 lid 1'}
          - name: betaald_bedrag
            type: number
            origin: {waarde: DOSSIER, grondslag: 'testregeling_betaling#1 lid 1'}
        output:
          - name: nog_te_betalen
            type: number
        actions:
          - output: nog_te_betalen
            value:
              operation: MAX
              values:
                - 0
                - operation: SUBTRACT
                  values: [$vastgesteld_bedrag, $betaald_bedrag]
"#;

    #[test]
    fn het_betalingsvoorbeeld_mist_een_leverancier() {
        let c = afnemer(
            zo,
            |t| {
                t.replace("    regeling: testregeling_afnemer\n", "    regeling: testregeling_betaling\n")
                    .replace(
                        "uitkomsten: [vastgesteld_bedrag, gebiedsbedrag, besluit_tijdig, besluitdeadline, zorgvuldig]",
                        "uitkomsten: [nog_te_betalen]",
                    )
            },
            &[BETALING],
        );
        assert_eq!(
            c.fouten,
            [
                "besluit: geen leverancier voor 'vastgesteld_bedrag' van testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
                "besluit: geen leverancier voor 'betaald_bedrag' van testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
            ]
        );
    }

    /// Een aanbod dat een dossierfeit vraagt, houdt de runtime tegen.
    #[test]
    fn een_aanbod_op_een_dossierfeit_is_een_fout() {
        let c = afnemer(
            zo,
            |t| {
                t.replace(
                    "    uitkomst: aanvraag_toelaatbaar\n",
                    "    uitkomst: aanvraag_toelaatbaar\n  aanbod: {regeling: testregeling_afnemer, uitkomst: besluitdeadline}\n",
                )
            },
            &[],
        );
        assert!(c.fouten.contains(
            &"aanbod: voorwaarde leunt op 'opgeschorte_dagen' (DOSSIER, grondslag testregeling_afnemer#3 lid 1), dat vooraf niet bekend is".to_string()
        ), "{:?}", c.fouten);
        assert!(c.fouten.contains(
            &"aanbod: voorwaarde leunt op 'aanvraagdatum' (BELANGHEBBENDE, grondslag testregeling_afnemer#1), dat vooraf niet bekend is".to_string()
        ), "{:?}", c.fouten);
        // Registerfeiten mogen.
        assert!(
            !c.fouten.iter().any(|f| f.contains("'jaar'")),
            "{:?}",
            c.fouten
        );
    }

    /// Een aanbod op register- en loginfeiten mag.
    #[test]
    fn een_aanbod_op_registerfeiten() {
        let c = afnemer(
            zo,
            |t| {
                t.replace(
                    "    uitkomst: aanvraag_toelaatbaar\n",
                    "    uitkomst: aanvraag_toelaatbaar\n  aanbod: {regeling: testregeling_afnemer, uitkomst: lijst_heeft_zetels}\n",
                )
            },
            &[],
        );
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    }

    /// Het besluitformulier: de OORDEEL-parameters van het besluit, met het
    /// label na "Naam:" en de groep uit de grondslag.
    #[test]
    fn het_besluitformulier_volgt_uit_origin() {
        let s = service(zo, &[]);
        let c = cellen(&s);
        let uit = controleer(&proces("afnemer", zo), &c["test_afnemer"], &c, &s);
        let o = oordelen(&uit, &s);
        let velden: Vec<(&str, &str, Option<&str>)> = o
            .iter()
            .map(|o| (o.parameter.as_str(), o.label.as_str(), o.groep.as_deref()))
            .collect();
        assert_eq!(
            velden,
            [
                (
                    "besluitdatum",
                    "Besluitdatum",
                    Some("Testregeling afnemer, artikel 3")
                ),
                (
                    "feiten_vergaard",
                    "De relevante feiten zijn vergaard",
                    Some("Testregeling afnemer, artikel 3")
                ),
            ]
        );
        // Zonder "Naam:" is het label de omschrijving, zonder omschrijving de
        // naam.
        let s = service(
            |t| {
                t.replace(
                    "'Het oordeel van de instantie bij het besluiten. Naam: Besluitdatum.'",
                    "De dag van het besluit.",
                )
            },
            &[],
        );
        let c = cellen(&s);
        let uit = controleer(&proces("afnemer", zo), &c["test_afnemer"], &c, &s);
        assert_eq!(oordelen(&uit, &s)[0].label, "De dag van het besluit");
        assert_eq!(
            leesbaar("een_regeling_zonder_naam"),
            "Een regeling zonder naam"
        );
    }

    /// Uitvoeringsbeleid van de actor geeft `jaar` een andere herkomst.
    const BELEID: &str = r#"
$id: testbeleid_afnemer
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
competent_authority: {name: Test afnemer}
articles:
  - number: '1'
    text: |-
      1. De afnemer stelt het jaar zelf vast.
    machine_readable:
      origins:
        - regulation: testregeling_afnemer
          parameter: jaar
          origin: {waarde: DOSSIER, grondslag: 'testbeleid_afnemer#1 lid 1'}
"#;

    #[test]
    fn een_overschrijving_in_beleid_wint() {
        let c = afnemer(zo, zo, &[BELEID]);
        assert_eq!(
            c.fouten,
            ["besluit: geen leverancier voor 'jaar' van testregeling_afnemer#3 (DOSSIER, grondslag testbeleid_afnemer#1 lid 1, uit testbeleid_afnemer#1); nu komt hij uit synthese-bron test_register/registerstatus"]
        );
        // Beleid van een ander gezag telt niet.
        let ander = BELEID.replace("name: Test afnemer", "name: Een ander");
        let c = afnemer(zo, zo, &[&ander]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
    }

    #[test]
    fn twee_botsende_overschrijvingen_zijn_een_fout() {
        let tweede = format!(
            "{BELEID}  - number: '2'\n    text: Tweede.\n    machine_readable:\n      origins:\n        - regulation: testregeling_afnemer\n          parameter: jaar\n          origin: {{waarde: REGISTER, register: testregeling_register, grondslag: 'testbeleid_afnemer#2'}}\n"
        );
        let c = afnemer(zo, zo, &[&tweede]);
        assert_eq!(
            c.fouten,
            ["origins: 'jaar' van testregeling_afnemer krijgt twee herkomsten: DOSSIER, grondslag testbeleid_afnemer#1 lid 1, uit testbeleid_afnemer#1 en REGISTER, register testregeling_register, grondslag testbeleid_afnemer#2, uit testbeleid_afnemer#2"]
        );
        // Een overschrijving van een parameter die niet bestaat.
        let onbekend = BELEID.replace("parameter: jaar", "parameter: bestaat_niet");
        let c = afnemer(zo, zo, &[&onbekend]);
        assert_eq!(
            c.fouten,
            ["origins in testbeleid_afnemer#1: regeling 'testregeling_afnemer' heeft geen parameter 'bestaat_niet'"]
        );
    }
}
