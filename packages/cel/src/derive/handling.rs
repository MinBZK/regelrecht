//! The actions of a process from policy (RFC-047, spec §3): what the
//! handler does follows from the streams of the cell, the stages of the
//! procedure (RFC-008) and the law.

use std::collections::BTreeSet;

use regelrecht_engine::LawExecutionService;

use super::establishing;
use crate::action::points_to_decision;
use crate::cell::Cell;
use crate::config::{ActionDefinition, Handling, LexostatusReference, Record};
use crate::policy::{ActorPolicy, ChannelKind};
use crate::stream::{Binding, Event, Stream, To};

/// The stage of a decision (RFC-008).
const DECISION: &str = crate::stream::DECISION;

/// What the handler does: one action per event of the cell whose intake is a
/// channel of the policy with `kind: handling`, in the order of the streams
/// (spec §3, decision 12 of the plan). A decision executes its establishing
/// article; a later stage of its procedure is a follow-up on it, one per
/// decision it can follow; a fact executes the article that reads it. The
/// worklist and the list of all cases are the ones the runtime offers
/// ([`crate::reduction::WORKLIST`], [`crate::reduction::CASES`]).
pub(super) fn handling(
    p: &ActorPolicy,
    cell: &Cell,
    service: &LawExecutionService,
    synthesis: &crate::deployment::SynthesisDeployment,
) -> Result<(Option<Handling>, Vec<String>), Vec<String>> {
    let intakes: Vec<&str> = p
        .channels
        .iter()
        .filter(|c| c.def.kind == ChannelKind::Handling)
        .map(|c| c.id.as_str())
        .collect();
    if intakes.is_empty() {
        return Ok((None, Vec::new()));
    }
    let events: Vec<(&Stream, &Event)> = cell
        .streams
        .iter()
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(_, e)| intakes.contains(&e.intake.as_str()))
        .collect();
    let mut errors = Vec::new();
    let mut warnings = Vec::new();
    let new = |s: &Stream, e: &Event, name: String, article: String| ActionDefinition {
        name,
        regulation: article
            .split_once('#')
            .map(|(r, _)| r.to_string())
            .unwrap_or_default(),
        record: Record {
            cell: cell.id().to_string(),
            stream: s.id.clone(),
            event: e.name.clone(),
        },
        article,
        ..ActionDefinition::default()
    };
    let label = |e: &Event, article: &str| {
        let group = crate::origin::group(service, article).unwrap_or_else(|| article.to_string());
        format!("{} ({group})", crate::form::readable(&e.name))
    };
    // Decisions first: facts and follow-ups refer to them.
    let mut decisions: Vec<ActionDefinition> = Vec::new();
    for (s, e) in events
        .iter()
        .filter(|(_, e)| e.stage.as_deref() == Some(DECISION))
    {
        let Some(article) = establishing(e) else {
            errors.push(format!("event '{}': no establishing article", e.name));
            continue;
        };
        let mut h = new(s, e, e.name.clone(), article.clone());
        h.outputs = external_outputs(e, service, &article);
        h.label = Some(label(e, &article));
        decisions.push(h);
    }
    // A decision that amends another acts on that one.
    let all = decisions.clone();
    for h in &mut decisions {
        let Some((_, e)) = events.iter().find(|(_, e)| e.name == h.record.event) else {
            continue;
        };
        if e.refers_to.contains_key(crate::stream::AMENDS) {
            match decision_of(e, crate::stream::AMENDS, &all, cell) {
                Ok(d) => h.decision = d,
                Err(m) => errors.push(m),
            }
        }
    }
    let mut out: Vec<ActionDefinition> = Vec::new();
    for (s, e) in &events {
        match e.stage.as_deref() {
            Some(DECISION) => out.extend(
                decisions
                    .iter()
                    .filter(|d| d.record.event == e.name)
                    .cloned(),
            ),
            Some(stage) => {
                // A later stage of the procedure of a decision article.
                let of: Vec<&ActionDefinition> = decisions
                    .iter()
                    .filter(|d| {
                        crate::action::procedure_of(service, &d.article).is_some_and(|p| {
                            let i = |n: &str| p.stages.iter().position(|x| x.name == n);
                            matches!((i(DECISION), i(stage)), (Some(a), Some(b)) if b > a)
                        })
                    })
                    .collect();
                if of.is_empty() {
                    errors.push(format!(
                        "event '{}': stage {stage} follows no decision of the actor",
                        e.name
                    ));
                }
                for d in &of {
                    let name = if of.len() == 1 {
                        e.name.clone()
                    } else {
                        format!("{}_{}", e.name, d.name)
                    };
                    let mut h = new(s, e, name, d.article.clone());
                    h.outputs = external_outputs(e, service, &d.article);
                    if h.outputs.is_empty() {
                        // It records nothing of the decision: the follow-up
                        // recomputes all of it (own choice of the plan).
                        h.outputs = d.outputs.clone();
                    }
                    let mut l = label(e, &establishing(e).unwrap_or_else(|| d.article.clone()));
                    if of.len() > 1 {
                        l.push_str(&format!(
                            " bij {}",
                            crate::form::readable(&d.name).to_lowercase()
                        ));
                    }
                    h.label = Some(l);
                    out.push(h);
                }
            }
            None => match fact(p, cell, s, e, service, &decisions) {
                Ok(Some(f)) => {
                    let mut h = new(s, e, e.name.clone(), f.article.clone());
                    h.outputs = f.outputs;
                    h.decision_parameter = f.parameter;
                    h.label = Some(label(e, &establishing(e).unwrap_or(f.article)));
                    if e.decision == Some(crate::stream::Decision::Follows) {
                        match decision_of(e, "", &decisions, cell) {
                            Ok(d) => h.decision = d,
                            Err(m) => errors.push(m),
                        }
                    }
                    out.push(h);
                }
                Ok(None) => warnings.push(format!(
                    "event '{}' has intake '{}', but no article apart from a decision reads it; it is not offered as an action",
                    e.name, e.intake
                )),
                Err(m) => errors.push(m),
            },
        }
    }
    for h in &mut out {
        h.rows = synthesis
            .action_rows
            .get(&h.name)
            .cloned()
            .unwrap_or_default();
    }
    for name in synthesis
        .action_rows
        .keys()
        .filter(|n| !out.iter().any(|h| &h.name == *n))
    {
        errors.push(format!(
            "synthesis.yaml: action_rows for '{name}' under '{}', which is no action of the process",
            cell.id()
        ));
    }
    if !errors.is_empty() {
        return Err(errors);
    }
    Ok((
        Some(Handling {
            worklist: LexostatusReference {
                cell: cell.id().to_string(),
                lexostatus: crate::reduction::WORKLIST.into(),
            },
            cases: LexostatusReference {
                cell: cell.id().to_string(),
                lexostatus: crate::reduction::CASES.into(),
            },
            actions: out,
        }),
        warnings,
    ))
}

