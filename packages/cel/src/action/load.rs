//! At load time: what an action is (decision, follow-up or fact), its
//! article, hooks, assessments and form, from the stream and the law.

use super::*;

/// The article of a regulation with this output, as `<regulation>#<article>`.
pub(super) fn article_with(
    service: &LawExecutionService,
    regulation: &str,
    output: &str,
) -> Option<String> {
    service
        .resolver()
        .get_article_by_output(regulation, output, None)
        .map(|a| format!("{regulation}#{}", a.number))
}

/// The parameters the caller of the article of an action must
/// supply.
pub fn required(
    service: &LawExecutionService,
    h: &ActionDefinition,
) -> Result<BTreeMap<String, Required>, String> {
    let a = regulations::article(service, &h.article)?;
    Ok(regulations::required_parameters(service, &h.regulation, a))
}

/// The procedure (RFC-008) of the legal character an article produces.
pub fn procedure_of<'s>(
    service: &'s LawExecutionService,
    article: &str,
) -> Option<&'s ProcedureDefinition> {
    let a = regulations::article(service, article).ok()?;
    let p = a.get_produces()?;
    service
        .resolver()
        .find_procedure(p.legal_character.as_deref()?, p.procedure_id.as_deref())
}

/// The hooks the law fires on a stage of the decision an
/// article produces (RFC-007, RFC-008), as `<regulation>#<article>`,
/// sorted. The engine finds them, with its own rules: the legal character,
/// the decision type and the stage, at every hook point.
pub fn hooks_at(service: &LawExecutionService, article: &str, stage: &str) -> Vec<String> {
    let Some(produces) = regulations::article(service, article)
        .ok()
        .and_then(|a| a.get_produces())
    else {
        return Vec::new();
    };
    let Some(lc) = produces.legal_character.as_deref() else {
        return Vec::new();
    };
    let dt = produces.decision_type.as_deref();
    let mut out: Vec<String> = [HookPoint::PreActions, HookPoint::PostActions]
        .into_iter()
        .flat_map(|point| service.resolver().find_hooks(point, lc, dt, stage))
        .map(|h| format!("{}#{}", h.law_id, h.article_number))
        .collect();
    out.sort();
    out.dedup();
    out
}

/// The outputs of an article, in declaration order.
pub(super) fn outputs_of(service: &LawExecutionService, article: &str) -> Vec<String> {
    regulations::article(service, article)
        .ok()
        .and_then(|a| a.get_execution_spec())
        .and_then(|e| e.output.as_ref())
        .map(|o| o.iter().map(|o| o.name.clone()).collect())
        .unwrap_or_default()
}

/// The outputs of an article, `<regulation>#<article>`, in declaration order.
pub fn outputs_of_article(service: &LawExecutionService, article: &str) -> Vec<String> {
    outputs_of(service, article)
}

/// The names an expression refers to (`$name`, `$name.field`).
fn names_in(v: &serde_json::Value, out: &mut BTreeSet<String>) {
    match v {
        serde_json::Value::String(s) => {
            if let Some(r) = s.strip_prefix('$') {
                out.insert(r.split('.').next().unwrap_or(r).to_string());
            }
        }
        serde_json::Value::Array(a) => a.iter().for_each(|x| names_in(x, out)),
        serde_json::Value::Object(o) => o.values().for_each(|x| names_in(x, out)),
        _ => {}
    }
}

/// The outputs of an article that depend on one of `parameters`: through the
/// value of their action, directly or through an input whose `source` passes
/// the parameter on, or through another output, to a fixed point. In
/// declaration order.
pub fn outputs_depending_on(
    article: &regelrecht_engine::Article,
    parameters: &BTreeSet<String>,
) -> Vec<String> {
    let Some(exec) = article.get_execution_spec() else {
        return Vec::new();
    };
    let mut uses: Vec<(String, BTreeSet<String>)> = Vec::new();
    for i in exec.input.iter().flatten() {
        let mut n = BTreeSet::new();
        if let Ok(v) = serde_json::to_value(&i.source) {
            names_in(&v, &mut n);
        }
        uses.push((i.name.clone(), n));
    }
    for a in exec.actions.iter().flatten() {
        let (Some(output), Ok(v)) = (&a.output, serde_json::to_value(a)) else {
            continue;
        };
        let mut n = BTreeSet::new();
        names_in(&v, &mut n);
        uses.push((output.clone(), n));
    }
    let mut dependent = parameters.clone();
    loop {
        let before = dependent.len();
        for (name, n) in &uses {
            if !n.is_disjoint(&dependent) {
                dependent.insert(name.clone());
            }
        }
        if dependent.len() == before {
            break;
        }
    }
    exec.output
        .iter()
        .flatten()
        .map(|o| o.name.clone())
        .filter(|o| dependent.contains(o))
        .collect()
}

