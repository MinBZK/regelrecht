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
//!    bekendmaking, met hun stand op het moment van besluiten.
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

use crate::celclient::{self, Besluitvelden, Vastlegverzoek};
use crate::config::BesluitDefinitie;
use crate::formulier::Veld;
use crate::proces::Proces;
use crate::reductie::Lexostatus;
use crate::regelingen::{self, Benodigd};
use crate::rijen::{self, Rijen};
use crate::stroom::{GeladenRegeling, Gram, Invoer, Receipt, StroomVerwijzing, Zaak};
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

/// De velden van het besluitformulier, met het type uit de regeling.
pub fn formuliervelden(service: &LawExecutionService, b: &BesluitDefinitie) -> Vec<Veld> {
    let benodigd = benodigd(service, b).unwrap_or_default();
    b.formulier
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
            }
        })
        .collect()
}

/// De controles op `rollen` en `behandeling` van een proces bij het
/// opstarten. Een fout hier houdt de runtime tegen:
///
/// - een portaal vraagt de rol aanvrager, en de rol aanvrager een portaal;
/// - een behandeling vraagt de rol behandelaar;
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
    match (d.portaal.is_some(), d.rollen.aanvrager.is_some()) {
        (true, false) => fouten
            .push("portaal zonder rol aanvrager: zet rollen: {aanvrager: eherkenning}".to_string()),
        (false, true) => {
            fouten.push("rol aanvrager zonder portaal: de aanvrager heeft niets te doen".into())
        }
        _ => {}
    }
    let Some(behandeling) = &d.behandeling else {
        for b in d.zaakbronnen() {
            fouten.push(format!(
                "synthese-bron {}/{}: een bron van de zaak (zaak: true) vraagt een behandeling; de toets leest het concept",
                b.cel, b.lexostatus
            ));
        }
        return fouten;
    };
    if d.rollen.behandelaar.is_none() {
        fouten.push(
            "behandeling zonder rol behandelaar: zet rollen: {behandelaar: medewerker}".into(),
        );
    }
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
        .flat_map(|s| s.invoer.values().map(|v| v.lexostatus.as_str()))
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
        for (i, v) in &bron.invoer {
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
) -> Result<Lexostatus, Weigering> {
    let def = proces
        .cel
        .lexostatussen
        .lexostatus(&bron.lexostatus)
        .ok_or_else(|| Weigering::Cel(format!("lexostatus '{}' bestaat niet", bron.lexostatus)))?;
    match cel
        .haal(&synthese::pad(&bron.cel, &bron.lexostatus, inputs))
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
            naam: def.name.clone(),
            zaakkenmerk: None,
            op_moment: None,
            parameters: BTreeMap::new(),
            extra_velden: BTreeMap::new(),
            niet_afgeleid: def.reduction.afleidingen.keys().cloned().collect(),
            lijst: None,
        }),
        Err(f) => Err(Weigering::Cel(format!(
            "cel '{}', lexostatus '{}': {f}",
            bron.cel, bron.lexostatus
        ))),
    }
}

