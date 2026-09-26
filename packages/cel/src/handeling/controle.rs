//! De controles op `behandeling` bij het opstarten, en welke synthese-bronnen
//! een handeling vraagt.

use super::*;

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
    // Een bedrag in het formulier noemt zijn eenheid (`type_spec.unit`): de
    // frontend vraagt eurocent in euro, en zonder eenheid weet zij niet of
    // een ingevuld getal euro of eurocent is.
    let bedrag_oordelen = benodigd(service, h).unwrap_or_default();
    let zonder_eenheid = h
        .feiten
        .iter()
        .filter(|f| f.soort.as_deref() == Some("bedrag") && f.eenheid.is_none())
        .map(|f| f.naam.as_str())
        .chain(
            h.oordelen
                .iter()
                .filter(|o| {
                    bedrag_oordelen.get(&o.parameter).is_some_and(|b| {
                        b.typering.soort == ParameterType::Amount && b.typering.eenheid.is_none()
                    })
                })
                .map(|o| o.parameter.as_str()),
        );
    for naam in zonder_eenheid {
        fouten.push(format!(
            "{wie}: '{naam}' is een bedrag zonder eenheid; geef de parameter in de regeling type_spec.unit (zoals eurocent)"
        ));
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
