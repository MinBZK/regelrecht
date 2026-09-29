//! Een handeling op proef in een zaak: het besluit waarop zij handelt, de
//! peildatum, de lexostatussen en de synthese, en de engine.

use super::*;

/// De besluiten in de zaak van de handeling `besluit`, in de volgorde waarin
/// de cel ze vastlegde, met de besluiten die ze wijzigen (van een handeling
/// met `besluit: wijzigt` die `besluit` noemt). Het laatste is het besluit
/// zoals het nu geldt.
fn keten<'z>(proces: &Proces, decision: &str, case: &'z Zaakstand) -> Vec<&'z Besluitstand> {
    let Some(b) = &proces.definitie.handling else {
        return Vec::new();
    };
    let events: Vec<&HandelingDefinitie> = b
        .actions
        .iter()
        .filter(|h| {
            h.name == decision
                || (h.decision_role == Some(Decision::Amends)
                    && h.decision.as_deref() == Some(decision))
        })
        .collect();
    case.decisions
        .iter()
        .filter(|s| {
            events
                .iter()
                .any(|h| s.van(&h.record.stream, &h.record.event))
        })
        .collect()
}

/// De besluiten in de zaak die de handeling `besluit` zelf vastlegde.
pub(super) fn eigen<'z>(
    proces: &Proces,
    decision: &str,
    case: &'z Zaakstand,
) -> Vec<&'z Besluitstand> {
    let Some(b) = proces
        .definitie
        .handling
        .as_ref()
        .and_then(|b| b.action(decision))
    else {
        return Vec::new();
    };
    case.decisions
        .iter()
        .filter(|s| s.van(&b.record.stream, &b.record.event))
        .collect()
}

/// Het besluit in de zaak waarop een handeling handelt, uit de
/// [`Zaakstand`]: bij een vervolg een besluit van de handeling van het
/// besluit (de stage gaat daarop verder), bij een feit dat een besluit volgt
/// en bij een wijziging een besluit van de handeling die zij noemen, een
/// wijziging ervan meegerekend. Noemt de behandelaar een besluit (een id)
/// (`gekozen`), dan dat besluit, als het er een van is; anders het laatste.
/// `Ok(None)`: de handeling hoort bij geen besluit, of opent er zelf een.
/// `Err`: het besluit ligt er nog niet, of het gekozen besluit is er geen
/// waarop de handeling handelt.
pub fn doel<'z>(
    proces: &Proces,
    h: &HandelingDefinitie,
    case: &'z Zaakstand,
    gekozen: Option<&str>,
) -> Result<Option<&'z Besluitstand>, String> {
    let (list, van) = match (&h.soort, h.decision_role) {
        (Handelingsoort::FollowUp { decision, .. }, _) => (eigen(proces, decision, case), decision),
        (_, Some(Decision::Follows | Decision::Amends)) => {
            let Some(b) = h.decision.as_ref() else {
                return Ok(None);
            };
            (keten(proces, b, case), b)
        }
        _ => {
            return match gekozen {
                Some(k) => Err(format!(
                    "handeling '{}' handelt op geen besluit, en er is besluit {k} genoemd",
                    h.name
                )),
                None => Ok(None),
            }
        }
    };
    if let Some(k) = gekozen {
        return list
            .iter()
            .find(|b| b.id == k)
            .map(|b| Some(*b))
            .ok_or_else(|| {
                let kan: Vec<&str> = list.iter().map(|b| b.id.as_str()).collect();
                format!(
                    "besluit {k} is geen besluit waarop handeling '{}' handelt (wel: {})",
                    h.name,
                    if kan.is_empty() {
                        "geen".to_string()
                    } else {
                        kan.join(", ")
                    }
                )
            });
    }
    match list.last() {
        Some(b) => Ok(Some(b)),
        None => {
            let label = proces
                .definitie
                .handling
                .as_ref()
                .and_then(|b| b.action(van))
                .map_or(van.as_str(), |b| b.label());
            Err(format!("wacht op het besluit ({label})"))
        }
    }
}

fn verwijzing(b: &Besluitstand) -> Option<BesluitVerwijzing> {
    let (stage, gram) = b.genomen()?;
    Some(BesluitVerwijzing {
        id: b.id.clone(),
        name: gram.event.clone(),
        stage: Some(stage.clone()),
        effective_at: gram.effective_at.clone(),
        recorded_at: gram.recorded_at.clone(),
    })
}

