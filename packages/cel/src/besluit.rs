//! Het besluit op een zaak: een proefbesluit, zonder vastleggen, en het
//! besluit zelf, dat het proces de cel laat vastleggen.
//!
//! Het blok `behandeling.besluit` in `proces.yaml` zegt welke uitkomsten van
//! welk artikel het besluit zijn, en waar elke parameter vandaan komt. Een
//! parameter komt uit precies een bron:
//!
//! 1. een lexostatus van de zaak (een synthese-bron met `zaak: true`), met als
//!    enige input `zaakkenmerk`, zoals de inhoud van de aanvraag of het
//!    verloop van de zaak; het proces vraagt haar aan de cel, zoals elke bron;
//! 2. een andere synthese-bron, met invoer uit een van die lexostatussen;
//! 3. het besluitformulier: de oordelen van de behandelaar, die pas bij het
//!    besluiten bestaan; het zijn de parameters met origin `OORDEEL`
//!    (zie [`crate::origin::oordelen`]);
//! 4. de stand bij besluit: feiten die pas na het besluit ontstaan, zoals de
//!    bekendmaking, met hun stand op het moment van besluiten. Ze staan niet
//!    in de configuratie maar volgen uit de procedure van de beschikking
//!    (RFC-008, zie [`stand_bij_besluit`]).
//!
//! Het proefbesluit voert het artikel uit met wat die bronnen leveren. Mist
//! er iets, dan is het antwoord "niet te nemen: mist X"; er wordt niets
//! aangevuld en niets vastgelegd. Per parameter staat de herkomst erbij, en
//! `niet_geleverd` noemt elke parameter die de aanroeper van het artikel moet
//! leveren en die geen bron leverde, met de omschrijving uit de regeling.

use std::collections::BTreeMap;

use chrono::{DateTime, FixedOffset};
use serde::Serialize;
use serde_json::{Map, Value};

use regelrecht_engine::LawExecutionService;

use crate::cel::Cel;
use crate::celclient::{self, Besluitvelden, Vastlegverzoek};
use crate::config::{BesluitDefinitie, NogNiet};
use crate::datum::{self, Tijdpunt};
use crate::formulier::Veld;
use crate::gezag::{self, Bevoegdheid};
use crate::gram::{GeladenRegeling, Gram, HandelendeActor, Invoer, Receipt, StroomVerwijzing};
use crate::kanaal::Sessie;
use crate::proces::Proces;
use crate::reductie::{Lexostatus, Peil};
use crate::regelingen::{self, Benodigd};
use crate::rijen::{self, Rijen};
use crate::stroom::Zaak;
use crate::synthese::{self, Bron, BronUitslag, Herkomst};
use crate::toets;
use crate::transport::{Transport, TransportFout};

/// Het antwoord van een proefbesluit.
#[derive(Debug, Clone, Serialize)]
pub struct Proefbesluit {
    pub regeling: String,
    /// `<regeling>#<artikel>` van de uitkomsten.
    pub artikel: String,
    pub peildatum: String,
    /// Of elke uitkomst een waarde heeft.
    pub te_nemen: bool,
    /// Alleen als het besluit te nemen is.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub uitkomsten: BTreeMap<String, Value>,
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
    /// De lexostatussen van de zaak.
    pub lexostatussen: Vec<Lexostatus>,
    /// Wat de synthese per regel opleverde (zie [`crate::rijen`]).
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub rijen: Vec<rijen::Uitslag>,
    /// De trace van de engine-run, voor wie wil zien hoe het bedrag tot
    /// stand kwam.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub trace_text: Option<String>,
}

/// Het artikel achter de uitkomsten, als `<regeling>#<artikel>`.
fn artikel_van(service: &LawExecutionService, b: &BesluitDefinitie) -> Result<String, String> {
    let eerste = b
        .uitkomsten
        .first()
        .ok_or("het besluit noemt geen uitkomst")?;
    let artikel = service
        .resolver()
        .get_article_by_output(&b.regeling, eerste, None)
        .ok_or_else(|| format!("regeling '{}' heeft geen uitkomst '{eerste}'", b.regeling))?;
    Ok(format!("{}#{}", b.regeling, artikel.number))
}

