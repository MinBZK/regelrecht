//! The worklist (RFC-047): the cases of the submissions in a cell on which
//! not every requested decision has been taken yet (in the NAPP corpus the
//! policy calls them the applications "waarop nog niet is besloten"). The
//! runtime offers it for every cell with a submission, as a list lexostatus
//! with the reduction DSL, so it goes through the same checks and the same
//! reduction as one from `lexostatuses.yaml`; no cell defines the name itself.
//!
//! **Which cases (per requested decision).** A case stays until every
//! decision on the submission has been taken: the articles that name its
//! establishing article in `decides_on` (`Event::decided_by`), or without
//! those the articles that establish a decision (stage BESLUIT) in its
//! chronicle. With exactly one such article a decision gram takes the case
//! off: `without: {stage: BESLUIT}`. With more than one (for example an
//! advance and a final determination) the case should leave only when each
//! has a BESLUIT gram. The reduction DSL cannot
//! say that (`without` is one filter, and any gram through it takes the case
//! off), so such a worklist leaves out `without` and keeps every case: a
//! known limitation, until the DSL can require a gram per article.
//!
//! **Which submission.** The one whose establishing article a portal channel
//! of the policy `submits`; without a portal, the only submission of the
//! cell. The filter names that event.
//!
//! **Columns**, derived and never configured per cell: the moment of receipt
//! (`received_at`) and of recording (`recorded_at`), the field of the
//! submission the portal channel names as
//! `owner` (who follows the case, see [`crate::channel::owner_binding`]), and
//! the field whose parameter has origin role `TIJDVAK` (the window of the
//! requested decision, Awb 4:2 lid 1). A derived column with the name of
//! another is an error, not a silent overwrite.
//!
//! **All cases.** Next to the worklist the runtime offers the list `cases`:
//! every case of the same submission, decided or not, with the same columns
//! and `decided_at`, the date of the latest decision gram (stage BESLUIT;
//! null without one). A chronicle without a decision event has no
//! `decided_at` column: the checks refuse a filter that selects no event. A
//! handler reaches a decided case through it, for what follows the decision
//! (the announcement, the payment). The name is reserved as well.

use std::collections::{BTreeMap, BTreeSet};

use chrono::NaiveDate;
use regelrecht_engine::LawExecutionService;
use regelrecht_law_model::OriginRole;
use serde_json::{json, Map, Value};

use super::{LexostatusDefinition, Period};
use crate::policy::{ActorPolicy, DeclaredChannel};
use crate::stream::{Binding, Event, Stream, DECISION};

/// The name of the worklist. Reserved.
pub const WORKLIST: &str = "worklist";

/// The name of the list of all cases. Reserved.
pub const CASES: &str = "cases";

/// The column of `cases` with the date of the decision.
const DECIDED: &str = "decided_at";

/// The column with the moment of receipt (`effective_at`).
const RECEIVED: &str = "received_at";

/// The column with the moment of recording.
const RECORDED: &str = "recorded_at";

/// A column of the worklist beyond the moments: its name and derivation.
pub type Column = (String, Value);

/// The worklist and the list of all cases of a cell, if it has a
/// submission (see the module); empty without one.
pub fn worklist(
    streams: &[Stream],
    service: &LawExecutionService,
    date: Option<NaiveDate>,
) -> Result<Vec<LexostatusDefinition>, String> {
    // A cell without a submission has no worklist, whatever its policy says.
    if !streams
        .iter()
        .any(|s| s.events.iter().any(Event::is_submission))
    {
        return Ok(Vec::new());
    }
    let policies = crate::policy::read(service, date)
        .map_err(|e| format!("worklist: the policy cannot be read ({})", e.join("; ")))?;
    let Some((portal, stream, event)) = submission(streams, &policies)? else {
        return Ok(Vec::new());
    };
    let mut columns = Vec::new();
    if let Some((p, c)) = portal {
        if let Some(owner) = &c.def.owner {
            let none = BTreeMap::new();
            let supplies = p.supplies.get(&c.id).map_or(&none, |(_, s)| s);
            let o = crate::channel::owner_binding(owner, &c.id, supplies, event)
                .map_err(|e| format!("worklist: {}: channel '{}': {e}", c.article, c.id))?;
            columns.push((owner.clone(), json!({"field": o.field})));
        }
    }
    if let Some(column) = window(event, service)? {
        columns.push(column);
    }
    Ok(vec![
        worklist_definition(&stream.chronicle, event, streams, &columns)?,
        cases_definition(&stream.chronicle, event, streams, &columns)?,
    ])
}

