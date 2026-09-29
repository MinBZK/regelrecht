//! The lexostatus definitions: what the `lexostatuses.yaml` of a cell declares
//! (`schema/chronolex/v0.3.0/lexostatus.json`), and what the checks at
//! startup ask about them. Execution is in [`super`].

use std::collections::BTreeMap;
use std::path::Path;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::load;
use crate::schema::Kind;

/// The lexostatus definitions of a cell (`schema/chronolex/v0.3.0/lexostatus.json`).
#[derive(Debug, Clone, Deserialize)]
pub struct Lexostatuses {
    pub cell: String,
    /// Supplements to the lexostatuses the law reads in this cell (see
    /// [`crate::law`]): extra fields for the synthesis, not parameters.
    #[serde(default)]
    pub law: Vec<LawSupplement>,
    pub lexostatus_definitions: Vec<LexostatusDefinition>,
}

/// Extra fields on a lexostatus from the law: what the cell passes along as
/// input for a later source (a KvK number, a designation), not a parameter of
/// an article. That is registration, so configuration of the cell, not the
/// law.
#[derive(Debug, Clone, Deserialize)]
pub struct LawSupplement {
    /// `<regulation>#<article>`: the reading article.
    pub article: String,
    pub extra_fields: BTreeMap<String, Derived>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct LexostatusDefinition {
    pub name: String,
    pub inputs: Vec<InputDefinition>,
    pub reduction: Reduction,
    /// From the law (`produces.extensions.chronolex.reads`, see
    /// [`crate::law`]): the reading article. Without: from `lexostatuses.yaml`.
    #[serde(skip)]
    pub law: Option<crate::law::LawReading>,
}

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct InputDefinition {
    pub name: String,
    #[serde(rename = "type")]
    pub kind: String,
}

/// Equality on the gram: a key from [`GRAM_KEYS`] is a field of the gram
/// itself, any other a field path under `fields`. A value `$x` comes from the
/// inputs.
pub type Filter = BTreeMap<String, String>;

/// The filter keys that are a field of the gram itself, not a field path:
/// every field with a text as value (see [`crate::gram::Gram::attribute`]):
/// the id, the root (from the index of the chronicle) and the fixed fields.
/// In addition, `refers_to.<name>` is the id a gram refers to under that name;
/// this is how a lexostatus filters per decision.
pub const GRAM_KEYS: &[&str] = &[
    "id",
    "root",
    "name",
    "type",
    "subtype",
    "stage",
    "recording_actor",
    "chronicle",
    "legal_character",
    "decision_type",
    "regulation",
    "competent_authority",
];

#[derive(Debug, Clone, Deserialize, Serialize)]
pub struct Reduction {
    pub chronicle: String,
    #[serde(default, skip_serializing_if = "Filter::is_empty")]
    pub filter: Filter,
    /// Makes the lexostatus a list with one row per case.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub group_by: Option<GroupBy>,
    /// Only with `group_by`: a case with a gram through this filter drops out.
    #[serde(default, skip_serializing_if = "Filter::is_empty")]
    pub without: Filter,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub pick: Option<Pick>,
    pub derivations: BTreeMap<String, Derived>,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub extra_fields: BTreeMap<String, Derived>,
}

/// A derivation with its legal basis: the articles (`<regulation>#<article>`,
/// optionally with `lid <n>`) it rests on. That is the article that asks for
/// the fact, or the article that carries the reading, such as the register
/// the cell keeps ("no gram is no"). The name of a parameter derivation is a
/// parameter of an article from the legal basis of the event or from this
/// legal basis; the check at startup verifies that every article is loaded
/// and the paragraph exists (see [`crate::check`]).
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Derived {
    #[serde(flatten)]
    pub derivation: Derivation,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub legal_basis: Vec<String>,
}

impl std::ops::Deref for Derived {
    type Target = Derivation;
    fn deref(&self) -> &Derivation {
        &self.derivation
    }
}

impl From<Derivation> for Derived {
    fn from(derivation: Derivation) -> Self {
        Self {
            derivation,
            legal_basis: Vec::new(),
        }
    }
}

impl<'de> Deserialize<'de> for Derived {
    /// `legal_basis` next to the keys of the derivation: take it off first,
    /// then the derivation from the rest (an untagged enum with `flatten` does
    /// not reliably ignore unknown keys).
    fn deserialize<D: serde::Deserializer<'de>>(d: D) -> Result<Self, D::Error> {
        let mut v = Value::deserialize(d)?;
        let legal_basis = match v.as_object_mut().and_then(|o| o.remove("legal_basis")) {
            Some(g) => serde_json::from_value(g).map_err(serde::de::Error::custom)?,
            None => Vec::new(),
        };
        let derivation = Derivation::deserialize(v).map_err(serde::de::Error::custom)?;
        Ok(Self {
            derivation,
            legal_basis,
        })
    }
}

/// What a list lexostatus groups by.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupBy {
    /// One row per root: the grams that through their references lead to the
    /// same gram without a reference (such as an application and what follows
    /// it).
    Root,
}

