//! The `produces.extensions.chronolex` block of an article (RFC-022 §3.2):
//! which facts it establishes, or which fact it extends.
//!
//! ```yaml
//! extensions:
//!   chronolex:
//!     establishes:
//!       - event: aanvraag_ontvangen      # this article establishes the event
//!         fields: parameters
//!       - extends: {submission: AANVRAAG} # a hook on every application
//!         effective_at:
//!           legal_basis: [algemene_wet_bestuursrecht#4:13 lid 1]
//! ```

use std::collections::BTreeMap;

use regelrecht_engine::Article;
use serde::Deserialize;

/// The namespace in `produces.extensions`.
pub const NAMESPACE: &str = "chronolex";

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Chronolex {
    #[serde(default)]
    pub establishes: Vec<Establishment>,
}

/// What an extension extends: an event by name, or the submission (such as
/// an application) that a hook of the article applies to (RFC-046).
#[derive(Debug, Clone, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Extends {
    Event(String),
    Submission { submission: String },
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
    /// `<name at the reader>: <field of the gram>`: two names for one field.
    #[serde(default)]
    pub aliases: BTreeMap<String, String>,
}

/// A reference to another gram: the article that establishes it.
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct Reference {
    pub to: String,
    #[serde(default)]
    pub required: bool,
}

/// The moment that counts, with the provision that says so. Without a
/// parameter: the moment of recording (Awb 4:13, the receipt).
#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct EffectiveAt {
    #[serde(default)]
    pub parameter: Option<String>,
    pub legal_basis: Vec<String>,
}

/// The fields a part contributes.
#[derive(Debug, Clone, Deserialize)]
#[serde(untagged)]
pub enum Fields {
    /// `parameters`: what the applicant or the channel supplies (origin
    /// BELANGHEBBENDE or KANAAL). `outputs`: the outputs of the article.
    Keyword(Keyword),
    /// Named parameters of the article: `{parameters: [a, b]}`.
    Selection { parameters: Vec<String> },
    /// Fields with their type, which the article names but does not ask as a
    /// parameter.
    Typed(BTreeMap<String, FieldType>),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Keyword {
    Parameters,
    Outputs,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldType {
    #[serde(rename = "type")]
    pub type_: regelrecht_law_model::ParameterType,
    #[serde(default)]
    pub columns: Option<Vec<String>>,
}

/// The chronolex block of an article, if it has one. A block this crate
/// cannot read is an error, with the article named: a typo must not make a
/// field silently disappear from the gram.
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