/// Een lexostatus van de zaak, gevraagd aan de cel. Heeft de cel er geen
/// gram voor (404), dan levert ze niets: een lege lexostatus. Met een
/// concept reduceert de cel op proef, alsof het concept al vastlag.
async fn zaaklexostatus(
    om: &Omgeving<'_>,
    source: &crate::config::SyntheseBron,
    root: &str,
    peil: &Peil,
    concept: Option<&Vastlegverzoek>,
) -> Result<(Lexostatus, Option<Gram>), Weigering> {
    let def = om
        .proces
        .cell
        .lexostatuses
        .lexostatus(&source.lexostatus)
        .ok_or_else(|| {
            Weigering::Cell(format!("lexostatus '{}' bestaat niet", source.lexostatus))
        })?;
    let mut inputs = Map::new();
    inputs.insert("root".into(), Value::String(root.to_string()));
    let antwoord = match concept {
        None => om
            .cell
            .haal(&synthese::path(
                &source.cell,
                &source.lexostatus,
                &inputs,
                peil,
            ))
            .await
            .and_then(|v| {
                serde_json::from_value::<Lexostatus>(v)
                    .map(|l| (l, None))
                    .map_err(|e| TransportFout::Json(e.to_string()))
            }),
        Some(c) => {
            for (k, v) in peil.query() {
                inputs.insert(k.into(), Value::String(v));
            }
            celclient::trial(om.cell, &source.cell, &source.lexostatus, c, &inputs)
                .await
                .map(|p| (p.lexostatus, Some(p.gram)))
        }
    };
    match antwoord {
        Ok(l) => Ok(l),
        // Kiest de definitie een gram en is er geen, dan levert zij niets.
        Err(TransportFout::Antwoord { status: 404, .. }) => Ok((
            Lexostatus {
                not_derived: def.reduction.derivations.keys().cloned().collect(),
                ..Lexostatus::leeg(&def.name)
            },
            None,
        )),
        // Het concept past niet in een gram: een fout in het formulier.
        Err(TransportFout::Antwoord { status: 400, error }) => Err(Weigering::Ongeldig(error)),
        Err(TransportFout::Antwoord { status: 409, error }) => Err(Weigering::Conflict(error)),
        Err(f) => Err(Weigering::Cell(format!(
            "cel '{}', lexostatus '{}': {f}",
            source.cell, source.lexostatus
        ))),
    }
}

/// De peildatum van een handeling: de dag van het `op_moment` dat het event
/// aan een veld van het formulier bindt (de dag van het besluit, de
/// bekendmaking, de betaling), met de grondslag die de stroom daarvoor
/// geeft; anders vandaag. Een besluit leest de wet en de cellen zo op de dag
/// waarop het genomen wordt, ook als de behandelaar het later vastlegt.
/// Het derde deel is een bezwaar tegen dat moment: het ligt na vandaag (wat
/// nog moet gebeuren, is geen feit), of op een dag voor het laatste feit van
/// de zaak (een zaak loopt vooruit in de tijd). De cel weigert zo'n gram ook;
/// het proces zegt het vooraf.
fn reference_date(
    event: &Event,
    form: &Map<String, Value>,
    nu: &DateTime<FixedOffset>,
    case: &Zaakstand,
) -> Result<(String, String, Option<String>), Weigering> {
    let gebonden = crate::stroom::gebonden_moment(event, None, form, *nu.offset())
        .map_err(Weigering::Ongeldig)?;
    let Some((moment, b)) = gebonden else {
        return Ok((datum::reference_date(nu), "vandaag".to_string(), None));
    };
    let path = b.source.strip_prefix("$external.").unwrap_or(&b.source);
    let dag = datum::reference_date(&moment);
    let laatste = case
        .latest_effective_at
        .as_deref()
        .map(datum::peildatum_van)
        .transpose()
        .map_err(Weigering::Cell)?;
    let bezwaar = if moment > *nu {
        Some(format!(
            "{path} {dag} ligt na vandaag: wat nog moet gebeuren, is geen feit"
        ))
    } else {
        laatste.filter(|l| dag < *l).map(|l| {
            format!(
                "{path} {dag} ligt voor de zaak: het laatste feit erin geldt op {l}; een zaak loopt vooruit in de tijd"
            )
        })
    };
    Ok((
        dag,
        format!("{path} (op_moment, {})", b.legal_basis.join(", ")),
        bezwaar,
    ))
}