/// The `$external` (or supplied) keys of the event that are outputs of the
/// article, in the order of the event.
fn external_outputs(e: &Event, service: &LawExecutionService, article: &str) -> Vec<String> {
    let outputs = crate::action::outputs_of_article(service, article);
    let mut out: Vec<String> = Vec::new();
    for l in e.leaves() {
        if let Binding::External(k) | Binding::Supplied(k) = l.binding {
            if outputs.contains(&k) && !out.contains(&k) {
                out.push(k);
            }
        }
    }
    out
}

/// The decision action a reference of `e` points to (`key`: that reference;
/// empty: the one that points to a decision). A reference to an event or an
/// article names its decision; one to a stage only if there is exactly one.
fn decision_of(
    e: &Event,
    key: &str,
    decisions: &[ActionDefinition],
    cell: &Cell,
) -> Result<Option<String>, String> {
    for (name, r) in &e.refers_to {
        if !key.is_empty() && name != key {
            continue;
        }
        if !points_to_decision(cell, &r.to) {
            continue;
        }
        let found: Vec<&ActionDefinition> = match &r.to {
            To::Event(ev) => decisions.iter().filter(|d| &d.record.event == ev).collect(),
            To::Article(a) => decisions.iter().filter(|d| &d.article == a).collect(),
            To::Stage(_) => decisions.iter().collect(),
        };
        return match found[..] {
            [one] => Ok(Some(one.name.clone())),
            _ => Err(format!(
                "event '{}': reference '{name}' points to {} decisions of the actor; name the article in refers_to (to: <regulation>#<article>)",
                e.name,
                found.len()
            )),
        };
    }
    Ok(None)
}