/// The parameter of an article with origin role `BESLUIT` (RFC-047): it
/// receives the id of the decision the action acts on. `None` without one
/// (or without the article); more than one is an error, because the runtime
/// cannot tell which one gets the decision.
pub fn decision_parameter_of(
    service: &LawExecutionService,
    article: &str,
) -> Result<Option<String>, String> {
    let Ok(a) = regulations::article(service, article) else {
        return Ok(None);
    };
    let found: Vec<&str> = a
        .get_parameters()
        .iter()
        .filter(|p| {
            p.origin
                .as_ref()
                .and_then(|o| o.as_valid())
                .is_some_and(|o| o.rol == Some(regelrecht_law_model::OriginRole::Besluit))
        })
        .map(|p| p.name.as_str())
        .collect();
    match found.as_slice() {
        [] => Ok(None),
        [one] => Ok(Some(one.to_string())),
        more => Err(format!(
            "{article} has more than one parameter with origin role BESLUIT ({})",
            more.join(", ")
        )),
    }
}

/// The assessments of an action that executes a decision: the boolean
/// outputs of a TOETS article that are the norm of the legal basis of
/// the event. Only for an executogram: that is the delivery or settlement
/// that executes a decision (position paper, P:54). If the assessment of exactly
/// that provision says no, the delivery would not happen on that legal basis, and
/// the process then does not take it on its own: a payment above the
/// subsidy establishment is not a payment "overeenkomstig de
/// subsidievaststelling" (Awb 4:52 lid 1). It is a conclusion of the
/// process before it acts, not a refusal by the cell: if the delivery happened
/// anyway, the cell records it (see [`take`]). An output is the norm
/// of a legal basis if its `legal_basis` names the article, and the paragraph if
/// the legal basis names one. That can be the article of the action itself,
/// or (note on source and gram id) an article that executes the policy of the action:
/// the policy of an administrative body that calls Awb 4:52 with a `source`
/// carries the assessment with `legal_basis` Awb 4:52 lid 1. An
/// establishment or a verdict (such as on default) is not an execution: what
/// it works out on its legal basis is exactly what it records.
pub fn assessments(service: &LawExecutionService, article: &str, event: &Event) -> Vec<String> {
    if event.type_ != "executogram" {
        return Vec::new();
    }
    let Ok(a) = regulations::article(service, article) else {
        return Vec::new();
    };
    if a.get_produces().and_then(|p| p.legal_character.as_deref()) != Some("TOETS") {
        return Vec::new();
    }
    let grounds: Vec<regulations::LegalBasis<'_>> = event
        .legal_basis
        .iter()
        .filter_map(|g| regulations::parse(g).ok())
        .collect();
    let paragraph_fits =
        |paragraph: Option<&str>, lb: &regelrecht_law_model::ProvisionReference| match paragraph {
            None => true,
            Some(l) => lb.paragraph.as_deref() == Some(l),
        };
    // The name of a regulation as a legal_basis writes it: its id,
    // its name, or its id in words ("Algemene wet bestuursrecht").
    let names = |lb: &regelrecht_law_model::ProvisionReference, regulation: &str| {
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
                    grounds.iter().any(|g| {
                        let own = g.article_ref() == article
                            && lb.article.as_deref().is_none_or(|x| x == a.number);
                        let executed =
                            lb.article.as_deref() == Some(g.article) && names(lb, g.regulation);
                        (own || executed) && paragraph_fits(g.paragraph, lb)
                    })
                })
                .map(|o| o.name.clone())
                .collect()
        })
        .unwrap_or_default()
}