/// The portal channel (with its policy) and the submission of the worklist:
/// the submission whose establishing article a channel `submits`, otherwise
/// the only submission of the cell.
#[allow(clippy::type_complexity)]
fn submission<'a>(
    streams: &'a [Stream],
    policies: &'a BTreeMap<String, ActorPolicy>,
) -> Result<
    Option<(
        Option<(&'a ActorPolicy, &'a DeclaredChannel)>,
        &'a Stream,
        &'a Event,
    )>,
    String,
> {
    let submissions: Vec<(&Stream, &Event)> = streams
        .iter()
        .flat_map(|s| s.events.iter().map(move |e| (s, e)))
        .filter(|(_, e)| e.is_submission())
        .collect();
    let mut found = Vec::new();
    for p in policies.values() {
        for c in &p.channels {
            let Some(submits) = c.def.submits.as_deref() else {
                continue;
            };
            for (s, e) in &submissions {
                if crate::derive::establishing(e).as_deref() == Some(submits) {
                    found.push((Some((p, c)), *s, *e));
                }
            }
        }
    }
    match (&found[..], &submissions[..]) {
        ([one], _) => Ok(Some(*one)),
        ([], [(s, e)]) => Ok(Some((None, s, e))),
        ([], _) => Ok(None),
        (more, _) => Err(format!(
            "worklist: {} portal channels submit a submission of this cell ({}); one portal per cell",
            more.len(),
            more.iter()
                .filter_map(|(c, _, e)| c.map(|(_, c)| format!("'{}' ({})", c.id, e.name)))
                .collect::<Vec<_>>()
                .join(", ")
        )),
    }
}

/// The articles whose decision the submission asks for: `decided_by`, or
/// without it those that establish a decision in its chronicle.
fn requested_decisions(chronicle: &str, event: &Event, streams: &[Stream]) -> BTreeSet<String> {
    if !event.decided_by.is_empty() {
        return event.decided_by.iter().cloned().collect();
    }
    streams
        .iter()
        .filter(|s| s.chronicle == chronicle)
        .flat_map(|s| &s.events)
        .filter(|e| e.stage.as_deref() == Some(DECISION))
        .filter_map(crate::derive::establishing)
        .collect()
}

/// The worklist of the submission `event` in `chronicle`, with `columns`
/// after the moments. A case leaves at a decision only when the submission
/// asks for exactly one (see the module).
pub fn worklist_definition(
    chronicle: &str,
    event: &Event,
    streams: &[Stream],
    columns: &[Column],
) -> Result<LexostatusDefinition, String> {
    let mut reduction = list_reduction(WORKLIST, chronicle, event, columns)?;
    if requested_decisions(chronicle, event, streams).len() == 1 {
        reduction["without"] = json!({"stage": DECISION});
    }
    list_definition(WORKLIST, reduction)
}

/// Every case of the submission `event` in `chronicle`, decided or not:
/// the columns of the worklist and, when the chronicle has a decision event,
/// `decided_at`, the date of the latest decision gram of the case (null
/// without one).
pub fn cases_definition(
    chronicle: &str,
    event: &Event,
    streams: &[Stream],
    columns: &[Column],
) -> Result<LexostatusDefinition, String> {
    let mut reduction = list_reduction(CASES, chronicle, event, columns)?;
    let decides = streams
        .iter()
        .filter(|s| s.chronicle == chronicle)
        .flat_map(|s| &s.events)
        .any(|e| e.stage.as_deref() == Some(DECISION));
    if decides {
        if reduction["derivations"].get(DECIDED).is_some() {
            return Err(collision(CASES, DECIDED));
        }
        reduction["derivations"][DECIDED] = json!({
        "filter": {"stage": DECISION},
        "pick": "latest",
        "moment": "effective_at",
        "no_gram": null,
        });
    }
    list_definition(CASES, reduction)
}

fn collision(list: &str, column: &str) -> String {
    format!("{list}: two columns are called '{column}'; rename the field of the submission")
}