/// Which gram counts if there are several.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Pick {
    /// The latest gram in time: the latest `effective_at`, on an equal moment
    /// the latest `recorded_at`. A remedy is a new gram.
    Latest,
}

/// A derivation: how a parameter follows from the chronicle.
///
/// The order matters: serde tries the variants from top to bottom and ignores
/// unknown keys, so the variants with more keys come first. The schema has
/// already checked the shape by then.
#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
#[serde(untagged)]
pub enum Derivation {
    /// Over the grams through `filter`: the date of a moment of the latest
    /// (such as the receipt of the application, in a list that also reads the
    /// other grams of a case). No gram: `no_gram`, if present.
    LatestMoment {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Pick,
        moment: Moment,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over the grams through `filter`: the value of `field` in the latest.
    /// If no gram passes the filter, `no_gram`, if present.
    LatestField {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Pick,
        field: String,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over the grams through `filter`: the year of the date in `year_of` in
    /// the latest gram. No gram: `no_gram`, if present.
    LatestYearOf {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Pick,
        year_of: String,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over the grams through `filter`: the period in which the date in
    /// `period_of` in the latest gram falls (see [`Derivation::PeriodOf`]).
    /// No gram: `no_gram`, if present.
    LatestPeriodOf {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Pick,
        period_of: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        period: Option<Period>,
        #[serde(
            default,
            deserialize_with = "present",
            skip_serializing_if = "Option::is_none"
        )]
        no_gram: Option<Value>,
    },
    /// Over the grams through `filter`: whether the list field in the latest
    /// contains the value.
    LatestContains {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        pick: Pick,
        contains: Contains,
    },
    /// Over the grams through `filter`: whether there is at least one. With
    /// `filled` only a gram in which that field is filled counts (see
    /// [`super::filled`]): a filter compares on equality, and "has a value" is
    /// not a value.
    Exists {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        exists: bool,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        filled: Option<String>,
    },
    /// Over the grams through `filter`: the sum of a number field.
    Sum {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        sum: String,
    },
    /// Over the grams through `filter`: per gram a row with these fields, in
    /// the order of the chronicle. A list for a table, for example the rows of
    /// a result; a field a gram does not have is null.
    Collect {
        #[serde(default, skip_serializing_if = "Filter::is_empty")]
        filter: Filter,
        collect: Vec<String>,
    },
    Field {
        field: String,
    },
    /// The year of a date field of the chosen gram.
    YearOf {
        year_of: String,
    },
    /// The period (year, quarter or month) in which a date field of the chosen
    /// gram falls, as its first day: a window as a date the engine can read.
    /// Without `period` the runtime sets, at load time, the period the
    /// regulation names (see [`Period`]).
    PeriodOf {
        period_of: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        period: Option<Period>,
    },
    Filled {
        filled: String,
    },
    Equals {
        equals: Equals,
    },
    EachRow {
        table: String,
        each_row: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        only_where: Option<String>,
    },
    OneRow {
        table: String,
        one_row: String,
    },
    Moment {
        moment: Moment,
    },
}

/// A key that is present, even with the value null: `Some(Value::Null)`.
fn present<'de, D: serde::Deserializer<'de>>(d: D) -> Result<Option<Value>, D::Error> {
    Value::deserialize(d).map(Some)
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Equals {
    pub field: String,
    pub value: Value,
}

#[derive(Debug, Clone, PartialEq, Deserialize, Serialize)]
pub struct Contains {
    pub field: String,
    pub value: Value,
}

/// A calendar period. A window (the period for which a decision is requested)
/// can be a year, a quarter or a month; the regulation says which with
/// `temporal.period_type` of the parameter (RFC-001: `year`, `month`). A
/// period is named after its first day.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Period {
    Year,
    Quarter,
    Month,
}

impl Period {
    /// The period a regulation names with `temporal.period_type`.
    pub fn from_period_type(t: &str) -> Option<Self> {
        match t {
            "year" => Some(Period::Year),
            "quarter" => Some(Period::Quarter),
            "month" => Some(Period::Month),
            _ => None,
        }
    }

