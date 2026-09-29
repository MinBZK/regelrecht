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
///   enige input `wortel`;
/// - elke parameter uit het formulier, de stand van wat nog niet gebeurd is
///   of een `rijen`-blok is een parameter die de aanroeper van het artikel
///   moet leveren, en komt uit maar een bron;
/// - het vastleg-event volgt een zaak; bij een besluit legt het elke uitkomst
///   vast, bij een vervolg wat de stage vraagt en de uitkomsten van de
///   haken; elk veld is een uitkomst of een veld van het formulier.
pub fn controleer(proces: &Proces) -> Vec<String> {
    let mut fouten = Vec::new();
    let d = &proces.definitie;
    let cell = &proces.cell;
    let Some(handling) = &d.handling else {
        for b in d.zaakbronnen() {
            fouten.push(format!(
                "synthese-bron {}/{}: een bron van de zaak (zaak: true) vraagt een behandeling; de toets leest het concept",
                b.cell, b.lexostatus
            ));
        }
        return fouten;
    };
    match cell.lexostatuses.lexostatus(&handling.worklist.lexostatus) {
        None => fouten.push(format!(
            "handling: werkvoorraad '{}' is geen lexostatus van de cel",
            handling.worklist.lexostatus
        )),
        Some(l) if !l.is_lijst() => fouten.push(format!(
            "handling: werkvoorraad '{}' is geen lijst (groepeer: wortel)",
            l.name
        )),
        Some(_) => {}
    }

    // De lexostatussen van de zaak: een keer, voor alle handelingen.
    let case: Vec<&String> = d.zaakbronnen().map(|z| &z.lexostatus).collect();
    let mut uit_de_zaak: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for name in case.iter().copied() {
        match cell.lexostatuses.lexostatus(name) {
            None => fouten.push(format!("handling: lexostatus '{name}' bestaat niet")),
            Some(l) => {
                if l.is_lijst() {
                    fouten.push(format!(
                        "handling: lexostatus '{name}' is een lijst, en een lijst gaat nooit naar de engine"
                    ));
                }
                let inputs: Vec<&str> = l.inputs.iter().map(|i| i.name.as_str()).collect();
                if inputs != ["root"] {
                    fouten.push(format!(
                        "handling: lexostatus '{name}' heeft inputs [{}]; een handeling geeft alleen 'wortel' mee",
                        inputs.join(", ")
                    ));
                }
                for p in l.reduction.derivations.keys() {
                    uit_de_zaak
                        .entry(p)
                        .or_default()
                        .push(format!("de eigen lexostatus '{name}'"));
                }
            }
        }
    }
    // Een invoer uit een eerdere bron (een doorgegeven extra veld) komt niet
    // uit een eigen lexostatus; de synthese zelf controleert haar.
    let doorgegeven: Vec<&str> = d
        .andere_bronnen()
        .filter(|s| !s.extra_fields.is_empty())
        .map(|s| s.lexostatus.as_str())
        .collect();
    let mut invoer_uit: Vec<&str> = d
        .andere_bronnen()
        .flat_map(|s| s.input.values().filter_map(|v| v.field()))
        .map(|v| v.lexostatus.as_str())
        .filter(|l| !doorgegeven.contains(l))
        .collect();
    invoer_uit.sort_unstable();
    invoer_uit.dedup();
    if invoer_uit.len() > 1 {
        fouten.push(format!(
            "handling: de invoer van de synthese komt uit meer dan een lexostatus ({}); een handeling geeft haar uit een",
            invoer_uit.join(", ")
        ));
    }
    for source in d.andere_bronnen() {
        for (i, v) in source
            .input
            .iter()
            .filter_map(|(i, v)| Some((i, v.field()?)))
        {
            if !case.contains(&&v.lexostatus) && !doorgegeven.contains(&v.lexostatus.as_str()) {
                fouten.push(format!(
                    "handling: synthese-bron {}/{}, invoer '{i}': komt uit lexostatus '{}', en die is geen lexostatus van de zaak (zaak: true)",
                    source.cell, source.lexostatus, v.lexostatus
                ));
            }
        }
    }

    for h in &handling.actions {
        fouten.extend(controleer_handeling(proces, h, &uit_de_zaak, &case));
    }
    fouten
}