/// What has not happened yet for a decision, from the law: the parameters the
/// procedure of the legal character (RFC-008) only asks for in a stage after that
/// of the record event. A boolean is false, everything else empty. What a
/// lexostatus of the case already derives is not listed here: no gram then means "not
/// happened", and the cell says that itself (a parameter comes from one source).
pub fn not_yet(
    service: &LawExecutionService,
    h: &ActionDefinition,
    procedure: &ProcedureDefinition,
    from_the_case: &BTreeSet<String>,
) -> BTreeMap<String, NotYet> {
    let mut out = BTreeMap::new();
    let Some(stage) = &h.stage else {
        return out;
    };
    let Some(i) = procedure.stages.iter().position(|s| &s.name == stage) else {
        return out;
    };
    let Ok(required) = required(service, h) else {
        return out;
    };
    for later in &procedure.stages[i + 1..] {
        for r in later.requires.iter().flatten() {
            let Some(p) = required.get(&r.name) else {
                continue;
            };
            if from_the_case.contains(&r.name) {
                continue;
            }
            let value = if p.typing.kind == ParameterType::Boolean {
                Value::Bool(false)
            } else {
                Value::Null
            };
            out.entry(r.name.clone()).or_insert(NotYet {
                value,
                stage: later.name.clone(),
            });
        }
    }
    out
}

