//! Wie een parameter levert, volgens de wet: `origin` op een parameter en
//! `origins` in uitvoeringsbeleid (RFC-043).
//!
//! De wet geeft per parameter de herkomst met een grondslag. Uitvoeringsbeleid
//! van de actor van het proces kan die overschrijven. Bij het opstarten gaat
//! het proces na dat elke parameter die de aanroeper van een uitgevoerde
//! uitkomst (toets, aanbod, elke uitkomst van het besluit) moet leveren, een
//! leverancier heeft die bij zijn herkomst past:
//!
//! | herkomst | leverancier |
//! |---|---|
//! | `BELANGHEBBENDE` | een afleiding van een eigen lexostatus die alleen indieningen leest (`type: indiening`: wat de aanvrager aanlevert, zijn inhoud of zijn login); het tijdvak (`rol: TIJDVAK`) bij het aanbod ook de keuze in het portaal |
//! | `DOSSIER` | een afleiding van een eigen lexostatus die alleen andere grammen van de eigen actor leest (het verloop van de zaak), of de stand bij besluit |
//! | `OORDEEL` | het besluitformulier, alleen bij het besluit, en verder niemand |
//! | `REGISTER` | een synthese-bron die de parameter levert (onder de naam die de synthese van het proces eraan geeft), en die een kroniek bijhoudt met een grondslag in `register` |
//! | `KANAAL` | een afleiding die alleen `$intake` leest |
//!
//! Een parameter die een leverancier heeft die niet bij zijn herkomst past, is
//! altijd een fout, ook naast een leverancier die wel past: de bron is dan
//! verkeerd. Heeft hij geen leverancier, dan is dat een fout, behalve bij
//! `required: false`: dan krijgt de engine hem niet, en rekent ze met een
//! onbekende waarde (RFC-036); dat is een waarschuwing. Een parameter zonder
//! `origin` is een waarschuwing, en in een proces met `herkomst: streng` een
//! fout. Een `BELANGHEBBENDE`-parameter zonder `required: false` is een
//! waarschuwing (RFC-036), behalve het tijdvak.
//!
//! Wat de runtime niet kan nagaan (een bron met een url, een interne cel die
//! niet draait) telt als leverancier, met een waarschuwing die zegt waarom het
//! niet na te gaan is.
//!
//! [`valideer`] controleert de vorm van `origin` en `origins` in een regeling
//! bij het laden, met het bestand in de melding: een ongeldige waarde houdt
//! de engine niet tegen (zie `Declared` in law-model), de runtime wel.

use std::collections::{BTreeMap, BTreeSet};
use std::sync::Arc;

use regelrecht_engine::{Article, LawExecutionService, RegulatoryLayer};
use regelrecht_law_model::{
    ArticleBasedLaw, Declared, Origin, OriginOverride, OriginRole, OriginValue, Parameter,
};

use crate::cel::Cel;
use crate::config::{
    HandelingDefinitie, Handelingsoort, Herkomstcontrole, Oordeel, ProcesDefinitie, RijenDefinitie,
};
use crate::gezag;
use crate::reductie::{Afleiding, Filter, LexostatusDefinitie};
use crate::regelingen::{self, Benodigd};
use crate::stroom::{Binding, Event, Stroom};

/// Het type van een gram dat een belanghebbende indient (RFC-022 par. 1,
/// `schema/chronolex/v0.1.0/stream.json`): wat de aanvrager aanlevert.
const INDIENING: &str = "indiening";

/// De herkomst die voor een parameter geldt, en waar ze staat.
#[derive(Debug, Clone, PartialEq)]
pub struct Geldend {
    pub origin: Origin,
    /// Het artikel van het beleid dat haar overschrijft; `None` als ze uit
    /// de wet komt.
    pub beleid: Option<String>,
}

impl Geldend {
    /// Voor een melding: `REGISTER, register een_registerwet, grondslag x#1`.
    pub fn beschrijving(&self) -> String {
        let mut s = self.origin.waarde.as_str().to_string();
        if let Some(r) = &self.origin.register {
            s.push_str(&format!(", register {r}"));
        }
        if let Some(r) = self.origin.rol {
            s.push_str(&format!(", rol {}", r.as_str()));
        }
        s.push_str(&format!(", grondslag {}", self.origin.grondslag));
        if let Some(b) = &self.beleid {
            s.push_str(&format!(", uit {b}"));
        }
        s
    }

    /// Of de parameter het tijdvak van de gevraagde beschikking is
    /// (`rol: TIJDVAK`). Los van de grondslag: Awb 4:2 lid 1 is ook de
    /// grondslag van andere onderdelen van de aanvraag.
    pub fn is_tijdvak(&self) -> bool {
        self.origin.rol == Some(OriginRole::Tijdvak)
    }
}

/// De vorm van een `origin` op zichzelf, los van het proces: de grondslag is
/// te ontleden, `REGISTER` noemt zijn register en alleen `REGISTER` doet dat,
/// en een tijdvak komt van de belanghebbende.
fn vorm(o: &Origin) -> Vec<String> {
    let mut fouten = Vec::new();
    if let Err(f) = regelingen::ontleed(&o.grondslag) {
        fouten.push(f);
    }
    match (o.waarde, o.register.as_deref().map(str::trim)) {
        (OriginValue::Register, None | Some("")) => fouten.push(
            "origin REGISTER zonder register: welke regeling het register houdt, is niet na te gaan"
                .into(),
        ),
        (OriginValue::Register, Some(_)) | (_, None) => {}
        (w, Some(r)) => fouten.push(format!(
            "origin {} met register '{r}': alleen REGISTER noemt een register",
            w.as_str()
        )),
    }
    if o.rol == Some(OriginRole::Tijdvak) && o.waarde != OriginValue::Belanghebbende {
        fouten.push(format!(
            "rol TIJDVAK bij origin {}: het tijdvak kiest de aanvrager als deel van de gevraagde beschikking (Awb 4:2 lid 1), dus BELANGHEBBENDE",
            o.waarde.as_str()
        ));
    }
    fouten
}

/// Controleer `origin` op elke parameter en `origins` op elk artikel van een
/// geladen regeling: een waarde die niet te lezen is, of een origin die op
/// zichzelf niet klopt (zie [`vorm`]). Elke melding noemt artikel en
/// parameter; de aanroeper zet het bestand ervoor.
pub fn valideer(law: &ArticleBasedLaw) -> Vec<String> {
    let mut fouten = Vec::new();
    for a in &law.articles {
        for p in a.get_parameters() {
            let Some(o) = &p.origin else { continue };
            let waar = format!("artikel {}, parameter '{}'", a.number, p.name);
            match o.valid() {
                Err(e) => fouten.push(format!("{waar}: ongeldige origin: {e}")),
                Ok(o) => fouten.extend(vorm(o).into_iter().map(|f| format!("{waar}: {f}"))),
            }
        }
        let Some(origins) = a.machine_readable.as_ref().and_then(|m| m.origins.as_ref()) else {
            continue;
        };
        if law.regulatory_layer != RegulatoryLayer::Uitvoeringsbeleid {
            fouten.push(format!(
                "artikel {}: origins staat alleen in uitvoeringsbeleid (RFC-043)",
                a.number
            ));
        }
        for (i, o) in origins.iter().enumerate() {
            match o.valid() {
                Err(e) => fouten.push(format!(
                    "artikel {}, origins[{i}]: ongeldige overschrijving: {e}",
                    a.number
                )),
                Ok(o) => fouten.extend(vorm(&o.origin).into_iter().map(|f| {
                    format!(
                        "artikel {}, origins voor '{}' van {}: {f}",
                        a.number, o.parameter, o.regulation
                    )
                })),
            }
        }
    }
    fouten
}