/// De parameters die de aanroeper van het besluitartikel moet leveren.
fn benodigd(
    service: &LawExecutionService,
    b: &BesluitDefinitie,
) -> Result<BTreeMap<String, Benodigd>, String> {
    let grondslag = artikel_van(service, b)?;
    let a = regelingen::artikel(service, &grondslag)?;
    Ok(regelingen::benodigde_parameters(service, &b.regeling, a))
}

/// De stand bij besluit, uit de wet: de parameters van het besluit die de
/// procedure van de beschikking (RFC-008, `procedure` bij het rechtskarakter
/// dat het artikel produceert) pas vraagt in een stage na die van het
/// vastleg-event. Bij het besluit zijn die nog niet gebeurd: een boolean is
/// onwaar, al het andere leeg (null). Zo draagt de Awb zelf dat de
/// bekendmaking (stage BEKENDMAKING, Awb 3:40 en 3:41) na het besluit komt,
/// en staat er in de configuratie niets over.
///
/// Geen procedure bij het rechtskarakter: geen stand (de controle op de
/// herkomst meldt dan wat er mist). Een vastleg-event zonder stage, of met een
/// stage die de procedure niet kent, is een fout.
pub fn stand_bij_besluit(
    service: &LawExecutionService,
    b: &BesluitDefinitie,
    cel: &Cel,
) -> Result<BTreeMap<String, NogNiet>, String> {
    let mut uit = BTreeMap::new();
    let Some(v) = &b.vastleggen else {
        return Ok(uit);
    };
    let Some(stage) = cel
        .event(&v.stroom, &v.event)
        .and_then(|(_, e)| e.stage.clone())
    else {
        // Het vastleg-event meldt de controle op het besluit.
        return Ok(uit);
    };
    let grondslag = artikel_van(service, b)?;
    let artikel = regelingen::artikel(service, &grondslag)?;
    let Some(produces) = artikel.get_produces() else {
        return Ok(uit);
    };
    let Some(lc) = produces.legal_character.as_deref() else {
        return Ok(uit);
    };
    let Some(procedure) = service
        .resolver()
        .find_procedure(lc, produces.procedure_id.as_deref())
    else {
        return Ok(uit);
    };
    let Some(i) = procedure.stages.iter().position(|s| s.name == stage) else {
        return Err(format!(
            "besluit, vastleggen {}/{}: stage '{stage}' staat niet in procedure '{}' van {lc} ({})",
            v.stroom,
            v.event,
            procedure.id,
            procedure
                .stages
                .iter()
                .map(|s| s.name.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        ));
    };
    let benodigd = regelingen::benodigde_parameters(service, &b.regeling, artikel);
    for later in &procedure.stages[i + 1..] {
        for r in later.requires.iter().flatten() {
            let Some(p) = benodigd.get(&r.name) else {
                continue;
            };
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
    Ok(uit)
}

/// Vul de stand bij besluit van een proces in uit de wet (zie
/// [`stand_bij_besluit`]), zodra de regeling van het besluit bekend is.
pub fn zet_stand_bij_besluit(
    d: &mut crate::config::ProcesDefinitie,
    service: &LawExecutionService,
    cel: &Cel,
) -> Result<(), String> {
    let Some(b) = d.behandeling.as_mut().map(|b| &mut b.besluit) else {
        return Ok(());
    };
    if b.regeling.is_empty() {
        return Ok(());
    }
    b.stand_bij_besluit = stand_bij_besluit(service, b, cel)?;
    Ok(())
}

/// De velden van het besluitformulier, met het type uit de regeling. Een
/// fout als het artikel van het besluit niet te vinden is (de controle bij
/// het opstarten vangt dat al af).
pub fn formuliervelden(
    service: &LawExecutionService,
    b: &BesluitDefinitie,
) -> Result<Vec<Veld>, String> {
    let benodigd = benodigd(service, b)?;
    Ok(b.formulier
        .iter()
        .map(|o| {
            let soort = benodigd
                .get(&o.parameter)
                .and_then(|p| p.soort.as_str())
                .map(|t| match t {
                    "boolean" => "janee",
                    "date" => "datum",
                    "number" | "amount" => "getal",
                    _ => "tekst",
                });
            Veld {
                naam: o.parameter.clone(),
                label: o.label.clone(),
                soort: soort.map(str::to_string),
                opties: None,
                kolommen: None,
                uitleg: o.uitleg.clone(),
                groep: o.groep.clone(),
                grondslag: Vec::new(),
            }
        })
        .collect())
}

/// De controles op `behandeling` van een proces bij het
/// opstarten. Een fout hier houdt de runtime tegen:
///
/// - de werkvoorraad is een lijst-lexostatus van de cel;
/// - een bron van de zaak vraagt een besluit;
/// - de uitkomsten van het besluit zijn uitkomsten van een en hetzelfde
///   artikel;
/// - de lexostatussen van de zaak bestaan in de cel, zijn geen lijst en
///   hebben als enige input `zaakkenmerk`;
/// - elke parameter uit het formulier of de stand bij besluit is een
///   parameter die de aanroeper van het artikel moet leveren;
/// - een parameter komt uit maar een bron;
/// - de invoer van elke synthese-bron komt uit een lexostatus van het besluit.
pub fn controleer(proces: &Proces) -> Vec<String> {
    let mut fouten = Vec::new();
    let d = &proces.definitie;
    let cel = &proces.cel;
    let service = proces.service.as_ref();
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

    let b = &behandeling.besluit;
    let mut artikelen = Vec::new();
    for u in &b.uitkomsten {
        match service
            .resolver()
            .get_article_by_output(&b.regeling, u, None)
        {
            None => fouten.push(format!(
                "besluit: regeling '{}' heeft geen uitkomst '{u}'",
                b.regeling
            )),
            Some(a) => {
                if !artikelen.contains(&a.number) {
                    artikelen.push(a.number.clone());
                }
            }
        }
    }
    if artikelen.len() > 1 {
        fouten.push(format!(
            "besluit: de uitkomsten komen uit meer dan een artikel ({}); een besluit is een artikel",
            artikelen.join(", ")
        ));
    }

    // parameter -> bronnen die hem leveren
    let mut per: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    let zaak: Vec<&String> = d.zaakbronnen().map(|z| &z.lexostatus).collect();
    for naam in zaak.iter().copied() {
        match cel.lexostatussen.lexostatus(naam) {
            None => fouten.push(format!("besluit: lexostatus '{naam}' bestaat niet")),
            Some(l) => {
                if l.is_lijst() {
                    fouten.push(format!(
                        "besluit: lexostatus '{naam}' is een lijst, en een lijst gaat nooit naar de engine"
                    ));
                }
                let inputs: Vec<&str> = l.inputs.iter().map(|i| i.name.as_str()).collect();
                if inputs != ["zaakkenmerk"] {
                    fouten.push(format!(
                        "besluit: lexostatus '{naam}' heeft inputs [{}]; het besluit geeft alleen 'zaakkenmerk' mee",
                        inputs.join(", ")
                    ));
                }
                for p in l.reduction.afleidingen.keys() {
                    per.entry(p)
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
            "besluit: de invoer van de synthese komt uit meer dan een lexostatus ({}); het besluit geeft haar uit een",
            invoer_uit.join(", ")
        ));
    }
    for bron in d.andere_bronnen() {
        for p in &bron.parameters {
            per.entry(p)
                .or_default()
                .push(format!("synthese-bron {}/{}", bron.cel, bron.lexostatus));
        }
        for (i, v) in bron.invoer.iter().filter_map(|(i, v)| Some((i, v.veld()?))) {
            if !zaak.contains(&&v.lexostatus) && !doorgegeven.contains(&v.lexostatus.as_str()) {
                fouten.push(format!(
                    "besluit: synthese-bron {}/{}, invoer '{i}': komt uit lexostatus '{}', en die is geen lexostatus van de zaak (zaak: true)",
                    bron.cel, bron.lexostatus, v.lexostatus
                ));
            }
        }
    }
    for o in &b.formulier {
        per.entry(&o.parameter)
            .or_default()
            .push("het besluitformulier".into());
    }
    for p in b.stand_bij_besluit.keys() {
        per.entry(p)
            .or_default()
            .push("de stand bij besluit".into());
    }
    for (p, wie) in &per {
        if wie.len() > 1 {
            fouten.push(format!(
                "besluit: parameter '{p}' komt uit meer dan een bron: {}",
                wie.join(", ")
            ));
        }
    }

    // Synthese per regel: het tabelveld, de kolomnamen en de bronnen.
    let zaak: Vec<&str> = zaak.iter().map(|z| z.as_str()).collect();
    for r in &b.rijen {
        per.entry(&r.parameter)
            .or_default()
            .push(format!("de synthese per regel uit '{}'", r.tabel.veld));
        fouten.extend(rijen::controleer(
            "besluit",
            r,
            &zaak,
            "geen lexostatus van de zaak (zaak: true)",
            d,
            cel,
        ));
    }

    // Waar het besluit wordt vastgelegd: een event met een zaak en een stage,
    // dat precies de uitkomsten als velden vraagt.
    if let Some(v) = &b.vastleggen {
        let wie = format!("besluit, vastleggen {}/{}", v.stroom, v.event);
        match cel
            .strommen
            .iter()
            .find(|s| s.id == v.stroom)
            .and_then(|s| s.event(&v.event))
        {
            None => fouten.push(format!("{wie}: die stroom of dat event bestaat niet")),
            Some(event) => {
                if event.zaak != Zaak::Volgt {
                    fouten.push(format!(
                        "{wie}: het event heeft zaak: {}, en een besluit volgt de zaak van de aanvraag",
                        event.zaak.als_tekst()
                    ));
                }
                if event.stage.is_none() {
                    fouten.push(format!(
                        "{wie}: het event heeft geen stage; een besluit is een stage-decretogram (RFC-008)"
                    ));
                }
                let mut sleutels = event.external_sleutels();
                sleutels.sort();
                let mut verwacht = b.uitkomsten.clone();
                verwacht.sort();
                if sleutels != verwacht {
                    fouten.push(format!(
                        "{wie}: het event legt [{}] vast, en het besluit heeft de uitkomsten [{}]",
                        sleutels.join(", "),
                        verwacht.join(", ")
                    ));
                }
            }
        }
    }

    if artikelen.len() == 1 {
        match benodigd(service, b) {
            Err(f) => fouten.push(format!("besluit: {f}")),
            Ok(benodigd) => {
                let artikel = format!("{}#{}", b.regeling, artikelen[0]);
                let namen = b
                    .formulier
                    .iter()
                    .map(|o| ("besluitformulier", o.parameter.as_str()))
                    .chain(
                        b.stand_bij_besluit
                            .keys()
                            .map(|p| ("stand_bij_besluit", p.as_str())),
                    )
                    .chain(b.rijen.iter().map(|r| ("rijen", r.parameter.as_str())));
                for (waar, p) in namen {
                    if !benodigd.contains_key(p) {
                        fouten.push(format!(
                            "besluit, {waar}: '{p}' is geen parameter van {artikel} of van een artikel dat het zonder eigen parameters aanroept"
                        ));
                    }
                }
            }
        }
    }
    fouten
}

/// Een fout in de vraag, niet in de cel.
#[derive(Debug, Clone, PartialEq)]
pub enum Weigering {
    /// Het formulier noemt iets dat geen oordeel van het besluit is.
    OnbekendOordeel(String),
    /// Het proefbesluit is niet compleet: er wordt geen gram vastgelegd.
    NietTeNemen(String),
    /// Er ligt al een besluit in deze zaak. Een tweede besluit is een
    /// wijziging, en die valt buiten deze stap.
    AlBesloten(String),
    /// De wet wijst een ander bevoegd gezag aan dan de actor van het proces.
    Onbevoegd(String),
    /// De configuratie, of de cel: haar kroniek, een reductie of het
    /// vastleggen.
    Cel(String),
}

/// Een lexostatus van de zaak, gevraagd aan de cel. Heeft de cel er geen
/// gram voor (404), dan levert ze niets: een lege lexostatus.
async fn zaaklexostatus(
    proces: &Proces,
    cel: &dyn Transport,
    bron: &crate::config::SyntheseBron,
    inputs: &Map<String, Value>,
    peil: &Peil,
) -> Result<Lexostatus, Weigering> {
    let def = proces
        .cel
        .lexostatussen
        .lexostatus(&bron.lexostatus)
        .ok_or_else(|| Weigering::Cel(format!("lexostatus '{}' bestaat niet", bron.lexostatus)))?;
    match cel
        .haal(&synthese::pad(&bron.cel, &bron.lexostatus, inputs, peil))
        .await
    {
        Ok(v) => serde_json::from_value(v).map_err(|e| {
            Weigering::Cel(format!(
                "cel '{}', lexostatus '{}': onleesbaar antwoord: {e}",
                bron.cel, bron.lexostatus
            ))
        }),
        // Kiest de definitie een gram en is er geen, dan levert zij niets.
        Err(TransportFout::Antwoord { status: 404, .. }) => Ok(Lexostatus {
            niet_afgeleid: def.reduction.afleidingen.keys().cloned().collect(),
            ..Lexostatus::leeg(&def.name)
        }),
        Err(f) => Err(Weigering::Cel(format!(
            "cel '{}', lexostatus '{}': {f}",
            bron.cel, bron.lexostatus
        ))),
    }
}

/// Reken het besluit op een zaak uit, zonder iets vast te leggen. `peildatum`
/// (JJJJ-MM-DD) is de datum waarop de engine de regeling leest, en ook het
/// peilmoment waarop elke cel haar kroniek reduceert: de stand zoals die
/// rechtens gold op die dag (zie [`Peil`]). Zo telt een feit dat pas later
/// ingaat niet mee, en geeft hetzelfde proefbesluit later dezelfde stand. De
/// lexostatussen van de zaak komen van de cel, via `cel`.
pub async fn proefbesluit(
    proces: &Proces,
    cel: &dyn Transport,
    bronnen: &[Bron],
    rijen: &[Rijen],
    zaakkenmerk: &str,
    formulier: &Map<String, Value>,
    peildatum: &str,
) -> Result<Proefbesluit, Weigering> {
    let service = proces.service.as_ref();
    let b = &proces
        .definitie
        .behandeling
        .as_ref()
        .ok_or_else(|| Weigering::Cel("geen behandeling geconfigureerd".into()))?
        .besluit;
    for naam in formulier.keys() {
        if !b.formulier.iter().any(|o| &o.parameter == naam) {
            return Err(Weigering::OnbekendOordeel(format!(
                "'{naam}' is geen veld van het besluitformulier"
            )));
        }
    }
    let artikel = artikel_van(service, b).map_err(Weigering::Cel)?;
    let peil = Peil::op(Tijdpunt::lees("peildatum", peildatum).map_err(Weigering::Cel)?);

    // 1. De lexostatussen van de zaak, uit de cel.
    let mut inputs = Map::new();
    inputs.insert("zaakkenmerk".into(), Value::String(zaakkenmerk.to_string()));
    let mut eigen = Vec::new();
    for bron in proces.definitie.zaakbronnen() {
        eigen.push(zaaklexostatus(proces, cel, bron, &inputs, &peil).await?);
    }

    // 2. Synthese, met de invoer uit de lexostatus van de zaak die haar
    // levert. Een invoer die een eerdere bron doorgeeft, wijst geen
    // lexostatus van de zaak aan.
    let hoofd = proces
        .definitie
        .andere_bronnen()
        .flat_map(|s| s.invoer.values().filter_map(|v| v.veld()))
        .find_map(|v| eigen.iter().position(|l| l.naam == v.lexostatus))
        .unwrap_or(0);
    let mut samen = match eigen.get(hoofd) {
        Some(l) => synthese::voeg_samen(l, bronnen, &peil).await,
        None => synthese::voeg_samen(&Lexostatus::leeg(""), bronnen, &peil).await,
    };
    for (i, l) in eigen.iter().enumerate() {
        if i == hoofd {
            continue;
        }
        for (p, w) in &l.parameters {
            samen.parameters.insert(p.clone(), w.clone());
            samen.herkomst.insert(
                p.clone(),
                Herkomst::Eigen {
                    lexostatus: l.naam.clone(),
                },
            );
        }
    }

    // 3. Synthese per regel: een tabelveld wordt een array-parameter.
    let wet = rijen::Omgeving {
        service,
        datum: peildatum,
        peil: &peil,
    };
    let uitslagen = rijen::pas_toe(rijen, &eigen, &mut samen, wet).await;

    // 4. De oordelen van de behandelaar; een leeg veld gaat niet mee.
    for (p, w) in formulier {
        if !w.is_null() {
            samen.parameters.insert(p.clone(), w.clone());
            samen.herkomst.insert(p.clone(), Herkomst::Behandelaar);
        }
    }

    // 5. De stand bij besluit: wat de procedure pas in een latere stage
    // vraagt, is nog niet gebeurd.
    for (p, n) in &b.stand_bij_besluit {
        samen.parameters.insert(p.clone(), n.waarde.clone());
        samen.herkomst.insert(
            p.clone(),
            Herkomst::StandBijBesluit {
                stage: n.stage.clone(),
            },
        );
    }

    let uitkomsten: Vec<&str> = b.uitkomsten.iter().map(String::as_str).collect();
    let e = toets::evalueer_met_trace(
        service,
        &b.regeling,
        &uitkomsten,
        &samen.parameters,
        peildatum,
    );
    let te_nemen = e.volledig(&uitkomsten);
    let mut reden = (!te_nemen).then(|| e.reden("niet te nemen"));
    if !te_nemen {
        if let Some(r) = samen.reden() {
            reden = Some(r.replacen("niet te beoordelen", "niet te nemen", 1));
        }
    }
    let niet_geleverd = benodigd(service, b)
        .map_err(Weigering::Cel)?
        .into_values()
        .filter(|p| !samen.parameters.contains_key(&p.naam))
        .collect();
    Ok(Proefbesluit {
        regeling: b.regeling.clone(),
        artikel,
        peildatum: peildatum.to_string(),
        te_nemen,
        trace_text: e.trace_text,
        uitkomsten: if te_nemen { e.waarden } else { BTreeMap::new() },
        mist: e.mist,
        reden,
        parameters: samen.parameters,
        herkomst: samen.herkomst,
        bronnen: samen.bronnen,
        niet_geleverd,
        lexostatussen: eigen,
        rijen: uitslagen,
    })
}

/// Het genomen besluit: het vastgelegde gram, het proefbesluit waaruit het
/// volgde, en wat er bij het vastleggen op te merken viel.
#[derive(Debug, Clone, Serialize)]
pub struct Besluit {
    pub gram: Gram,
    /// Het gram als YAML, zoals de cel het teruggaf.
    pub yaml: String,
    pub proefbesluit: Proefbesluit,
    /// Bijvoorbeeld: de regeling noemt geen bevoegd gezag.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub waarschuwingen: Vec<String>,
}

/// Neem het besluit op een zaak en laat de cel het vastleggen als
/// stage-decretogram.
///
/// Het proefbesluit moet compleet zijn; is het dat niet, dan komt er geen
/// gram en zegt het proces wat er mist. Ligt de stage van het besluit al vast
/// in de zaak, dan weigert de cel (zie `api::cel::toets_zaak`): een tweede besluit
/// is een wijziging, en die valt buiten deze stap. Wijst de wet een ander bevoegd
/// gezag aan dan het gezag waarvoor het proces handelt (`namens`), en noemt het
/// proces geen mandaat van dat gezag, dan weigert het ook; noemt de wet er
/// geen, dan laat het vastleggen met een waarschuwing (zie [`crate::gezag`]).
/// Het gram draagt wie handelde: de rol, het kanaal en de identiteit van de
/// ingelogde gebruiker, namens welk gezag en, bij mandaat, op welke grondslag.
///
/// Het proces draait de engine, dus het proces stelt samen wat het besluit
/// tot besluit maakt: rechtskarakter, regeling, invoer met herkomst en het
/// receipt (de geladen regelingen en de stromen van de cel, met hun hash). De
/// cel bouwt het gram uit haar stroom, controleert de actor en legt vast.
#[allow(clippy::too_many_arguments)]
pub async fn neem_besluit(
    proces: &Proces,
    cel: &dyn Transport,
    bronnen: &[Bron],
    rijen: &[Rijen],
    regelingen: &[GeladenRegeling],
    zaakkenmerk: &str,
    formulier: &Map<String, Value>,
    op_moment: DateTime<FixedOffset>,
    handelend: &Sessie,
) -> Result<Besluit, Weigering> {
    let service = proces.service.as_ref();
    let actor = &proces.definitie.actor;
    let behandeling = proces
        .definitie
        .behandeling
        .as_ref()
        .ok_or_else(|| Weigering::Cel("geen behandeling geconfigureerd".into()))?;
    let b = &behandeling.besluit;
    let v = b
        .vastleggen
        .as_ref()
        .ok_or_else(|| Weigering::Cel("het besluit zegt niet waar het wordt vastgelegd".into()))?;

    let peildatum = datum::peildatum(&op_moment);
    let proef = proefbesluit(
        proces,
        cel,
        bronnen,
        rijen,
        zaakkenmerk,
        formulier,
        &peildatum,
    )
    .await?;
    if !proef.te_nemen {
        return Err(Weigering::NietTeNemen(
            proef
                .reden
                .clone()
                .unwrap_or_else(|| "niet te nemen".to_string()),
        ));
    }

    // Het bevoegd gezag: gelijk is vastleggen, ongelijk is weigeren,
    // ontbrekend is een waarschuwing.
    let mut waarschuwingen = Vec::new();
    let nummer = regelingen::ontleed(&proef.artikel)
        .map_err(Weigering::Cel)?
        .artikel;
    let gezag = gezag::gezag_van(service, &b.regeling, nummer);
    let eigen = proces.gezag.as_deref();
    let (namens, mandaat) = match &gezag {
        Some(g) => match gezag::toets(eigen, &proces.definitie.mandaten, g) {
            Ok(Bevoegdheid::Eigen) => (Some(g.clone()), None),
            Ok(Bevoegdheid::Mandaat(m)) => (Some(g.clone()), Some(m.grondslag.clone())),
            Err(reden) => {
                return Err(Weigering::Onbevoegd(format!("{}: {reden}", proef.artikel)));
            }
        },
        None => {
            waarschuwingen.push(format!(
                "regeling '{}' noemt geen bevoegd gezag bij {}; het besluit is vastgelegd zonder competent_authority",
                b.regeling, proef.artikel
            ));
            (eigen.map(str::to_string), None)
        }
    };
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

    // De uitkomsten worden de velden van het gram, elk onder zijn eigen naam.
    let external: Map<String, Value> = proef
        .uitkomsten
        .iter()
        .map(|(k, w)| (k.clone(), w.clone()))
        .collect();
    let artikel = regelingen::artikel(service, &proef.artikel).map_err(Weigering::Cel)?;
    let produces = artikel
        .get_execution_spec()
        .and_then(|e| e.produces.as_ref());
    // Elke parameter die meedeed gaat mee, met zijn herkomst; een parameter
    // zonder herkomst is een fout in het proces, geen reden hem weg te laten.
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
    // De stromen van de cel, met de hash die de cel voor elk bijhoudt.
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
    let verzoek = Vastlegverzoek {
        actor: actor.clone(),
        stroom: v.stroom.clone(),
        event: v.event.clone(),
        intake: Value::Null,
        external,
        zaakkenmerk: Some(zaakkenmerk.to_string()),
        besluit: Some(Besluitvelden {
            legal_character: produces.and_then(|p| p.legal_character.clone()),
            decision_type: produces.and_then(|p| p.decision_type.clone()),
            regulation: Some(b.regeling.clone()),
            regulation_valid_from: service.resolver().get_law(&b.regeling).and_then(|l| {
                l.valid_from
                    .clone()
                    .or_else(|| Some(l.publication_date.clone()))
            }),
            competent_authority: gezag,
            handelende_actor: Some(handelende_actor),
            inputs,
            receipt: Some(Receipt::nieuw(regelingen.to_vec(), stromen)),
        }),
    };
    let celclient::MetYaml { gram, yaml } = celclient::leg_vast(cel, &v.cel, &verzoek)
        .await
        .map_err(|f| match f {
            // De cel weigert: in deze zaak ligt die stage al vast. Of een
            // besluit vastlegbaar is, beslist de cel, onder haar slot.
            TransportFout::Antwoord { status: 409, fout } => Weigering::AlBesloten(fout),
            f => Weigering::Cel(format!("het besluit is niet vastgelegd: {f}")),
        })?;
    Ok(Besluit {
        gram,
        yaml,
        proefbesluit: proef,
        waarschuwingen,
    })
}