/// The article a fact executes, its outputs and the parameter that receives
/// the decision id (decision 12 of the plan): (a) a policy article of the
/// actor with a parameter of origin role BESLUIT that executes an article
/// in the legal basis of the event, for a fact that follows a decision;
/// otherwise (b) the article that reads the event, through the
/// lexostatuses that read it. `None`: no article apart from a decision
/// reads it.
/// The article a fact executes, with its outputs and the parameter that
/// receives the decision id.
#[derive(Debug, Clone)]
struct FactArticle {
    article: String,
    outputs: Vec<String>,
    parameter: Option<String>,
}

fn fact(
    p: &ActorPolicy,
    cell: &Cell,
    s: &Stream,
    e: &Event,
    service: &LawExecutionService,
    decisions: &[ActionDefinition],
) -> Result<Option<FactArticle>, String> {
    let resolver = service.resolver();
    let minus = |article: &str, outputs: Vec<String>| {
        let t = crate::action::assessments(service, article, e);
        outputs
            .into_iter()
            .filter(|o| !t.contains(o))
            .collect::<Vec<_>>()
    };
    // (a)
    if e.decision == Some(crate::stream::Decision::Follows) {
        let basis: Vec<String> = e
            .legal_basis
            .iter()
            .filter_map(|g| crate::regulations::parse(g).ok())
            .map(|g| g.article_ref())
            .collect();
        let mut found = Vec::new();
        for id in service.list_laws() {
            if crate::authority::authority_of_regulation(service, id).as_deref()
                != Some(p.authority.as_str())
            {
                continue;
            }
            let Some(law) = resolver.get_law(id) else {
                continue;
            };
            for a in &law.articles {
                if !a.get_executes().any(|x| basis.contains(&x.article)) {
                    continue;
                }
                let reference = format!("{id}#{}", a.number);
                let Some(parameter) = crate::action::decision_parameter_of(service, &reference)
                    .map_err(|m| format!("event '{}': {m}", e.name))?
                else {
                    continue;
                };
                let outputs = crate::action::outputs_of_article(service, &reference);
                found.push(FactArticle {
                    outputs: minus(&reference, outputs),
                    article: reference,
                    parameter: Some(parameter),
                });
            }
        }
        match &found[..] {
            [] => {}
            [one] => return Ok(Some(one.clone())),
            more => {
                return Err(format!(
                    "event '{}': more than one policy article of '{}' executes its legal basis with a parameter of origin role BESLUIT ({})",
                    e.name,
                    p.authority,
                    more.iter().map(|f| f.article.as_str()).collect::<Vec<_>>().join(", ")
                ))
            }
        }
    }
    // (b) Among the regulations of the event and those that name no other
    // authority than the actor's (an Awb, a BW, the actor's own; decision 2
    // of the controller).
    let of_event: BTreeSet<String> = e
        .legal_basis
        .iter()
        .chain(&e.establishes)
        .filter_map(|g| crate::regulations::parse(g).ok())
        .map(|g| g.regulation.to_string())
        .collect();
    let may = |id: &str| {
        of_event.contains(id)
            || crate::authority::authority_of_regulation(service, id)
                .is_none_or(|a| a == p.authority)
    };
    let mut candidates: Vec<(String, Vec<String>)> = Vec::new();
    for d in cell
        .lexostatuses
        .lexostatus_definitions
        .iter()
        .filter(|d| !d.is_list())
    {
        let names: BTreeSet<String> =
            crate::check::derivations_reading(d, &cell.streams, &s.id, &e.name)
                .into_iter()
                .collect();
        if names.is_empty() {
            continue;
        }
        let articles: Vec<String> = match &d.law {
            // A lexostatus from the law: what the reading article executes.
            Some(reading) => {
                let g = crate::regulations::parse(&reading.article).map_err(|m| {
                    format!(
                        "event '{}': lexostatus '{}' is read by '{}': {m}",
                        e.name, d.name, reading.article
                    )
                })?;
                resolver
                    .executes_of(g.regulation, g.article)
                    .into_iter()
                    .map(|x| x.target)
                    .collect()
            }
            // One of the cell: every article with a parameter it derives
            // from the event, apart from a decision.
            None => service
                .list_laws()
                .into_iter()
                .filter(|id| may(id))
                .filter_map(|id| resolver.get_law(id).map(|l| (id, l)))
                .flat_map(|(id, l)| l.articles.iter().map(move |a| (id, a)))
                .filter(|(_, a)| a.get_parameters().iter().any(|p| names.contains(&p.name)))
                .filter(|(_, a)| {
                    a.get_produces().and_then(|p| p.legal_character.as_deref())
                        != Some("BESCHIKKING")
                })
                .map(|(id, a)| format!("{id}#{}", a.number))
                .collect(),
        };
        for article in articles {
            let Ok(a) = crate::regulations::article(service, &article) else {
                continue;
            };
            let outputs = crate::action::outputs_depending_on(a, &names);
            if !outputs.is_empty() && !candidates.iter().any(|(c, _)| c == &article) {
                let outputs = minus(&article, outputs);
                candidates.push((article, outputs));
            }
        }
    }
    if candidates.len() > 1 {
        // The regulation of a decision of the actor wins.
        let preferred: Vec<(String, Vec<String>)> = candidates
            .iter()
            .filter(|(a, _)| {
                decisions
                    .iter()
                    .any(|d| a.starts_with(&format!("{}#", d.regulation)))
            })
            .cloned()
            .collect();
        if !preferred.is_empty() {
            candidates = preferred;
        }
    }
    match &candidates[..] {
        [] => Ok(None),
        [(a, o)] => Ok(Some(FactArticle {
            article: a.clone(),
            outputs: o.clone(),
            parameter: None,
        })),
        more => Err(format!(
            "event '{}': more than one article reads it ({}); narrow the reading in the cell or the law",
            e.name,
            more.iter().map(|(a, _)| a.as_str()).collect::<Vec<_>>().join(", ")
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::config::ProcessDefinition;
    use crate::derive::tests::{cell, derive, derived, errors_after, has, setup, setup_with};

    /// The actions follow from the streams, the stages and the law, with the
    /// renamings of the table in the plan (name = event name).
    #[test]
    fn the_actions_follow_from_the_streams_and_the_law() {
        type Row = (String, String, Vec<String>, Option<String>);
        let summary = |d: &ProcessDefinition| -> Vec<Row> {
            d.handling
                .as_ref()
                .unwrap()
                .actions
                .iter()
                .map(|h| {
                    (
                        h.name.clone(),
                        h.article.clone(),
                        h.outputs.clone(),
                        h.decision.clone(),
                    )
                })
                .collect()
        };
        let a = derived("test_afnemer");
        let s = summary(&a);
        assert_eq!(s[0].1, "testregeling_awb#5");
        assert_eq!(s[0].2, ["termijn_opgeschort"]);
        assert_eq!(s[1].1, "testregeling_afnemer#3");
        assert_eq!(
            s[1].2,
            [
                "vastgesteld_bedrag",
                "gebiedsbedrag",
                "besluit_tijdig",
                "besluitdeadline",
                "zorgvuldig"
            ]
        );
        assert_eq!(s[2].1, "testregeling_afnemer#3");
        assert_eq!(s[2].2, ["besluit_tijdig"]);
        assert_eq!(s[3].1, "testregeling_awb#3");
        assert_eq!(s[3].2, ["nog_te_betalen", "onverschuldigd_betaald"]);
        assert_eq!(s[3].3.as_deref(), Some("besluit_genomen"));
        let actions = &a.handling.as_ref().unwrap().actions;
        let labels: Vec<&str> = actions.iter().map(|h| h.label()).collect();
        assert_eq!(
            labels,
            [
                "Aanvulling gevraagd (Testregeling afnemer, artikel 3)",
                "Besluit genomen (Testregeling afnemer, artikel 3)",
                "Besluit bekendgemaakt (Testregeling verzuim, artikel 2)",
                "Betaling verricht (Testregeling verzuim, artikel 3)",
            ]
        );
        assert_eq!(actions[1].rows.len(), 1, "rows from synthesis.yaml");

        let t = derived("test_toeslag");
        let s = summary(&t);
        assert_eq!(s[2].1, "testregeling_toeslag#4");
        assert_eq!(s[2].3.as_deref(), Some("toeslag_vastgesteld"));
        assert_eq!(
            s[4].2,
            ["voorschot"],
            "a follow-up that records nothing of the decision recomputes all of it"
        );
        assert_eq!(s[4].1, "testregeling_toeslag#2");
        assert_eq!(s[8].1, "testregeling_toeslag#8");
        assert_eq!(s[8].2, ["nog_te_betalen_voorschot"]);
        assert_eq!(s[8].3.as_deref(), Some("voorschot_verleend"));
        assert_eq!(s[9].1, "testregeling_awb#6");
        assert_eq!(s[9].2, ["nog_terug_te_betalen"]);
        assert_eq!(s[9].3.as_deref(), Some("terugvordering_vastgesteld"));
        let actions = &t.handling.as_ref().unwrap().actions;
        assert_eq!(
            actions[0].label(),
            "Voorschot verleend (Testregeling maandtoeslag, artikel 2)"
        );
        assert_eq!(
            actions[4].label(),
            "Besluit bekendgemaakt (Testregeling verzuim, artikel 2) bij voorschot verleend"
        );
        // The kind is set later by `prepare_for` when the process loads; it
        // keeps the article the derivation set.
        assert!(actions
            .iter()
            .all(|h| h.kind == crate::config::ActionKind::default()));
        let (service, cells, _) = setup();
        for (mut d, kinds) in [
            (a, vec!["fact", "decision", "follow_up", "fact"]),
            (
                t,
                vec![
                    "decision",
                    "decision",
                    "decision",
                    "decision",
                    "follow_up",
                    "follow_up",
                    "follow_up",
                    "follow_up",
                    "fact",
                    "fact",
                ],
            ),
        ] {
            let before: Vec<String> = d
                .handling
                .as_ref()
                .unwrap()
                .actions
                .iter()
                .map(|h| h.article.clone())
                .collect();
            let cell = cells[&d.id].clone();
            let errors = crate::action::prepare_for(&mut d, &service, &cell);
            assert!(errors.is_empty(), "{}: {errors:?}", d.id);
            let actions = &d.handling.as_ref().unwrap().actions;
            let after: Vec<String> = actions.iter().map(|h| h.article.clone()).collect();
            assert_eq!(before, after, "{}", d.id);
            let got: Vec<String> = actions
                .iter()
                .map(|h| {
                    serde_json::to_value(&h.kind).unwrap()["kind"]
                        .as_str()
                        .unwrap()
                        .to_string()
                })
                .collect();
            assert_eq!(got, kinds, "{}", d.id);
        }
    }

    #[test]
    fn a_fact_no_article_reads_is_no_action_but_a_warning() {
        let (s, cells, d) = setup();
        let all = derive(&s, &cells, &d).unwrap();
        let a = all
            .iter()
            .find(|p| p.definition.id == "test_afnemer")
            .unwrap();
        assert!(
            a.warnings
                .iter()
                .any(|w| w.contains("'termijn_opgeschort'")),
            "{:?}",
            a.warnings
        );
        let h = a.definition.handling.as_ref().unwrap();
        assert!(h.action("termijn_opgeschort").is_none());
        // The other processes miss nothing.
        for p in all.iter().filter(|p| p.definition.id != "test_afnemer") {
            assert!(
                p.warnings.is_empty(),
                "{}: {:?}",
                p.definition.id,
                p.warnings
            );
        }
    }

    /// A reference to a stage with more than one decision of the actor does
    /// not say which; the runtime does not start.
    #[test]
    fn a_reference_to_one_of_more_decisions_names_the_article() {
        let e = errors_after(|_, cells, _| {
            let events = &mut cell(cells, "test_toeslag")
                .streams
                .iter_mut()
                .find(|s| s.id == "test_toeslag_zaakverloop")
                .unwrap()
                .events;
            let paid = events
                .iter_mut()
                .find(|e| e.name == "voorschot_betaald")
                .unwrap();
            paid.refers_to.get_mut("decision").unwrap().to = To::Stage("BESLUIT".into());
        });
        assert!(
            has(&e, &["'voorschot_betaald'", "4 decisions", "refers_to"]),
            "{e:?}"
        );
    }

    #[test]
    fn rows_for_no_action_stop_the_derivation() {
        let e = errors_after(|_, _, d| {
            let s = d.synthesis.get_mut("test_afnemer").unwrap();
            let rows = s.action_rows.remove("besluit_genomen").unwrap();
            s.action_rows.insert("besluit".into(), rows);
        });
        assert!(
            has(&e, &["synthesis.yaml", "'besluit'", "no action"]),
            "{e:?}"
        );
    }

    /// Policy of the toeslag actor that carries out the payment of an
    /// advance: an article with a parameter of origin role BESLUIT that
    /// executes the article in the legal basis of `voorschot_betaald`
    /// (testregeling_toeslag art. 8), like NAPP UB 15. Loaded only here, so
    /// that the other tests keep their action.
    const PAYING_POLICY: &str = r#"
$id: testbeleid_toeslag_betaling
name: Testbeleid betaling maandtoeslag
regulatory_layer: UITVOERINGSBELEID
publication_date: '2025-01-01'
valid_from: '2025-01-01'
competent_authority: {name: De Toeslagdienst van Voorbeeld}
articles:
  - number: '1'
    text: De dienst betaalt een voorschot overeenkomstig het besluit tot verlening ervan.
    machine_readable:
      executes:
        - {article: 'testregeling_toeslag#8'}
      execution:
        produces: {legal_character: TOETS, decision_type: GEEN_BESLUIT}
        parameters:
          - name: besluit
            type: string
            required: true
            origin: {waarde: DOSSIER, grondslag: 'testbeleid_toeslag_betaling#1', rol: BESLUIT}
        output:
          - {name: betaling_bij_besluit, type: boolean}
        actions:
          - {output: betaling_bij_besluit, value: true}
"#;

    /// Rule (a) of the facts: a fact that follows a decision executes the
    /// policy article of the actor whose parameter of origin role BESLUIT
    /// executes an article of the legal basis of the event; the decision id
    /// goes to that parameter, and the decision is the one the event refers
    /// to.
    #[test]
    fn a_fact_after_a_decision_executes_the_policy_with_rol_besluit() {
        let before = derived("test_toeslag");
        let h = before
            .handling
            .as_ref()
            .unwrap()
            .actions
            .iter()
            .find(|a| a.name == "voorschot_betaald")
            .unwrap();
        assert_eq!(h.article, "testregeling_toeslag#8");
        assert_eq!(h.decision_parameter, None);

        let (s, cells, d) = setup_with(&[PAYING_POLICY]);
        let p = derive(&s, &cells, &d)
            .unwrap()
            .into_iter()
            .find(|p| p.definition.id == "test_toeslag")
            .unwrap()
            .definition;
        let h = p
            .handling
            .as_ref()
            .unwrap()
            .actions
            .iter()
            .find(|a| a.name == "voorschot_betaald")
            .unwrap();
        assert_eq!(h.article, "testbeleid_toeslag_betaling#1");
        assert_eq!(h.regulation, "testbeleid_toeslag_betaling");
        assert_eq!(h.decision.as_deref(), Some("voorschot_verleend"));
        assert_eq!(h.decision_parameter.as_deref(), Some("besluit"));
        assert_eq!(h.outputs, ["betaling_bij_besluit"]);
    }
}