fn controleer_handeling(
    proces: &Proces,
    h: &HandelingDefinitie,
    uit_de_zaak: &BTreeMap<&str, Vec<String>>,
    case: &[&String],
) -> Vec<String> {
    let mut fouten = Vec::new();
    let d = &proces.definitie;
    let cell = &proces.cell;
    let service = proces.service.as_ref();
    let wie = format!("handeling '{}'", h.name);
    if let Some(r) = &h.role {
        match d.roles.get(r) {
            None => fouten.push(format!("{wie}: rol '{r}' staat niet onder rollen")),
            Some(role) if !role.mag(crate::kanaal::Routes::Handling) => fouten.push(format!(
                "{wie}: rol '{r}' mag de behandeling niet (routes: behandeling)"
            )),
            Some(_) => {}
        }
    }
    if h.article.is_empty() {
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
        .filter(|f| f.soort.as_deref() == Some("amount") && f.unit.is_none())
        .map(|f| f.name.as_str())
        .chain(
            h.verdicts
                .iter()
                .filter(|o| {
                    bedrag_oordelen.get(&o.parameter).is_some_and(|b| {
                        b.typing.soort == ParameterType::Amount && b.typing.unit.is_none()
                    })
                })
                .map(|o| o.parameter.as_str()),
        );
    for name in zonder_eenheid {
        fouten.push(format!(
            "{wie}: '{name}' is een bedrag zonder eenheid; geef de parameter in de regeling type_spec.unit (zoals eurocent)"
        ));
    }
    // De uitkomsten: van een artikel, bij een vervolg ook van de haken.
    let haakuitkomsten: BTreeSet<String> = h
        .hooks
        .iter()
        .flat_map(|a| uitkomsten_van(service, a))
        .collect();
    let mut artikelen = BTreeSet::new();
    for u in &h.outputs {
        if haakuitkomsten.contains(u) {
            continue;
        }
        match artikel_met(service, &h.regulation, u) {
            None => fouten.push(format!(
                "{wie}: regeling '{}' heeft geen uitkomst '{u}'",
                h.regulation
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
    let vl = format!("{wie}, vastleggen {}/{}", h.record.stream, h.record.event);
    if h.record.cell != cell.id() {
        // De cel van het proces meldt `proces::de_cel`.
    } else if let Some((_, event)) = cell.event(&h.record.stream, &h.record.event) {
        if event.case != Zaak::Follows {
            fouten.push(format!(
                "{vl}: het event heeft case: {}, en een handeling volgt de zaak van de aanvraag",
                event.case.als_tekst()
            ));
        }
        fouten.extend(controleer_besluit(proces, h, &vl));
        let sleutels = event.external_sleutels();
        let feiten: Vec<&str> = h.feiten.iter().map(|f| f.name.as_str()).collect();
        match &h.soort {
            Handelingsoort::Decision => {
                let absent: Vec<&str> = h
                    .outputs
                    .iter()
                    .filter(|u| !sleutels.contains(u))
                    .map(String::as_str)
                    .collect();
                if !absent.is_empty() {
                    fouten.push(format!(
                        "{vl}: het event legt de uitkomsten [{}] van het besluit niet vast",
                        absent.join(", ")
                    ));
                }
                if !feiten.is_empty() {
                    fouten.push(format!(
                        "{vl}: het event legt [{}] vast, en dat is geen uitkomst en geen oordeel van het besluit",
                        feiten.join(", ")
                    ));
                }
            }
            Handelingsoort::FollowUp { .. } => {
                let absent: Vec<&str> = feiten
                    .iter()
                    .copied()
                    .chain(haakuitkomsten.iter().map(String::as_str))
                    .filter(|k| !sleutels.iter().any(|s| s == k))
                    .collect();
                if !absent.is_empty() {
                    fouten.push(format!(
                        "{vl}: het event legt [{}] niet vast; een vervolg legt vast wat de stage vraagt en wat de haken uitrekenen (RFC-008, RFC-022 par. 3.3)",
                        absent.join(", ")
                    ));
                }
                let over: Vec<&str> = sleutels
                    .iter()
                    .map(String::as_str)
                    .filter(|k| !h.outputs.iter().any(|u| u == k) && !feiten.contains(k))
                    .collect();
                if !over.is_empty() {
                    fouten.push(format!(
                        "{vl}: [{}] is geen uitkomst en niets wat de stage vraagt",
                        over.join(", ")
                    ));
                }
            }
            Handelingsoort::Fact => {}
        }
    }

    if matches!(h.soort, Handelingsoort::FollowUp { .. }) {
        // Een vervolg leest het vastgelegde besluit, geen bronnen.
        if !h.rows.is_empty() {
            fouten.push(format!(
                "{wie}: een vervolg rekent op de invoer van het besluit, zonder synthese per regel"
            ));
        }
        return fouten;
    }

    // Een parameter komt uit maar een bron.
    let mut per: BTreeMap<&str, Vec<String>> = uit_de_zaak.clone();
    for source in d.andere_bronnen() {
        for p in &source.parameters {
            per.entry(p).or_default().push(format!(
                "synthese-bron {}/{}",
                source.cell, source.lexostatus
            ));
        }
    }
    for o in &h.verdicts {
        per.entry(&o.parameter)
            .or_default()
            .push("het formulier".into());
    }
    for p in h.not_yet.keys() {
        per.entry(p)
            .or_default()
            .push("de stand van wat nog niet gebeurd is".into());
    }
    let case: Vec<&str> = case.iter().map(|z| z.as_str()).collect();
    for r in &h.rows {
        per.entry(&r.parameter)
            .or_default()
            .push(format!("de synthese per regel uit '{}'", r.table.field));
        fouten.extend(rijen::controleer(
            &wie,
            r,
            &case,
            "geen lexostatus van de zaak (zaak: true)",
            d,
            cell,
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
        .verdicts
        .iter()
        .map(|o| ("formulier", o.parameter.as_str()))
        .chain(h.not_yet.keys().map(|p| ("nog niet gebeurd", p.as_str())))
        .chain(h.rows.iter().map(|r| ("rijen", r.parameter.as_str())));
    for (waar, p) in namen {
        if !benodigd.contains_key(p) {
            fouten.push(format!(
                "{wie}, {waar}: '{p}' is geen parameter van {} of van een artikel dat het zonder eigen parameters aanroept",
                h.article
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
    let role = h.decision_role.map_or("geen", Decision::als_tekst);
    let genoemd = h.decision.as_deref();
    let besluit_handeling = |name: &str| {
        proces
            .definitie
            .handling
            .as_ref()
            .and_then(|b| b.action(name))
            .filter(|b| b.soort == Handelingsoort::Decision && b.name != h.name)
    };
    match (&h.soort, h.decision_role) {
        (Handelingsoort::Decision, Some(Decision::Opens)) => {
            if let Some(b) = genoemd {
                fouten.push(format!(
                    "{vl}: het event opent een besluit, en een nieuw besluit noemt geen ander (besluit: {b}); een besluit dat een ander wijzigt, legt vast in een event met decision: wijzigt"
                ));
            }
        }
        (Handelingsoort::Decision, Some(Decision::Amends))
        | (Handelingsoort::Fact, Some(Decision::Follows)) => match genoemd {
            None => fouten.push(format!(
                "{vl}: het event {role} een besluit; noem met besluit de handeling van dat besluit"
            )),
            Some(b) if besluit_handeling(b).is_none() => fouten.push(format!(
                "handeling '{}': besluit '{b}' is geen andere handeling van een besluit in dit proces",
                h.name
            )),
            Some(_) => {}
        },
        (Handelingsoort::Decision, _) => fouten.push(format!(
            "{vl}: een besluit legt vast in een event met decision: opent (of wijzigt, als het een ander besluit wijzigt), niet decision: {role}"
        )),
        (Handelingsoort::FollowUp { decision, .. }, Some(Decision::Follows)) => {
            if genoemd.is_some_and(|b| b != decision) {
                fouten.push(format!(
                    "handeling '{}': een vervolg op het besluit van '{decision}' (hetzelfde artikel) noemt besluit '{}'",
                    h.name,
                    genoemd.unwrap_or_default()
                ));
            }
        }
        (Handelingsoort::FollowUp { .. }, _) => fouten.push(format!(
            "{vl}: een vervolg is een latere stage van een besluit, en legt vast in een event met decision: volgt, niet decision: {role}"
        )),
        (Handelingsoort::Fact, _) => {
            if let Some(b) = genoemd {
                fouten.push(format!(
                    "{vl}: het event volgt geen besluit (besluit: {role}), en de handeling noemt besluit '{b}'"
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
    if matches!(h.soort, Handelingsoort::FollowUp { .. }) {
        return Vec::new();
    }
    let benodigd: BTreeSet<String> = benodigd(&proces.service, h)
        .map(|b| b.into_keys().collect())
        .unwrap_or_default();
    let sources: Vec<&crate::config::SyntheseBron> = d.andere_bronnen().collect();
    let mut nodig: BTreeSet<usize> = sources
        .iter()
        .enumerate()
        .filter(|(_, b)| b.parameters.iter().any(|p| benodigd.contains(p)))
        .map(|(i, _)| i)
        .collect();
    // Lexostatussen waaruit een invoer komt: van de gekozen bronnen en van
    // de synthese per regel.
    let mut gevraagd: BTreeSet<String> = h
        .rows
        .iter()
        .flat_map(|r| {
            std::iter::once(r.table.lexostatus.clone()).chain(r.sources.iter().flat_map(|b| {
                b.input.values().filter_map(|i| match i {
                    crate::config::RijInvoer::Own { lexostatus, .. } => Some(lexostatus.clone()),
                    _ => None,
                })
            }))
        })
        .collect();
    loop {
        for i in &nodig {
            for v in sources[*i].input.values().filter_map(|v| v.field()) {
                gevraagd.insert(v.lexostatus.clone());
            }
        }
        let erbij: BTreeSet<usize> = sources
            .iter()
            .enumerate()
            .filter(|(i, b)| {
                !nodig.contains(i) && !b.extra_fields.is_empty() && gevraagd.contains(&b.lexostatus)
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
