//! What an article establishes in a chronicle, or which fact it extends.
//!
//! The law does not know chronolexography: it says in its own words that an
//! application arises (`produces.submission`), that a decision is taken on it
//! (`produces: BESCHIKKING` with `decides_on`), which general articles hook
//! onto it (`applies_to.submission`), who supplies each parameter (`origin`)
//! and when the fact counts (`produces.moment`). [`derive`] reads the
//! recording out of that. An article may still carry an explicit
//! `produces.extensions.chronolex` block (RFC-022 §3.2), which then wins: a
//! policy that records an execution says so itself.
//!
//! ```yaml
//! extensions:
//!   chronolex:
//!     establishes:
//!       - event: aanvraag_ontvangen      # this article establishes the event
//!         fields: parameters
//!       - event: toegekend               # a decision on a calendar year
//!         period: {parameter: jaar, unit: year}
//!         dated_by: besluitdatum         # the date the decision bears
//!       - extends: {submission: AANVRAAG} # a hook on every application
//!         effective_at:
//!           legal_basis: [algemene_wet_bestuursrecht#4:13 lid 1]
//!       - event: betaald                 # an execution, on a day
//!         type: executogram
//!         refers_to: {besluit: {stage: BESLUIT, required: true}}
//!         fields: [bedrag]
//!         executed_on: {parameter: maand, once_per: month, day: 1}
//!         record_when: betaling_in_maand
//!         until: {stage: EINDE}
//!       - event: bijgeschreven           # on receipt, once per reference
//!         type: executogram
//!         fields: [kenmerk, bedrag]
//!         record_when: wordt_bijgeschreven
//!         identified_by: kenmerk
//! ```

use std::collections::BTreeMap;

use regelrecht_engine::Article;
use regelrecht_law_model::{OriginValue, ParameterType, Stage, StageRequirement};
use serde::{Deserialize, Serialize};

/// The namespace in `produces.extensions`.
pub const NAMESPACE: &str = "chronolex";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chronolex {
    #[serde(default)]
    pub establishes: Vec<Establishment>,
    /// Derived from what the law says in its own words, not written as a
    /// block: its establishments have no event name, the stream gives them
    /// one (and, for an article that decides at more than one stage, says
    /// the stage).
    #[serde(skip)]
    pub derived: bool,
}

impl Chronolex {
    /// The establishments that are the article's own (not what it extends).
    pub fn own(&self) -> impl Iterator<Item = &Establishment> {
        self.establishes.iter().filter(|e| e.extends.is_none())
    }
}

/// What an extension extends: the submission (such as an application) that
/// a hook of the article applies to (RFC-046).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Extends {
    pub submission: String,
}

/// A fact that an article establishes, or the extension of one.
#[derive(Debug, Clone, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Establishment {
    #[serde(default)]
    pub event: Option<String>,
    #[serde(default)]
    pub extends: Option<Extends>,
    /// The gram type; `submission` for an article with
    /// `produces.submission`, otherwise required.
    #[serde(default, rename = "type")]
    pub type_: Option<String>,
    #[serde(default)]
    pub stage: Option<String>,
    /// Which gram a gram of this event refers to, per name from the law text
    /// ("besluit op de aanvraag": `on_application`).
    #[serde(default)]
    pub refers_to: BTreeMap<String, Reference>,
    #[serde(default)]
    pub effective_at: Option<EffectiveAt>,
    #[serde(default)]
    pub fields: Option<Fields>,
    /// The period the fact concerns, if the law says it concerns one: the
    /// cell applies the law of that period, not of the day it records.
    #[serde(default)]
    pub period: Option<PeriodParameter>,
    /// For an execution (an executogram): the parameter the day it is
    /// executed on goes into, and how often it may arise.
    #[serde(default)]
    pub executed_on: Option<ExecutedOn>,
    /// For an execution: the boolean output of the article that says whether
    /// the fact arises. The cell executes the article and records a gram only
    /// if it is true: the law says when a gram arises, not the cell.
    #[serde(default)]
    pub record_when: Option<String>,
    /// For a receipt (an execution with `record_when` and no
    /// `executed_on`): the parameter whose value identifies the message,
    /// such as the reference of a payment order. The cell records one
    /// answer per value: a message whose value a gram of the article
    /// already holds is refused as answered, so a sender that delivers again
    /// records nothing twice, also when the message refers to no gram of
    /// the receiving cell.
    #[serde(default)]
    pub identified_by: Option<String>,
    /// For an execution: no gram arises once the case has a gram of this
    /// stage (what follows from it is derived, not recorded).
    #[serde(default)]
    pub until: Option<Until>,
    /// For a decision: the parameter that is the date the decision bears
    /// (its dagtekening). The cell fills it with the day the decision is
    /// taken; every other value its stage requires must be given.
    #[serde(default)]
    pub dated_by: Option<String>,
}

