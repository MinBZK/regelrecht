//! Bij het laden: wat een handeling is (besluit, vervolg of feit), haar
//! artikel, haken, toetsen en formulier, uit de stroom en de wet.

use super::*;

/// Het artikel van een regeling met deze uitkomst, als `<regeling>#<artikel>`.
pub(super) fn artikel_met(
    service: &LawExecutionService,
    regulation: &str,
    output: &str,
) -> Option<String> {
    service
        .resolver()
        .get_article_by_output(regulation, output, None)
        .map(|a| format!("{regulation}#{}", a.number))
}

/// De parameters die de aanroeper van het artikel van een handeling moet
/// leveren.
pub fn benodigd(
    service: &LawExecutionService,
    h: &HandelingDefinitie,
) -> Result<BTreeMap<String, Benodigd>, String> {
    let a = regelingen::article(service, &h.article)?;
    Ok(regelingen::benodigde_parameters(service, &h.regulation, a))
}

/// De procedure (RFC-008) van het rechtskarakter dat een artikel produceert.
pub fn procedure_van<'s>(
    service: &'s LawExecutionService,
    article: &str,
) -> Option<&'s ProcedureDefinition> {
    let a = regelingen::article(service, article).ok()?;
    let p = a.get_produces()?;
    service
        .resolver()
        .find_procedure(p.legal_character.as_deref()?, p.procedure_id.as_deref())
}

/// De haken die de wet laat vuren op een stage van het besluit dat een
/// artikel produceert (RFC-007, RFC-008), als `<regeling>#<artikel>`,
/// gesorteerd. De engine zoekt ze, met haar eigen regels: het rechtskarakter,
/// het soort besluit en de stage, op elk haakpunt.
pub fn haken_op(service: &LawExecutionService, article: &str, stage: &str) -> Vec<String> {
    let Some(produces) = regelingen::article(service, article)
        .ok()
        .and_then(|a| a.get_produces())
    else {
        return Vec::new();
    };
    let Some(lc) = produces.legal_character.as_deref() else {
        return Vec::new();
    };
    let dt = produces.decision_type.as_deref();
    let mut uit: Vec<String> = [HookPoint::PreActions, HookPoint::PostActions]
        .into_iter()
        .flat_map(|punt| service.resolver().find_hooks(punt, lc, dt, stage))
        .map(|h| format!("{}#{}", h.law_id, h.article_number))
        .collect();
    uit.sort();
    uit.dedup();
    uit
}

/// De uitkomsten van een artikel, in de volgorde van declaratie.
pub(super) fn uitkomsten_van(service: &LawExecutionService, article: &str) -> Vec<String> {
    regelingen::article(service, article)
        .ok()
        .and_then(|a| a.get_execution_spec())
        .and_then(|e| e.output.as_ref())
        .map(|o| o.iter().map(|o| o.name.clone()).collect())
        .unwrap_or_default()
}

