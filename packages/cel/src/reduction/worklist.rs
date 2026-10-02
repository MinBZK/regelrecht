//! The worklist (RFC-047): the roots of the submissions in a cell on which
//! nothing has been decided yet (stage BESLUIT, RFC-008), the applications
//! "waarop nog niet is besloten" (DVB art. 3 lid 1). The runtime offers it
//! for every cell with submissions, as a list lexostatus with the reduction
//! DSL, so it goes through the same checks and the same reduction as one
//! from `lexostatuses.yaml`; no cell defines the name itself.
//!
//! Its columns are derived, never configured per cell: the moment of receipt
//! and of recording, the owner field of the portal channel of the policy (who
//! applied), and the field of the submission whose parameter has origin role
//! `TIJDVAK` (the window of the requested decision, Awb 4:2 lid 1).

use std::collections::BTreeMap;

use chrono::NaiveDate;
use regelrecht_engine::LawExecutionService;
use regelrecht_law_model::OriginRole;
use serde_json::{json, Map, Value};

use super::{LexostatusDefinition, Period};
use crate::stream::{Binding, Event, Stream, DECISION};

/// The name of the worklist. Reserved.
pub const WORKLIST: &str = "worklist";

/// A column of the worklist beyond the moments: its name and derivation.
pub type Column = (String, Value);

/// The worklist of the chronicle with submissions, if the cell has them in
/// exactly one chronicle, with `columns` after the moments. Without a
/// decision event in that chronicle nothing is left out: nothing has been
/// decided.
pub fn worklist_definition(streams: &[Stream], columns: &[Column]) -> Option<LexostatusDefinition> {
    let mut chronicles: Vec<&str> = streams
        .iter()
        .filter(|s| s.events.iter().any(is_submission))
        .map(|s| s.chronicle.as_str())
        .collect();
    chronicles.sort_unstable();
    chronicles.dedup();
    let [chronicle] = chronicles[..] else {
        return None;
    };
    let decides = streams
        .iter()
        .filter(|s| s.chronicle == chronicle)
        .flat_map(|s| &s.events)
        .any(|e| e.stage.as_deref() == Some(DECISION));
    let mut derivations = Map::new();
    derivations.insert("ontvangen_op".into(), json!({"moment": "effective_at"}));
    derivations.insert("vastgelegd_op".into(), json!({"moment": "recorded_at"}));
    for (name, d) in columns {
        derivations.insert(name.clone(), d.clone());
    }
    let mut reduction = json!({
        "chronicle": chronicle,
        "filter": {"type": "submission"},
        "group_by": "root",
        "pick": "latest",
        "derivations": derivations,
    });
    if decides {
        reduction["without"] = json!({"stage": DECISION});
    }
    serde_json::from_value(json!({"name": WORKLIST, "inputs": [], "reduction": reduction})).ok()
}

fn is_submission(e: &Event) -> bool {
    e.type_ == "submission"
}

/// The columns of the worklist of a cell beyond the moments, from the law:
/// the owner field of the portal channel whose `submits` the submission
/// establishes, and the field with origin role `TIJDVAK`. A policy that
/// cannot be read gives no owner column; the derivation of the process
/// reports it.
pub fn worklist_columns(
    streams: &[Stream],
    service: &LawExecutionService,
    date: Option<NaiveDate>,
) -> Result<Vec<Column>, String> {
    let submissions: Vec<&Event> = streams
        .iter()
        .flat_map(|s| &s.events)
        .filter(|e| is_submission(e))
        .collect();
    let policies = crate::policy::read(service, date).unwrap_or_default();
    let portal = policies
        .values()
        .flat_map(|p| p.channels.iter().map(move |c| (p, c)))
        .filter_map(|(p, c)| {
            let submits = c.def.submits.as_deref()?;
            let e = submissions
                .iter()
                .find(|e| crate::derive::establishing(e).as_deref() == Some(submits))?;
            Some((p, c, *e))
        })
        .next();
    // The submission the columns come from: the one of the portal, otherwise
    // the only one.
    let event = match (portal, &submissions[..]) {
        (Some((_, _, e)), _) => e,
        (None, [one]) => *one,
        _ => return Ok(Vec::new()),
    };
    let mut out = Vec::new();
    if let Some((p, c, _)) = portal {
        if let Some(owner) = &c.def.owner {
            let none = BTreeMap::new();
            let supplies = p.supplies.get(&c.id).map_or(&none, |(_, s)| s);
            let path = crate::channel::owner_path(&c.id, owner, supplies);
            let bound: Vec<String> = event
                .leaves()
                .into_iter()
                .filter(|l| {
                    l.binding == Binding::Intake(path.clone())
                        || l.binding == Binding::Supplied(path.clone())
                })
                .map(|l| l.path)
                .collect();
            if let Some(field) = bound.first() {
                out.push((owner.clone(), json!({"field": field})));
            }
        }
    }
    if let Some(column) = window(event, service)? {
        out.push(column);
    }
    Ok(out)
}