/// The day an execution is executed on: the parameter of the article it goes
/// into, and how often the fact may arise per reference.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ExecutedOn {
    pub parameter: String,
    #[serde(default)]
    pub once_per: Option<Every>,
    /// The day of the period of `once_per` the execution falls on (`day: 1`,
    /// the first of the month), or the first day the execution may arise if
    /// that is later. Which day it is, is the choice of whoever executes the
    /// law; the cell does not choose it.
    #[serde(default)]
    pub day: Option<u32>,
}

/// How often an execution may arise for the gram it refers to.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Every {
    /// Once per calendar month.
    Month,
}

/// The stage whose gram ends an execution.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Until {
    pub stage: String,
}

/// The period a fact concerns: the one the parameter `parameter` gives, in
/// `unit`.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct PeriodParameter {
    pub parameter: String,
    pub unit: PeriodUnit,
}

/// The unit of a period.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum PeriodUnit {
    /// A calendar year. This is what `unit: year` means, and not something
    /// the cell assumes: a fact that concerns a year is decided under the
    /// version of the law in force on the first day of that year (1 January).
    /// A law that wants another day must say so with another unit.
    Year,
}

/// A reference to another gram: the article that establishes it (`to`), or
/// the stage of the procedure it belongs to (`stage`), so a regulation can
/// refer to the decision of a stage without naming the law that takes it.
/// One of the two.
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub to: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stage: Option<String>,
    #[serde(default)]
    pub required: bool,
}

impl Reference {
    /// Whether `gram` is what the reference refers to.
    pub fn admits(&self, gram: &crate::chronicle::Gram) -> bool {
        match (&self.to, &self.stage) {
            (Some(to), None) => &gram.establishes == to,
            (None, Some(stage)) => gram.stage.as_deref() == Some(stage.as_str()),
            _ => false,
        }
    }

    /// What it refers to, as a person reads it.
    pub fn target(&self) -> String {
        match (&self.to, &self.stage) {
            (Some(to), None) => to.clone(),
            (None, Some(stage)) => format!("stage {stage}"),
            _ => "nothing (it names both `to` and `stage`, or neither)".to_string(),
        }
    }

    /// One of `to` and `stage`.
    pub fn check(&self, name: &str) -> Result<(), String> {
        if self.to.is_some() == self.stage.is_some() {
            return Err(format!(
                "refers_to.{name}: names `to` or `stage`, one of the two"
            ));
        }
        Ok(())
    }
}

/// The provision that makes the moment of recording the moment that counts
/// (Awb 4:13, the receipt).
#[derive(Debug, Clone, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveAt {
    pub legal_basis: Vec<String>,
}

/// The fields a part contributes.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Fields {
    /// What the applicant or the channel supplies (origin BELANGHEBBENDE or
    /// KANAAL).
    Parameters,
    /// The outputs of the article.
    Outputs,
    /// These outputs of the article (a list of names).
    Named(Vec<String>),
}

impl<'de> Deserialize<'de> for Fields {
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        #[serde(untagged)]
        enum Raw {
            Kind(String),
            Names(Vec<String>),
        }
        match Raw::deserialize(d)? {
            Raw::Kind(kind) => match kind.as_str() {
                "parameters" => Ok(Fields::Parameters),
                "outputs" => Ok(Fields::Outputs),
                other => Err(serde::de::Error::custom(format!(
                    "fields: '{other}' is not `parameters`, `outputs` or a list of outputs"
                ))),
            },
            Raw::Names(names) => Ok(Fields::Named(names)),
        }
    }
}

/// The explicit chronolex block of an article, if it has one. A block this
/// crate cannot read is an error, with the article named: a typo must not
/// make a field silently disappear from the gram.
pub fn of_article(article: &Article, reference: &str) -> Result<Option<Chronolex>, String> {
    let Some(block) = article
        .get_produces()
        .and_then(|p| p.extensions.as_ref())
        .and_then(|e| e.get(NAMESPACE))
    else {
        return Ok(None);
    };
    serde_json::from_value(block.clone())
        .map(Some)
        .map_err(|e| format!("{reference}: extensions.{NAMESPACE}: {e}"))
}

