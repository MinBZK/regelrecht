//! Een handeling nemen: de proef, het bevoegd gezag, en het vastleggen door
//! de cel.

use super::*;

/// Een genomen handeling: het vastgelegde gram, de proef waaruit het
/// volgde, en wat er bij het vastleggen op te merken viel.
#[derive(Debug, Clone, Serialize)]
pub struct Genomen {
    pub gram: Gram,
    pub yaml: String,
    pub proef: Proefhandeling,
    pub waarschuwingen: Vec<String>,
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
    zaakkenmerk: &str,
    zaak: &Zaakstand,
    opgave: &Opgave,
    handelend: &Sessie,
) -> Result<Genomen, Weigering> {
    let (formulier, gebeurd) = (&opgave.formulier, opgave.gebeurd);
    let proces = om.proces;
    let service = proces.service.as_ref();
    let actor = &proces.definitie.actor;
    if gebeurd && h.soort == Handelingsoort::Besluit {
        return Err(Weigering::Ongeldig(format!(
            "handeling '{}' is een besluit: dat neemt het proces zelf, het wordt niet als gebeurd gemeld",
            h.naam
        )));
    }
    let proef = proef(om, h, zaakkenmerk, zaak, opgave).await?;
    let mut waarschuwingen = Vec::new();
    if !proef.te_nemen {
        let reden = proef
            .reden
            .clone()
            .unwrap_or_else(|| "niet te nemen".to_string());
        if !(gebeurd && proef.te_melden) {
            return Err(Weigering::NietTeNemen(if proef.te_melden {
                format!("{reden}; is het toch gebeurd, meld het dan als gebeurd (gebeurd: true)")
            } else {
                reden
            }));
        }
        waarschuwingen.push(format!(
            "gemeld als gebeurd, tegen de conclusie van het proces in: {reden}"
        ));
    }
    let (stroom, event) = proces
        .cel
        .event(&h.vastleggen.stroom, &h.vastleggen.event)
        .ok_or_else(|| Weigering::Cel(format!("handeling '{}': geen vastleg-event", h.naam)))?;

    let eigen = proces.gezag.as_deref();
    let mut gezag = None;
    let (mut namens, mut mandaat) = (None, None);
    if !matches!(h.soort, Handelingsoort::Feit) {
        let nummer = regelingen::ontleed(&h.artikel)
            .map_err(Weigering::Cel)?
            .artikel;
        gezag = gezag::gezag_van(service, &h.regeling, nummer);
        match &gezag {
            Some(g) => match gezag::toets(eigen, &proces.definitie.mandaten, g) {
                Ok(Bevoegdheid::Eigen) => namens = Some(g.clone()),
                Ok(Bevoegdheid::Mandaat(m)) => {
                    namens = Some(g.clone());
                    mandaat = Some(m.grondslag.clone());
                }
                Err(reden) => {
                    return Err(Weigering::Onbevoegd(format!("{}: {reden}", h.artikel)));
                }
            },
            None => {
                waarschuwingen.push(format!(
                    "regeling '{}' noemt geen bevoegd gezag bij {}; vastgelegd zonder competent_authority",
                    h.regeling, h.artikel
                ));
                namens = eigen.map(str::to_string);
            }
        }
    }
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

    let external = event_velden(event, &h.uitkomsten, formulier, &proef.uitkomsten);
    // Elke parameter die meedeed gaat mee, met zijn herkomst.
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
    // Het rechtskarakter en de regeling horen bij een besluit (een
    // decretogram); invoer en receipt bij elke handeling die de engine
    // uitrekende.
    let decretogram = event.type_ == "decretogram";
    let artikel = regelingen::artikel(service, &h.artikel).map_err(Weigering::Cel)?;
    let produces = artikel
        .get_execution_spec()
        .and_then(|e| e.produces.as_ref())
        .filter(|_| decretogram);
    // De versie van de regeling: haar `valid_from`. Noemt zij die niet, dan
    // de publicatiedatum, met een waarschuwing: de versie is dan een
    // aanname.
    let mut regulation_valid_from = None;
    if decretogram {
        let law = service
            .resolver()
            .get_law(&h.regeling)
            .ok_or_else(|| Weigering::Cel(format!("regeling '{}' is niet geladen", h.regeling)))?;
        regulation_valid_from = Some(match &law.valid_from {
            Some(v) => v.clone(),
            None => {
                waarschuwingen.push(format!(
                    "regeling '{}' noemt geen valid_from; regulation_valid_from is haar publicatiedatum ({})",
                    h.regeling, law.publication_date
                ));
                law.publication_date.clone()
            }
        });
    }
    let verzoek = Vastlegverzoek {
        actor: actor.clone(),
        stroom: stroom.id.clone(),
        event: event.name.clone(),
        intake: Value::Null,
        external,
        zaakkenmerk: Some(zaakkenmerk.to_string()),
        // Een nieuw besluit krijgt zijn kenmerk van de cel; een gram dat een
        // besluit volgt of wijzigt, noemt dat besluit.
        besluitkenmerk: match h.besluitrol {
            Some(Besluit::Volgt | Besluit::Wijzigt) => {
                proef.besluit.as_ref().map(|b| b.besluitkenmerk.clone())
            }
            _ => None,
        },
        besluit: Some(Besluitvelden {
            legal_character: produces.and_then(|p| p.legal_character.clone()),
            decision_type: produces.and_then(|p| p.decision_type.clone()),
            regulation: decretogram.then(|| h.regeling.clone()),
            regulation_valid_from,
            competent_authority: gezag.filter(|_| decretogram),
            handelende_actor: Some(handelende_actor),
            inputs,
            receipt: Some(Receipt::nieuw(om.regelingen.to_vec(), stromen)),
        }),
        zaak_grammen: Some(zaak.grammen),
    };
    let MetYaml { gram, yaml } = celclient::leg_vast(om.cel, &h.vastleggen.cel, &verzoek)
        .await
        .map_err(|f| match f {
            // De vorm (de stage, de zaak, het moment) toetst de cel, onder haar slot.
            TransportFout::Antwoord { status: 409, fout } => Weigering::Conflict(fout),
            TransportFout::Antwoord { status: 400, fout } => Weigering::Ongeldig(fout),
            f => Weigering::Cel(format!("de handeling is niet vastgelegd: {f}")),
        })?;
    Ok(Genomen {
        gram,
        yaml,
        proef,
        waarschuwingen,
    })
}