/// De overschrijvingen door het uitvoeringsbeleid van een actor, per
/// (regeling, parameter).
#[derive(Debug, Default)]
pub struct Overschrijvingen(BTreeMap<(String, String), Geldend>);

/// Lees `origins` uit elk geladen uitvoeringsbeleid waarvan het bevoegd
/// gezag (van het artikel, anders van de regeling) het gezag is waarvoor het
/// proces handelt (`namens`, zie [`crate::gezag`]); zonder dat gezag geen. Twee
/// artikelen die dezelfde parameter een andere herkomst geven, zijn een fout.
pub fn overschrijvingen(
    service: &LawExecutionService,
    gezag: Option<&str>,
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
            let van_actor =
                gezag.is_some() && gezag::gezag_van(service, id, &a.number).as_deref() == gezag;
            if !van_actor {
                continue;
            }
            let artikel = format!("{id}#{}", a.number);
            for o in origins
                .iter()
                .filter_map(Declared::<OriginOverride>::as_valid)
            {
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
    /// beleid, anders die uit de wet. Een origin die niet te lezen is, telt
    /// als geen; het laden van de regeling heeft hem al gemeld.
    pub fn geldend(&self, regeling: &str, p: &Parameter) -> Option<Geldend> {
        if let Some(g) = self.0.get(&(regeling.to_string(), p.name.clone())) {
            return Some(g.clone());
        }
        p.origin
            .as_ref()
            .and_then(Declared::as_valid)
            .map(|origin| Geldend {
                origin: origin.clone(),
                beleid: None,
            })
    }
}

/// Een uitkomst die het proces uitvoert.
#[derive(Debug, Clone, Copy)]
pub enum Uitvoering<'a> {
    Toets,
    Aanbod,
    /// Een handeling in een zaak (geen vervolg: dat rekent op de invoer van
    /// het vastgelegde besluit).
    Handeling(&'a HandelingDefinitie),
}

impl Uitvoering<'_> {
    fn naam(self) -> String {
        match self {
            Uitvoering::Toets => "toets".into(),
            Uitvoering::Aanbod => "aanbod".into(),
            Uitvoering::Handeling(h) => h.naam.clone(),
        }
    }

    fn is_aanbod(self) -> bool {
        matches!(self, Uitvoering::Aanbod)
    }

    fn is_handeling(self) -> bool {
        matches!(self, Uitvoering::Handeling(_))
    }
}

/// Wat een afleiding van een eigen lexostatus leest, naar de grammen die door
/// haar filters kunnen komen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Gelezen {
    /// Alleen `$intake` van een indiening: de login.
    Kanaal,
    /// Alleen indieningen: wat de aanvrager aanlevert.
    Indiening,
    /// Alleen andere grammen van de eigen actor: het verloop van de zaak.
    Verloop,
    /// Beide, of niets dat statisch na te gaan is.
    Onbepaald,
}

impl Gelezen {
    fn woorden(self) -> &'static str {
        match self {
            Gelezen::Kanaal => "de login ($intake)",
            Gelezen::Indiening => "wat de aanvrager indiende",
            Gelezen::Verloop => "het verloop van de zaak",
            Gelezen::Onbepaald => "indieningen en het verloop van de zaak door elkaar",
        }
    }
}

/// Een synthese-bron die een parameter levert.
#[derive(Debug, Clone)]
struct BronLevering {
    cel: String,
    lexostatus: String,
    /// De url van een bron buiten deze runtime; `None`: intern.
    url: Option<String>,
}

/// Een leverancier van een parameter bij een uitvoering.
#[derive(Debug, Clone)]
enum Levering {
    /// Een afleiding van een eigen lexostatus, of een tabel per regel uit een
    /// eigen extra veld.
    Eigen {
        lexostatus: String,
        gelezen: Gelezen,
    },
    /// De stand bij besluit.
    Stand,
    /// De keuze van het tijdvak in het portaal.
    Keuze,
    Bron(BronLevering),
}

impl Levering {
    fn woorden(&self) -> String {
        match self {
            Levering::Eigen {
                lexostatus,
                gelezen,
            } => format!("eigen lexostatus {lexostatus} ({})", gelezen.woorden()),
            Levering::Stand => "de stand bij besluit".into(),
            Levering::Keuze => "de keuze in het portaal".into(),
            Levering::Bron(b) => format!("synthese-bron {}/{}", b.cel, b.lexostatus),
        }
    }

    /// Of deze leverancier bij de herkomst past (een register-bron nog
    /// zonder de controle op het register).
    fn past(&self, g: &Geldend, uitvoering: Uitvoering<'_>) -> bool {
        match (self, g.origin.waarde) {
            (
                Levering::Eigen {
                    gelezen: Gelezen::Kanaal,
                    ..
                },
                OriginValue::Kanaal | OriginValue::Belanghebbende,
            )
            | (
                Levering::Eigen {
                    gelezen: Gelezen::Indiening,
                    ..
                },
                OriginValue::Belanghebbende,
            )
            | (
                Levering::Eigen {
                    gelezen: Gelezen::Verloop,
                    ..
                },
                OriginValue::Dossier,
            )
            | (Levering::Bron(_), OriginValue::Register) => true,
            (Levering::Stand, OriginValue::Dossier) => uitvoering.is_handeling(),
            (Levering::Keuze, OriginValue::Belanghebbende) => {
                uitvoering.is_aanbod() && g.is_tijdvak()
            }
            _ => false,
        }
    }
}

/// Wat het proces bij een uitvoering kan leveren, per parameter.
#[derive(Debug, Default)]
struct Leveranciers {
    per_parameter: BTreeMap<String, Vec<Levering>>,
    /// Of het beleid bij het aanbod tijdvakken aanbiedt (`aanbod.tijdvakken`).
    keuze: bool,
}

/// De keuze van het tijdvak, als leverancier.
static KEUZE: Levering = Levering::Keuze;

impl Leveranciers {
    fn voeg_toe(&mut self, naam: &str, l: Levering) {
        self.per_parameter
            .entry(naam.to_string())
            .or_default()
            .push(l);
    }

