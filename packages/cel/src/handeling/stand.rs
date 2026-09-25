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
    let reden = match doel(proces, h, zaak, None) {
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
pub(super) fn al_genomen(
    proces: &Proces,
    h: &HandelingDefinitie,
    zaak: &Zaakstand,
) -> Option<String> {
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
                    doel(proces, x, zaak, None)
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
