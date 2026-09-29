//! De stand van een handeling en van de besluiten in een zaak, voor het
//! zaakscherm.

use super::*;

/// Of een handeling in een zaak kan, naar de besluiten en hun stages: een
/// besluit zolang de zaak geen besluit van die handeling heeft (een ander
/// besluit over dezelfde aanvraag vraagt een eigen grondslag: een
/// wijziging), een wijziging als er een besluit is om te wijzigen, een
/// vervolg als het besluit er ligt en zijn stage nog niet, een feit dat een
/// besluit volgt als dat besluit er ligt, en elk ander feit altijd (wat het
/// doet, zegt de proef).
#[derive(Debug, Clone, Serialize)]
pub struct Stand {
    pub available: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reason: Option<String>,
    /// De grammen die de handeling in deze zaak al vastlegde.
    pub recorded: usize,
    /// Het besluit waarop de handeling nu zou handelen (zie [`doel`]).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub decision: Option<String>,
}

/// De stand van een handeling in een zaak, uit de [`Zaakstand`] die de cel
/// afleidt: welke besluiten er liggen, welke stages elk doorliep en hoe vaak
/// het event van de handeling.
pub fn stand(proces: &Proces, h: &HandelingDefinitie, case: &Zaakstand) -> Stand {
    let recorded = case.aantal(&h.record.stream, &h.record.event);
    let mut decision = None;
    let reason = match doel(proces, h, case, None) {
        Err(r) => Some(r),
        Ok(b) => {
            decision = b.map(|b| b.id.clone());
            match (&h.soort, b) {
                (Handelingsoort::Decision, _) => al_genomen(proces, h, case),
                (Handelingsoort::FollowUp { .. }, Some(b)) => h
                    .stage
                    .as_ref()
                    .filter(|s| b.stages.contains_key(*s))
                    .map(|s| format!("stage {s} ligt al in besluit {}", b.id)),
                _ => None,
            }
        }
    };
    Stand {
        available: reason.is_none(),
        reason,
        recorded,
        decision,
    }
}