/// Prepare the actions of a process, at load time: the regulation (the
/// beschikking of the authority the process acts for, `on_behalf_of`, if the
/// action names none), the article,
/// the stage, the kind, the hooks of a follow-up, the assessments and what has not
/// happened yet for a decision. The form follows later, from the origin
/// check (see [`set_form`]).
pub fn prepare_for(
    d: &mut ProcessDefinition,
    authority: Option<&str>,
    service: &LawExecutionService,
    cell: &Cell,
) -> Vec<String> {
    let mut errors = Vec::new();
    let from_the_case: BTreeSet<String> = d
        .case_sources()
        .filter_map(|b| cell.lexostatuses.lexostatus(&b.lexostatus))
        .flat_map(|l| l.reduction.derivations.keys().cloned())
        .collect();
    let Some(handling) = d.handling.as_mut() else {
        return errors;
    };
    let mut names = BTreeSet::new();
    for h in &mut handling.actions {
        let who = format!("action '{}'", h.name);
        if !names.insert(h.name.clone()) {
            errors.push(format!("{who}: the name appears more than once"));
        }
        let Some((_, event)) = cell.event(&h.record.stream, &h.record.event) else {
            errors.push(format!(
                "{who}, record {}/{}: that stream or event does not exist",
                h.record.stream, h.record.event
            ));
            continue;
        };
        h.stage = event.stage.clone();
        h.decision_role = event.decision;
        // The regulation: named, or the beschikking for which the authority of the
        // process is competent.
        let mut decision_order = None;
        if h.regulation.is_empty() {
            // Without an authority the check on `on_behalf_of` already reports it.
            let Some(actor) = authority else {
                continue;
            };
            let candidates = authority::decision_orders_of(service, actor);
            match candidates.as_slice() {
                [(r, a)] => {
                    h.regulation = r.clone();
                    decision_order = Some(format!("{r}#{a}"));
                }
                [] => {
                    errors.push(format!(
                        "{who}: no regulation names '{actor}' as competent authority for a BESCHIKKING; name the regulation in the action"
                    ));
                    continue;
                }
                more => {
                    let list: Vec<String> = more.iter().map(|(r, a)| format!("{r}#{a}")).collect();
                    errors.push(format!(
                        "{who}: '{actor}' is competent for more than one beschikking ({}); choose one with regulation",
                        list.join(", ")
                    ));
                    continue;
                }
            }
        }
        // The article: set by a process from policy (RFC-047), otherwise
        // that of the first output, or the beschikking.
        let article = if !h.article.is_empty() {
            Some(h.article.clone())
        } else {
            match h.outputs.first() {
                Some(u) => article_with(service, &h.regulation, u),
                None => decision_order.clone(),
            }
        };
        let Some(article) = article else {
            errors.push(match h.outputs.first() {
                Some(u) => format!("{who}: regulation '{}' has no output '{u}'", h.regulation),
                None => format!("{who}: name an output of regulation '{}'", h.regulation),
            });
            continue;
        };
        if let Some(b) = &decision_order {
            if b != &article {
                errors.push(format!(
                    "{who}: output '{}' does not come from {b}, the beschikking for which '{}' is competent",
                    h.outputs[0],
                    authority.unwrap_or_default()
                ));
            }
        }
        h.article = article;
        // The parameter that gets the decision id follows from the law
        // (origin role BESLUIT, RFC-047); a configuration that names another
        // one contradicts the law.
        match decision_parameter_of(service, &h.article) {
            Err(e) => errors.push(format!("{who}: {e}")),
            Ok(Some(by_law)) => match &h.decision_parameter {
                Some(configured) if configured != &by_law => errors.push(format!(
                    "{who}: decision_parameter '{configured}' contradicts {}, whose parameter with origin role BESLUIT is '{by_law}'",
                    h.article
                )),
                _ => h.decision_parameter = Some(by_law),
            },
            Ok(None) => {}
        }
        h.assessments = assessments(service, &h.article, event)
            .into_iter()
            .filter(|t| !h.outputs.contains(t))
            .collect();
    }
    // The kind: per article with a procedure the earliest stage is the
    // decision, every later one a follow-up.
    let list = &mut handling.actions;
    let mut earliest: BTreeMap<String, (usize, String)> = BTreeMap::new();
    for h in list.iter() {
        let (Some(stage), Some(p)) = (&h.stage, procedure_of(service, &h.article)) else {
            continue;
        };
        let Some(i) = p.stages.iter().position(|s| &s.name == stage) else {
            continue;
        };
        let e = earliest
            .entry(h.article.clone())
            .or_insert((i, h.name.clone()));
        if i < e.0 {
            *e = (i, h.name.clone());
        }
    }
    for h in list.iter_mut() {
        let who = format!("action '{}'", h.name);
        let Some(stage) = h.stage.clone() else {
            h.kind = ActionKind::Fact;
            continue;
        };
        let Some(p) = procedure_of(service, &h.article) else {
            // Without a procedure: a decision without a state of what comes later.
            h.kind = ActionKind::Decision;
            continue;
        };
        if !p.stages.iter().any(|s| s.name == stage) {
            errors.push(format!(
                "{who}, record {}/{}: stage '{stage}' is not in procedure '{}' of {} ({})",
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
        let decision = earliest.get(&h.article).map(|(_, n)| n.clone());
        if decision.as_deref() == Some(h.name.as_str()) {
            h.kind = ActionKind::Decision;
            h.not_yet = not_yet(service, h, p, &from_the_case);
        } else if let Some(decision) = decision {
            h.kind = ActionKind::FollowUp {
                decision,
                procedure: p.id.clone(),
            };
            h.hooks = hooks_at(service, &h.article, &stage);
            for hook in &h.hooks {
                for u in outputs_of(service, hook) {
                    if !h.outputs.contains(&u) {
                        h.outputs.push(u);
                    }
                }
            }
            h.assessments.clear();
        }
    }
    for h in list.iter_mut() {
        let mut types = regulations::output_types(service, &h.article);
        for hook in &h.hooks {
            types.extend(regulations::output_types(service, hook));
        }
        types.retain(|name, _| h.outputs.contains(name) || h.assessments.contains(name));
        h.types = types;
    }
    errors
}

/// The kind of form field for a type from the regulation. An
/// `amount` is an amount of money; in which unit (such as `eurocent`) the
/// regulation says with `type_spec.unit`, and the field gets that as `unit`.
pub fn field_kind(kind: ParameterType) -> String {
    match kind {
        ParameterType::Boolean => "yes_no",
        ParameterType::Date => "date",
        ParameterType::Amount => "amount",
        ParameterType::Number => "number",
        _ => "text",
    }
    .to_string()
}

/// Set the form of every action, after the origin check: the
/// verdicts come from it (see [`crate::origin::verdicts`]). The facts are,
/// for a follow-up, what the stage asks for (`requires`), and otherwise the fields of
/// the event that are neither an output nor a verdict. The type of a fact
/// comes from the law: from the parameter a derivation of the case makes from that
/// field, or from the stage; a field that only binds the `effective_at` is
/// a date.
pub fn set_form(d: &mut ProcessDefinition, service: &LawExecutionService, cell: &Cell) {
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
        h.facts = match &h.kind {
            ActionKind::FollowUp { decision, .. } => {
                let required = decisions
                    .get(decision)
                    .and_then(|a| regulations::article(service, a).ok())
                    .map(|a| regulations::required_parameters(service, &h.regulation, a))
                    .unwrap_or_default();
                let stage = procedure_of(service, &h.article)
                    .and_then(|p| p.stages.iter().find(|s| Some(&s.name) == h.stage.as_ref()));
                stage
                    .into_iter()
                    .flat_map(|s| s.requires.iter().flatten())
                    .map(|r| {
                        let b = required.get(&r.name);
                        Field {
                            name: r.name.clone(),
                            label: b
                                .and_then(|b| b.description.as_deref())
                                .map(crate::origin::label_from)
                                .unwrap_or_else(|| readable(&r.name)),
                            kind: Some(field_kind(r.req_type)),
                            unit: b.and_then(|b| b.typing.unit.clone()),
                            options: None,
                            columns: None,
                            explanation: None,
                            group: None,
                            legal_basis: Vec::new(),
                            optional: false,
                            supplied: None,
                            why: None,
                        }
                    })
                    .collect()
            }
            _ => {
                let verdicts: Vec<&str> = h.verdicts.iter().map(|o| o.parameter.as_str()).collect();
                event
                    .external_keys()
                    .into_iter()
                    .filter(|k| !h.outputs.contains(k) && !verdicts.contains(&k.as_str()))
                    .map(|k| fact_field(service, cell, stream, event, &k))
                    .collect()
            }
        };
    }
}

/// Whether a derivation takes over the value of the field (and so has its
/// type), instead of deriving something from it.
fn takes_over(a: &crate::reduction::Derived) -> bool {
    use crate::reduction::Derivation as A;
    matches!(
        a.derivation,
        A::Field { .. } | A::LatestField { .. } | A::Sum { .. }
    )
}

/// The form field of a fact: the `$external` field `key` of the
/// event, with the type of the parameter a lexostatus of the cell derives
/// from it.
fn fact_field(
    service: &LawExecutionService,
    cell: &Cell,
    stream: &crate::stream::Stream,
    event: &Event,
    key: &str,
) -> Field {
    let mut kind = None;
    let mut unit = None;
    let mut explanation = None;
    // Which field of the gram binds to this key?
    let paths: Vec<String> = event
        .leaves()
        .into_iter()
        .filter(|b| matches!(&b.binding, Binding::External(s) | Binding::Supplied(s) if s == key))
        .map(|b| b.path)
        .collect();
    // First a derivation that takes over the field (field, sum), then one that
    // derives something from it (a year, whether the field is filled): the type of the
    // first is that of the field itself.
    let mut candidates: Vec<(&String, &crate::reduction::Derived)> = cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| d.reduction.chronicle == stream.chronicle)
        .flat_map(|d| d.all_derivations())
        .filter(|(_, a)| a.read_paths().iter().any(|p| paths.iter().any(|q| q == *p)))
        .collect();
    candidates.sort_by_key(|(_, a)| !takes_over(a));
    'find: for (name, a) in candidates {
        // The parameter with this name, in the legal basis of the event or of the
        // derivation.
        for g in event.legal_basis.iter().chain(a.legal_basis.iter()) {
            let Ok(art) = regulations::article(service, g) else {
                continue;
            };
            if let Some(p) = art.get_parameters().iter().find(|p| &p.name == name) {
                kind = Some(field_kind(p.param_type));
                unit = p.type_spec.as_ref().and_then(|t| t.unit.clone());
                explanation = p.description.clone();
                break 'find;
            }
        }
    }
    // If the law states the type of the field, use that (Awb 4:87: the amount).
    if kind.is_none() {
        if let Some(t) = paths.iter().find_map(|p| event.field_types.get(p)) {
            kind = Some(field_kind(t.type_));
            unit = t.unit.clone();
            explanation = Some(format!(
                "Het type van dit veld zegt de wet die het feit vestigt ({}).",
                event.establishes.join(", ")
            ));
        }
    }
    let effective_at = event.effective_at.as_ref().is_some_and(
        |b| matches!(b.binding(), Binding::External(s) | Binding::Supplied(s) if s == key),
    );
    if kind.is_none() && effective_at {
        kind = Some("date".into());
        explanation = event.effective_at.as_ref().map(|b| {
            format!(
                "Het moment waarop het feit rechtens plaatsvond ({}).",
                b.legal_basis.join(", ")
            )
        });
    }
    Field {
        name: key.to_string(),
        label: readable(key),
        kind,
        unit,
        options: None,
        columns: None,
        explanation,
        group: None,
        legal_basis: field_legal_basis(event, &paths, effective_at),
        optional: false,
        supplied: None,
        why: None,
    }
}