    fn van(d: &ProcesDefinitie, cel: &Cel, uitvoering: Uitvoering<'_>) -> Self {
        let mut l = Leveranciers::default();
        let mut eigen: Vec<&str> = d.zaakbronnen().map(|b| b.lexostatus.as_str()).collect();
        if let Some(p) = &d.portaal {
            l.keuze =
                uitvoering.is_aanbod() && p.aanbod.as_ref().is_some_and(|a| a.tijdvakken.is_some());
            eigen.push(&p.toets.lexostatus);
        }
        eigen.sort_unstable();
        eigen.dedup();
        for naam in eigen {
            let Some(def) = cel.lexostatussen.lexostatus(naam) else {
                continue;
            };
            for (param, afleiding) in &def.reduction.afleidingen {
                l.voeg_toe(
                    param,
                    Levering::Eigen {
                        lexostatus: naam.to_string(),
                        gelezen: gelezen(cel, &d.actor, def, afleiding),
                    },
                );
            }
        }
        for b in d.andere_bronnen() {
            for p in &b.parameters {
                l.voeg_toe(
                    p,
                    Levering::Bron(BronLevering {
                        cel: b.cel.clone(),
                        lexostatus: b.lexostatus.clone(),
                        url: b.url.clone(),
                    }),
                );
            }
        }
        if let Uitvoering::Handeling(h) = uitvoering {
            for naam in h.nog_niet.keys() {
                l.voeg_toe(naam, Levering::Stand);
            }
        }
        // De synthese per regel levert alleen aan de uitvoering die haar
        // uitvoert: de toets of het besluit.
        let rijen: &[RijenDefinitie] = match uitvoering {
            Uitvoering::Toets => d.portaal.as_ref().map(|p| p.toets.rijen.as_slice()),
            Uitvoering::Handeling(h) => Some(h.rijen.as_slice()),
            Uitvoering::Aanbod => None,
        }
        .unwrap_or_default();
        for r in rijen {
            l.rijen(d, cel, r);
        }
        l
    }

    /// De synthese per regel: uit een extra veld van een bron een levering
    /// van die bron, uit een eigen tabel een eigen levering, naar wat het
    /// extra veld leest.
    fn rijen(&mut self, d: &ProcesDefinitie, cel: &Cel, r: &RijenDefinitie) {
        let bron = d
            .andere_bronnen()
            .find(|b| b.lexostatus == r.tabel.lexostatus && b.extra_velden.contains(&r.tabel.veld));
        let levering = match bron {
            Some(b) => Levering::Bron(BronLevering {
                cel: b.cel.clone(),
                lexostatus: b.lexostatus.clone(),
                url: b.url.clone(),
            }),
            None => {
                let gelezen = cel
                    .lexostatussen
                    .lexostatus(&r.tabel.lexostatus)
                    .and_then(|def| {
                        def.alle_afleidingen()
                            .find(|(naam, _)| **naam == r.tabel.veld)
                            .map(|(_, a)| gelezen(cel, &d.actor, def, a))
                    })
                    .unwrap_or(Gelezen::Onbepaald);
                Levering::Eigen {
                    lexostatus: r.tabel.lexostatus.clone(),
                    gelezen,
                }
            }
        };
        self.voeg_toe(&r.parameter, levering);
    }

    /// Wie een parameter levert. De keuze in het portaal levert alleen het
    /// tijdvak.
    fn van_parameter(&self, naam: &str, tijdvak: bool) -> Vec<&Levering> {
        let mut uit: Vec<&Levering> = self.per_parameter.get(naam).into_iter().flatten().collect();
        if self.keuze && tijdvak {
            uit.push(&KEUZE);
        }
        uit
    }
}

/// Of een gram van dit event door een filter kan komen, voor zover dat
/// zonder de invoer vaststaat: een `$`-waarde past altijd, een veldpad als
/// het event het veld heeft.
fn kan_passen(filter: &Filter, stroom: &Stroom, event: &Event) -> bool {
    filter.iter().all(|(sleutel, waarde)| {
        if waarde.starts_with('$') {
            return match sleutel.as_str() {
                "zaakkenmerk" => event.zaak.heeft_kenmerk(),
                "name" | "type" | "soort" | "stage" | "recording_actor" | "chronicle" => true,
                pad => event.heeft_pad(pad),
            };
        }
        match sleutel.as_str() {
            "name" => event.name == *waarde,
            "type" => event.type_ == *waarde,
            "soort" => event.soort.as_deref() == Some(waarde.as_str()),
            "stage" => event.stage.as_deref() == Some(waarde.as_str()),
            "zaakkenmerk" => event.zaak.heeft_kenmerk(),
            "recording_actor" => stroom.recording_actor == *waarde,
            "chronicle" => stroom.chronicle == *waarde,
            pad => event.heeft_pad(pad),
        }
    })
}

/// Wat een afleiding leest: de events van de kroniek van de lexostatus die
/// door het filter van de lexostatus en dat van de afleiding kunnen komen.
/// Leest ze alleen `$intake` van een indiening, dan is het de login.
fn gelezen(cel: &Cel, actor: &str, def: &LexostatusDefinitie, a: &Afleiding) -> Gelezen {
    let events: Vec<(&Stroom, &Event)> = cel
        .strommen
        .iter()
        .filter(|s| s.chronicle == def.reduction.kroniek)
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(s, e)| {
            kan_passen(&def.reduction.filter, s, e)
                && a.filter().is_none_or(|f| kan_passen(f, s, e))
        })
        .collect();
    if events.is_empty() {
        return Gelezen::Onbepaald;
    }
    if events.iter().all(|(_, e)| e.type_ == INDIENING) {
        let paden = a.gelezen_paden();
        let alleen_intake = !paden.is_empty()
            && events.iter().all(|(_, e)| {
                let intake: BTreeSet<String> = e
                    .bladeren()
                    .into_iter()
                    .filter(|b| matches!(b.binding, Binding::Intake(_)))
                    .map(|b| b.pad)
                    .collect();
                paden.iter().all(|p| intake.contains(*p))
            });
        return if alleen_intake {
            Gelezen::Kanaal
        } else {
            Gelezen::Indiening
        };
    }
    if events
        .iter()
        .all(|(s, e)| e.type_ != INDIENING && s.recording_actor == actor)
    {
        return Gelezen::Verloop;
    }
    Gelezen::Onbepaald
}