/// Of een besluit dat geen ander wijzigt, al in de zaak ligt: de cel legt
/// geen tweede besluit van hetzelfde event in een zaak vast. Een ander
/// besluit over dezelfde aanvraag is een wijziging, met een eigen grondslag.
pub(super) fn al_genomen(
    proces: &Proces,
    h: &HandelingDefinitie,
    case: &Zaakstand,
) -> Option<String> {
    if h.decision_role != Some(Decision::Opens) {
        return None;
    }
    eigen(proces, &h.name, case).first().map(|b| {
        format!(
            "besluit {} ligt al in de zaak; een ander besluit hierover vraagt een eigen grondslag (een wijziging)",
            b.id
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
    pub recorded: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub action: Option<String>,
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
    pub after: String,
    /// De stage die nu loopt.
    pub stage: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
    /// De haken die de route uitrekenden, als `<regeling>#<artikel>`.
    pub legal_basis: Vec<String>,
    /// Hun uitkomsten, zoals het gram van de laatste stage ze vastlegde.
    pub outputs: BTreeMap<String, Value>,
}

/// Een besluit in de zaak, voor het zaakscherm: welke handeling het nam, de
/// procedure met zijn stages, de rechtsbescherming die daaruit volgt, en de
/// handelingen die nu op dit besluit handelen (zijn vervolg, de feiten die
/// het volgen, een wijziging).
#[derive(Debug, Clone, Serialize)]
pub struct BesluitInZaak {
    /// Het id van het gram dat het besluit is.
    pub id: String,
    /// De handeling die het besluit vastlegde, en haar artikel.
    pub action: String,
    pub label: String,
    pub article: String,
    pub event: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub effective_at: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub amends: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub procedure: Option<ProcedureStand>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub legal_protection: Option<Rechtsbescherming>,
    /// De handelingen die nu op dit besluit handelen.
    pub actions: Vec<String>,
}

/// De besluiten in een zaak, elk met zijn procedure en rechtsbescherming.
/// Welke besluiten er liggen, welke stages elk doorliep en wat hun grammen
/// vastlegden, zegt de [`Zaakstand`] van de cel; de procedure en de haken
/// komen uit de wet. Een stage die bij geen besluit hoort (de aanvraag),
/// telt voor elk besluit van de zaak.
pub fn besluiten_in_zaak(proces: &Proces, case: &Zaakstand) -> Vec<BesluitInZaak> {
    let Some(handling) = &proces.definitie.handling else {
        return Vec::new();
    };
    case.decisions
        .iter()
        .filter_map(|b| {
            let h = handling.actions.iter().find(|h| {
                h.soort == Handelingsoort::Decision && b.van(&h.record.stream, &h.record.event)
            })?;
            let (procedure, legal_protection) = procedure_en_route(proces, h, b, case);
            let actions = handling
                .actions
                .iter()
                .filter(|x| x.name != h.name)
                .filter(|x| {
                    doel(proces, x, case, None)
                        .ok()
                        .flatten()
                        .is_some_and(|d| d.id == b.id)
                })
                .map(|x| x.name.clone())
                .collect();
            Some(BesluitInZaak {
                id: b.id.clone(),
                action: h.name.clone(),
                label: h.label().to_string(),
                article: h.article.clone(),
                event: b.event.clone(),
                effective_at: b.genomen().map(|(_, g)| g.effective_at.clone()),
                amends: b.amends.clone(),
                procedure,
                legal_protection,
                actions,
            })
        })
        .collect()
}

/// De procedure en de rechtsbescherming van een besluit in de zaak.
fn procedure_en_route(
    proces: &Proces,
    decision: &HandelingDefinitie,
    stand: &Besluitstand,
    case: &Zaakstand,
) -> (Option<ProcedureStand>, Option<Rechtsbescherming>) {
    let service = proces.service.as_ref();
    let Some(handling) = &proces.definitie.handling else {
        return (None, None);
    };
    let Some(p) = procedure_van(service, &decision.article) else {
        return (None, None);
    };
    let door = |stage: &str| {
        handling
            .actions
            .iter()
            .find(|h| h.stage.as_deref() == Some(stage) && h.article == decision.article)
    };
    let gram = |stage: &str| stand.stages.get(stage).or_else(|| case.stages.get(stage));
    let stages: Vec<StageStand> = p
        .stages
        .iter()
        .map(|s| StageStand {
            name: s.name.clone(),
            description: s.description.clone(),
            recorded: gram(&s.name).is_some(),
            action: door(&s.name).map(|h| h.name.clone()),
        })
        .collect();
    let route = stages.iter().rposition(|s| s.recorded).and_then(|i| {
        let after = &p.stages[i];
        let volgende = p.stages.get(i + 1)?;
        if door(&volgende.name).is_some() {
            return None;
        }
        let h = door(&after.name)?;
        if h.hooks.is_empty() {
            return None;
        }
        let gram = gram(&after.name)?;
        let outputs = h
            .hooks
            .iter()
            .flat_map(|a| uitkomsten_van(service, a))
            .filter_map(|u| gram.fields.get(&u).map(|w| (u, w.clone())))
            .collect();
        Some(Rechtsbescherming {
            procedure: p.id.clone(),
            after: after.name.clone(),
            stage: volgende.name.clone(),
            description: volgende.description.clone(),
            legal_basis: h.hooks.clone(),
            outputs,
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
pub fn procedure_van_de_zaak(proces: &Proces, case: &Zaakstand) -> Option<ProcedureStand> {
    let b = proces
        .definitie
        .handling
        .as_ref()?
        .actions
        .iter()
        .find(|h| {
            h.soort == Handelingsoort::Decision && h.decision_role == Some(Decision::Opens)
        })?;
    let p = procedure_van(proces.service.as_ref(), &b.article)?;
    Some(ProcedureStand {
        id: p.id.clone(),
        stages: p
            .stages
            .iter()
            .map(|s| StageStand {
                name: s.name.clone(),
                description: s.description.clone(),
                recorded: case.stages.contains_key(&s.name),
                action: None,
            })
            .collect(),
    })
}