/// What the law says in its own words, read as a chronolex block
/// (RFC-022 §1.2), for an article without an explicit one:
///
/// - `produces.submission`: the article establishes a submission; its fields
///   are the parameters the belanghebbende supplies (origin BELANGHEBBENDE
///   or KANAAL), and `produces.moment` is the moment that counts.
/// - `produces: BESCHIKKING` with `decides_on`: the article takes a decision
///   on that submission, a decretogram at every stage of its procedure that
///   is a decision (`is: BESLUIT`, or BESLUIT itself), referring to the
///   submission as `on_application`; its fields are the outputs. Without
///   `decides_on` the decision is taken ex officio (the aanslag of AWR 11):
///   the same decretogram, referring to nothing. The date
///   the decision bears is the one date requirement of the stage (the
///   dagtekening, Awir 16; `besluit_datum` in the Awb), or the parameter
///   `produces.moment` names.
/// - a hook with `applies_to.submission`: the article extends every
///   submission of that kind with what it asks of the belanghebbende, and
///   with `produces.moment` as the moment that counts (Awb 4:13 lid 1).
///
/// `None` if the law says none of this; an error if it says something the
/// cell cannot record, such as a decision on more than one submission. What
/// the law says the fact concerns
/// (its period) is read where the decision is executed, from the parameter
/// with origin role TIJDVAK ([`crate::shape`]).
pub fn derive(article: &Article, decision_stages: &[&Stage]) -> Result<Option<Chronolex>, String> {
    let produces = article.get_produces();
    let moment = produces.and_then(|p| p.moment.as_ref());
    let effective_at = moment.map(|m| EffectiveAt {
        legal_basis: m.legal_basis.clone(),
    });
    let supplies = article.get_parameters().iter().any(|p| {
        p.origin
            .as_ref()
            .and_then(|o| o.valid().ok())
            .is_some_and(|o| matches!(o.waarde, OriginValue::Belanghebbende | OriginValue::Kanaal))
    });
    let mut establishes = Vec::new();
    if produces.is_some_and(|p| p.submission.is_some()) {
        establishes.push(Establishment {
            fields: Some(Fields::Parameters),
            effective_at: effective_at.clone(),
            ..Default::default()
        });
    } else if let Some(produces) =
        produces.filter(|p| p.legal_character.as_deref() == Some("BESCHIKKING"))
    {
        // On a submission (`decides_on`), or, without one, ex officio: a
        // decision the administrative body takes of its own accord, such as
        // the aanslag the inspecteur sets (AWR 11), which refers to no gram.
        let on = produces
            .decides_on
            .as_ref()
            .map(|d| match d.as_slice() {
                [one] => Ok(one),
                more => Err(format!(
                    "decides_on names {} submissions; a decision is taken on one",
                    more.len()
                )),
            })
            .transpose()?;
        for stage in decision_stages {
            let dated_by = moment.and_then(|m| m.parameter.clone()).or_else(|| {
                let dates: Vec<&StageRequirement> = stage
                    .requires
                    .iter()
                    .flatten()
                    .filter(|r| r.req_type == ParameterType::Date)
                    .collect();
                match dates.as_slice() {
                    [one] => Some(one.name.clone()),
                    _ => None,
                }
            });
            establishes.push(Establishment {
                type_: Some("decretogram".to_string()),
                stage: Some(stage.name.clone()),
                refers_to: on
                    .map(|on| {
                        BTreeMap::from([(
                            "on_application".to_string(),
                            Reference {
                                to: Some(on.clone()),
                                stage: None,
                                required: true,
                            },
                        )])
                    })
                    .unwrap_or_default(),
                effective_at: effective_at.clone(),
                fields: Some(Fields::Outputs),
                dated_by,
                ..Default::default()
            });
        }
    }
    let mut kinds: Vec<&str> = Vec::new();
    for hook in article.get_hooks().into_iter().flatten() {
        let Some(kind) = hook.applies_to.submission.as_deref() else {
            continue;
        };
        if kinds.contains(&kind) {
            continue;
        }
        kinds.push(kind);
        establishes.push(Establishment {
            extends: Some(Extends {
                submission: kind.to_string(),
            }),
            effective_at: effective_at.clone(),
            fields: supplies.then_some(Fields::Parameters),
            ..Default::default()
        });
    }
    Ok((!establishes.is_empty()).then_some(Chronolex {
        establishes,
        derived: true,
    }))
}