/// Wat de controle oplevert.
#[derive(Debug, Default)]
pub struct Controle {
    pub fouten: Vec<String>,
    pub waarschuwingen: Vec<String>,
    /// Per uitvoering de parameters met hun geldende herkomst, in de volgorde
    /// van declaratie; bij een handeling over alle uitkomsten samen,
    /// onder de naam van de handeling.
    pub parameters: BTreeMap<String, Vec<(Benodigd, Option<Geldend>)>>,
    /// De parameter van het aanbod-artikel die het tijdvak is: `rol:
    /// TIJDVAK`.
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

/// De uitkomsten die het proces uitvoert: de toets, het aanbod en elke
/// uitkomst van het besluit (RFC-043: "every outcome").
fn uitvoeringen(d: &ProcesDefinitie) -> Vec<(Uitvoering<'_>, &str, &str)> {
    let mut uit: Vec<(Uitvoering<'_>, &str, &str)> = Vec::new();
    if let Some(p) = &d.portaal {
        uit.push((Uitvoering::Toets, &p.toets.regeling, &p.toets.uitkomst));
        if let Some(a) = &p.aanbod {
            uit.push((Uitvoering::Aanbod, &a.regeling, &a.uitkomst));
        }
    }
    for h in d.behandeling.iter().flat_map(|b| b.handelingen.iter()) {
        if matches!(h.soort, Handelingsoort::Vervolg { .. }) {
            continue;
        }
        for u in h.uitkomsten.iter().chain(h.toetsen.iter()) {
            uit.push((Uitvoering::Handeling(h), &h.regeling, u));
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
    let gezag = gezag::eigen(d, service);
    let overschrijvingen = match overschrijvingen(service, gezag.as_deref()) {
        Ok(o) => o,
        Err(f) => {
            c.fouten.extend(f);
            return c;
        }
    };
    // Een melding per parameter van een artikel, ook als meer uitvoeringen
    // hem vragen.
    let mut gemeld: BTreeSet<(String, String, &'static str)> = BTreeSet::new();
    // Wat niet na te gaan is, per bron en reden: de parameters erbij.
    let mut niet_na_te_gaan: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for (uitvoering, regeling, uitkomst) in uitvoeringen(d) {
        // Een uitkomst die niet bestaat, meldt de controle op portaal of
        // besluit.
        let Some(artikel) = service
            .resolver()
            .get_article_by_output(regeling, uitkomst, None)
        else {
            continue;
        };
        let leveranciers = Leveranciers::van(d, cel, uitvoering);
        let lijst = c.parameters.entry(uitvoering.naam()).or_default();
        let mut nieuw = Vec::new();
        for b in in_volgorde(service, regeling, artikel) {
            if lijst.iter().any(|(eerder, _)| *eerder == b) {
                continue;
            }
            let Some(p) = parameter(service, &b) else {
                continue;
            };
            let law = b
                .artikel
                .split_once('#')
                .map(|(r, _)| r)
                .unwrap_or(regeling);
            let g = overschrijvingen.geldend(law, p);
            nieuw.push((b, p, g));
        }
        for (b, p, g) in nieuw {
            let mut meld = |soort: &'static str, tekst: String, fout: bool| {
                if gemeld.insert((b.artikel.clone(), b.naam.clone(), soort)) {
                    if fout {
                        c.fouten.push(tekst);
                    } else {
                        c.waarschuwingen.push(tekst);
                    }
                }
            };
            controleer_parameter(
                Parameterplek {
                    d,
                    uitvoering,
                    b: &b,
                    p,
                    g: g.as_ref(),
                },
                &leveranciers,
                cellen,
                service,
                &mut meld,
                &mut niet_na_te_gaan,
            );
            c.parameters
                .entry(uitvoering.naam())
                .or_default()
                .push((b, g));
        }
    }
    for (reden, namen) in niet_na_te_gaan {
        let namen: Vec<String> = namen.into_iter().map(|n| format!("'{n}'")).collect();
        c.waarschuwingen.push(format!(
            "herkomst van {} niet na te gaan: {reden}",
            namen.join(", ")
        ));
    }
    tijdvak(d, &mut c);
    c
}

/// Een parameter die een uitvoering vraagt, met wat de controle erover
/// weet.
struct Parameterplek<'a> {
    d: &'a ProcesDefinitie,
    uitvoering: Uitvoering<'a>,
    b: &'a Benodigd,
    p: &'a Parameter,
    g: Option<&'a Geldend>,
}

/// De controles op een parameter: de aanbodregel, de herkomst zelf en zijn
/// leverancier.
fn controleer_parameter(
    plek: Parameterplek<'_>,
    l: &Leveranciers,
    cellen: &BTreeMap<String, Arc<Cel>>,
    service: &LawExecutionService,
    meld: &mut impl FnMut(&'static str, String, bool),
    niet_na_te_gaan: &mut BTreeMap<String, BTreeSet<String>>,
) {
    let Parameterplek {
        d,
        uitvoering,
        b,
        p,
        g,
    } = plek;
    if uitvoering.is_aanbod() && !vooraf_bekend(g) {
        let herkomst = g
            .map(Geldend::beschrijving)
            .unwrap_or_else(|| "geen origin".into());
        meld(
            "aanbod-regel",
            format!(
                "aanbod: voorwaarde leunt op '{}' ({herkomst}), dat vooraf niet bekend is",
                b.naam
            ),
            true,
        );
    }
    let Some(g) = g else {
        let streng = d.herkomst == Herkomstcontrole::Streng;
        meld(
            "zonder",
            format!(
                "herkomst: parameter '{}' van {} heeft geen origin; wie hem levert is niet na te gaan{}",
                b.naam,
                b.artikel,
                if streng { " (herkomst: streng)" } else { "" }
            ),
            streng,
        );
        return;
    };
    let waar = format!(
        "parameter '{}' van {} ({})",
        b.naam,
        b.artikel,
        g.beschrijving()
    );
    // Een grondslag in een regeling die niet geladen is, kan kloppen; de
    // runtime kan het alleen niet nagaan.
    if let Err(f) = regelingen::artikel(service, &g.origin.grondslag) {
        meld(
            "grondslag",
            format!("herkomst: {waar}: {f}; niet na te gaan"),
            false,
        );
    }
    if let Some(r) = &g.origin.register {
        if service.resolver().get_law(r).is_none() {
            meld(
                "register",
                format!("herkomst: {waar}: register '{r}' is geen geladen regeling"),
                true,
            );
        }
    }
    if g.origin.waarde == OriginValue::Belanghebbende
        && !g.is_tijdvak()
        && p.required != Some(false)
    {
        meld(
            "required",
            format!(
                "herkomst: parameter '{}' van {} komt van de belanghebbende, maar heeft geen required: false (RFC-036)",
                b.naam, b.artikel
            ),
            false,
        );
    }
    match leverancier(uitvoering, &b.naam, g, l, cellen) {
        Uitslag::Past { waarschuwingen } => {
            for w in waarschuwingen {
                niet_na_te_gaan.entry(w).or_default().insert(b.naam.clone());
            }
        }
        Uitslag::Verkeerd(reden) => meld(
            "leverancier",
            format!("{}: verkeerde bron voor {waar}: {reden}", uitvoering.naam()),
            true,
        ),
        Uitslag::Geen(reden) => {
            let tekst = format!("{}: geen leverancier voor {waar}{reden}", uitvoering.naam());
            if p.required == Some(false) {
                meld(
                    "leverancier",
                    format!(
                        "{tekst}; required: false, dus de engine krijgt hem niet en rekent met een onbekende waarde (RFC-036)"
                    ),
                    false,
                );
            } else {
                meld("leverancier", tekst, true);
            }
        }
    }
}

/// Het tijdvak van het aanbod: hoogstens een parameter met `rol: TIJDVAK`,
/// en die vraagt `aanbod.tijdvakken`; tijdvakken zonder zo'n parameter zijn
/// ook een fout.
fn tijdvak(d: &ProcesDefinitie, c: &mut Controle) {
    let Some(aanbod) = d.portaal.as_ref().and_then(|p| p.aanbod.as_ref()) else {
        return;
    };
    let namen: Vec<String> = c
        .parameters
        .get(&Uitvoering::Aanbod.naam())
        .into_iter()
        .flatten()
        .filter(|(_, g)| g.as_ref().is_some_and(Geldend::is_tijdvak))
        .map(|(b, _)| b.naam.clone())
        .collect();
    match (namen.as_slice(), aanbod.tijdvakken.is_some()) {
        ([], false) => {}
        ([], true) => c.fouten.push(format!(
            "aanbod: tijdvakken, maar {} vraagt geen tijdvak (een parameter met origin BELANGHEBBENDE en rol TIJDVAK)",
            aanbod.regeling
        )),
        ([naam], true) => c.tijdvak = Some(naam.clone()),
        ([naam], false) => c.fouten.push(format!(
            "aanbod: het tijdvak '{naam}' (rol TIJDVAK) vraagt aanbod.tijdvakken: de uitkomst van het beleid met de tijdvakken die het portaal aanbiedt"
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
pub fn oordelen(c: &Controle, service: &LawExecutionService, handeling: &str) -> Vec<Oordeel> {
    c.parameters
        .get(handeling)
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
    let tekst = label_uit(p.description.as_deref().unwrap_or_default());
    if tekst.is_empty() {
        p.name.clone()
    } else {
        tekst
    }
}

/// Het label uit een omschrijving: het deel na "Naam:", anders de hele
/// omschrijving, zonder punt aan het eind.
pub fn label_uit(omschrijving: &str) -> String {
    let tekst = omschrijving.trim();
    let tekst = match tekst.rsplit_once("Naam:") {
        Some((_, naam)) => naam.trim(),
        None => tekst,
    };
    let tekst = tekst.strip_suffix('.').unwrap_or(tekst).trim();
    tekst.split_whitespace().collect::<Vec<_>>().join(" ")
}

/// De groep van een oordeel: de regeling en het artikel van zijn grondslag.
fn groep(service: &LawExecutionService, grondslag: &str) -> Option<String> {
    let g = regelingen::ontleed(grondslag).ok()?;
    let naam = service
        .resolver()
        .get_law(g.regeling)
        .and_then(|l| l.name.clone())
        .unwrap_or_else(|| crate::formulier::leesbaar(g.regeling));
    Some(format!("{naam}, artikel {}", g.artikel))
}

/// Of het aanbod op een parameter mag leunen: wat vooraf vaststaat, is wie
/// inlogt (`KANAAL`), wat een register weet (`REGISTER`) en welk tijdvak de
/// aanvrager kiest (`BELANGHEBBENDE` met `rol: TIJDVAK`). Of een aanvraag
/// volledig is, weet je pas na het invullen.
fn vooraf_bekend(g: Option<&Geldend>) -> bool {
    g.is_some_and(|g| {
        matches!(g.origin.waarde, OriginValue::Kanaal | OriginValue::Register) || g.is_tijdvak()
    })
}

/// Hoe het met de leverancier van een parameter zit.
#[derive(Debug)]
enum Uitslag {
    /// Een leverancier past, eventueel met wat niet na te gaan is.
    Past { waarschuwingen: Vec<String> },
    /// Een leverancier past niet bij de herkomst, ook als een andere wel
    /// past.
    Verkeerd(String),
    /// Geen leverancier; de tekst begint met `: ` of is leeg.
    Geen(String),
}

/// Of een parameter een leverancier heeft die bij zijn herkomst past, en
/// geen die er niet bij past.
fn leverancier(
    uitvoering: Uitvoering<'_>,
    naam: &str,
    g: &Geldend,
    l: &Leveranciers,
    cellen: &BTreeMap<String, Arc<Cel>>,
) -> Uitslag {
    let leveringen = l.van_parameter(naam, g.is_tijdvak());
    if g.origin.waarde == OriginValue::Oordeel {
        if !leveringen.is_empty() {
            let wie: Vec<String> = leveringen.iter().map(|lv| lv.woorden()).collect();
            return Uitslag::Verkeerd(format!(
                "een oordeel geeft de behandelaar in het formulier van de handeling, maar hij komt uit {}",
                wie.join(" en ")
            ));
        }
        return if uitvoering.is_handeling() {
            Uitslag::Past {
                waarschuwingen: Vec::new(),
            }
        } else {
            Uitslag::Geen(
                ": een oordeel geeft de behandelaar pas bij een handeling in de zaak".into(),
            )
        };
    }
    let mut verkeerd = Vec::new();
    let mut past = false;
    let mut waarschuwingen = Vec::new();
    for lv in leveringen {
        if !lv.past(g, uitvoering) {
            verkeerd.push(format!("hij komt uit {}", lv.woorden()));
            continue;
        }
        match lv {
            Levering::Bron(bron) => match register_bron(bron, g, cellen) {
                Ok(w) => {
                    past = true;
                    waarschuwingen.extend(w);
                }
                Err(r) => verkeerd.push(r),
            },
            _ => past = true,
        }
    }
    if !verkeerd.is_empty() {
        Uitslag::Verkeerd(verkeerd.join("; "))
    } else if past {
        Uitslag::Past { waarschuwingen }
    } else {
        Uitslag::Geen(String::new())
    }
}

/// Of een synthese-bron een register-parameter mag leveren: haar lexostatus
/// houdt een kroniek bij met een grondslag in het register van de herkomst.
/// Onder welke naam de afnemer het feit vraagt, zegt de synthese van het
/// proces (de vertaling hoort bij de afnemer). `Ok` met
/// een waarschuwing als dat niet na te gaan is: een bron met een url, of een
/// interne cel die niet in deze runtime draait.
fn register_bron(
    bron: &BronLevering,
    g: &Geldend,
    cellen: &BTreeMap<String, Arc<Cel>>,
) -> Result<Option<String>, String> {
    let wie = format!("synthese-bron {}/{}", bron.cel, bron.lexostatus);
    let register = g.origin.register.as_deref().unwrap_or_default();
    if let Some(url) = &bron.url {
        return Ok(Some(format!(
            "{wie} draait buiten deze runtime ({url}); of haar lexostatus een kroniek bijhoudt met een grondslag in '{register}', is bij het opstarten niet te zien"
        )));
    }
    let Some(cel) = cellen.get(&bron.cel) else {
        return Ok(Some(format!(
            "{wie} heeft geen url en draait niet in deze runtime; of haar lexostatus een kroniek bijhoudt met een grondslag in '{register}', is niet te zien"
        )));
    };
    let Some(def) = cel.lexostatussen.lexostatus(&bron.lexostatus) else {
        return Err(format!("{wie} bestaat niet"));
    };
    // `vorm` houdt een REGISTER zonder register al tegen.
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
    Ok(None)
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
                // De regeling van de afnemer, en de Awb met de procedure.
                if tekst.contains("$id: testregeling_afnemer")
                    || tekst.contains("$id: testregeling_awb")
                {
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
        controleer_met_stand(d, &c, &s)
    }

    /// Zoals bij het laden van een proces: de handelingen voorbereiden (de
    /// soort en wat nog niet gebeurd is, uit de procedure), dan de controle.
    fn controleer_met_stand(
        mut d: ProcesDefinitie,
        c: &BTreeMap<String, Arc<Cel>>,
        s: &Arc<LawExecutionService>,
    ) -> Controle {
        let gezag = crate::gezag::eigen(&d, s);
        let f = crate::handeling::bereid_voor(&mut d, gezag.as_deref(), s, &c["test_afnemer"]);
        assert!(f.is_empty(), "{f:?}");
        controleer(&d, &c["test_afnemer"], c, s)
    }

    fn zo(t: String) -> String {
        t
    }

    /// De origin van `jaar` (art. 3) vervangen.
    fn jaar_met(origin: &'static str) -> impl Fn(String) -> String {
        move |t: String| {
            t.replace(
                "origin: {waarde: REGISTER, register: testregeling_register, grondslag: testregeling_register#3}\n          - name: gebiedstabel",
                &format!("origin: {origin}\n          - name: gebiedstabel"),
            )
        }
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
        // De procedure vraagt de datum van bekendmaking niet meer in een
        // latere stage, en de lexostatus die haar leest is geen bron: dan
        // levert niets haar.
        let c = afnemer(
            |t| t.replace("          - {name: datum_bekendmaking, type: date}\n", ""),
            |t| {
                alleen_het_besluit(t).replace(
                    "  - {cel: test_afnemer, lexostatus: besluit, zaak: true}\n",
                    "",
                )
            },
            &[],
        );
        assert_eq!(
            c.fouten,
            ["besluit: geen leverancier voor parameter 'datum_bekendmaking' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2)"]
        );
    }

    /// Het proces van de afnemer met alleen de handeling van het besluit.
    fn alleen_het_besluit(t: String) -> String {
        let begin = t.find("    # De bekendmaking").unwrap();
        let eind = t.find("# Standaardgegevens").unwrap();
        format!("{}{}", &t[..begin], &t[eind..])
    }

    /// Met required: false en zonder leverancier krijgt de engine de waarde
    /// niet en rekent ze met een onbekende (RFC-036): een waarschuwing.
    #[test]
    fn zonder_leverancier_en_niet_verplicht_is_een_waarschuwing() {
        let c = afnemer(
            |t| t.replace("          - {name: bekendgemaakt, type: boolean}\n", ""),
            zo,
            &[],
        );
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert_eq!(
            c.waarschuwingen,
            ["besluit: geen leverancier voor parameter 'bekendgemaakt' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3 lid 2); required: false, dus de engine krijgt hem niet en rekent met een onbekende waarde (RFC-036)"]
        );
    }

    /// Een leverancier van de verkeerde soort is een fout, en de melding zegt
    /// waar de parameter nu vandaan komt.
    #[test]
    fn een_leverancier_die_niet_bij_de_herkomst_past() {
        let c = afnemer(
            jaar_met("{waarde: DOSSIER, grondslag: 'testregeling_afnemer#3'}"),
            zo,
            &[],
        );
        assert_eq!(
            c.fouten,
            ["besluit: verkeerde bron voor parameter 'jaar' van testregeling_afnemer#3 (DOSSIER, grondslag testregeling_afnemer#3): hij komt uit synthese-bron test_register/registerstatus"]
        );
    }

    /// Een verkeerde bron is ook een fout als de parameter required: false
    /// is: de engine zou dan rekenen met een waarde van de verkeerde partij.
    #[test]
    fn een_verkeerde_bron_is_ook_bij_required_false_een_fout() {
        let c = afnemer(
            |t| {
                t.replace(
                    "          - name: bekendgemaakt\n            type: boolean\n            required: false\n            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 2}",
                    "          - name: bekendgemaakt\n            type: boolean\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#3 lid 2}",
                )
            },
            zo,
            &[],
        );
        assert_eq!(
            c.fouten,
            ["besluit: verkeerde bron voor parameter 'bekendgemaakt' van testregeling_afnemer#3 (BELANGHEBBENDE, grondslag testregeling_afnemer#3 lid 2): hij komt uit de stand bij besluit"]
        );
        assert!(c.waarschuwingen.is_empty(), "{:?}", c.waarschuwingen);
    }

    /// Wat de aanvrager indient (een gram van type indiening) is van de
    /// belanghebbende; wat de eigen actor verder vastlegt (het verloop van de
    /// zaak) is dossier. Wie ze verwisselt, krijgt een fout.
    #[test]
    fn belanghebbende_en_dossier_volgen_uit_wat_de_afleiding_leest() {
        let c = afnemer(
            |t| {
                t.replace(
                    "          - name: aanvraagdatum\n            type: date\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}",
                    "          - name: aanvraagdatum\n            type: date\n            required: false\n            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#1}",
                )
                .replace(
                    "            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen",
                    "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen",
                )
            },
            zo,
            &[],
        );
        assert!(c.fouten.contains(
            &"toets: verkeerde bron voor parameter 'aanvraagdatum' van testregeling_afnemer#1 (DOSSIER, grondslag testregeling_afnemer#1): hij komt uit eigen lexostatus aanvraag_inhoud (wat de aanvrager indiende)".to_string()
        ), "{:?}", c.fouten);
        assert!(c.fouten.contains(
            &"besluit: verkeerde bron voor parameter 'datum_uitnodiging_aanvulling' van testregeling_afnemer#3 (BELANGHEBBENDE, grondslag testregeling_afnemer#3 lid 1): hij komt uit eigen lexostatus zaakverloop (het verloop van de zaak)".to_string()
        ), "{:?}", c.fouten);
    }

    /// Een oordeel geeft de behandelaar in het besluitformulier; een oordeel
    /// dat ook uit een lexostatus komt, heeft een verkeerde bron.
    #[test]
    fn een_oordeel_uit_een_lexostatus_is_een_fout() {
        let c = afnemer(
            |t| {
                t.replace(
                    "origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: zetels_op_lijst",
                    "origin: {waarde: OORDEEL, grondslag: testregeling_afnemer#3 lid 1}\n          - name: zetels_op_lijst",
                )
            },
            zo,
            &[],
        );
        assert!(c.fouten.contains(
            &"besluit: verkeerde bron voor parameter 'aanvraagdatum' van testregeling_afnemer#3 (OORDEEL, grondslag testregeling_afnemer#3 lid 1): een oordeel geeft de behandelaar in het formulier van de handeling, maar hij komt uit eigen lexostatus aanvraag_inhoud (wat de aanvrager indiende)".to_string()
        ), "{:?}", c.fouten);
    }

    #[test]
    fn een_register_moet_bij_de_bron_passen() {
        let c = afnemer(
            jaar_met("{waarde: REGISTER, register: testregeling_afnemer, grondslag: testregeling_register#3}"),
            zo,
            &[],
        );
        assert_eq!(
            c.fouten,
            ["besluit: verkeerde bron voor parameter 'jaar' van testregeling_afnemer#3 (REGISTER, register testregeling_afnemer, grondslag testregeling_register#3): synthese-bron test_register/registerstatus houdt geen kroniek bij met een grondslag in 'testregeling_afnemer'"]
        );
    }

    /// Het register van een herkomst is een geladen regeling (een fout); een
    /// grondslag in een regeling die niet geladen is, is niet na te gaan (een
    /// waarschuwing).
    #[test]
    fn register_en_grondslag_zijn_geladen() {
        let c = afnemer(
            jaar_met("{waarde: REGISTER, register: een_onbekend_register, grondslag: 'een_onbekende_wet#1'}"),
            zo,
            &[],
        );
        assert!(c.waarschuwingen.contains(
            &"herkomst: parameter 'jaar' van testregeling_afnemer#3 (REGISTER, register een_onbekend_register, grondslag een_onbekende_wet#1): grondslag 'een_onbekende_wet#1': regeling 'een_onbekende_wet' is niet geladen; niet na te gaan".to_string()
        ), "{:?}", c.waarschuwingen);
        assert!(c.fouten.contains(
            &"herkomst: parameter 'jaar' van testregeling_afnemer#3 (REGISTER, register een_onbekend_register, grondslag een_onbekende_wet#1): register 'een_onbekend_register' is geen geladen regeling".to_string()
        ), "{:?}", c.fouten);
    }

    /// Een bron met een url is niet na te gaan: ze telt, met een
    /// waarschuwing die zegt waarom.
    #[test]
    fn een_bron_met_een_url_telt_met_een_waarschuwing() {
        let c = afnemer(
            zo,
            |t| {
                t.replace(
                    "  - cel: test_register\n    lexostatus: registerstatus\n",
                    "  - cel: test_register\n    url: http://register.example\n    lexostatus: registerstatus\n",
                )
            },
            &[],
        );
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert_eq!(
            c.waarschuwingen,
            ["herkomst van 'datum_mededeling', 'geblokkeerd_raad', 'jaar', 'zetels_op_lijst' niet na te gaan: synthese-bron test_register/registerstatus draait buiten deze runtime (http://register.example); of haar lexostatus een kroniek bijhoudt met een grondslag in 'testregeling_register', is bij het opstarten niet te zien"]
        );
    }

    /// Een interne bron waarvan de cel niet in deze runtime draait, telt
    /// ook, met een waarschuwing.
    #[test]
    fn een_interne_bron_die_niet_draait_telt_met_een_waarschuwing() {
        let s = service(zo, &[]);
        let mut c = cellen(&s);
        c.remove("test_register");
        let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
        assert!(uit.fouten.is_empty(), "{:?}", uit.fouten);
        assert_eq!(
            uit.waarschuwingen,
            [
                "herkomst van 'is_geschrapt_raad', 'is_ingeschreven_raad' niet na te gaan: synthese-bron test_register/register heeft geen url en draait niet in deze runtime; of haar lexostatus een kroniek bijhoudt met een grondslag in 'testregeling_register', is niet te zien",
                "herkomst van 'datum_mededeling', 'geblokkeerd_raad', 'jaar', 'zetels_op_lijst' niet na te gaan: synthese-bron test_register/registerstatus heeft geen url en draait niet in deze runtime; of haar lexostatus een kroniek bijhoudt met een grondslag in 'testregeling_register', is niet te zien",
            ]
        );
    }

    /// De rijen van de toets leveren de tabel aan de toets; die van het
    /// besluit niet.
    #[test]
    fn de_rijen_van_de_toets_leveren_aan_de_toets() {
        let vraagt_tabel = |t: String| {
            t.replacen(
                "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: is_ingeschreven_raad",
                "            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: gebiedstabel\n            type: array\n            nullable: true\n            required: false\n            origin: {waarde: BELANGHEBBENDE, grondslag: testregeling_afnemer#1}\n          - name: is_ingeschreven_raad",
                1,
            )
        };
        let c = afnemer(vraagt_tabel, zo, &[]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert_eq!(
            c.waarschuwingen,
            ["toets: geen leverancier voor parameter 'gebiedstabel' van testregeling_afnemer#1 (BELANGHEBBENDE, grondslag testregeling_afnemer#1); required: false, dus de engine krijgt hem niet en rekent met een onbekende waarde (RFC-036)"]
        );
        let met_rijen = |t: String| {
            t.replace(
                "    uitkomst: aanvraag_toelaatbaar\n",
                "    uitkomst: aanvraag_toelaatbaar\n    rijen:\n      - parameter: gebiedstabel\n        tabel: {lexostatus: aanvraag_inhoud, veld: gebieden}\n        kolommen: {gebied: gebied}\n",
            )
        };
        let c = afnemer(vraagt_tabel, met_rijen, &[]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert!(c.waarschuwingen.is_empty(), "{:?}", c.waarschuwingen);
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

        let zonder = |t: String| {
            t.replace("            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen", "          - name: opgeschorte_dagen")
        };
        let s = service(zonder, &[]);
        let c = cellen(&s);
        let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
        assert_eq!(
            uit.waarschuwingen,
            ["herkomst: parameter 'datum_uitnodiging_aanvulling' van testregeling_afnemer#3 heeft geen origin; wie hem levert is niet na te gaan"]
        );
    }

    /// In een proces met `herkomst: streng` is een parameter zonder origin
    /// een fout.
    #[test]
    fn strikte_herkomst_maakt_een_parameter_zonder_origin_een_fout() {
        let zonder = |t: String| {
            t.replace("            origin: {waarde: DOSSIER, grondslag: testregeling_afnemer#3 lid 1}\n          - name: opgeschorte_dagen", "          - name: opgeschorte_dagen")
        };
        let streng = |t: String| {
            t.replace(
                "actor: test_afnemer\n",
                "actor: test_afnemer\nherkomst: streng\n",
            )
        };
        let c = afnemer(zonder, streng, &[]);
        assert_eq!(
            c.fouten,
            ["herkomst: parameter 'datum_uitnodiging_aanvulling' van testregeling_afnemer#3 heeft geen origin; wie hem levert is niet na te gaan (herkomst: streng)"]
        );
        assert!(c.waarschuwingen.is_empty(), "{:?}", c.waarschuwingen);
        // Zonder streng blijft het een waarschuwing (zie hierboven), en
        // `ruim` is hetzelfde als niets.
        let ruim = |t: String| {
            t.replace(
                "actor: test_afnemer\n",
                "actor: test_afnemer\nherkomst: ruim\n",
            )
        };
        let c = afnemer(zonder, ruim, &[]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert_eq!(c.waarschuwingen.len(), 1, "{:?}", c.waarschuwingen);
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
  - number: '2'
    text: |-
      1. Een besluit vermeldt de dag waarop het is genomen.
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        output:
          - name: vermeldt_dag
            type: boolean
        actions:
          - output: vermeldt_dag
            value: true
"#;

    #[test]
    fn het_betalingsvoorbeeld_mist_een_leverancier() {
        let c = afnemer(
            zo,
            |t| {
                alleen_het_besluit(t)
                    .replace("  - {cel: test_afnemer, lexostatus: besluit, zaak: true}\n", "")
                    .replace("      regeling: testregeling_afnemer\n", "      regeling: testregeling_betaling\n")
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
                "besluit: geen leverancier voor parameter 'vastgesteld_bedrag' van testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
                "besluit: geen leverancier voor parameter 'betaald_bedrag' van testregeling_betaling#1 (DOSSIER, grondslag testregeling_betaling#1 lid 1)",
            ]
        );
    }

    /// Elke uitkomst van het besluit telt (RFC-043: "every outcome"), niet
    /// alleen de eerste: de parameters van een tweede artikel ook.
    #[test]
    fn elke_uitkomst_van_het_besluit_telt() {
        let met = |uitkomsten: &'static str| {
            move |t: String| {
                alleen_het_besluit(t)
                    .replace("  - {cel: test_afnemer, lexostatus: besluit, zaak: true}\n", "")
                    .replace("      regeling: testregeling_afnemer\n", "      regeling: testregeling_betaling\n")
                    .replace(
                        "uitkomsten: [vastgesteld_bedrag, gebiedsbedrag, besluit_tijdig, besluitdeadline, zorgvuldig]",
                        uitkomsten,
                    )
            }
        };
        // Alleen het tweede artikel: niets te leveren.
        let c = afnemer(zo, met("uitkomsten: [vermeldt_dag]"), &[BETALING]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        // Het artikel zonder parameters eerst: het tweede telt nog steeds.
        let c = afnemer(
            zo,
            met("uitkomsten: [vermeldt_dag, nog_te_betalen]"),
            &[BETALING],
        );
        assert_eq!(c.fouten.len(), 2, "{:?}", c.fouten);
        assert!(
            c.fouten[0].contains("'vastgesteld_bedrag'"),
            "{:?}",
            c.fouten
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

    /// Het tijdvak is een rol, geen grondslag: de grondslag Awb 4:2 lid 1
    /// zonder `rol: TIJDVAK` is een deel van de aanvraag dat pas na het
    /// invullen bekend is, dus geen tijdvak en niet vooraf bekend.
    #[test]
    fn het_tijdvak_is_een_rol_geen_grondslag() {
        let met_aanbod = |t: String| {
            t.replace(
                "    uitkomst: aanvraag_toelaatbaar\n",
                "    uitkomst: aanvraag_toelaatbaar\n  aanbod:\n    regeling: testregeling_afnemer\n    uitkomst: aanvraag_aangeboden\n    termijn: aanvraagtermijn\n    tijdvakken: aangeboden_jaren\n    begin: begin_aanvraagjaar\n",
            )
        };
        let c = afnemer(zo, met_aanbod, &[]);
        assert!(c.fouten.is_empty(), "{:?}", c.fouten);
        assert_eq!(c.tijdvak.as_deref(), Some("aanvraagjaar"));

        let zonder_rol = |t: String| t.replace(", rol: TIJDVAK}", "}");
        let c = afnemer(zonder_rol, met_aanbod, &[]);
        assert_eq!(c.tijdvak, None);
        assert_eq!(
            c.fouten,
            [
                "aanbod: voorwaarde leunt op 'aanvraagjaar' (BELANGHEBBENDE, grondslag algemene_wet_bestuursrecht#4:2 lid 1), dat vooraf niet bekend is",
                "aanbod: tijdvakken, maar testregeling_afnemer vraagt geen tijdvak (een parameter met origin BELANGHEBBENDE en rol TIJDVAK)",
            ]
        );
    }

    /// De vorm van een origin, bij het laden van elke regeling: een REGISTER
    /// noemt zijn register, alleen een REGISTER doet dat, een tijdvak komt
    /// van de belanghebbende, een grondslag is te ontleden, en een waarde
    /// die niet te lezen is, is een fout met artikel en parameter.
    #[test]
    fn de_vorm_van_een_origin_bij_het_laden() {
        let wet = |origin: &str| -> ArticleBasedLaw {
            serde_yaml_ng::from_str(&format!(
                "$id: een_wet\nregulatory_layer: WET\npublication_date: '2025-01-01'\narticles:\n  - number: '1'\n    text: Tekst.\n    machine_readable:\n      execution:\n        parameters:\n          - name: een_feit\n            type: boolean\n            origin: {origin}\n"
            ))
            .unwrap()
        };
        let fout = |origin: &str| valideer(&wet(origin));
        assert!(
            fout("{waarde: REGISTER, register: een_registerwet, grondslag: 'een_wet#1'}")
                .is_empty()
        );
        assert_eq!(
            fout("{waarde: REGISTER, grondslag: 'een_wet#1'}"),
            ["artikel 1, parameter 'een_feit': origin REGISTER zonder register: welke regeling het register houdt, is niet na te gaan"]
        );
        assert_eq!(
            fout("{waarde: DOSSIER, register: een_registerwet, grondslag: 'een_wet#1'}"),
            ["artikel 1, parameter 'een_feit': origin DOSSIER met register 'een_registerwet': alleen REGISTER noemt een register"]
        );
        assert_eq!(
            fout("{waarde: DOSSIER, grondslag: 'een_wet#1', rol: TIJDVAK}"),
            ["artikel 1, parameter 'een_feit': rol TIJDVAK bij origin DOSSIER: het tijdvak kiest de aanvrager als deel van de gevraagde beschikking (Awb 4:2 lid 1), dus BELANGHEBBENDE"]
        );
        assert_eq!(
            fout("{waarde: BELANGHEBBENDE, grondslag: een_wet}"),
            ["artikel 1, parameter 'een_feit': grondslag 'een_wet' heeft niet de vorm <regeling>#<artikel>"]
        );
        let f = fout("{waarde: KADER, grondslag: 'een_wet#1'}");
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].starts_with(
                "artikel 1, parameter 'een_feit': ongeldige origin: unknown variant `KADER`"
            ),
            "{f:?}"
        );
    }

    /// `origins` staat alleen in uitvoeringsbeleid, en een overschrijving die
    /// niet te lezen is, is een fout.
    #[test]
    fn de_vorm_van_origins_bij_het_laden() {
        let wet: ArticleBasedLaw = serde_yaml_ng::from_str(&BELEID.replace(
            "regulatory_layer: UITVOERINGSBELEID",
            "regulatory_layer: WET",
        ))
        .unwrap();
        assert_eq!(
            valideer(&wet),
            ["artikel 1: origins staat alleen in uitvoeringsbeleid (RFC-043)"]
        );
        let beleid: ArticleBasedLaw =
            serde_yaml_ng::from_str(&BELEID.replace("parameter: jaar", "parameter_: jaar"))
                .unwrap();
        let f = valideer(&beleid);
        assert_eq!(f.len(), 1, "{f:?}");
        assert!(
            f[0].starts_with("artikel 1, origins[0]: ongeldige overschrijving:"),
            "{f:?}"
        );
        let beleid: ArticleBasedLaw =
            serde_yaml_ng::from_str(&BELEID.replace("waarde: DOSSIER", "waarde: REGISTER"))
                .unwrap();
        assert_eq!(
            valideer(&beleid),
            ["artikel 1, origins voor 'jaar' van testregeling_afnemer: origin REGISTER zonder register: welke regeling het register houdt, is niet na te gaan"]
        );
    }

    /// Een ongeldige origin houdt het laden van het corpus tegen, met het
    /// bestand, het artikel en de parameter; de engine zelf laadt de
    /// regeling wel.
    #[test]
    fn een_ongeldige_origin_noemt_bestand_en_parameter() {
        // Niet met een punt vooraan: de lader slaat verborgen mappen over.
        let map = tempfile::Builder::new()
            .prefix("regelingen")
            .tempdir()
            .unwrap();
        let tekst = std::fs::read_to_string(
            fixtures().join("regulation/testregeling_afnemer/2025-01-01.yaml"),
        )
        .unwrap()
        .replacen("waarde: OORDEEL", "waarde: OORDEL", 1);
        let pad = map.path().join("afnemer.yaml");
        std::fs::write(&pad, tekst).unwrap();
        let fouten = regelingen::laad(map.path()).err().unwrap();
        assert_eq!(fouten.len(), 1, "{fouten:?}");
        assert!(
            fouten[0].starts_with(&format!(
                "{}: artikel 3, parameter 'besluitdatum': ongeldige origin: unknown variant `OORDEL`",
                pad.display()
            )),
            "{fouten:?}"
        );
    }

    /// Het besluitformulier: de OORDEEL-parameters van het besluit, met het
    /// label na "Naam:" en de groep uit de grondslag.
    #[test]
    fn het_besluitformulier_volgt_uit_origin() {
        let s = service(zo, &[]);
        let c = cellen(&s);
        let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
        let o = oordelen(&uit, &s, "besluit");
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
        let uit = controleer_met_stand(proces("afnemer", zo), &c, &s);
        assert_eq!(
            oordelen(&uit, &s, "besluit")[0].label,
            "De dag van het besluit"
        );
        assert_eq!(
            crate::formulier::leesbaar("een_regeling_zonder_naam"),
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
            ["besluit: verkeerde bron voor parameter 'jaar' van testregeling_afnemer#3 (DOSSIER, grondslag testbeleid_afnemer#1 lid 1, uit testbeleid_afnemer#1): hij komt uit synthese-bron test_register/registerstatus"]
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