/// Waarom een handeling niet uit zichzelf genomen wordt.
enum Bezwaar {
    /// De vorm: het formulier, de volgorde van de zaak, het moment. Dan
    /// wordt er ook niets gemeld.
    Vorm(String),
    /// De inhoud: de wet zegt nee, of kan niet zeggen wat het feit doet.
    /// Een gebeurd feit legt de cel dan toch vast.
    Inhoud(String),
}

/// Reken een handeling in een zaak uit, zonder iets vast te leggen. `zaak`
/// is de stand van de zaak zoals de cel haar afleidt.
pub async fn trial(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    root: &str,
    case: &Zaakstand,
    opgave: &Opgave,
) -> Result<Proefhandeling, Weigering> {
    let proces = om.proces;
    let form = &opgave.form;
    for name in form.keys() {
        if !h.verdicts.iter().any(|o| &o.parameter == name)
            && !h.feiten.iter().any(|f| &f.name == name)
        {
            return Err(Weigering::Ongeldig(format!(
                "'{name}' is geen veld van het formulier van handeling '{}'",
                h.name
            )));
        }
    }
    let (_, event) = proces
        .cell
        .event(&h.record.stream, &h.record.event)
        .ok_or_else(|| Weigering::Cell(format!("handeling '{}': geen vastleg-event", h.name)))?;
    let (reference_date, reference_date_from, tijd) = reference_date(event, form, &om.nu, case)?;
    let mut p = Proefhandeling {
        action: h.name.clone(),
        soort: h.soort.clone(),
        stage: h.stage.clone(),
        regulation: h.regulation.clone(),
        article: h.article.clone(),
        reference_date: reference_date.clone(),
        reference_date_from,
        takeable: false,
        reportable: false,
        outputs: BTreeMap::new(),
        assessments: BTreeMap::new(),
        types: h.types.clone(),
        missing: Vec::new(),
        reason: None,
        parameters: BTreeMap::new(),
        provenance: BTreeMap::new(),
        sources: Vec::new(),
        not_delivered: Vec::new(),
        lexostatuses: Vec::new(),
        rows: Vec::new(),
        decision: None,
        trace_text: None,
    };
    let ontbrekend: Vec<String> = h
        .feiten
        .iter()
        .filter(|f| form.get(&f.name).is_none_or(Value::is_null))
        .map(|f| f.name.clone())
        .collect();
    // Het besluit waarop de handeling handelt. Ligt het er nog niet, dan is
    // er niets uit te rekenen: dat is de vorm (de volgorde van de zaak).
    let doel = match doel(proces, h, case, opgave.decision.as_deref()) {
        Ok(b) => b,
        Err(r) => {
            p.reason = Some(format!("niet te nemen: {r}"));
            return Ok(p);
        }
    };
    p.decision = doel.and_then(verwijzing);
    if let Some(r) = al_genomen(proces, h, case) {
        p.reason = Some(format!("niet te nemen: {r}"));
        return Ok(p);
    }
    let output = match (&h.soort, doel) {
        (Handelingsoort::FollowUp { decision, .. }, Some(b)) => {
            vervolg(om, h, decision, b, form, &mut p)?
        }
        (Handelingsoort::FollowUp { .. }, None) => {
            return Err(Weigering::Cell(format!(
                "handeling '{}': geen besluit",
                h.name
            )))
        }
        // Een onvolledige uitkomst (een waarde mist, een bron antwoordde niet)
        // is geen conclusie over de inhoud: dan ligt er niets vast, ook niet
        // gemeld, want de invoer en het receipt zouden niet kloppen.
        _ => op_de_zaak(om, h, event, root, form, &mut p)
            .await?
            .map(Bezwaar::Vorm),
    };
    // Een wijziging waarvan de wet een uitkomst leeg laat, neemt het proces
    // niet: de wet wijzigt dan niets (er is geen grond voor een wijziging).
    // Net als een haak die geen waarde geeft bij een vervolg. Een besluit dat
    // niets wijzigt, kan een leeg hulpgegeven (een termijn) wel hebben.
    let leeg: Vec<&str> = h
        .outputs
        .iter()
        .filter(|u| p.outputs.get(*u) == Some(&Value::Null))
        .map(String::as_str)
        .collect();
    let output = match output {
        None if h.decision_role == Some(Decision::Amends) && !leeg.is_empty() => {
            Some(Bezwaar::Inhoud(format!(
                "niet te nemen: {} geeft geen waarde voor {}",
                h.article,
                leeg.join(", ")
            )))
        }
        u => u,
    };
    let onwaar: Vec<&String> = p
        .assessments
        .iter()
        .filter(|(_, w)| **w == Value::Bool(false))
        .map(|(n, _)| n)
        .collect();
    let assessment = (!onwaar.is_empty()).then(|| {
        format!(
            "niet te nemen: {} zegt nee ({})",
            h.article,
            onwaar
                .iter()
                .map(|s| s.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )
    });
    let vorm = tijd
        .map(|t| format!("niet te nemen: {t}"))
        .or((!ontbrekend.is_empty())
            .then(|| format!("niet te nemen: vul in: {}", ontbrekend.join(", "))))
        .or(match &output {
            Some(Bezwaar::Vorm(r)) => Some(r.clone()),
            _ => None,
        });
    let content = match output {
        Some(Bezwaar::Inhoud(r)) => Some(r),
        _ => None,
    }
    .or(assessment);
    p.takeable = vorm.is_none() && content.is_none();
    p.reportable = vorm.is_none() && content.is_some() && h.soort != Handelingsoort::Decision;
    p.reason = vorm.or(content);
    Ok(p)
}

/// Een besluit of een feit: de lexostatussen van de zaak (bij een feit met
/// het concept erbij), de synthese, de synthese per regel, het formulier en
/// wat nog niet gebeurd is, en dan de engine. Geeft de reden terug als de
/// uitkomsten niet volledig zijn.
async fn op_de_zaak(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    event: &Event,
    root: &str,
    form: &Map<String, Value>,
    p: &mut Proefhandeling,
) -> Result<Option<String>, Weigering> {
    let proces = om.proces;
    let service = proces.service.as_ref();
    let peil = Peil::op(Tijdpunt::lees("peildatum", &p.reference_date).map_err(Weigering::Cell)?);

    // 1. De lexostatussen van de zaak. Een feit telt op proef mee: de cel
    // reduceert alsof het concept al vastlag.
    let concept = (!h.feiten.is_empty()).then(|| Vastlegverzoek {
        actor: proces.definitie.actor.clone(),
        stream: h.record.stream.clone(),
        event: h.record.event.clone(),
        intake: Value::Null,
        external: event_velden(event, &h.outputs, form, &BTreeMap::new()),
        refers_to: super::verwijzingen(
            &proces.cell,
            event,
            root,
            p.decision.as_ref().map(|b| b.id.as_str()),
        ),
        decision: None,
        root_grams: None,
    });
    let mut eigen = Vec::new();
    // Het gram van het concept, zoals de cel het bouwde: een register dat
    // het beleid bevraagt, telt het op proef mee (zie [`crate::register`]).
    let mut proefgram: Option<Gram> = None;
    for source in proces.definitie.zaakbronnen() {
        let (l, g) = zaaklexostatus(om, source, root, &peil, concept.as_ref()).await?;
        proefgram = proefgram.or(g);
        eigen.push(l);
    }

    // 2. Synthese, met de invoer uit de lexostatus van de zaak die haar
    // levert (de controle bij het laden zegt: hooguit een). Vraagt geen bron
    // een invoer uit de zaak, dan begint de synthese leeg; de parameters van
    // de zaak komen er daarna bij.
    let hoofd = om
        .sources
        .iter()
        .flat_map(|s| s.definitie.input.values().filter_map(|v| v.field()))
        .find_map(|v| eigen.iter().position(|l| l.name == v.lexostatus));
    let mut combined = match hoofd.and_then(|i| eigen.get(i)) {
        Some(l) => synthese::voeg_samen(l, om.sources, &peil).await,
        None => synthese::voeg_samen(&Lexostatus::leeg(""), om.sources, &peil).await,
    };
    for (i, l) in eigen.iter().enumerate() {
        if Some(i) == hoofd {
            continue;
        }
        for (name, w) in &l.parameters {
            combined.parameters.insert(name.clone(), w.clone());
            combined.provenance.insert(
                name.clone(),
                Herkomst::Own {
                    lexostatus: l.name.clone(),
                },
            );
        }
    }

    // 3. Synthese per regel.
    let law = rijen::Omgeving {
        service,
        date: &p.reference_date,
        peil: &peil,
    };
    p.rows = rijen::pas_toe(om.rows, &eigen, &mut combined, law).await;

    // 4. De oordelen van de behandelaar; een leeg veld gaat niet mee.
    for o in &h.verdicts {
        if let Some(w) = form.get(&o.parameter).filter(|w| !w.is_null()) {
            combined.parameters.insert(o.parameter.clone(), w.clone());
            combined
                .provenance
                .insert(o.parameter.clone(), Herkomst::Handler);
        }
    }

    // 4b. Het besluit waarop de handeling handelt, als het artikel het als
    // parameter vraagt (het eigen beleid dat per besluit leest).
    if let (Some(name), Some(b)) = (&h.decision_parameter, &p.decision) {
        combined
            .parameters
            .insert(name.clone(), Value::String(b.id.clone()));
        combined.provenance.insert(name.clone(), Herkomst::Decision);
    }

    // 5. Wat een latere stage pas vraagt, is nog niet gebeurd.
    for (name, n) in &h.not_yet {
        combined.parameters.insert(name.clone(), n.value.clone());
        combined.provenance.insert(
            name.clone(),
            Herkomst::StateAtDecision {
                stage: n.stage.clone(),
            },
        );
    }

    // 6. De engine: de uitkomsten en de toetsen, in een run.
    let gevraagd: Vec<&str> = h
        .outputs
        .iter()
        .chain(h.assessments.iter())
        .map(String::as_str)
        .collect();
    let e = crate::register::met_proef(proefgram.into_iter().collect(), || {
        toets::evalueer_met_trace(
            service,
            &h.regulation,
            &gevraagd,
            &combined.parameters,
            &p.reference_date,
        )
    });
    let volledig = e.volledig(&gevraagd);
    let mut reason = (!volledig).then(|| e.reason("niet te nemen"));
    if !volledig {
        if let Some(r) = combined.reason() {
            reason = Some(r.replacen("niet te beoordelen", "niet te nemen", 1));
        }
    }
    for (name, w) in e.waarden {
        if h.assessments.contains(&name) {
            p.assessments.insert(name, w);
        } else {
            p.outputs.insert(name, w);
        }
    }
    p.trace_text = e.trace_text;
    p.missing = e.missing;
    p.not_delivered = benodigd(service, h)
        .map_err(Weigering::Cell)?
        .into_values()
        .filter(|b| !combined.parameters.contains_key(&b.name))
        .collect();
    p.parameters = combined.parameters;
    p.provenance = combined.provenance;
    p.sources = combined.sources;
    p.lexostatuses = eigen;
    Ok(reason)
}

/// Een vervolg: de engine voert de stage van de handeling uit op de invoer
/// en de uitkomsten van het vastgelegde besluit (RFC-008: het besluit is de
/// toestand, de orkestratie bewaart haar en levert wat de stage vraagt). Die
/// toestand leidt de cel af (de stage van het besluit in de [`Zaakstand`]);
/// het proces leest geen gram. Wat de stage vraagt, komt uit het formulier.
/// De haken van die stage vuren (RFC-007), zoals de aanvang en het einde van
/// de bezwaartermijn.
fn vervolg(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    decision: &str,
    stand: &Besluitstand,
    form: &Map<String, Value>,
    p: &mut Proefhandeling,
) -> Result<Option<Bezwaar>, Weigering> {
    let proces = om.proces;
    let service = proces.service.as_ref();
    let b = proces
        .definitie
        .handling
        .as_ref()
        .and_then(|b| b.action(decision))
        .ok_or_else(|| Weigering::Cell(format!("geen handeling '{decision}'")))?;
    let Some((_, gram)) = stand.genomen() else {
        return Err(Weigering::Cell(format!(
            "besluit {} heeft geen stage van het besluit",
            stand.id
        )));
    };
    if let Some(s) = h.stage.as_ref().filter(|s| stand.stages.contains_key(*s)) {
        return Ok(Some(Bezwaar::Vorm(format!(
            "niet te nemen: stage {s} ligt al in besluit {}",
            stand.id
        ))));
    }
    let (Handelingsoort::FollowUp { procedure, .. }, Some(stage)) = (&h.soort, h.stage.clone())
    else {
        return Err(Weigering::Cell(format!(
            "handeling '{}' is geen vervolg met een stage",
            h.name
        )));
    };
    let state = StageState {
        procedure_id: procedure.clone(),
        contextual_law: gram
            .regulation
            .clone()
            .unwrap_or_else(|| h.regulation.clone()),
        current_stage: stage.clone(),
        accumulated_outputs: gram
            .fields
            .iter()
            .map(|(k, v)| (k.clone(), EngineValue::from(v)))
            .collect(),
        parameters: gram
            .input
            .iter()
            .map(|(k, w)| (k.clone(), EngineValue::from(w)))
            .collect(),
    };
    let mut input = BTreeMap::new();
    for f in &h.feiten {
        if let Some(w) = form.get(&f.name).filter(|w| !w.is_null()) {
            input.insert(f.name.clone(), EngineValue::from(w));
            p.parameters.insert(f.name.clone(), w.clone());
            p.provenance.insert(f.name.clone(), Herkomst::Handler);
        }
    }
    let output = b
        .outputs
        .first()
        .ok_or_else(|| Weigering::Cell(format!("handeling '{decision}' noemt geen uitkomst")))?;
    let outputs =
        match service.execute_stage(&h.regulation, output, Some(state), input, &p.reference_date) {
            Ok(ExecutionOutcome::Complete(r)) => r.outputs,
            Ok(ExecutionOutcome::Yielded {
                state,
                outputs,
                pending_inputs,
            }) => {
                if state.current_stage == stage {
                    p.missing = pending_inputs;
                    return Ok(Some(Bezwaar::Vorm(format!(
                        "niet te nemen: stage {stage} vraagt {}",
                        p.missing.join(", ")
                    ))));
                }
                outputs
            }
            Err(e) => return Ok(Some(Bezwaar::Vorm(format!("niet te nemen: {e}")))),
        };
    for u in &h.outputs {
        match outputs.get(u) {
            Some(w) if w.contains_unknown() => {
                for f in w.missing_facts() {
                    if !p.missing.contains(&f.name) {
                        p.missing.push(f.name.clone());
                    }
                }
            }
            Some(w) => {
                if let Ok(v) = serde_json::to_value(w) {
                    p.outputs.insert(u.clone(), v);
                }
            }
            None => {}
        }
    }
    let leeg: Vec<&str> = h
        .outputs
        .iter()
        .filter(|u| p.outputs.get(*u).is_none_or(Value::is_null))
        .map(String::as_str)
        .collect();
    if !p.missing.is_empty() {
        return Ok(Some(Bezwaar::Vorm(format!(
            "niet te nemen: mist {}",
            p.missing.join(", ")
        ))));
    }
    if !leeg.is_empty() {
        // Een haak die geen waarde geeft, zegt dat de stage niet op de
        // voorgeschreven wijze plaatsvond (zoals een bekendmaking die niet
        // aan Awb 3:41 voldoet: de bezwaartermijn vangt dan niet aan). Een
        // conclusie over de inhoud: gebeurde het toch, dan ligt het vast, met
        // een lege termijn.
        return Ok(Some(Bezwaar::Inhoud(format!(
            "niet te nemen: geen waarde voor {} ({})",
            leeg.join(", "),
            h.hooks.join(", ")
        ))));
    }
    Ok(None)
}

/// De velden van het gram: per `$external`-sleutel van het event een
/// uitkomst, of de waarde uit het formulier.
pub(super) fn event_velden(
    event: &Event,
    uitkomsten_namen: &[String],
    form: &Map<String, Value>,
    outputs: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    event
        .external_sleutels()
        .into_iter()
        .map(|k| {
            let w = if uitkomsten_namen.contains(&k) {
                outputs.get(&k).cloned().unwrap_or(Value::Null)
            } else {
                form.get(&k).cloned().unwrap_or(Value::Null)
            };
            (k, w)
        })
        .collect()
}
