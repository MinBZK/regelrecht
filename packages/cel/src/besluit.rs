//! Het besluit op een zaak: een proefbesluit, zonder vastleggen.
//!
//! Het blok `behandeling.besluit` in `cel.yaml` zegt welke uitkomsten van
//! welk artikel het besluit zijn, en waar elke parameter vandaan komt. Een
//! parameter komt uit precies een bron:
//!
//! 1. een eigen lexostatus met als enige input `zaakkenmerk`, zoals de inhoud
//!    van de aanvraag of het verloop van de zaak;
//! 2. een synthese-bron (`synthese` in `cel.yaml`), met invoer uit een van die
//!    eigen lexostatussen;
//! 3. het besluitformulier: de oordelen van de behandelaar, die pas bij het
//!    besluiten bestaan;
//! 4. de stand bij besluit: feiten die pas na het besluit ontstaan, zoals de
//!    bekendmaking, met hun stand op het moment van besluiten.
//!
//! Het proefbesluit voert het artikel uit met wat die bronnen leveren. Mist
//! er iets, dan is het antwoord "niet te nemen: mist X"; er wordt niets
//! aangevuld en niets vastgelegd. Per parameter staat de herkomst erbij, en
//! `niet_geleverd` noemt elke parameter die de aanroeper van het artikel moet
//! leveren en die geen bron leverde, met de omschrijving uit de regeling.

use std::collections::BTreeMap;

use serde::Serialize;
use serde_json::{Map, Value};

use crate::cel::Cel;
use crate::config::BesluitDefinitie;
use crate::formulier::Veld;
use crate::kroniek::Kroniek;
use crate::reductie::{self, Lexostatus};
use crate::regelingen::{self, Benodigd};
use crate::synthese::{self, Bron, BronUitslag, Herkomst};
use crate::toets;

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
    /// De eigen lexostatussen van de zaak.
    pub lexostatussen: Vec<Lexostatus>,
}

/// Het artikel achter de uitkomsten, als `<regeling>#<artikel>`.
fn artikel_van(cel: &Cel, b: &BesluitDefinitie) -> Result<String, String> {
    let eerste = b
        .uitkomsten
        .first()
        .ok_or("het besluit noemt geen uitkomst")?;
    let artikel = cel
        .service
        .resolver()
        .get_article_by_output(&b.regeling, eerste, None)
        .ok_or_else(|| format!("regeling '{}' heeft geen uitkomst '{eerste}'", b.regeling))?;
    Ok(format!("{}#{}", b.regeling, artikel.number))
}

/// De parameters die de aanroeper van het besluitartikel moet leveren.
fn benodigd(cel: &Cel, b: &BesluitDefinitie) -> Result<BTreeMap<String, Benodigd>, String> {
    let grondslag = artikel_van(cel, b)?;
    let a = regelingen::artikel(&cel.service, &grondslag)?;
    Ok(regelingen::benodigde_parameters(
        &cel.service,
        &b.regeling,
        a,
    ))
}