/// Reken het besluit op een zaak uit, zonder iets vast te leggen. `peildatum`
/// (JJJJ-MM-DD) is de datum waarop de engine de regeling leest. De
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

    // 1. De lexostatussen van de zaak, uit de cel.
    let mut inputs = Map::new();
    inputs.insert("zaakkenmerk".into(), Value::String(zaakkenmerk.to_string()));
    let mut eigen = Vec::new();
    for bron in proces.definitie.zaakbronnen() {
        eigen.push(zaaklexostatus(proces, cel, bron, &inputs).await?);
    }

    // 2. Synthese, met de invoer uit de lexostatus van de zaak die haar
    // levert. Een invoer die een eerdere bron doorgeeft, wijst geen
    // lexostatus van de zaak aan.
    let hoofd = proces
        .definitie
        .andere_bronnen()
        .flat_map(|s| s.invoer.values())
        .find_map(|v| eigen.iter().position(|l| l.naam == v.lexostatus))
        .unwrap_or(0);
    let mut samen = match eigen.get(hoofd) {
        Some(l) => synthese::voeg_samen(l, bronnen).await,
        None => synthese::voeg_samen(&leeg(), bronnen).await,
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
    let uitslagen = rijen::pas_toe(rijen, &eigen, &mut samen).await;

    // 4. De oordelen van de behandelaar; een leeg veld gaat niet mee.
    for (p, w) in formulier {
        if !w.is_null() {
            samen.parameters.insert(p.clone(), w.clone());
            samen.herkomst.insert(p.clone(), Herkomst::Behandelaar);
        }
    }

    // 5. De stand bij besluit.
    for (p, w) in &b.stand_bij_besluit {
        samen.parameters.insert(p.clone(), w.clone());
        samen.herkomst.insert(p.clone(), Herkomst::StandBijBesluit);
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

/// Een naam vergelijkbaar maken: kleine letters, en alles wat geen letter of
/// cijfer is wordt een liggend streepje. Zo is een gezag dat de regeling
/// voluit noemt ("De Raad van Voorbeeld") te vergelijken met de id van een
/// cel (`de_raad_van_voorbeeld`).
pub(crate) fn genormaliseerd(naam: &str) -> String {
    let mut uit = String::new();
    for c in naam.to_lowercase().chars() {
        if c.is_alphanumeric() {
            uit.push(c);
        } else if !uit.ends_with('_') {
            uit.push('_');
        }
    }
    uit.trim_matches('_').to_string()
}

/// De beschikkingen waarvoor `gezag` bevoegd is, als (regeling, artikel):
/// elk artikel dat een `BESCHIKKING` produceert en waarvan het bevoegd gezag
/// (van het artikel, anders van de regeling) na normalisatie gelijk is aan
/// `gezag`. Zo vindt een proces zijn besluit in de wet, zonder dat de
/// configuratie het aanwijst.
pub fn beschikkingen_van(
    service: &regelrecht_engine::LawExecutionService,
    gezag: &str,
) -> Vec<(String, String)> {
    let mut uit = Vec::new();
    for id in service.list_laws() {
        let Some(law) = service.resolver().get_law(id) else {
            continue;
        };
        for a in &law.articles {
            let beschikking = a
                .get_execution_spec()
                .and_then(|e| e.produces.as_ref())
                .and_then(|p| p.legal_character.as_deref())
                == Some("BESCHIKKING");
            if beschikking
                && gezag_van(service, id, &a.number)
                    .is_some_and(|g| genormaliseerd(&g) == genormaliseerd(gezag))
            {
                uit.push((id.to_string(), a.number.clone()));
            }
        }
    }
    uit.sort();
    uit.dedup();
    uit
}

/// Het bevoegd gezag volgens de wet: van het artikel zelf, anders van de
/// regeling. Een verwijzing (`#bevoegd_gezag`) telt niet als een naam.
pub(crate) fn gezag_van(
    service: &regelrecht_engine::LawExecutionService,
    regeling: &str,
    artikel: &str,
) -> Option<String> {
    fn naam(a: &Value) -> Option<String> {
        match a {
            // Een verwijzing zoals '#bevoegd_gezag' is geen naam.
            Value::String(s) => s.strip_prefix('#').is_none().then(|| s.clone()),
            Value::Object(o) => o.get("name").and_then(Value::as_str).map(str::to_string),
            _ => None,
        }
    }
    let law = service.resolver().get_law(regeling)?;
    let gezag = law
        .find_article_by_number(artikel)
        .and_then(|a| a.machine_readable.as_ref())
        .and_then(|m| m.competent_authority.as_ref())
        .or(law.competent_authority.as_ref())?;
    naam(&serde_json::to_value(gezag).ok()?)
}

/// Neem het besluit op een zaak en laat de cel het vastleggen als
/// stage-decretogram.
///
/// Het proefbesluit moet compleet zijn; is het dat niet, dan komt er geen
/// gram en zegt het proces wat er mist. Ligt de stage van het besluit al vast
/// in de zaak, dan weigert de cel (zie `api::cel::toets_zaak`): een tweede besluit
/// is een wijziging, en die valt buiten deze stap. Wijst de wet een ander bevoegd
/// gezag aan dan de actor van het proces, dan weigert het ook; noemt de wet
/// er geen, dan laat het vastleggen met een waarschuwing.
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

    let peildatum = op_moment.format("%Y-%m-%d").to_string();
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
    let nummer = proef.artikel.split_once('#').map(|(_, n)| n).unwrap_or("");
    let gezag = gezag_van(service, &b.regeling, nummer);
    match &gezag {
        Some(g) if genormaliseerd(g) != genormaliseerd(actor) => {
            return Err(Weigering::Onbevoegd(format!(
                "{} wijst '{g}' aan als bevoegd gezag, en deze cel is '{actor}'",
                proef.artikel
            )))
        }
        Some(_) => {}
        None => waarschuwingen.push(format!(
            "regeling '{}' noemt geen bevoegd gezag bij {}; het besluit is vastgelegd zonder competent_authority",
            b.regeling, proef.artikel
        )),
    }

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
    let inputs: BTreeMap<String, Invoer> = proef
        .parameters
        .iter()
        .filter_map(|(naam, waarde)| {
            Some((
                naam.clone(),
                Invoer {
                    waarde: waarde.clone(),
                    herkomst: proef.herkomst.get(naam)?.clone(),
                },
            ))
        })
        .collect();
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

fn leeg() -> Lexostatus {
    Lexostatus {
        naam: String::new(),
        zaakkenmerk: None,
        op_moment: None,
        parameters: BTreeMap::new(),
        extra_velden: BTreeMap::new(),
        niet_afgeleid: Vec::new(),
        lijst: None,
    }
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
        output: [{name: toets, type: boolean}]
        actions: [{output: toets, value: {operation: GREATER_THAN, subject: $x, value: 0}}]
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

    #[test]
    fn de_beschikking_van_het_bevoegd_gezag_wordt_gevonden() {
        let s = service();
        assert_eq!(
            beschikkingen_van(&s, "de_instantie_van_voorbeeld"),
            vec![("testregeling_bevoegd".to_string(), "2".to_string())]
        );
    }

    #[test]
    fn een_ander_gezag_vindt_niets() {
        assert!(beschikkingen_van(&service(), "een_ander_orgaan").is_empty());
    }
}