    /// The first day of the period in which a date falls.
    pub fn start(self, d: chrono::NaiveDate) -> Option<chrono::NaiveDate> {
        use chrono::Datelike;
        let month = match self {
            Period::Year => 1,
            Period::Quarter => (d.month0() / 3) * 3 + 1,
            Period::Month => d.month(),
        };
        chrono::NaiveDate::from_ymd_opt(d.year(), month, 1)
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Moment {
    /// When the fact legally holds or took place.
    EffectiveAt,
    /// When the cell recorded it.
    RecordedAt,
}

/// Read lexostatus definitions from text and validate them against the schema.
pub fn parse(text: &str, source: &str) -> Result<Lexostatuses, Vec<String>> {
    load::definition(text, source, Kind::Lexostatus)
}

/// Load the lexostatus definitions from a file.
pub fn load(path: &Path) -> Result<Lexostatuses, Vec<String>> {
    load::load(path, parse)
}

impl Lexostatuses {
    pub fn lexostatus(&self, name: &str) -> Option<&LexostatusDefinition> {
        self.lexostatus_definitions.iter().find(|d| d.name == name)
    }
}

impl LexostatusDefinition {
    /// All derivations: the parameters and the extra fields.
    pub fn all_derivations(&self) -> impl Iterator<Item = (&String, &Derived)> {
        self.reduction
            .derivations
            .iter()
            .chain(self.reduction.extra_fields.iter())
    }

    /// Whether the lexostatus is a list (`group_by`): no parameters, and never
    /// to the engine.
    pub fn is_list(&self) -> bool {
        self.reduction.group_by.is_some()
    }

    /// The names this lexostatus delivers: parameters and extra fields.
    pub fn delivers(&self, name: &str) -> bool {
        self.reduction.derivations.contains_key(name)
            || self.reduction.extra_fields.contains_key(name)
    }
}

/// Whether a filter key is a field of the gram itself.
pub fn is_gram_key(key: &str) -> bool {
    GRAM_KEYS.contains(&key) || key.starts_with(crate::gram::REFERS_TO)
}

/// The field paths under `fields` a filter selects on.
pub fn filter_paths(filter: &Filter) -> Vec<&str> {
    filter
        .keys()
        .map(String::as_str)
        .filter(|k| !is_gram_key(k))
        .collect()
}

impl Derivation {
    /// The own filter of a derivation over a collection of grams; `None` for a
    /// derivation on the chosen gram.
    pub fn filter(&self) -> Option<&Filter> {
        match self {
            Derivation::LatestField { filter, .. }
            | Derivation::LatestMoment { filter, .. }
            | Derivation::LatestYearOf { filter, .. }
            | Derivation::LatestPeriodOf { filter, .. }
            | Derivation::LatestContains { filter, .. }
            | Derivation::Exists { filter, .. }
            | Derivation::Sum { filter, .. }
            | Derivation::Collect { filter, .. } => Some(filter),
            _ => None,
        }
    }

    /// Whether the derivation reads the chosen gram (and thus requires `pick`).
    pub fn at_chosen_gram(&self) -> bool {
        self.filter().is_none()
    }

    /// The field paths this derivation reads, including those of its filter.
    pub fn read_paths(&self) -> Vec<&str> {
        let mut paths = match self {
            Derivation::Field { field } | Derivation::LatestField { field, .. } => {
                vec![field.as_str()]
            }
            Derivation::YearOf { year_of } | Derivation::LatestYearOf { year_of, .. } => {
                vec![year_of.as_str()]
            }
            Derivation::PeriodOf { period_of, .. }
            | Derivation::LatestPeriodOf { period_of, .. } => vec![period_of.as_str()],
            Derivation::Filled { filled } => vec![filled.as_str()],
            Derivation::Equals { equals } => vec![equals.field.as_str()],
            Derivation::EachRow { table, .. } | Derivation::OneRow { table, .. } => {
                vec![table.as_str()]
            }
            Derivation::Sum { sum, .. } => vec![sum.as_str()],
            Derivation::Collect { collect, .. } => collect.iter().map(String::as_str).collect(),
            Derivation::LatestContains { contains, .. } => vec![contains.field.as_str()],
            Derivation::Exists { filled, .. } => filled.iter().map(String::as_str).collect(),
            Derivation::Moment { .. } | Derivation::LatestMoment { .. } => vec![],
        };
        if let Some(f) = self.filter() {
            paths.extend(filter_paths(f));
        }
        paths
    }

    /// For a period derivation: the period, which the runtime sets from the
    /// regulation at load time if needed.
    pub fn period_mut(&mut self) -> Option<&mut Option<Period>> {
        match self {
            Derivation::PeriodOf { period, .. } | Derivation::LatestPeriodOf { period, .. } => {
                Some(period)
            }
            _ => None,
        }
    }

    /// For a table derivation: the table field and the columns it reads
    /// (`each_row` or `one_row`, and `only_where`).
    pub fn table_columns(&self) -> Option<(&str, Vec<&str>)> {
        match self {
            Derivation::EachRow {
                table,
                each_row,
                only_where,
            } => {
                let mut k = vec![each_row.as_str()];
                k.extend(only_where.as_deref());
                Some((table, k))
            }
            Derivation::OneRow { table, one_row } => Some((table, vec![one_row])),
            _ => None,
        }
    }

    /// Whether the derivation assesses whether something is present (`filled`,
    /// `table` with `each_row`). False then means: this is absent from the
    /// gram. For `equals` and `one_row` false is an answer, not a gap.
    pub fn assesses_presence(&self) -> bool {
        matches!(self, Derivation::Filled { .. } | Derivation::EachRow { .. })
    }

    /// How the derivation reads absence (`no_gram`), if it says so.
    pub fn no_gram(&self) -> Option<&Value> {
        match self {
            Derivation::LatestMoment { no_gram, .. }
            | Derivation::LatestField { no_gram, .. }
            | Derivation::LatestYearOf { no_gram, .. }
            | Derivation::LatestPeriodOf { no_gram, .. } => no_gram.as_ref(),
            _ => None,
        }
    }
}
