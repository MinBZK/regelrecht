//! Een handeling op proef in een zaak: het besluit waarop zij handelt, de
//! peildatum, de lexostatussen en de synthese, en de engine.

use super::*;

/// De besluiten in de zaak van de handeling `besluit`, in de volgorde waarin
/// de cel ze vastlegde, met de besluiten die ze wijzigen (van een handeling
/// met `besluit: wijzigt` die `besluit` noemt). Het laatste is het besluit
/// zoals het nu geldt.
fn keten<'z>(proces: &Proces, besluit: &str, zaak: &'z Zaakstand) -> Vec<&'z Besluitstand> {
    let Some(b) = &proces.definitie.behandeling else {
        return Vec::new();
    };
    let events: Vec<&HandelingDefinitie> = b
        .handelingen
        .iter()
        .filter(|h| {
            h.naam == besluit
                || (h.besluitrol == Some(Besluit::Wijzigt) && h.besluit.as_deref() == Some(besluit))
        })
        .collect();
    zaak.besluiten
        .iter()
        .filter(|s| {
            events
                .iter()
                .any(|h| s.van(&h.vastleggen.stroom, &h.vastleggen.event))
        })
        .collect()
}

/// De besluiten in de zaak die de handeling `besluit` zelf vastlegde.
pub(super) fn eigen<'z>(
    proces: &Proces,
    besluit: &str,
    zaak: &'z Zaakstand,
) -> Vec<&'z Besluitstand> {
    let Some(b) = proces
        .definitie
        .behandeling
        .as_ref()
        .and_then(|b| b.handeling(besluit))
    else {
        return Vec::new();
    };
    zaak.besluiten
        .iter()
        .filter(|s| s.van(&b.vastleggen.stroom, &b.vastleggen.event))
        .collect()
}

/// Het besluit in de zaak waarop een handeling handelt, uit de
/// [`Zaakstand`]: bij een vervolg een besluit van de handeling van het
/// besluit (de stage gaat daarop verder), bij een feit dat een besluit volgt
/// en bij een wijziging een besluit van de handeling die zij noemen, een
/// wijziging ervan meegerekend. Noemt de behandelaar een besluitkenmerk
/// (`gekozen`), dan dat besluit, als het er een van is; anders het laatste.
/// `Ok(None)`: de handeling hoort bij geen besluit, of opent er zelf een.
/// `Err`: het besluit ligt er nog niet, of het gekozen besluit is er geen
/// waarop de handeling handelt.
pub fn doel<'z>(
    proces: &Proces,
    h: &HandelingDefinitie,
    zaak: &'z Zaakstand,
    gekozen: Option<&str>,
) -> Result<Option<&'z Besluitstand>, String> {
    let (lijst, van) = match (&h.soort, h.besluitrol) {
        (Handelingsoort::Vervolg { besluit, .. }, _) => (eigen(proces, besluit, zaak), besluit),
        (_, Some(Besluit::Volgt | Besluit::Wijzigt)) => {
            let Some(b) = h.besluit.as_ref() else {
                return Ok(None);
            };
            (keten(proces, b, zaak), b)
        }
        _ => {
            return match gekozen {
                Some(k) => Err(format!(
                    "handeling '{}' handelt op geen besluit, en er is besluit {k} genoemd",
                    h.naam
                )),
                None => Ok(None),
            }
        }
    };
    if let Some(k) = gekozen {
        return lijst
            .iter()
            .find(|b| b.besluitkenmerk == k)
            .map(|b| Some(*b))
            .ok_or_else(|| {
                let kan: Vec<&str> = lijst.iter().map(|b| b.besluitkenmerk.as_str()).collect();
                format!(
                    "besluit {k} is geen besluit waarop handeling '{}' handelt (wel: {})",
                    h.naam,
                    if kan.is_empty() {
                        "geen".to_string()
                    } else {
                        kan.join(", ")
                    }
                )
            });
    }
    match lijst.last() {
        Some(b) => Ok(Some(b)),
        None => {
            let label = proces
                .definitie
                .behandeling
                .as_ref()
                .and_then(|b| b.handeling(van))
                .map_or(van.as_str(), |b| b.label());
            Err(format!("wacht op het besluit ({label})"))
        }
    }
}

fn verwijzing(b: &Besluitstand) -> Option<BesluitVerwijzing> {
    let (stage, gram) = b.genomen()?;
    Some(BesluitVerwijzing {
        besluitkenmerk: b.besluitkenmerk.clone(),
        name: gram.event.clone(),
        stage: Some(stage.clone()),
        op_moment: gram.op_moment.clone(),
        vastgelegd_op: gram.vastgelegd_op.clone(),
    })
}