/// A list of the cases of the submission `event`, one row per root, with
/// the moments and `columns`. Two columns with one name are an error.
fn list_reduction(
    list: &str,
    chronicle: &str,
    event: &Event,
    columns: &[Column],
) -> Result<Value, String> {
    let mut derivations = Map::new();
    derivations.insert(RECEIVED.into(), json!({"moment": "effective_at"}));
    derivations.insert(RECORDED.into(), json!({"moment": "recorded_at"}));
    for (name, d) in columns {
        if derivations.insert(name.clone(), d.clone()).is_some() {
            return Err(collision(list, name));
        }
    }
    Ok(json!({
        "chronicle": chronicle,
        "filter": {"name": event.name},
        "group_by": "root",
        "pick": "latest",
        "derivations": derivations,
    }))
}

fn list_definition(name: &str, reduction: Value) -> Result<LexostatusDefinition, String> {
    serde_json::from_value(json!({"name": name, "inputs": [], "reduction": reduction}))
        .map_err(|e| format!("{name}: the runtime's list is not a lexostatus: {e}"))
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

    const APPLICATION: &str = "  - {name: aanvraag_ontvangen, intake: portaal, legal_basis: ['r#1'], type: submission, subtype: aanvraag, fields: {x: $external.x}}\n";

    fn stream(events: &str) -> Stream {
        crate::stream::parse(
            &format!("$id: s\nrecording_actor: a\nchronicle: k\nevents:\n{events}"),
            "t",
        )
        .unwrap()
    }

    fn definition(s: &Stream) -> Value {
        let d = worklist_definition(
            "k",
            &s.events[0],
            std::slice::from_ref(s),
            &[("x".into(), json!({"field": "x"}))],
        )
        .unwrap();
        assert_eq!(d.name, WORKLIST);
        assert!(d.is_list());
        serde_json::to_value(&d).unwrap()
    }

    /// A derived column with the name of another is an error, not an
    /// overwrite.
    #[test]
    fn two_columns_with_one_name_are_an_error() {
        let s = stream(APPLICATION);
        let f = worklist_definition(
            "k",
            &s.events[0],
            std::slice::from_ref(&s),
            &[("received_at".into(), json!({"field": "x"}))],
        )
        .unwrap_err();
        assert!(f.contains("two columns are called 'received_at'"), "{f}");
    }

    /// A cell without a submission has no worklist, also when the policy
    /// cannot be read: its policy is not the worklist's business.
    #[test]
    fn a_cell_without_a_submission_does_not_read_the_policy() {
        let s = stream("  - {name: ingeschreven, intake: medewerker, legal_basis: ['r#1'], type: act, fields: {x: $external.x}}\n");
        let mut service = LawExecutionService::new();
        service
            .load_law(
                "$id: kapot_beleid\nregulatory_layer: MINISTERIELE_REGELING\npublication_date: '2025-01-01'\ncompetent_authority: {name: X}\narticles:\n  - number: '1'\n    text: x\n    machine_readable:\n      execution:\n        produces:\n          legal_character: TOETS\n          decision_type: GEEN_BESLUIT\n          extensions:\n            chronolex:\n              channels:\n                medewerker: {kind: handling, role: behandelaar, legal_basis: 'kapot_beleid#1'}\n",
            )
            .unwrap();
        assert!(crate::policy::read(&service, None).is_err());
        assert_eq!(worklist(&[s], &service, None).unwrap().len(), 0);
    }

    /// The list of all cases keeps every case and adds the date of the
    /// decision, read over the whole case.
    #[test]
    fn cases_keeps_every_case_and_says_when_it_was_decided() {
        let s = stream(&format!(
            "{APPLICATION}  - {{name: besluit, intake: medewerker, legal_basis: ['r#2'], type: decretogram, stage: BESLUIT, fields: {{y: $external.y}}}}\n"
        ));
        let d = cases_definition(
            "k",
            &s.events[0],
            std::slice::from_ref(&s),
            &[("x".into(), json!({"field": "x"}))],
        )
        .unwrap();
        assert_eq!(d.name, CASES);
        assert!(d.is_list());
        let v = serde_json::to_value(&d).unwrap();
        assert!(
            v["reduction"].get("without").is_none_or(Value::is_null),
            "{v}"
        );
        assert_eq!(
            v["reduction"]["filter"],
            json!({"name": "aanvraag_ontvangen"})
        );
        let d = &v["reduction"]["derivations"];
        let columns: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(columns, ["decided_at", "received_at", "recorded_at", "x"]);
        assert_eq!(
            d["decided_at"],
            json!({"filter": {"stage": "BESLUIT"}, "pick": "latest", "moment": "effective_at", "no_gram": null})
        );
    }

    #[test]
    fn one_requested_decision_takes_the_case_off() {
        let s = stream(&format!(
            "{APPLICATION}  - {{name: besluit, intake: medewerker, legal_basis: ['r#2'], type: decretogram, stage: BESLUIT, fields: {{y: $external.y}}}}\n"
        ));
        let v = definition(&s);
        assert_eq!(v["reduction"]["chronicle"], "k");
        assert_eq!(
            v["reduction"]["filter"],
            json!({"name": "aanvraag_ontvangen"})
        );
        assert_eq!(v["reduction"]["without"], json!({"stage": "BESLUIT"}));
        let columns: Vec<&String> = v["reduction"]["derivations"]
            .as_object()
            .unwrap()
            .keys()
            .collect();
        assert_eq!(columns, ["received_at", "recorded_at", "x"]);
    }

    /// Without a decision in the chronicle every application is undecided;
    /// with two requested decisions the DSL cannot say "each of them", so
    /// the case stays (the known limitation of the module).
    #[test]
    fn no_or_several_requested_decisions_take_nothing_off() {
        for events in [
            APPLICATION.to_string(),
            format!(
                "{APPLICATION}  - {{name: b1, intake: medewerker, legal_basis: ['r#2'], type: decretogram, stage: BESLUIT, fields: {{y: $external.y}}}}\n  - {{name: b2, intake: medewerker, legal_basis: ['r#3'], type: decretogram, stage: BESLUIT, fields: {{z: $external.z}}}}\n"
            ),
        ] {
            let v = definition(&stream(&events));
            assert!(v["reduction"].get("without").is_none_or(Value::is_null), "{v}");
        }
    }

    /// The columns of the fixtures follow from the policy and the law: the
    /// field of the submission the portal channel names as owner, and the
    /// window.
    #[test]
    fn the_columns_follow_from_policy_and_law() {
        let (_, cells, _) = crate::derive::tests::setup();
        let def = |cell: &str| {
            serde_json::to_value(cells[cell].lexostatuses.lexostatus(WORKLIST).unwrap()).unwrap()
        };
        let a = def("test_afnemer");
        let d = &a["reduction"]["derivations"];
        let names: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(names, ["kvk_nummer", "received_at", "recorded_at"]);
        assert_eq!(d["kvk_nummer"]["field"], "core.signed_via.kvk_nummer");
        assert_eq!(
            a["reduction"]["filter"],
            json!({"name": "aanvraag_ontvangen"})
        );
        assert_eq!(a["reduction"]["without"], json!({"stage": "BESLUIT"}));
        let t = def("test_toeslag");
        let d = &t["reduction"]["derivations"];
        let names: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(
            names,
            ["maand", "persoonsnummer", "received_at", "recorded_at"]
        );
        assert_eq!(d["maand"], json!({"period_of": "maand", "period": "month"}));
        assert_eq!(d["persoonsnummer"]["field"], "persoonsnummer");
        // The voorschot and the vaststelling are both asked for (decides_on).
        assert!(t["reduction"]["without"].is_null(), "{t}");
        let i = def("test_instantie");
        let d = &i["reduction"]["derivations"];
        let names: Vec<&String> = d.as_object().unwrap().keys().collect();
        assert_eq!(
            names,
            ["aanvraagjaar", "kvk_nummer", "received_at", "recorded_at"]
        );
        assert_eq!(d["aanvraagjaar"]["field"], "content.aanvraagjaar");
        assert!(i["reduction"]["without"].is_null());
        for name in [WORKLIST, CASES] {
            assert!(cells["test_register"]
                .lexostatuses
                .lexostatus(name)
                .is_none());
        }
        // The list of all cases has the columns of the worklist and
        // `decided_at`; the instantie has no decision, so no `decided_at`.
        for (cell, decides) in [
            ("test_afnemer", true),
            ("test_toeslag", true),
            ("test_instantie", false),
        ] {
            let w = def(cell);
            let c =
                serde_json::to_value(cells[cell].lexostatuses.lexostatus(CASES).unwrap()).unwrap();
            let keys = |v: &Value| -> Vec<String> {
                v["reduction"]["derivations"]
                    .as_object()
                    .unwrap()
                    .keys()
                    .cloned()
                    .collect()
            };
            let mut expected = keys(&w);
            if decides {
                expected.push(DECIDED.to_string());
            }
            expected.sort();
            let got = keys(&c);
            assert_eq!(got, expected, "{cell}");
            assert!(
                c["reduction"].get("without").is_none_or(Value::is_null),
                "{cell}"
            );
        }
    }
}