/// The legal basis of the form field that binds to `paths`: what the article
/// that declares the path gives as legal basis (a path covers everything
/// below it). The field that gives the `effective_at` of the gram has the
/// legal basis of that moment. Otherwise, or when neither carries a legal
/// basis: that of the whole event.
fn field_legal_basis(event: &Event, paths: &[String], effective_at: bool) -> Vec<String> {
    let mut out: Vec<String> = Vec::new();
    for leaf in paths {
        for (path, basis) in &event.field_legal_basis {
            if leaf == path || leaf.starts_with(&format!("{path}.")) {
                for x in basis {
                    if !out.contains(x) {
                        out.push(x.clone());
                    }
                }
            }
        }
    }
    if out.is_empty() && effective_at {
        if let Some(b) = &event.effective_at {
            out = b.legal_basis.clone();
        }
    }
    if out.is_empty() {
        event.legal_basis.clone()
    } else {
        out
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    #[test]
    fn an_output_depends_on_a_parameter_through_inputs_and_outputs() {
        let (s, _, _) = crate::derive::tests::setup();
        let a = crate::regulations::article(&s, "testregeling_awb#3").unwrap();
        let out = outputs_depending_on(
            a,
            &std::collections::BTreeSet::from(["betaald_bedrag".to_string()]),
        );
        assert_eq!(
            out,
            [
                "nog_te_betalen",
                "betaling_conform",
                "onverschuldigd_betaald"
            ]
        );
        // Through an input whose source passes the parameter on.
        let out = outputs_depending_on(
            a,
            &std::collections::BTreeSet::from(["datum_bekendmaking".to_string()]),
        );
        assert_eq!(out, ["betaling_conform"]);
        let a = crate::regulations::article(&s, "testregeling_awb#5").unwrap();
        assert!(outputs_depending_on(
            a,
            &std::collections::BTreeSet::from(["iets_anders".to_string()])
        )
        .is_empty());
    }

    use super::*;
    use regelrecht_engine::LawExecutionService;

    /// A fictitious regulation with two articles: a beschikking of "De
    /// Instantie van Voorbeeld" and an assessment by the same authority.
    const REGULATION: &str = r#"
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

    /// Hooks on the beschikking of [`REGULATION`]: one without a decision type, one
    /// for a rejection (does not fire on a grant), and one without a stage
    /// (then the stage BESLUIT).
    const HOOKS: &str = r#"
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
        s.load_law(REGULATION).unwrap();
        s.load_law(HOOKS).unwrap();
        s
    }

    /// The hooks of a stage come from the engine: the decision type
    /// counts, a hook without a stage fires on the decision, and every hook point counts.
    #[test]
    fn hooks_come_from_the_engine() {
        let s = service();
        assert_eq!(
            hooks_at(&s, "testregeling_bevoegd#2", "BEKENDMAKING"),
            ["testregeling_haken#1"]
        );
        assert_eq!(
            hooks_at(&s, "testregeling_bevoegd#2", "BESLUIT"),
            ["testregeling_haken#3"]
        );
        // A TOETS is not a beschikking: no hooks.
        assert!(hooks_at(&s, "testregeling_bevoegd#1", "BESLUIT").is_empty());
    }

    /// A form field shows the legal basis of the article that declares its
    /// path (a path covers what lies below it); the field that gives the
    /// `effective_at` shows the legal basis of that moment; otherwise the
    /// field shows that of the event.
    #[test]
    fn a_form_field_shows_the_legal_basis_of_its_own_field() {
        let mut event: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\nlegal_basis: ['w#1', 'w#2 lid 1', 'w#2 lid 2']\ntype: submission\neffective_at: {source: $external.dag, legal_basis: ['w#1 lid 3']}\nfields: {akte: {datum: $external.datum}, verklaring: $external.verklaring, notitie: $external.notitie}\n",
        )
        .unwrap();
        event
            .field_legal_basis
            .insert("akte".into(), vec!["w#2 lid 1".into()]);
        event
            .field_legal_basis
            .insert("verklaring".into(), vec!["w#2 lid 2".into()]);
        assert_eq!(
            field_legal_basis(&event, &["akte.datum".into()], false),
            ["w#2 lid 1"]
        );
        assert_eq!(
            field_legal_basis(&event, &["verklaring".into()], false),
            ["w#2 lid 2"]
        );
        assert_eq!(field_legal_basis(&event, &[], true), ["w#1 lid 3"]);
        assert_eq!(
            field_legal_basis(&event, &["notitie".into()], false),
            ["w#1", "w#2 lid 1", "w#2 lid 2"]
        );
    }

    /// An assessment only counts if the article is in the legal basis of the event,
    /// and only a boolean output with the norm of that paragraph as
    /// `legal_basis`.
    #[test]
    fn assessments_from_the_legal_basis_of_the_event() {
        let s = service();
        let event: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\nlegal_basis: ['testregeling_bevoegd#1 lid 1']\ntype: executogram\ncase: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert_eq!(assessments(&s, "testregeling_bevoegd#1", &event), ["toets"]);
        // A verdict or establishment does not execute a decision: no assessment.

        let mut verdict = event.clone();
        verdict.type_ = "act".into();
        assert!(assessments(&s, "testregeling_bevoegd#1", &verdict).is_empty());
        assert!(assessments(&s, "testregeling_bevoegd#2", &event).is_empty());
        let other: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\nlegal_basis: ['testregeling_bevoegd#2']\ntype: executogram\ncase: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert!(assessments(&s, "testregeling_bevoegd#1", &other).is_empty());
        let other_paragraph: Event = serde_yaml_ng::from_str(
            "name: e\nintake: behandelaar\nlegal_basis: ['testregeling_bevoegd#1 lid 2']\ntype: executogram\ncase: volgt\nfields: {x: $external.x}\n",
        )
        .unwrap();
        assert!(assessments(&s, "testregeling_bevoegd#1", &other_paragraph).is_empty());
    }

    const PAYMENT_POLICY: &str = r#"