/// Een lexostatus van de zaak, gevraagd aan de cel. Heeft de cel er geen
/// gram voor (404), dan levert ze niets: een lege lexostatus. Met een
/// concept reduceert de cel op proef, alsof het concept al vastlag.
async fn zaaklexostatus(
    om: &Omgeving<'_>,
    bron: &crate::config::SyntheseBron,
    zaakkenmerk: &str,
    peil: &Peil,
    concept: Option<&Vastlegverzoek>,
) -> Result<Lexostatus, Weigering> {
    let def = om
        .proces
        .cel
        .lexostatussen
        .lexostatus(&bron.lexostatus)
        .ok_or_else(|| Weigering::Cel(format!("lexostatus '{}' bestaat niet", bron.lexostatus)))?;
    let mut inputs = Map::new();
    inputs.insert("zaakkenmerk".into(), Value::String(zaakkenmerk.to_string()));
    let antwoord = match concept {
        None => om
            .cel
            .haal(&synthese::pad(&bron.cel, &bron.lexostatus, &inputs, peil))
            .await
            .and_then(|v| {
                serde_json::from_value::<Lexostatus>(v)
                    .map_err(|e| TransportFout::Json(e.to_string()))
            }),
        Some(c) => {
            for (k, v) in peil.query() {
                inputs.insert(k.into(), Value::String(v));
            }
            celclient::proef(om.cel, &bron.cel, &bron.lexostatus, c, &inputs)
                .await
                .map(|p| p.lexostatus)
        }
    };
    match antwoord {
        Ok(l) => Ok(l),
        // Kiest de definitie een gram en is er geen, dan levert zij niets.
        Err(TransportFout::Antwoord { status: 404, .. }) => Ok(Lexostatus {
            niet_afgeleid: def.reduction.afleidingen.keys().cloned().collect(),
            ..Lexostatus::leeg(&def.name)
        }),
        // Het concept past niet in een gram: een fout in het formulier.
        Err(TransportFout::Antwoord { status: 400, fout }) => Err(Weigering::Ongeldig(fout)),
        Err(TransportFout::Antwoord { status: 409, fout }) => Err(Weigering::Conflict(fout)),
        Err(f) => Err(Weigering::Cel(format!(
            "cel '{}', lexostatus '{}': {f}",
            bron.cel, bron.lexostatus
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
fn peildatum(
    event: &Event,
    formulier: &Map<String, Value>,
    nu: &DateTime<FixedOffset>,
    zaak: &Zaakstand,
) -> Result<(String, String, Option<String>), Weigering> {
    let gebonden = crate::stroom::gebonden_moment(event, None, formulier, *nu.offset())
        .map_err(Weigering::Ongeldig)?;
    let Some((moment, b)) = gebonden else {
        return Ok((datum::peildatum(nu), "vandaag".to_string(), None));
    };
    let pad = b.bron.strip_prefix("$external.").unwrap_or(&b.bron);
    let dag = datum::peildatum(&moment);
    let laatste = zaak
        .laatste_op_moment
        .as_deref()
        .map(datum::peildatum_van)
        .transpose()
        .map_err(Weigering::Cel)?;
    let bezwaar = if moment > *nu {
        Some(format!(
            "{pad} {dag} ligt na vandaag: wat nog moet gebeuren, is geen feit"
        ))
    } else {
        laatste.filter(|l| dag < *l).map(|l| {
            format!(
                "{pad} {dag} ligt voor de zaak: het laatste feit erin geldt op {l}; een zaak loopt vooruit in de tijd"
            )
        })
    };
    Ok((
        dag,
        format!("{pad} (op_moment, {})", b.grondslag.join(", ")),
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
pub async fn proef(
    om: &Omgeving<'_>,
    h: &HandelingDefinitie,
    zaakkenmerk: &str,
    zaak: &Zaakstand,
    opgave: &Opgave,
) -> Result<Proefhandeling, Weigering> {
    let proces = om.proces;
    let formulier = &opgave.formulier;
    for naam in formulier.keys() {
        if !h.oordelen.iter().any(|o| &o.parameter == naam)
            && !h.feiten.iter().any(|f| &f.naam == naam)
        {
            return Err(Weigering::Ongeldig(format!(
                "'{naam}' is geen veld van het formulier van handeling '{}'",
                h.naam
            )));
        }
    }
    let (_, event) = proces
        .cel
        .event(&h.vastleggen.stroom, &h.vastleggen.event)
        .ok_or_else(|| Weigering::Cel(format!("handeling '{}': geen vastleg-event", h.naam)))?;
    let (peildatum, peildatum_uit, tijd) = peildatum(event, formulier, &om.nu, zaak)?;
    let mut p = Proefhandeling {
        handeling: h.naam.clone(),
        soort: h.soort.clone(),
        stage: h.stage.clone(),
        regeling: h.regeling.clone(),
        artikel: h.artikel.clone(),
        peildatum: peildatum.clone(),
        peildatum_uit,
        te_nemen: false,
        te_melden: false,
        uitkomsten: BTreeMap::new(),
        toetsen: BTreeMap::new(),
        typen: h.typen.clone(),
        mist: Vec::new(),
        reden: None,
        parameters: BTreeMap::new(),
        herkomst: BTreeMap::new(),
        bronnen: Vec::new(),
        niet_geleverd: Vec::new(),
        lexostatussen: Vec::new(),
        rijen: Vec::new(),
        besluit: None,
        trace_text: None,
    };
    let ontbrekend: Vec<String> = h
        .feiten
        .iter()
        .filter(|f| formulier.get(&f.naam).is_none_or(Value::is_null))
        .map(|f| f.naam.clone())
        .collect();
    // Het besluit waarop de handeling handelt. Ligt het er nog niet, dan is
    // er niets uit te rekenen: dat is de vorm (de volgorde van de zaak).
    let doel = match doel(proces, h, zaak, opgave.besluitkenmerk.as_deref()) {
        Ok(b) => b,
        Err(r) => {
            p.reden = Some(format!("niet te nemen: {r}"));
            return Ok(p);
        }
    };
    p.besluit = doel.and_then(verwijzing);
    if let Some(r) = al_genomen(proces, h, zaak) {
        p.reden = Some(format!("niet te nemen: {r}"));
        return Ok(p);
    }
    let uitkomst = match (&h.soort, doel) {
        (Handelingsoort::Vervolg { besluit, .. }, Some(b)) => {
            vervolg(om, h, besluit, b, formulier, &mut p)?
        }
        (Handelingsoort::Vervolg { .. }, None) => {
            return Err(Weigering::Cel(format!(
                "handeling '{}': geen besluit",
                h.naam
            )))
        }
        // Een onvolledige uitkomst (een waarde mist, een bron antwoordde niet)
        // is geen conclusie over de inhoud: dan ligt er niets vast, ook niet
        // gemeld, want de invoer en het receipt zouden niet kloppen.
        _ => op_de_zaak(om, h, event, zaakkenmerk, formulier, &mut p)
            .await?
            .map(Bezwaar::Vorm),
    };
    // Een wijziging waarvan de wet een uitkomst leeg laat, neemt het proces
    // niet: de wet wijzigt dan niets (er is geen grond voor een wijziging).
    // Net als een haak die geen waarde geeft bij een vervolg. Een besluit dat
    // niets wijzigt, kan een leeg hulpgegeven (een termijn) wel hebben.
    let leeg: Vec<&str> = h
        .uitkomsten
        .iter()
        .filter(|u| p.uitkomsten.get(*u) == Some(&Value::Null))
        .map(String::as_str)
        .collect();
    let uitkomst = match uitkomst {
        None if h.besluitrol == Some(Besluit::Wijzigt) && !leeg.is_empty() => {
            Some(Bezwaar::Inhoud(format!(
                "niet te nemen: {} geeft geen waarde voor {}",
                h.artikel,
                leeg.join(", ")
            )))
        }
        u => u,
    };
    let onwaar: Vec<&String> = p
        .toetsen
        .iter()
        .filter(|(_, w)| **w == Value::Bool(false))
        .map(|(n, _)| n)
        .collect();
    let toets = (!onwaar.is_empty()).then(|| {
        format!(
            "niet te nemen: {} zegt nee ({})",
            h.artikel,
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
        .or(match &uitkomst {
            Some(Bezwaar::Vorm(r)) => Some(r.clone()),
            _ => None,
        });
    let inhoud = match uitkomst {
        Some(Bezwaar::Inhoud(r)) => Some(r),
        _ => None,
    }
    .or(toets);
    p.te_nemen = vorm.is_none() && inhoud.is_none();
    p.te_melden = vorm.is_none() && inhoud.is_some() && h.soort != Handelingsoort::Besluit;
    p.reden = vorm.or(inhoud);
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
    zaakkenmerk: &str,
    formulier: &Map<String, Value>,
    p: &mut Proefhandeling,
) -> Result<Option<String>, Weigering> {
    let proces = om.proces;
    let service = proces.service.as_ref();
    let peil = Peil::op(Tijdpunt::lees("peildatum", &p.peildatum).map_err(Weigering::Cel)?);

    // 1. De lexostatussen van de zaak. Een feit telt op proef mee: de cel
    // reduceert alsof het concept al vastlag.
    let concept = (!h.feiten.is_empty()).then(|| Vastlegverzoek {
        actor: proces.definitie.actor.clone(),
        stroom: h.vastleggen.stroom.clone(),
        event: h.vastleggen.event.clone(),
        intake: Value::Null,
        external: event_velden(event, &h.uitkomsten, formulier, &BTreeMap::new()),
        zaakkenmerk: Some(zaakkenmerk.to_string()),
        besluitkenmerk: p.besluit.as_ref().map(|b| b.besluitkenmerk.clone()),
        besluit: None,
        zaak_grammen: None,
    });
    let mut eigen = Vec::new();
    for bron in proces.definitie.zaakbronnen() {
        eigen.push(zaaklexostatus(om, bron, zaakkenmerk, &peil, concept.as_ref()).await?);
    }

    // 2. Synthese, met de invoer uit de lexostatus van de zaak die haar
    // levert (de controle bij het laden zegt: hooguit een). Vraagt geen bron
    // een invoer uit de zaak, dan begint de synthese leeg; de parameters van
    // de zaak komen er daarna bij.
    let hoofd = om
        .bronnen
        .iter()
        .flat_map(|s| s.definitie.invoer.values().filter_map(|v| v.veld()))
        .find_map(|v| eigen.iter().position(|l| l.naam == v.lexostatus));
    let mut samen = match hoofd.and_then(|i| eigen.get(i)) {
        Some(l) => synthese::voeg_samen(l, om.bronnen, &peil).await,
        None => synthese::voeg_samen(&Lexostatus::leeg(""), om.bronnen, &peil).await,
    };
    for (i, l) in eigen.iter().enumerate() {
        if Some(i) == hoofd {
            continue;
        }
        for (naam, w) in &l.parameters {
            samen.parameters.insert(naam.clone(), w.clone());
            samen.herkomst.insert(
                naam.clone(),
                Herkomst::Eigen {
                    lexostatus: l.naam.clone(),
                },
            );
        }
    }

    // 3. Synthese per regel.
    let wet = rijen::Omgeving {
        service,
        datum: &p.peildatum,
        peil: &peil,
    };
    p.rijen = rijen::pas_toe(om.rijen, &eigen, &mut samen, wet).await;

    // 4. De oordelen van de behandelaar; een leeg veld gaat niet mee.
    for o in &h.oordelen {
        if let Some(w) = formulier.get(&o.parameter).filter(|w| !w.is_null()) {
            samen.parameters.insert(o.parameter.clone(), w.clone());
            samen
                .herkomst
                .insert(o.parameter.clone(), Herkomst::Behandelaar);
        }
    }

    // 5. Wat een latere stage pas vraagt, is nog niet gebeurd.
    for (naam, n) in &h.nog_niet {
        samen.parameters.insert(naam.clone(), n.waarde.clone());
        samen.herkomst.insert(
            naam.clone(),
            Herkomst::StandBijBesluit {
                stage: n.stage.clone(),
            },
        );
    }

    // 6. De engine: de uitkomsten en de toetsen, in een run.
    let gevraagd: Vec<&str> = h
        .uitkomsten
        .iter()
        .chain(h.toetsen.iter())
        .map(String::as_str)
        .collect();
    let e = toets::evalueer_met_trace(
        service,
        &h.regeling,
        &gevraagd,
        &samen.parameters,
        &p.peildatum,
    );
    let volledig = e.volledig(&gevraagd);
    let mut reden = (!volledig).then(|| e.reden("niet te nemen"));
    if !volledig {
        if let Some(r) = samen.reden() {
            reden = Some(r.replacen("niet te beoordelen", "niet te nemen", 1));
        }
    }
    for (naam, w) in e.waarden {
        if h.toetsen.contains(&naam) {
            p.toetsen.insert(naam, w);
        } else {
            p.uitkomsten.insert(naam, w);
        }
    }
    p.trace_text = e.trace_text;
    p.mist = e.mist;
    p.niet_geleverd = benodigd(service, h)
        .map_err(Weigering::Cel)?
        .into_values()
        .filter(|b| !samen.parameters.contains_key(&b.naam))
        .collect();
    p.parameters = samen.parameters;
    p.herkomst = samen.herkomst;
    p.bronnen = samen.bronnen;
    p.lexostatussen = eigen;
    Ok(reden)
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
    besluit: &str,
    stand: &Besluitstand,
    formulier: &Map<String, Value>,
    p: &mut Proefhandeling,
) -> Result<Option<Bezwaar>, Weigering> {
    let proces = om.proces;
    let service = proces.service.as_ref();
    let b = proces
        .definitie
        .behandeling
        .as_ref()
        .and_then(|b| b.handeling(besluit))
        .ok_or_else(|| Weigering::Cel(format!("geen handeling '{besluit}'")))?;
    let Some((_, gram)) = stand.genomen() else {
        return Err(Weigering::Cel(format!(
            "besluit {} heeft geen stage van het besluit",
            stand.besluitkenmerk
        )));
    };
    if let Some(s) = h.stage.as_ref().filter(|s| stand.stages.contains_key(*s)) {
        return Ok(Some(Bezwaar::Vorm(format!(
            "niet te nemen: stage {s} ligt al in besluit {}",
            stand.besluitkenmerk
        ))));
    }
    let (Handelingsoort::Vervolg { procedure, .. }, Some(stage)) = (&h.soort, h.stage.clone())
    else {
        return Err(Weigering::Cel(format!(
            "handeling '{}' is geen vervolg met een stage",
            h.naam
        )));
    };
    let state = StageState {
        procedure_id: procedure.clone(),
        contextual_law: gram
            .regulation
            .clone()
            .unwrap_or_else(|| h.regeling.clone()),
        current_stage: stage.clone(),
        accumulated_outputs: gram
            .velden
            .iter()
            .map(|(k, v)| (k.clone(), EngineValue::from(v)))
            .collect(),
        parameters: gram
            .invoer
            .iter()
            .map(|(k, w)| (k.clone(), EngineValue::from(w)))
            .collect(),
    };
    let mut invoer = BTreeMap::new();
    for f in &h.feiten {
        if let Some(w) = formulier.get(&f.naam).filter(|w| !w.is_null()) {
            invoer.insert(f.naam.clone(), EngineValue::from(w));
            p.parameters.insert(f.naam.clone(), w.clone());
            p.herkomst.insert(f.naam.clone(), Herkomst::Behandelaar);
        }
    }
    let uitkomst = b
        .uitkomsten
        .first()
        .ok_or_else(|| Weigering::Cel(format!("handeling '{besluit}' noemt geen uitkomst")))?;
    let outputs =
        match service.execute_stage(&h.regeling, uitkomst, Some(state), invoer, &p.peildatum) {
            Ok(ExecutionOutcome::Complete(r)) => r.outputs,
            Ok(ExecutionOutcome::Yielded {
                state,
                outputs,
                pending_inputs,
            }) => {
                if state.current_stage == stage {
                    p.mist = pending_inputs;
                    return Ok(Some(Bezwaar::Vorm(format!(
                        "niet te nemen: stage {stage} vraagt {}",
                        p.mist.join(", ")
                    ))));
                }
                outputs
            }
            Err(e) => return Ok(Some(Bezwaar::Vorm(format!("niet te nemen: {e}")))),
        };
    for u in &h.uitkomsten {
        match outputs.get(u) {
            Some(w) if w.contains_unknown() => {
                for f in w.missing_facts() {
                    if !p.mist.contains(&f.name) {
                        p.mist.push(f.name.clone());
                    }
                }
            }
            Some(w) => {
                if let Ok(v) = serde_json::to_value(w) {
                    p.uitkomsten.insert(u.clone(), v);
                }
            }
            None => {}
        }
    }
    let leeg: Vec<&str> = h
        .uitkomsten
        .iter()
        .filter(|u| p.uitkomsten.get(*u).is_none_or(Value::is_null))
        .map(String::as_str)
        .collect();
    if !p.mist.is_empty() {
        return Ok(Some(Bezwaar::Vorm(format!(
            "niet te nemen: mist {}",
            p.mist.join(", ")
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
            h.haken.join(", ")
        ))));
    }
    Ok(None)
}

/// De velden van het gram: per `$external`-sleutel van het event een
/// uitkomst, of de waarde uit het formulier.
pub(super) fn event_velden(
    event: &Event,
    uitkomsten_namen: &[String],
    formulier: &Map<String, Value>,
    uitkomsten: &BTreeMap<String, Value>,
) -> Map<String, Value> {
    event
        .external_sleutels()
        .into_iter()
        .map(|k| {
            let w = if uitkomsten_namen.contains(&k) {
                uitkomsten.get(&k).cloned().unwrap_or(Value::Null)
            } else {
                formulier.get(&k).cloned().unwrap_or(Value::Null)
            };
            (k, w)
        })
        .collect()
}