/// De velden van het besluitformulier, met het type uit de regeling.
pub fn formuliervelden(cel: &Cel, b: &BesluitDefinitie) -> Vec<Veld> {
    let benodigd = benodigd(cel, b).unwrap_or_default();
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

/// De controles op `rollen` en `behandeling` bij het opstarten. Een fout hier
/// houdt de cel tegen:
///
/// - een portaal vraagt de rol aanvrager, en de rol aanvrager een portaal;
/// - een behandeling vraagt de rol behandelaar;
/// - de werkvoorraad is een lijst-lexostatus van de cel;
/// - de uitkomsten van het besluit zijn uitkomsten van een en hetzelfde
///   artikel;
/// - de lexostatussen van het besluit bestaan, zijn geen lijst en hebben als
///   enige input `zaakkenmerk`;
/// - elke parameter uit het formulier of de stand bij besluit is een
///   parameter die de aanroeper van het artikel moet leveren;
/// - een parameter komt uit maar een bron;
/// - de invoer van elke synthese-bron komt uit een lexostatus van het besluit.
pub fn controleer(cel: &Cel) -> Vec<String> {
    let mut fouten = Vec::new();
    let d = &cel.definitie;
    match (d.portaal.is_some(), d.rollen.aanvrager.is_some()) {
        (true, false) => fouten
            .push("portaal zonder rol aanvrager: zet rollen: {aanvrager: eherkenning}".to_string()),
        (false, true) => {
            fouten.push("rol aanvrager zonder portaal: de aanvrager heeft niets te doen".into())
        }
        _ => {}
    }
    let Some(behandeling) = &d.behandeling else {
        return fouten;
    };
    if d.rollen.behandelaar.is_none() {
        fouten.push(
            "behandeling zonder rol behandelaar: zet rollen: {behandelaar: medewerker}".into(),
        );
    }
    match cel.lexostatussen.lexostatus(&behandeling.werkvoorraad) {
        None => fouten.push(format!(
            "behandeling: werkvoorraad '{}' is geen lexostatus van de cel",
            behandeling.werkvoorraad
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
        match cel
            .service
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
    for naam in &b.lexostatussen {
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
    let mut invoer_uit: Vec<&str> = d
        .synthese
        .iter()
        .flat_map(|s| s.invoer.values().map(|v| v.lexostatus.as_str()))
        .collect();
    invoer_uit.sort_unstable();
    invoer_uit.dedup();
    if invoer_uit.len() > 1 {
        fouten.push(format!(
            "besluit: de invoer van de synthese komt uit meer dan een lexostatus ({}); het besluit geeft haar uit een",
            invoer_uit.join(", ")
        ));
    }
    for bron in &d.synthese {
        for p in &bron.parameters {
            per.entry(p)
                .or_default()
                .push(format!("synthese-bron {}/{}", bron.cel, bron.lexostatus));
        }
        for (i, v) in &bron.invoer {
            if !b.lexostatussen.contains(&v.lexostatus) {
                fouten.push(format!(
                    "besluit: synthese-bron {}/{}, invoer '{i}': komt uit lexostatus '{}', en die staat niet in de lexostatussen van het besluit",
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

    if artikelen.len() == 1 {
        match benodigd(cel, b) {
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
                    );
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
    /// De cel zelf: configuratie, kroniek of reductie.
    Cel(String),
}

/// Reken het besluit op een zaak uit, zonder iets vast te leggen. `peildatum`
/// (JJJJ-MM-DD) is de datum waarop de engine de regeling leest.
pub async fn proefbesluit(
    cel: &Cel,
    kroniek: &Kroniek,
    bronnen: &[Bron],
    zaakkenmerk: &str,
    formulier: &Map<String, Value>,
    peildatum: &str,
) -> Result<Proefbesluit, Weigering> {
    let b = &cel
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
    let artikel = artikel_van(cel, b).map_err(Weigering::Cel)?;

    // 1. De eigen lexostatussen van de zaak.
    let mut inputs = Map::new();
    inputs.insert("zaakkenmerk".into(), Value::String(zaakkenmerk.to_string()));
    let mut eigen = Vec::new();
    for naam in &b.lexostatussen {
        let def = cel
            .lexostatussen
            .lexostatus(naam)
            .ok_or_else(|| Weigering::Cel(format!("lexostatus '{naam}' bestaat niet")))?;
        let grammen = kroniek
            .lees(&def.reduction.kroniek)
            .map_err(Weigering::Cel)?;
        // Kiest de definitie een gram en is er geen, dan levert zij niets.
        let l = reductie::reduceer(def, &inputs, &grammen)
            .map_err(Weigering::Cel)?
            .unwrap_or_else(|| Lexostatus {
                naam: def.name.clone(),
                zaakkenmerk: None,
                op_moment: None,
                parameters: BTreeMap::new(),
                extra_velden: BTreeMap::new(),
                niet_afgeleid: def.reduction.afleidingen.keys().cloned().collect(),
                lijst: None,
            });
        eigen.push(l);
    }

    // 2. Synthese, met de invoer uit de eigen lexostatus die haar levert.
    let invoer_uit = cel
        .definitie
        .synthese
        .iter()
        .flat_map(|s| s.invoer.values())
        .map(|v| v.lexostatus.as_str())
        .next();
    let hoofd = invoer_uit
        .and_then(|n| eigen.iter().position(|l| l.naam == n))
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

    // 3. De oordelen van de behandelaar; een leeg veld gaat niet mee.
    for (p, w) in formulier {
        if !w.is_null() {
            samen.parameters.insert(p.clone(), w.clone());
            samen.herkomst.insert(p.clone(), Herkomst::Behandelaar);
        }
    }

    // 4. De stand bij besluit.
    for (p, w) in &b.stand_bij_besluit {
        samen.parameters.insert(p.clone(), w.clone());
        samen.herkomst.insert(p.clone(), Herkomst::StandBijBesluit);
    }

    let uitkomsten: Vec<&str> = b.uitkomsten.iter().map(String::as_str).collect();
    let e = toets::evalueer(
        &cel.service,
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
    let niet_geleverd = benodigd(cel, b)
        .map_err(Weigering::Cel)?
        .into_values()
        .filter(|p| !samen.parameters.contains_key(&p.naam))
        .collect();
    Ok(Proefbesluit {
        regeling: b.regeling.clone(),
        artikel,
        peildatum: peildatum.to_string(),
        te_nemen,
        uitkomsten: if te_nemen { e.waarden } else { BTreeMap::new() },
        mist: e.mist,
        reden,
        parameters: samen.parameters,
        herkomst: samen.herkomst,
        bronnen: samen.bronnen,
        niet_geleverd,
        lexostatussen: eigen,
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