/// De toetsen van een handeling die een besluit uitvoert: de booleaanse
/// uitkomsten van een TOETS-artikel die de norm zijn van de grondslag van
/// het event. Alleen bij een executogram: dat is de levering of afhandeling
/// die een besluit uitvoert (positionpaper, P:54). Zegt de toets van precies
/// die bepaling nee, dan zou de levering niet op die grondslag gebeuren, en
/// het proces doet haar dan niet uit zichzelf: een betaling boven de
/// subsidievaststelling is geen betaling "overeenkomstig de
/// subsidievaststelling" (Awb 4:52 lid 1). Het is een conclusie van het
/// proces voor het handelt, geen weigering van de cel: gebeurde de levering
/// toch, dan legt de cel haar vast (zie [`neem`]). Een uitkomst is de norm
/// van een grondslag als haar `legal_basis` het artikel noemt, en het lid als
/// de grondslag er een noemt. Dat kan het artikel van de handeling zelf zijn,
/// of (notitie bron en gram-id) een artikel dat het beleid van de handeling
/// uitvoert: het beleid van een bestuursorgaan dat Awb 4:52 met een `source`
/// aanroept, draagt de toets met `legal_basis` Awb 4:52 lid 1. Een
/// vaststelling of een oordeel (zoals over verzuim) is geen uitvoering: wat
/// die op haar grondslag uitwerkt, is juist wat zij vastlegt.
pub fn assessments(service: &LawExecutionService, article: &str, event: &Event) -> Vec<String> {
    if event.type_ != "executogram" {
        return Vec::new();
    }
    let Ok(a) = regelingen::article(service, article) else {
        return Vec::new();
    };
    if a.get_produces().and_then(|p| p.legal_character.as_deref()) != Some("TOETS") {
        return Vec::new();
    }
    let gronden: Vec<regelingen::Grondslag<'_>> = event
        .legal_basis
        .iter()
        .filter_map(|g| regelingen::ontleed(g).ok())
        .collect();
    let lid_past =
        |paragraph: Option<&str>, lb: &regelrecht_law_model::ProvisionReference| match paragraph {
            None => true,
            Some(l) => lb.paragraph.as_deref() == Some(l),
        };
    // De naam van een regeling zoals een legal_basis haar schrijft: haar id,
    // haar naam, of haar id in woorden ("Algemene wet bestuursrecht").
    let noemt = |lb: &regelrecht_law_model::ProvisionReference, regulation: &str| {
        lb.law.as_deref().is_some_and(|w| {
            w == regulation
                || w.to_lowercase().replace(' ', "_") == regulation
                || service
                    .resolver()
                    .get_law(regulation)
                    .and_then(|l| l.name.as_deref())
                    == Some(w)
        })
    };
    a.get_execution_spec()
        .and_then(|e| e.output.as_ref())
        .map(|o| {
            o.iter()
                .filter(|o| o.output_type == ParameterType::Boolean)
                .filter(|o| {
                    let Some(lb) = &o.legal_basis else {
                        return false;
                    };
                    gronden.iter().any(|g| {
                        let eigen = format!("{}#{}", g.regulation, g.article) == article
                            && lb.article.as_deref().is_none_or(|x| x == a.number);
                        let uitgevoerd =
                            lb.article.as_deref() == Some(g.article) && noemt(lb, g.regulation);
                        (eigen || uitgevoerd) && lid_past(g.paragraph, lb)
                    })
                })
                .map(|o| o.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// Wat bij een besluit nog niet gebeurd is, uit de wet: de parameters die de
/// procedure van het rechtskarakter (RFC-008) pas vraagt in een stage na die
/// van het vastleg-event. Een boolean is onwaar, al het andere leeg. Wat een
/// lexostatus van de zaak al afleidt, staat hier niet: geen gram is dan "niet
/// gebeurd", en dat zegt de cel zelf (een parameter komt uit een bron).
pub fn not_yet(
    service: &LawExecutionService,
    h: &HandelingDefinitie,
    procedure: &ProcedureDefinition,
    uit_de_zaak: &BTreeSet<String>,
) -> BTreeMap<String, NogNiet> {
    let mut uit = BTreeMap::new();
    let Some(stage) = &h.stage else {
        return uit;
    };
    let Some(i) = procedure.stages.iter().position(|s| &s.name == stage) else {
        return uit;
    };
    let Ok(benodigd) = benodigd(service, h) else {
        return uit;
    };
    for later in &procedure.stages[i + 1..] {
        for r in later.requires.iter().flatten() {
            let Some(p) = benodigd.get(&r.name) else {
                continue;
            };
            if uit_de_zaak.contains(&r.name) {
                continue;
            }
            let value = if p.typing.soort == ParameterType::Boolean {
                Value::Bool(false)
            } else {
                Value::Null
            };
            uit.entry(r.name.clone()).or_insert(NogNiet {
                value,
                stage: later.name.clone(),
            });
        }
    }
    uit
}

/// Bereid de handelingen van een proces voor, bij het laden: de regeling (de
/// beschikking van het gezag waarvoor het proces handelt, `namens`, als de
/// handeling er geen noemt), het artikel,
/// de stage, de soort, de haken van een vervolg, de toetsen en wat bij een
/// besluit nog niet gebeurd is. Het formulier volgt later, uit de controle
/// op de herkomst (zie [`zet_formulier`]).
pub fn bereid_voor(
    d: &mut ProcesDefinitie,
    authority: Option<&str>,
    service: &LawExecutionService,
    cell: &Cell,
) -> Vec<String> {
    let mut fouten = Vec::new();
    let uit_de_zaak: BTreeSet<String> = d
        .zaakbronnen()
        .filter_map(|b| cell.lexostatuses.lexostatus(&b.lexostatus))
        .flat_map(|l| l.reduction.derivations.keys().cloned())
        .collect();
    let Some(handling) = d.handling.as_mut() else {
        return fouten;
    };
    let mut namen = BTreeSet::new();
    for h in &mut handling.actions {
        let wie = format!("handeling '{}'", h.name);
        if !namen.insert(h.name.clone()) {
            fouten.push(format!("{wie}: de naam staat er meer dan een keer"));
        }
        let Some((_, event)) = cell.event(&h.record.stream, &h.record.event) else {
            fouten.push(format!(
                "{wie}, vastleggen {}/{}: die stroom of dat event bestaat niet",
                h.record.stream, h.record.event
            ));
            continue;
        };
        h.stage = event.stage.clone();
        h.decision_role = event.decision;
        // De regeling: genoemd, of de beschikking waarvoor het gezag van het
        // proces bevoegd is.
        let mut beschikking = None;
        if h.regulation.is_empty() {
            // Zonder gezag meldt de controle op `namens` het al.
            let Some(actor) = authority else {
                continue;
            };
            let kandidaten = gezag::beschikkingen_van(service, actor);
            match kandidaten.as_slice() {
                [(r, a)] => {
                    h.regulation = r.clone();
                    beschikking = Some(format!("{r}#{a}"));
                }
                [] => {
                    fouten.push(format!(
                        "{wie}: geen regeling noemt '{actor}' als bevoegd gezag bij een BESCHIKKING; noem de regeling in de handeling"
                    ));
                    continue;
                }
                meer => {
                    let list: Vec<String> = meer.iter().map(|(r, a)| format!("{r}#{a}")).collect();
                    fouten.push(format!(
                        "{wie}: '{actor}' is bevoegd voor meer dan een beschikking ({}); kies er een met regeling",
                        list.join(", ")
                    ));
                    continue;
                }
            }
        }
        // Het artikel: dat van de eerste uitkomst, of de beschikking.
        let article = match h.outputs.first() {
            Some(u) => artikel_met(service, &h.regulation, u),
            None => beschikking.clone(),
        };
        let Some(article) = article else {
            fouten.push(match h.outputs.first() {
                Some(u) => format!(
                    "{wie}: regeling '{}' heeft geen uitkomst '{u}'",
                    h.regulation
                ),
                None => format!("{wie}: noem een uitkomst van regeling '{}'", h.regulation),
            });
            continue;
        };
        if let Some(b) = &beschikking {
            if b != &article {
                fouten.push(format!(
                    "{wie}: uitkomst '{}' komt niet uit {b}, de beschikking waarvoor '{}' bevoegd is",
                    h.outputs[0],
                    authority.unwrap_or_default()
                ));
            }
        }
        h.article = article;
        h.assessments = assessments(service, &h.article, event)
            .into_iter()
            .filter(|t| !h.outputs.contains(t))
            .collect();
    }
    // De soort: per artikel met een procedure is de vroegste stage het
    // besluit, elke latere een vervolg.
    let list = &mut handling.actions;
    let mut vroegste: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for h in list.iter() {
        let (Some(stage), Some(p)) = (&h.stage, procedure_van(service, &h.article)) else {
            continue;
        };
        let Some(i) = p.stages.iter().position(|s| &s.name == stage) else {
            continue;
        };
        let e = vroegste
            .entry(h.article.clone())
            .or_insert((i, h.name.clone()));
        if i < e.0 {
            *e = (i, h.name.clone());
        }
    }
    for h in list.iter_mut() {
        let wie = format!("handeling '{}'", h.name);
        let Some(stage) = h.stage.clone() else {
            h.soort = Handelingsoort::Fact;
            continue;
        };
        let Some(p) = procedure_van(service, &h.article) else {
            // Zonder procedure: een besluit zonder stand van wat later komt.
            h.soort = Handelingsoort::Decision;
            continue;
        };
        if !p.stages.iter().any(|s| s.name == stage) {
            fouten.push(format!(
                "{wie}, vastleggen {}/{}: stage '{stage}' staat niet in procedure '{}' van {} ({})",
                h.record.stream,
                h.record.event,
                p.id,
                h.article,
                p.stages
                    .iter()
                    .map(|s| s.name.as_str())
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
            continue;
        }
        let decision = vroegste.get(&h.article).map(|(_, n)| n.clone());
        if decision.as_deref() == Some(h.name.as_str()) {
            h.soort = Handelingsoort::Decision;
            h.not_yet = not_yet(service, h, p, &uit_de_zaak);
        } else if let Some(decision) = decision {
            h.soort = Handelingsoort::FollowUp {
                decision,
                procedure: p.id.clone(),
            };
            h.hooks = haken_op(service, &h.article, &stage);
            for haak in &h.hooks {
                for u in uitkomsten_van(service, haak) {
                    if !h.outputs.contains(&u) {
                        h.outputs.push(u);
                    }
                }
            }
            h.assessments.clear();
        }
    }
    for h in list.iter_mut() {
        let mut types = regelingen::uitkomsttypen(service, &h.article);
        for haak in &h.hooks {
            types.extend(regelingen::uitkomsttypen(service, haak));
        }
        types.retain(|name, _| h.outputs.contains(name) || h.assessments.contains(name));
        h.types = types;
    }
    fouten
}

/// Het soort veld van een formulier bij een type uit de regeling. Een
/// `amount` is een bedrag; in welke eenheid (zoals `eurocent`), zegt de
/// regeling met `type_spec.unit`, en dat krijgt het veld mee als `eenheid`.
pub fn veldsoort(soort: ParameterType) -> String {
    match soort {
        ParameterType::Boolean => "yes_no",
        ParameterType::Date => "date",
        ParameterType::Amount => "amount",
        ParameterType::Number => "number",
        _ => "text",
    }
    .to_string()
}

/// Zet het formulier van elke handeling, na de controle op de herkomst: de
/// oordelen komen daaruit (zie [`crate::origin::oordelen`]). De feiten zijn
/// bij een vervolg wat de stage vraagt (`requires`), en anders de velden van
/// het event die geen uitkomst en geen oordeel zijn. Het type van een feit
/// komt uit de wet: uit de parameter die een afleiding van de zaak uit dat
/// veld maakt, of uit de stage; een veld dat alleen het `op_moment` bindt, is
/// een datum.
pub fn zet_formulier(d: &mut ProcesDefinitie, service: &LawExecutionService, cell: &Cell) {
    let Some(handling) = d.handling.as_mut() else {
        return;
    };
    let decisions: BTreeMap<String, String> = handling
        .actions
        .iter()
        .map(|h| (h.name.clone(), h.article.clone()))
        .collect();
    for h in &mut handling.actions {
        let Some((stream, event)) = cell.event(&h.record.stream, &h.record.event) else {
            continue;
        };
        h.feiten = match &h.soort {
            Handelingsoort::FollowUp { decision, .. } => {
                let benodigd = decisions
                    .get(decision)
                    .and_then(|a| regelingen::article(service, a).ok())
                    .map(|a| regelingen::benodigde_parameters(service, &h.regulation, a))
                    .unwrap_or_default();
                let stage = procedure_van(service, &h.article)
                    .and_then(|p| p.stages.iter().find(|s| Some(&s.name) == h.stage.as_ref()));
                stage
                    .into_iter()
                    .flat_map(|s| s.requires.iter().flatten())
                    .map(|r| {
                        let b = benodigd.get(&r.name);
                        Veld {
                            name: r.name.clone(),
                            label: b
                                .and_then(|b| b.description.as_deref())
                                .map(crate::origin::label_uit)
                                .unwrap_or_else(|| leesbaar(&r.name)),
                            soort: Some(veldsoort(r.req_type)),
                            unit: b.and_then(|b| b.typing.unit.clone()),
                            options: None,
                            columns: None,
                            explanation: None,
                            group: None,
                            legal_basis: Vec::new(),
                        }
                    })
                    .collect()
            }
            _ => {
                let verdicts: Vec<&str> = h.verdicts.iter().map(|o| o.parameter.as_str()).collect();
                event
                    .external_sleutels()
                    .into_iter()
                    .filter(|k| !h.outputs.contains(k) && !verdicts.contains(&k.as_str()))
                    .map(|k| feitveld(service, cell, stream, event, &k))
                    .collect()
            }
        };
    }
}

/// Of een afleiding de waarde van het veld overneemt (en dus zijn type
/// heeft), in plaats van er iets van af te leiden.
fn overneemt(a: &crate::reductie::Afgeleid) -> bool {
    use crate::reductie::Afleiding as A;
    matches!(
        a.derivation,
        A::Veld { .. } | A::LaatsteVeld { .. } | A::Som { .. }
    )
}

/// Het formulierveld van een feit: het `$external`-veld `sleutel` van het
/// event, met het type van de parameter die een lexostatus van de cel eruit
/// afleidt.
fn feitveld(
    service: &LawExecutionService,
    cell: &Cell,
    stream: &crate::stroom::Stroom,
    event: &Event,
    sleutel: &str,
) -> Veld {
    let mut soort = None;
    let mut unit = None;
    let mut explanation = None;
    // Welk veld van het gram bindt aan deze sleutel?
    let paden: Vec<String> = event
        .bladeren()
        .into_iter()
        .filter(|b| matches!(&b.binding, Binding::External(s) if s == sleutel))
        .map(|b| b.path)
        .collect();
    // Eerst een afleiding die het veld overneemt (veld, som), dan een die er
    // iets van afleidt (een jaar, of het veld gevuld is): het type van de
    // eerste is dat van het veld zelf.
    let mut kandidaten: Vec<(&String, &crate::reductie::Afgeleid)> = cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| d.reduction.chronicle == stream.chronicle)
        .flat_map(|d| d.alle_afleidingen())
        .filter(|(_, a)| {
            a.gelezen_paden()
                .iter()
                .any(|p| paden.iter().any(|q| q == *p))
        })
        .collect();
    kandidaten.sort_by_key(|(_, a)| !overneemt(a));
    'zoek: for (name, a) in kandidaten {
        // De parameter met deze naam, in de grondslag van het event of van de
        // afleiding.
        for g in event.legal_basis.iter().chain(a.legal_basis.iter()) {
            let Ok(art) = regelingen::article(service, g) else {
                continue;
            };
            if let Some(p) = art.get_parameters().iter().find(|p| &p.name == name) {
                soort = Some(veldsoort(p.param_type));
                unit = p.type_spec.as_ref().and_then(|t| t.unit.clone());
                explanation = p.description.clone();
                break 'zoek;
            }
        }
    }
    // Zegt de wet het type van het veld, dan dat (Awb 4:87: het bedrag).
    if soort.is_none() {
        if let Some(t) = paden.iter().find_map(|p| event.field_types.get(p)) {
            soort = Some(veldsoort(t.type_));
            unit = t.unit.clone();
            explanation = Some(format!(
                "Het type van dit veld zegt de wet die het feit vestigt ({}).",
                event.establishes.join(", ")
            ));
        }
    }
    let effective_at = event
        .effective_at
        .as_ref()
        .is_some_and(|b| matches!(b.binding(), Binding::External(s) if s == sleutel));
    if soort.is_none() && effective_at {
        soort = Some("date".into());
        explanation = event.effective_at.as_ref().map(|b| {
            format!(
                "Het moment waarop het feit rechtens plaatsvond ({}).",
                b.legal_basis.join(", ")
            )
        });
    }
    Veld {
        name: sleutel.to_string(),
        label: leesbaar(sleutel),
        soort,
        unit,
        options: None,
        columns: None,
        explanation,
        group: None,
        legal_basis: event.legal_basis.clone(),
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
        output:
          - {name: toets, type: boolean, legal_basis: {article: '1', paragraph: '1'}}
          - {name: zonder_grondslag, type: boolean}
        actions:
          - {output: toets, value: {operation: GREATER_THAN, subject: $x, value: 0}}
          - {output: zonder_grondslag, value: true}
  - number: '2'
    text: Besluit
    machine_readable:
      execution:
        produces: {legal_character: BESCHIKKING, decision_type: TOEKENNING}
        parameters: [{name: x, type: number, required: false}]
        output: [{name: bedrag, type: number}]
        actions: [{output: bedrag, value: $x}]
"#;

    /// Haken op de beschikking van [`REGELING`]: een zonder soort besluit, een
    /// voor een afwijzing (vuurt niet op een toekenning), en een zonder stage
    /// (dan de stage BESLUIT).
    const HAKEN: &str = r#"
$id: testregeling_haken
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Termijn
    machine_readable:
      hooks:
        - hook_point: post_actions
          applies_to: {legal_character: BESCHIKKING, stage: BEKENDMAKING}
      execution:
        output: [{name: termijn, type: number}]
        actions: [{output: termijn, value: 6}]
  - number: '2'
    text: Alleen bij afwijzing
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {legal_character: BESCHIKKING, decision_type: AFWIJZING, stage: BEKENDMAKING}
      execution:
        output: [{name: afgewezen, type: boolean}]
        actions: [{output: afgewezen, value: true}]
  - number: '3'
    text: Bij het besluit
    machine_readable:
      hooks:
        - hook_point: pre_actions
          applies_to: {legal_character: BESCHIKKING}
      execution:
        output: [{name: gemotiveerd, type: boolean}]
        actions: [{output: gemotiveerd, value: true}]
"#;

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(REGELING).unwrap();
        s.load_law(HAKEN).unwrap();
        s
    }

    /// De haken van een stage komen uit de engine: het soort besluit telt
    /// mee, een haak zonder stage vuurt op het besluit, en elk haakpunt telt.
    #[test]
    fn haken_uit_de_engine() {
        let s = service();
        assert_eq!(
            haken_op(&s, "testregeling_bevoegd#2", "BEKENDMAKING"),
            ["testregeling_haken#1"]
        );
        assert_eq!(
            haken_op(&s, "testregeling_bevoegd#2", "BESLUIT"),
            ["testregeling_haken#3"]
        );
        // Een TOETS is geen beschikking: geen haken.
        assert!(haken_op(&s, "testregeling_bevoegd#1", "BESLUIT").is_empty());
    }

    /// Een toets telt alleen als het artikel in de grondslag van het event
    /// staat, en alleen een booleaanse uitkomst met de norm van dat lid als
    /// `legal_basis`.
    #[test]
    fn toetsen_uit_de_grondslag_van_het_event() {
        let s = service();
        let event: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\nlegal_basis: ['testregeling_bevoegd#1 lid 1']\ntype: executogram\ncase: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert_eq!(assessments(&s, "testregeling_bevoegd#1", &event), ["toets"]);
        // Een oordeel of vaststelling voert geen besluit uit: geen toets.
        let mut verdict = event.clone();
        verdict.type_ = "act".into();
        assert!(assessments(&s, "testregeling_bevoegd#1", &verdict).is_empty());
        assert!(assessments(&s, "testregeling_bevoegd#2", &event).is_empty());
        let ander: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\nlegal_basis: ['testregeling_bevoegd#2']\ntype: executogram\ncase: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert!(assessments(&s, "testregeling_bevoegd#1", &ander).is_empty());
        let ander_lid: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\nlegal_basis: ['testregeling_bevoegd#1 lid 2']\ntype: executogram\ncase: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert!(assessments(&s, "testregeling_bevoegd#1", &ander_lid).is_empty());
    }
}