$id: testbeleid_betaling
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: De dienst betaalt wat bij het besluit nog te betalen is.
    machine_readable:
      execution:
        parameters:
          - name: besluit
            type: string
            required: true
            origin: {waarde: DOSSIER, grondslag: 'testbeleid_betaling#1', rol: BESLUIT}
          - {name: bedrag, type: amount, required: true}
        output: [{name: nog_te_betalen, type: amount}]
        actions: [{output: nog_te_betalen, value: $bedrag}]
"#;

    /// The parameter that receives the decision id follows from the law
    /// (origin role BESLUIT), not from the configuration.
    #[test]
    fn the_decision_parameter_follows_from_the_origin_role() {
        let mut s = LawExecutionService::new();
        s.load_law(PAYMENT_POLICY).unwrap();
        assert_eq!(
            decision_parameter_of(&s, "testbeleid_betaling#1"),
            Ok(Some("besluit".to_string()))
        );
        let without = PAYMENT_POLICY.replace(", rol: BESLUIT", "");
        let mut s = LawExecutionService::new();
        s.load_law(&without).unwrap();
        assert_eq!(decision_parameter_of(&s, "testbeleid_betaling#1"), Ok(None));
        // Two parameters with the role: the runtime cannot tell which one.
        let two = PAYMENT_POLICY.replace(
            "- {name: bedrag, type: amount, required: true}",
            "- {name: bedrag, type: amount, required: true, origin: {waarde: DOSSIER, grondslag: 'testbeleid_betaling#1', rol: BESLUIT}}",
        );
        let mut s = LawExecutionService::new();
        s.load_law(&two).unwrap();
        let e = decision_parameter_of(&s, "testbeleid_betaling#1").unwrap_err();
        assert!(e.contains("besluit, bedrag"), "{e}");
    }
}
