//! Een handeling nemen: de proef, het bevoegd gezag, en het vastleggen door
//! de cel.

use super::*;

/// Een genomen handeling: het vastgelegde gram, de proef waaruit het
/// volgde, en wat er bij het vastleggen op te merken viel.
#[derive(Debug, Clone, Serialize)]
pub struct Genomen {
    pub gram: Gram,
    pub yaml: String,
    pub trial: Proefhandeling,
    pub warnings: Vec<String>,
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
    root: &str,
    case: &Zaakstand,
    opgave: &Opgave,
    handelend: &Sessie,
) -> Result<Genomen, Weigering> {
    let (form, happened) = (&opgave.form, opgave.happened);
    let proces = om.proces;
    let service = proces.service.as_ref();
    let actor = &proces.definitie.actor;
    if happened && h.soort == Handelingsoort::Decision {
        return Err(Weigering::Ongeldig(format!(
            "handeling '{}' is een decision: dat neemt het proces zelf, het wordt niet als gebeurd gemeld",
            h.name
        )));
    }
    let trial = trial(om, h, root, case, opgave).await?;
    let mut warnings = Vec::new();
    if !trial.takeable {
        let reason = trial
            .reason
            .clone()
            .unwrap_or_else(|| "niet te nemen".to_string());
        if !(happened && trial.reportable) {
            return Err(Weigering::NietTeNemen(if trial.reportable {
                format!("{reason}; is het toch gebeurd, meld het dan als gebeurd (gebeurd: true)")
            } else {
                reason
            }));
        }
        warnings.push(format!(
            "gemeld als gebeurd, tegen de conclusie van het proces in: {reason}"
        ));
    }
    let (stream, event) = proces
        .cell
        .event(&h.record.stream, &h.record.event)
        .ok_or_else(|| Weigering::Cell(format!("handeling '{}': geen vastleg-event", h.name)))?;

    let eigen = proces.authority.as_deref();
    let mut authority = None;
    let (mut on_behalf_of, mut mandate) = (None, None);
    if !matches!(h.soort, Handelingsoort::Fact) {
        let nummer = regelingen::ontleed(&h.article)
            .map_err(Weigering::Cell)?
            .article;
        authority = gezag::gezag_van(service, &h.regulation, nummer);
        match &authority {
            Some(g) => match gezag::assessment(eigen, &proces.definitie.mandates, g) {
                Ok(Bevoegdheid::Own) => on_behalf_of = Some(g.clone()),
                Ok(Bevoegdheid::Mandaat(m)) => {
                    on_behalf_of = Some(g.clone());
                    mandate = Some(m.legal_basis.clone());
                }
                Err(reason) => {
                    return Err(Weigering::Onbevoegd(format!("{}: {reason}", h.article)));
                }
            },
            None => {
                warnings.push(format!(
                    "regeling '{}' noemt geen bevoegd gezag bij {}; vastgelegd zonder competent_authority",
                    h.regulation, h.article
                ));
                on_behalf_of = eigen.map(str::to_string);
            }
        }
    }
    let acting_actor = HandelendeActor {
        role: handelend.role.clone(),
        channel: handelend.channel.clone(),
        identity: handelend.fields.clone(),
        legal_basis: proces
            .definitie
            .roles
            .get(&handelend.role)
            .and_then(|r| r.legal_basis.clone()),
        on_behalf_of,
        mandate,
    };

    let external = event_velden(event, &h.outputs, form, &trial.outputs);
    // Elke parameter die meedeed gaat mee, met zijn herkomst.
    let mut inputs: BTreeMap<String, Invoer> = BTreeMap::new();
    for (name, value) in &trial.parameters {
        let provenance = trial
            .provenance
            .get(name)
            .ok_or_else(|| Weigering::Cell(format!("parameter '{name}' heeft geen herkomst")))?;
        inputs.insert(
            name.clone(),
            Invoer {
                value: value.clone(),
                provenance: provenance.clone(),
            },
        );
    }
    let mut streams: Vec<StroomVerwijzing> = proces
        .cell
        .streams
        .iter()
        .map(|s| StroomVerwijzing {
            id: s.id.clone(),
            sha256: s.sha256.clone(),
        })
        .collect();
    streams.sort_by(|a, b| a.id.cmp(&b.id));
    // Het rechtskarakter en de regeling horen bij een besluit (een
    // decretogram); invoer en receipt bij elke handeling die de engine
    // uitrekende.
    let decretogram = event.type_ == "decretogram";
    let article = regelingen::article(service, &h.article).map_err(Weigering::Cell)?;
    let produces = article
        .get_execution_spec()
        .and_then(|e| e.produces.as_ref())
        .filter(|_| decretogram);
    // De versie van de regeling: haar `valid_from`. Noemt zij die niet, dan
    // de publicatiedatum, met een waarschuwing: de versie is dan een
    // aanname.
    let mut regulation_valid_from = None;
    if decretogram {
        let law = service.resolver().get_law(&h.regulation).ok_or_else(|| {
            Weigering::Cell(format!("regeling '{}' is niet geladen", h.regulation))
        })?;
        regulation_valid_from = Some(match &law.valid_from {
            Some(v) => v.clone(),
            None => {
                warnings.push(format!(
                    "regeling '{}' noemt geen valid_from; regulation_valid_from is haar publicatiedatum ({})",
                    h.regulation, law.publication_date
                ));
                law.publication_date.clone()
            }
        });
    }
    let verzoek = Vastlegverzoek {
        actor: actor.clone(),
        stream: stream.id.clone(),
        event: event.name.clone(),
        intake: Value::Null,
        external,
        // Een besluit verwijst naar de aanvraag (de wortel); een gram dat een
        // besluit volgt of wijzigt, naar dat besluit. Het id geeft de cel.
        refers_to: super::verwijzingen(
            &proces.cell,
            event,
            root,
            match h.decision_role {
                Some(Decision::Follows | Decision::Amends) => {
                    trial.decision.as_ref().map(|b| b.id.as_str())
                }
                _ => None,
            },
        ),
        decision: Some(Besluitvelden {
            legal_character: produces.and_then(|p| p.legal_character.clone()),
            decision_type: produces.and_then(|p| p.decision_type.clone()),
            regulation: decretogram.then(|| h.regulation.clone()),
            regulation_valid_from,
            competent_authority: authority.filter(|_| decretogram),
            acting_actor: Some(acting_actor),
            inputs,
            receipt: Some(Receipt::nieuw(om.regulations.to_vec(), streams)),
        }),
        root_grams: Some(case.grams),
    };
    let MetYaml { gram, yaml } = celclient::leg_vast(om.cell, &h.record.cell, &verzoek)
        .await
        .map_err(|f| match f {
            // De vorm (de stage, de zaak, het moment) toetst de cel, onder haar slot.
            TransportFout::Antwoord { status: 409, error } => Weigering::Conflict(error),
            TransportFout::Antwoord { status: 400, error } => Weigering::Ongeldig(error),
            f => Weigering::Cell(format!("de handeling is niet vastgelegd: {f}")),
        })?;
    Ok(Genomen {
        gram,
        yaml,
        trial,
        warnings,
    })
}
