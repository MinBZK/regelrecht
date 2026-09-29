//! The state of a case: a lexostatus the runtime offers for every cell with a
//! case, under the name [`CASE_STATE`].
//!
//! A process that acts in a case must know how far that case has come: which
//! decisions are in it and which stages each decision went through (RFC-008),
//! how often each event is recorded in it, what each decision recorded (the
//! state from which a later stage of that decision continues), the latest
//! `effective_at`, and whether an applicant knows the case. Those are
//! derivations from the grams of the case, and deriving is reduction: that
//! happens in the cell (paper P:58, P:66), not in the process. The process
//! reads this lexostatus, not grams.
//!
//! The name has stayed; since chronolex v0.2.0 "the case" is the group around
//! a root: the gram without a reference (the application) with everything
//! that follows from it through references. A decision is a gram with stage
//! BESLUIT; whatever refers to it follows it.
//!
//! The runtime offers it, not `lexostatuses.yaml`: the group (the root, the
//! references and a stage per decision) is a concept of the runtime itself,
//! not of a case study, and what a process asks about it is the same for every
//! cell. A separate definition per cell would repeat the same rules in every
//! cell, and the process would depend on their name and shape. A cell
//! therefore cannot use the name itself.
//!
//! The lexostatus never goes to the engine: it has no parameters, only extra
//! fields ([`CaseState`]).

use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use super::{AsOf, Lexostatus};
use crate::gram::Gram;

/// The name of the lexostatus. Reserved: no cell defines it itself.
pub const CASE_STATE: &str = "case_state";

/// The optional inputs for the question whether someone knows the case: the
/// `$intake` path of the owner field of a channel, and the value of whoever
/// asks.
pub const OWNER_PATH: &str = "owner_path";
pub const OWNER: &str = "owner";

/// What the cell derives about a case. It is in the lexostatus as extra
/// fields.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct CaseState {
    /// How many grams the group has. A process sends this along as
    /// `root_grams` when recording: what it computed held for the case as it
    /// was then.
    pub grams: usize,
    /// Per event (`<stream>/<event>`) how many grams are in the case.
    pub events: BTreeMap<String, usize>,
    /// The latest `effective_at` in the case: a fact that follows the case
    /// does not legally lie on an earlier day.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub latest_effective_at: Option<String>,
    /// Per stage (RFC-008) the gram that recorded it, for the stages that
    /// belong to no decision (such as the application). Each is in the case
    /// once; the cell enforces that.
    pub stages: BTreeMap<String, StageState>,
    /// The decisions in the case, in the order in which the cell recorded
    /// them: per decision its stages and the grams that follow it.
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub decisions: Vec<DecisionState>,
    /// Only if asked for: whether there is a gram in the case whose field
    /// bound to the owner path has the requested value.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub owner: Option<bool>,
}

/// A stage in the case: what the gram of that stage recorded. For a decision
/// that is the state from which a later stage continues (RFC-008: the
/// decision is the state container).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct StageState {
    pub stream: String,
    pub event: String,
    pub effective_at: String,
    pub recorded_at: String,
    /// The regulation a decision rests on, if the gram names it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regulation: Option<String>,
    /// The fields of the gram: for a decision, its outputs.
    pub fields: Map<String, Value>,
    /// The values of the input the engine computed it with.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub input: BTreeMap<String, Value>,
}

/// A decision in the case (RFC-008: the decision is the state container): the
/// gram that opened or amended it, the stages it went through, and how many
/// grams of each event follow it (such as payments that execute it).
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct DecisionState {
    /// The id of the gram that is the decision.
    pub id: String,
    /// The event that recorded the decision.
    pub stream: String,
    pub event: String,
    /// The decision this decision amends, if it is an amendment.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub amends: Option<String>,
    /// Per stage the gram that recorded it for this decision; the stage of the
    /// decision itself (such as BESLUIT) carries its outputs and input.
    pub stages: BTreeMap<String, StageState>,
    /// Per event (`<stream>/<event>`) how many grams follow this decision,
    /// the decision itself included.
    pub events: BTreeMap<String, usize>,
}

impl DecisionState {
    /// The stage in which the decision was taken: that of the gram that
    /// opened it.
    pub fn taken(&self) -> Option<(&String, &StageState)> {
        self.stages
            .iter()
            .find(|(_, s)| s.event == self.event && s.stream == self.stream)
    }

    /// Whether the decision was recorded by this event.
    pub fn of(&self, stream: &str, event: &str) -> bool {
        self.stream == stream && self.event == event
    }
}

impl CaseState {
    /// The decision with this id.
    pub fn decision(&self, id: &str) -> Option<&DecisionState> {
        self.decisions.iter().find(|b| b.id == id)
    }

    /// The key of an event in [`CaseState::events`].
    pub fn key(stream: &str, event: &str) -> String {
        format!("{stream}/{event}")
    }

    /// How many grams of this event are in the case.
    pub fn count(&self, stream: &str, event: &str) -> usize {
        self.events
            .get(&Self::key(stream, event))
            .copied()
            .unwrap_or(0)
    }