/// The field of the submission whose parameter, in a regulation that takes
/// part in the event, has origin role `TIJDVAK`; a period if the law gives
/// it a `temporal.period_type`, otherwise its value.
fn window(event: &Event, service: &LawExecutionService) -> Result<Option<Column>, String> {
    let mut regulations: Vec<String> = event
        .establishes
        .iter()
        .chain(&event.legal_basis)
        .chain(event.field_defs.iter().map(|f| &f.declared_by))
        .filter_map(|g| crate::regulations::parse(g).ok())
        .map(|g| g.regulation.to_string())
        .collect();
    regulations.sort();
    regulations.dedup();
    // Per parameter name with role TIJDVAK: its period type, if any.
    let mut windows: BTreeMap<String, Option<String>> = BTreeMap::new();
    for r in &regulations {
        let Some(law) = service.resolver().get_law(r) else {
            continue;
        };
        for a in &law.articles {
            for p in a.get_parameters() {
                let role = p
                    .origin
                    .as_ref()
                    .and_then(|o| o.as_valid())
                    .and_then(|o| o.rol);
                if role == Some(OriginRole::Tijdvak) {
                    let t = p.temporal.as_ref().and_then(|t| t.period_type.clone());
                    let entry = windows.entry(p.name.clone()).or_insert(None);
                    if entry.is_none() {
                        *entry = t;
                    }
                }
            }
        }
    }
    let mut found: Vec<(String, String)> = Vec::new();
    for l in event.leaves() {
        let key = match &l.binding {
            Binding::External(k) | Binding::Supplied(k) => k.clone(),
            _ => continue,
        };
        let name = key.rsplit('.').next().unwrap_or(&key).to_string();
        if windows.contains_key(&name) && !found.iter().any(|(_, p)| p == &l.path) {
            found.push((name, l.path));
        }
    }
    match &found[..] {
        [] => Ok(None),
        [(name, path)] => {
            let period = windows[name].as_deref().and_then(Period::from_period_type);
            let d = match period {
                Some(p) => json!({"period_of": path, "period": p}),
                None => json!({"field": path}),
            };
            Ok(Some((name.clone(), d)))
        }
        more => Err(format!(
            "worklist: more than one field of submission '{}' has origin role TIJDVAK ({})",
            event.name,
            more.iter()
                .map(|(_, p)| p.as_str())
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    #[test]
    fn a_cell_with_submissions_gets_the_worklist() {
        let s = crate::stream::parse(
            "$id: s\nrecording_actor: a\nchronicle: k\nevents:\n  - {name: aanvraag_ontvangen, intake: portaal, legal_basis: ['r#1'], type: submission, subtype: aanvraag, fields: {x: $external.x}}\n  - {name: besluit, intake: medewerker, legal_basis: ['r#2'], type: decretogram, stage: BESLUIT, fields: {y: $external.y}}\n",
            "t",
        )
        .unwrap();
        let d = worklist_definition(
            std::slice::from_ref(&s),
            &[("x".into(), json!({"field": "x"}))],
        )
        .unwrap();
        assert_eq!(d.name, WORKLIST);
        assert!(d.is_list());
        assert_eq!(d.reduction.chronicle, "k");
        let v = serde_json::to_value(&d).unwrap();
        assert_eq!(v["reduction"]["without"], json!({"stage": "BESLUIT"}));
        let columns: Vec<&String> = d.reduction.derivations.keys().collect();
        assert_eq!(columns, ["ontvangen_op", "vastgelegd_op", "x"]);
        let none = crate::stream::parse("$id: s\nrecording_actor: a\nchronicle: k\nevents:\n  - {name: x, intake: i, legal_basis: ['r#1'], type: executogram, fields: {y: $external.y}}\n", "t").unwrap();
        assert!(worklist_definition(&[none], &[]).is_none());
    }

    /// The columns of the fixtures follow from the policy and the law: the
    /// owner of the portal channel and the window of the submission.
    #[test]
    fn the_columns_follow_from_policy_and_law() {
        let (_, cells, _) = crate::derive::tests::setup();
        let columns = |cell: &str| -> Vec<(String, Value)> {
            let d = cells[cell].lexostatuses.lexostatus(WORKLIST).unwrap();
            d.reduction
                .derivations
                .iter()
                .map(|(n, a)| (n.clone(), serde_json::to_value(&a.derivation).unwrap()))
                .collect()
        };
        let without = |cell: &str| {
            serde_json::to_value(cells[cell].lexostatuses.lexostatus(WORKLIST).unwrap()).unwrap()
                ["reduction"]["without"]
                .clone()
        };
        let a = columns("test_afnemer");
        let names: Vec<&str> = a.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["kvk", "ontvangen_op", "vastgelegd_op"]);
        assert_eq!(a[0].1["field"], "core.signed_via.kvk_nummer");
        assert_eq!(without("test_afnemer"), json!({"stage": "BESLUIT"}));
        let t = columns("test_toeslag");
        let names: Vec<&str> = t.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(names, ["maand", "nummer", "ontvangen_op", "vastgelegd_op"]);
        assert_eq!(t[0].1, json!({"period_of": "maand", "period": "month"}));
        // The supplied field the portal checks the owner on (`owner_path`).
        assert_eq!(t[1].1["field"], "ondertekening");
        assert_eq!(without("test_toeslag"), json!({"stage": "BESLUIT"}));
        let i = columns("test_instantie");
        let names: Vec<&str> = i.iter().map(|(n, _)| n.as_str()).collect();
        assert_eq!(
            names,
            ["aanvraagjaar", "kvk", "ontvangen_op", "vastgelegd_op"]
        );
        assert_eq!(i[0].1["field"], "content.aanvraagjaar");
        assert!(without("test_instantie").is_null());
        assert!(cells["test_register"]
            .lexostatuses
            .lexostatus(WORKLIST)
            .is_none());
    }

    /// Without a decision in the chronicle every application is undecided.
    #[test]
    fn without_a_decision_nothing_is_left_out() {
        let s = crate::stream::parse(
            "$id: s\nrecording_actor: a\nchronicle: k\nevents:\n  - {name: aanvraag_ontvangen, intake: portaal, legal_basis: ['r#1'], type: submission, subtype: aanvraag, fields: {x: $external.x}}\n",
            "t",
        )
        .unwrap();
        let v = serde_json::to_value(worklist_definition(&[s], &[]).unwrap()).unwrap();
        assert!(
            v["reduction"].get("without").is_none_or(Value::is_null),
            "{v}"
        );
    }
}