    /// The state as a lexostatus: everything as an extra field.
    pub fn as_lexostatus(&self, root: &str, as_of: &AsOf) -> Result<Lexostatus, String> {
        let Value::Object(fields) = serde_json::to_value(self).map_err(|e| e.to_string())? else {
            return Err("the case state is not an object".into());
        };
        Ok(Lexostatus {
            root: Some(root.to_string()),
            as_of: as_of.as_of.map(|t| t.to_string()),
            known_at: as_of.known_at.map(|t| t.to_string()),
            extra_fields: fields.into_iter().collect(),
            ..Lexostatus::empty(CASE_STATE)
        })
    }

    /// The state from the lexostatus the cell gave.
    pub fn out(l: &Lexostatus) -> Result<Self, String> {
        let fields: Map<String, Value> = l.extra_fields.clone().into_iter().collect();
        serde_json::from_value(Value::Object(fields))
            .map_err(|e| format!("the cell gave no case state: {e}"))
    }
}

/// Derive the state of a case from its grams, as of an as-of point. `binds`
/// says per gram which fields bind to an `$intake` path (from the stream of
/// the cell); `owner` is the path and the value being asked about. `None`: no
/// gram of the case counts at this as-of point.
pub fn reduce_case<'g>(
    grams: impl IntoIterator<Item = &'g Gram>,
    as_of: &AsOf,
    owner: Option<(&str, &str)>,
    binds: impl Fn(&Gram, &str) -> Vec<String>,
) -> Result<Option<CaseState>, String> {
    let mut state = CaseState::default();
    let mut latest: Option<(chrono::DateTime<chrono::FixedOffset>, String)> = None;
    let mut is_owner = false;
    for g in grams {
        if !as_of.let_through(g)? {
            continue;
        }
        state.grams += 1;
        *state
            .events
            .entry(CaseState::key(&g.stream.id, &g.name))
            .or_default() += 1;
        let m = g.moment()?;
        if latest.as_ref().is_none_or(|(l, _)| m > *l) {
            latest = Some((m, g.effective_at.clone()));
        }
        // A gram with stage BESLUIT is a decision (with `amends`, an
        // amendment); a gram that refers to a decision in the group follows
        // it. The rest belongs to the group itself (such as the application).
        let is_decision = g.stage.as_deref() == Some(crate::stream::DECISION);
        let followed = state
            .decisions
            .iter()
            .position(|b| g.refers_to.values().any(|d| *d == b.id));
        let stages = if is_decision {
            state.decisions.push(DecisionState {
                id: g.id.clone(),
                stream: g.stream.id.clone(),
                event: g.name.clone(),
                amends: g.refers_to.get(crate::stream::AMENDS).cloned(),
                ..DecisionState::default()
            });
            state.decisions.last_mut().map(|b| {
                *b.events
                    .entry(CaseState::key(&g.stream.id, &g.name))
                    .or_default() += 1;
                &mut b.stages
            })
        } else if let Some(i) = followed {
            state.decisions.get_mut(i).map(|b| {
                *b.events
                    .entry(CaseState::key(&g.stream.id, &g.name))
                    .or_default() += 1;
                &mut b.stages
            })
        } else {
            Some(&mut state.stages)
        };
        if let (Some(stages), Some(s)) = (stages, &g.stage) {
            // A stage is in a decision (or in the case) once; if there was a
            // second one anyway (an older chronicle), the latest in time
            // counts.
            let later = match stages.get(s) {
                None => true,
                Some(earlier) => {
                    let e = crate::date::moment(&earlier.effective_at)?;
                    m >= e
                }
            };
            if later {
                stages.insert(
                    s.clone(),
                    StageState {
                        stream: g.stream.id.clone(),
                        event: g.name.clone(),
                        effective_at: g.effective_at.clone(),
                        recorded_at: g.recorded_at.clone(),
                        regulation: g.regulation.clone(),
                        fields: g.fields.clone(),
                        input: g
                            .inputs
                            .iter()
                            .map(|(k, i)| (k.clone(), i.value.clone()))
                            .collect(),
                    },
                );
            }
        }
        if let Some((path, value)) = owner {
            is_owner |= binds(g, path)
                .iter()
                .any(|field| g.field(field).and_then(Value::as_str) == Some(value));
        }
    }
    if state.grams == 0 {
        return Ok(None);
    }
    state.latest_effective_at = latest.map(|(_, t)| t);
    state.owner = owner.map(|_| is_owner);
    Ok(Some(state))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::gram::{Input, StreamReference};
    use crate::synthesis::Provenance;
    use serde_json::json;

    fn gram(name: &str, stage: Option<&str>, effective_at: &str, fields: Value) -> Gram {
        Gram {
            kind: "chronolexogram".into(),
            id: uuid::Uuid::now_v7().to_string(),
            type_: "act".into(),
            subtype: None,
            stage: stage.map(str::to_string),
            name: name.into(),
            chronicle: "voorbeeld".into(),
            recording_actor: "voorbeeld_actor".into(),
            legal_basis: vec!["voorbeeldregeling#1".into()],
            legal_character: None,
            decision_type: None,
            regulation: None,
            regulation_valid_from: None,
            competent_authority: None,
            acting_actor: None,
            effective_at: effective_at.into(),
            effective_at_legal_basis: None,
            recorded_at: "2025-03-12T10:00:00+01:00".into(),
            refers_to: BTreeMap::new(),
            stream: StreamReference {
                id: "voorbeeldstroom".into(),
                sha256: "0".repeat(64),
            },
            provenance: None,
            fields: fields.as_object().unwrap().clone(),
            field_provenance: BTreeMap::new(),
            inputs: BTreeMap::new(),
            receipt: None,
            times: Default::default(),
            root: Some("z1".into()),
        }
    }

    fn case() -> Vec<Gram> {
        let mut decision = gram(
            "besluit_genomen",
            Some("BESLUIT"),
            "2025-03-10T00:00:00+01:00",
            json!({"bedrag": 100}),
        );
        decision.regulation = Some("voorbeeldregeling".into());
        decision.inputs.insert(
            "x".into(),
            Input {
                value: json!(4),
                provenance: Provenance::Handler,
            },
        );
        let paid = |amount: i64| {
            let mut g = gram(
                "betaald",
                None,
                "2025-03-11T00:00:00+01:00",
                json!({"bedrag": amount}),
            );
            g.refers_to.insert("decision".into(), decision.id.clone());
            g
        };
        let (b40, b60) = (paid(40), paid(60));
        vec![
            gram(
                "aanvraag_ontvangen",
                Some("AANVRAAG"),
                "2025-03-01T09:00:00+01:00",
                json!({"nummer": "12345678"}),
            ),
            decision,
            b40,
            b60,
        ]
    }

    fn binds(g: &Gram, path: &str) -> Vec<String> {
        if g.name == "aanvraag_ontvangen" && path == "kanaal.nummer" {
            vec!["nummer".into()]
        } else {
            Vec::new()
        }
    }

    /// The cell derives what a process asks about the case: the stages with
    /// what their gram recorded, the count per event, the latest moment.
    #[test]
    fn the_state_of_a_case() {
        let z = case();
        let s = reduce_case(&z, &AsOf::default(), None, binds)
            .unwrap()
            .unwrap();
        assert_eq!(s.grams, 4);
        assert_eq!(s.count("voorbeeldstroom", "betaald"), 2);
        assert_eq!(s.count("voorbeeldstroom", "bestaat_niet"), 0);
        assert_eq!(s.stages.keys().collect::<Vec<_>>(), ["AANVRAAG"]);
        // The decision is a decision of its own in the group; the payments
        // that refer to it follow it.
        assert_eq!(s.decisions.len(), 1);
        assert_eq!(s.decisions[0].id, z[1].id);
        assert_eq!(s.decisions[0].events["voorbeeldstroom/betaald"], 2);
        let b = &s.decisions[0].stages["BESLUIT"];
        assert_eq!(b.event, "besluit_genomen");
        assert_eq!(b.fields["bedrag"], json!(100));
        assert_eq!(b.input["x"], json!(4));
        assert_eq!(b.regulation.as_deref(), Some("voorbeeldregeling"));
        assert_eq!(
            s.latest_effective_at.as_deref(),
            Some("2025-03-11T00:00:00+01:00")
        );
        assert_eq!(s.owner, None);
        // Round trip through the lexostatus.
        let l = s.as_lexostatus("z1", &AsOf::default()).unwrap();
        assert!(l.parameters.is_empty(), "never goes to the engine");
        assert_eq!(CaseState::out(&l).unwrap(), s);
    }

    /// Whether someone knows the case, the cell says: a gram whose field bound
    /// to the owner path has the value.
    #[test]
    fn the_owner_of_a_case() {
        let z = case();
        let yes = reduce_case(
            &z,
            &AsOf::default(),
            Some(("kanaal.nummer", "12345678")),
            binds,
        )
        .unwrap()
        .unwrap();
        assert_eq!(yes.owner, Some(true));
        let no = reduce_case(
            &z,
            &AsOf::default(),
            Some(("kanaal.nummer", "87654321")),
            binds,
        )
        .unwrap()
        .unwrap();
        assert_eq!(no.owner, Some(false));
    }

    /// As of an as-of point only what held then counts; a case without a gram
    /// at the as-of point has no state.
    #[test]
    fn the_state_as_of_a_point() {
        let z = case();
        let as_of = AsOf::at(crate::date::TimePoint::read("p", "2025-03-05").unwrap());
        let s = reduce_case(&z, &as_of, None, binds).unwrap().unwrap();
        assert_eq!(s.grams, 1);
        assert!(!s.stages.contains_key("BESLUIT"));
        let early = AsOf::at(crate::date::TimePoint::read("p", "2025-02-01").unwrap());
        assert!(reduce_case(&z, &early, None, binds).unwrap().is_none());
    }
}
