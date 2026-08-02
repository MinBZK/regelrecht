//! Article-based law document model.
//!
//! These structs/enums are the canonical Rust representation of the law-YAML
//! format (the files under `corpus/regulation/`). They carry only the document
//! shape and allocation-free accessors — no YAML loading, security limits or
//! evaluation logic. The engine owns loading (`LawLoad`) and execution and
//! re-exports these types at `regelrecht_engine::article`.
use crate::{Operation, ParameterType, RegulatoryLayer, Value};
use serde::{Deserialize, Serialize};
use std::collections::{BTreeMap, HashMap};

/// Represents a competent authority - can be a simple string or a structured object
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum CompetentAuthority {
    /// Simple string reference (e.g., "#bevoegd_gezag")
    String(String),
    /// Structured authority with name field
    Structured {
        name: String,
        /// `INSTANCE` (a named organisation, the schema default) or `CATEGORY`
        /// (must be resolved per context)
        #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
        authority_type: Option<AuthorityType>,
    },
}

/// Whether a competent authority names one organisation or a category of them
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AuthorityType {
    /// A specific organisation (schema default)
    #[serde(rename = "INSTANCE")]
    Instance,
    /// A category that must be resolved per context
    #[serde(rename = "CATEGORY")]
    Category,
}

/// Fine-grained anchoring of a field, action or operation in the legal text.
///
/// Distinct from the document-level [`LegalBasis`] (`law_id`/`article`/
/// `description`); the schema defines both shapes under different names.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct FieldLegalBasis {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub bwb_id: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    /// Paragraph/lid number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraph: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sentence: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// Juriconnect BWB 1.3 reference
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub juriconnect: Option<String>,
    /// How this element relates to the law text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub explanation: Option<String>,
}

/// How a field's value relates to time
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Temporal {
    /// `period` or `point_in_time`
    #[serde(rename = "type")]
    pub temporal_type: String,
    /// Granularity of the period (`year`, `month`, `continuous`)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub period_type: Option<String>,
    /// Reference date for point-in-time values (a `$`-prefixed variable)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub reference: Option<String>,
}

/// Legal basis reference to another law
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegalBasis {
    pub law_id: String,
    pub article: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Type specification for input/output/definition fields.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TypeSpec {
    /// Unit of measurement (e.g., "eurocent", "euro", "ratio", "percentage", "days").
    /// A unit is a label, never a computational constraint (RFC-023).
    #[serde(default)]
    pub unit: Option<String>,
    /// Number of decimal places for the value, in its own unit (RFC-023 §2,
    /// issue #444). Parsed metadata; rounding itself is an explicit operation
    /// (RFC-024), never inferred from this field.
    #[serde(default)]
    pub precision: Option<i64>,
    /// Minimum allowed value (issue #444). Parsed metadata, not yet enforced.
    ///
    /// Serialized as a JSON number, because that is what the schema types it
    /// as; `Decimal`'s default is a string, which makes the re-serialized
    /// document schema-invalid. A bound beyond f64 precision is out of range
    /// for what these fields describe.
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub min: Option<rust_decimal::Decimal>,
    /// Maximum allowed value (issue #444). Parsed metadata, not yet enforced.
    /// Serialized as a JSON number; see [`TypeSpec::min`].
    #[serde(default, with = "rust_decimal::serde::float_option")]
    pub max: Option<rust_decimal::Decimal>,
}

/// Source specification for input fields
///
/// Defines where an input value comes from. Can be:
/// - Simple regulation reference: `regulation: "other_law"` + `output: "field_name"`
/// - Same-law reference: `output: "field_name"` (resolved within the same law)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Source {
    /// Simple cross-law reference (law ID)
    #[serde(default)]
    pub regulation: Option<String>,
    /// Output field to retrieve from the source.
    /// When None (e.g. `source: {}`), the input is resolved from the DataSourceRegistry.
    #[serde(default)]
    pub output: Option<String>,
    /// Parameters to pass to the source execution
    #[serde(default)]
    pub parameters: Option<BTreeMap<String, String>>,
    /// Endpoint to call on the delegated regulation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    /// Human-readable description or legal reference for this data source
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub description: Option<String>,
}

/// Parameter definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Parameter {
    pub name: String,
    #[serde(rename = "type")]
    pub param_type: ParameterType,
    #[serde(default)]
    pub required: Option<bool>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<Temporal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<FieldLegalBasis>,
}

/// Input definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Input {
    pub name: String,
    #[serde(rename = "type")]
    pub input_type: ParameterType,
    #[serde(default)]
    pub source: Option<Source>,
    #[serde(default)]
    pub type_spec: Option<TypeSpec>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<Temporal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<FieldLegalBasis>,
}

/// Output definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Output {
    pub name: String,
    #[serde(rename = "type")]
    pub output_type: ParameterType,
    #[serde(default)]
    pub type_spec: Option<TypeSpec>,
    #[serde(default)]
    pub description: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub temporal: Option<Temporal>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<FieldLegalBasis>,
}

/// Produces specification for execution.
///
/// Describes the legal character of what an article produces.
/// May be extended with additional metadata (appeal_period, notification_requirement) as schema evolves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Produces {
    /// Legal character of the output (e.g., "BESCHIKKING", "TOETS")
    #[serde(default)]
    pub legal_character: Option<String>,
    /// Type of decision (e.g., "TOEKENNING", "GOEDKEURING")
    #[serde(default)]
    pub decision_type: Option<String>,
    /// Selects a specific AWB procedure variant (RFC-008).
    /// When absent, the default procedure for the legal_character is used.
    #[serde(default)]
    pub procedure_id: Option<String>,
}

/// A single case in an IF operation (cases/default syntax)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Case {
    /// Condition to evaluate
    pub when: ActionValue,
    /// Value to return if condition is true
    pub then: ActionValue,
}

/// Represents a value in an action - can be a literal, variable reference, or nested operation.
///
/// Uses `#[serde(untagged)]` for flexible YAML parsing. The Operation variant is tried first,
/// but this is safe because `ActionOperation` is an internally-tagged enum keyed on `"operation"` -
/// any YAML object lacking an `operation` key will fail to deserialize as ActionOperation and
/// fall through to the Literal variant.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum ActionValue {
    /// Nested operation (tried first; requires `operation` field to match)
    Operation(Box<ActionOperation>),
    /// Literal value (number, string, boolean, variable reference like "$var", etc.)
    Literal(Value),
}

/// Represents an operation within an action.
///
/// Uses an internally-tagged enum (`"operation"` field) so that each variant
/// only carries the fields it actually needs.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(tag = "operation")]
pub enum ActionOperation {
    // Comparison (subject + value)
    #[serde(rename = "EQUALS")]
    Equals {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "NOT_EQUALS")]
    NotEquals {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "GREATER_THAN")]
    GreaterThan {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "LESS_THAN")]
    LessThan {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "GREATER_THAN_OR_EQUAL")]
    GreaterThanOrEqual {
        subject: ActionValue,
        value: ActionValue,
    },
    #[serde(rename = "LESS_THAN_OR_EQUAL")]
    LessThanOrEqual {
        subject: ActionValue,
        value: ActionValue,
    },

    // Arithmetic (values)
    #[serde(rename = "ADD")]
    Add { values: Vec<ActionValue> },
    #[serde(rename = "SUBTRACT")]
    Subtract { values: Vec<ActionValue> },
    #[serde(rename = "MULTIPLY")]
    Multiply { values: Vec<ActionValue> },
    #[serde(rename = "DIVIDE")]
    Divide { values: Vec<ActionValue> },

    // Aggregate (values)
    #[serde(rename = "MAX")]
    Max { values: Vec<ActionValue> },
    #[serde(rename = "MIN")]
    Min { values: Vec<ActionValue> },

    // Rounding (unary value + precision; RFC-024)
    #[serde(rename = "ROUND")]
    Round { value: ActionValue, precision: i64 },
    #[serde(rename = "CEIL")]
    Ceil { value: ActionValue, precision: i64 },
    #[serde(rename = "FLOOR")]
    Floor { value: ActionValue, precision: i64 },

    // Logical
    #[serde(rename = "AND")]
    And { conditions: Vec<ActionValue> },
    #[serde(rename = "OR")]
    Or { conditions: Vec<ActionValue> },
    #[serde(rename = "NOT")]
    Not { value: ActionValue },

    // Conditional
    #[serde(rename = "IF", alias = "SWITCH")]
    If {
        cases: Vec<Case>,
        #[serde(default)]
        default: Option<ActionValue>,
    },

    // Null checking
    #[serde(rename = "IS_NULL")]
    IsNull { subject: ActionValue },
    #[serde(rename = "NOT_NULL")]
    NotNull { subject: ActionValue },

    // Collection
    #[serde(rename = "IN")]
    In {
        subject: ActionValue,
        #[serde(default)]
        value: Option<ActionValue>,
        #[serde(default)]
        values: Option<Vec<ActionValue>>,
    },
    #[serde(rename = "NOT_IN")]
    NotIn {
        subject: ActionValue,
        #[serde(default)]
        value: Option<ActionValue>,
        #[serde(default)]
        values: Option<Vec<ActionValue>>,
    },
    #[serde(rename = "LIST")]
    List { items: Vec<ActionValue> },

    // Date
    #[serde(rename = "AGE")]
    Age {
        date_of_birth: ActionValue,
        reference_date: ActionValue,
    },
    #[serde(rename = "DATE_ADD")]
    DateAdd {
        date: ActionValue,
        #[serde(default)]
        years: Option<ActionValue>,
        #[serde(default)]
        months: Option<ActionValue>,
        #[serde(default)]
        weeks: Option<ActionValue>,
        #[serde(default)]
        days: Option<ActionValue>,
    },
    #[serde(rename = "DATE")]
    Date {
        year: ActionValue,
        month: ActionValue,
        day: ActionValue,
    },
    #[serde(rename = "DAY_OF_WEEK")]
    DayOfWeek { date: ActionValue },
    #[serde(rename = "DATE_DIFF")]
    DateDiff {
        from: ActionValue,
        to: ActionValue,
        /// Unit the difference is expressed in: "days", "months", or "years".
        /// Named `in` in YAML so the operation reads as one clause
        /// ("date diff from X to Y in days").
        #[serde(rename = "in")]
        unit: ActionValue,
    },
}

impl ActionOperation {
    /// Get the operation name as a static uppercase string (for tracing).
    pub fn operation_name(&self) -> &'static str {
        match self {
            ActionOperation::Equals { .. } => "EQUALS",
            ActionOperation::NotEquals { .. } => "NOT_EQUALS",
            ActionOperation::GreaterThan { .. } => "GREATER_THAN",
            ActionOperation::LessThan { .. } => "LESS_THAN",
            ActionOperation::GreaterThanOrEqual { .. } => "GREATER_THAN_OR_EQUAL",
            ActionOperation::LessThanOrEqual { .. } => "LESS_THAN_OR_EQUAL",
            ActionOperation::Add { .. } => "ADD",
            ActionOperation::Subtract { .. } => "SUBTRACT",
            ActionOperation::Multiply { .. } => "MULTIPLY",
            ActionOperation::Divide { .. } => "DIVIDE",
            ActionOperation::Max { .. } => "MAX",
            ActionOperation::Min { .. } => "MIN",
            ActionOperation::Round { .. } => "ROUND",
            ActionOperation::Ceil { .. } => "CEIL",
            ActionOperation::Floor { .. } => "FLOOR",
            ActionOperation::And { .. } => "AND",
            ActionOperation::Or { .. } => "OR",
            ActionOperation::Not { .. } => "NOT",
            ActionOperation::If { .. } => "IF",
            ActionOperation::IsNull { .. } => "IS_NULL",
            ActionOperation::NotNull { .. } => "NOT_NULL",
            ActionOperation::In { .. } => "IN",
            ActionOperation::NotIn { .. } => "NOT_IN",
            ActionOperation::List { .. } => "LIST",
            ActionOperation::Age { .. } => "AGE",
            ActionOperation::DateAdd { .. } => "DATE_ADD",
            ActionOperation::Date { .. } => "DATE",
            ActionOperation::DayOfWeek { .. } => "DAY_OF_WEEK",
            ActionOperation::DateDiff { .. } => "DATE_DIFF",
        }
    }
}

/// Action definition in execution spec
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Action {
    #[serde(default)]
    pub output: Option<String>,
    #[serde(default)]
    pub operation: Option<Operation>,
    /// Single value (can be literal, variable reference, or nested operation)
    #[serde(default)]
    pub value: Option<ActionValue>,
    /// Multiple values for aggregate/arithmetic operations
    #[serde(default)]
    pub values: Option<Vec<ActionValue>>,
    /// Subject for comparison operations
    #[serde(default)]
    pub subject: Option<ActionValue>,
    /// Conditions for AND/OR operations
    #[serde(default)]
    pub conditions: Option<Vec<ActionValue>>,
    /// Decimal places for rounding operations (ROUND/CEIL/FLOOR; RFC-024)
    #[serde(default)]
    pub precision: Option<i64>,
    /// Delegation resolution: take the value from an implementing regulation.
    /// Parsed but not yet executed by the engine.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolve: Option<ResolveSpec>,
    /// Anchoring of this action in the legal text
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_basis: Option<FieldLegalBasis>,
}

/// Delegation resolution on an action: which implementing regulation supplies
/// the value.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ResolveSpec {
    /// Regulatory layer to search for an implementation
    #[serde(default, rename = "type", skip_serializing_if = "Option::is_none")]
    pub resolve_type: Option<String>,
    /// Output field to retrieve from the implementation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output: Option<String>,
    /// Matching criteria for selecting the right implementation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub r#match: Option<BTreeMap<String, Value>>,
}

/// Execution specification within machine_readable section
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct Execution {
    #[serde(default)]
    pub produces: Option<Produces>,
    #[serde(default)]
    pub parameters: Option<Vec<Parameter>>,
    #[serde(default)]
    pub input: Option<Vec<Input>>,
    #[serde(default)]
    pub output: Option<Vec<Output>>,
    #[serde(default)]
    pub actions: Option<Vec<Action>>,
}

/// Definition value in definitions section.
///
/// A constant may be a bare value (`naam: 123`, backward-compatible) or the
/// optionally-structured form carrying a `type` and `type_spec` so it can
/// declare its quantity-kind (RFC-023).
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Definition {
    /// Structured form: `value` plus optional `type`/`type_spec` (RFC-023).
    Structured {
        value: Value,
        #[serde(rename = "type", default)]
        def_type: Option<ParameterType>,
        #[serde(default)]
        type_spec: Option<TypeSpec>,
    },
    /// Simple value (for backward compatibility)
    Simple(Value),
}

impl Definition {
    /// Get the value from this definition
    pub fn value(&self) -> &Value {
        match self {
            Definition::Structured { value, .. } => value,
            Definition::Simple(v) => v,
        }
    }

    /// The declared unit of this constant, if any (RFC-023). Bare constants and
    /// structured constants without a `type_spec.unit` return `None`.
    pub fn unit(&self) -> Option<&str> {
        match self {
            Definition::Structured { type_spec, .. } => {
                type_spec.as_ref().and_then(|t| t.unit.as_deref())
            }
            Definition::Simple(_) => None,
        }
    }
}

/// Default execution block for an open term (used when no implementing regulation exists)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenTermDefault {
    #[serde(default)]
    pub actions: Option<Vec<Action>>,
}

/// Open term declared by an article — a value that can or must be filled by
/// implementing regulations at a lower level.
///
/// Any regulatory layer can declare open_terms. A law (`WET`) typically has
/// `required: true` with no default, while lower layers often provide defaults
/// that can be refined further down.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OpenTerm {
    /// Identifier for this open term (e.g., "standaardpremie")
    pub id: String,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// Data type of the expected value
    #[serde(rename = "type")]
    pub term_type: ParameterType,
    /// Whether an implementation is mandatory (default: true)
    #[serde(default = "default_true")]
    pub required: bool,
    /// Who is authorized to fill this term (e.g., "minister")
    #[serde(default)]
    pub delegated_to: Option<String>,
    /// Expected regulatory layer of the implementation
    #[serde(default)]
    pub delegation_type: Option<String>,
    /// The regulation the article itself names as the one that fills this term
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected_source: Option<String>,
    /// The authority that decides this term per individual case (Awb 3:46)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decided_per_case_by: Option<String>,
    /// Legal basis text
    #[serde(default)]
    pub legal_basis: Option<String>,
    /// Default execution if no implementing regulation exists
    #[serde(default)]
    pub default: Option<OpenTermDefault>,
}

fn default_true() -> bool {
    true
}

/// A dependency of an article on another article, law, ministerial regulation
/// or royal decree.
///
/// Every field is optional in the schema (`machineReadableSection.requires`),
/// so an entry only names the dimensions it needs: an intra-law dependency
/// carries just `article`, a cross-law dependency `law` + `values`.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct ArticleRequirement {
    /// Article number within this law
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub article: Option<String>,
    /// Name of the external law depended on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub law: Option<String>,
    /// Name of the ministerial regulation depended on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub regeling: Option<String>,
    /// Name of the royal decree depended on
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub koninklijk_besluit: Option<String>,
    /// Values/outputs required from the dependency
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub values: Option<Vec<String>>,
}

/// Declares that this article fills an open term from a higher-level law.
/// Maps to the "Gelet op" clause in Dutch legislation.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ImplementsDeclaration {
    /// The $id of the higher-level law being implemented
    pub law: String,
    /// Article number in the higher law that declares the open_term
    pub article: String,
    /// The open_term id being filled
    pub open_term: String,
    /// Legal reference text (e.g., "Gelet op artikel 4 van de Wet op de zorgtoeslag")
    #[serde(default)]
    pub gelet_op: Option<String>,
}

/// Lifecycle point at which a hook fires
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum HookPoint {
    /// Fires between open-term resolution and action execution
    PreActions,
    /// Fires between action execution and result return
    PostActions,
}

impl HookPoint {
    /// Returns the hook point as a lowercase static string.
    pub fn as_str(&self) -> &'static str {
        match self {
            HookPoint::PreActions => "pre_actions",
            HookPoint::PostActions => "post_actions",
        }
    }
}

/// Filter that determines when a hook fires
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HookFilter {
    /// Match articles that produce this legal character (e.g., "BESCHIKKING")
    #[serde(default)]
    pub legal_character: Option<String>,
    /// Optionally narrow to a specific decision type (e.g., "TOEKENNING")
    #[serde(default)]
    pub decision_type: Option<String>,
    /// Lifecycle stage at which this hook fires (e.g., "BESLUIT", "BEKENDMAKING")
    /// When absent, defaults to BESLUIT for backward compatibility.
    #[serde(default)]
    pub stage: Option<String>,
}

/// Declaration that an article fires as a hook on matching lifecycle events (RFC-007)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct HookDeclaration {
    /// When in the lifecycle this hook fires
    pub hook_point: HookPoint,
    /// What triggers this hook
    pub applies_to: HookFilter,
}

/// Declaration that an article overrides another article's output (RFC-007, lex specialis)
///
/// Used for "in afwijking van artikel X" patterns where one law unilaterally
/// replaces another law's output value.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct OverrideDeclaration {
    /// The $id of the law being overridden
    pub law: String,
    /// The article number being overridden
    pub article: String,
    /// The specific output being replaced
    pub output: String,
    /// The overridden output does not arise at all, rather than taking another
    /// value.
    ///
    /// "Bestaat geen aanspraak" is not an entitlement of zero. With an
    /// entitlement of zero there is a decision carrying legal remedies and a
    /// ground for recovery; with no entitlement there is neither. An engine
    /// reads this flag and needs no knowledge of administrative law to act on
    /// it, which is why the ground sits in a quotation beside it rather than in
    /// a vocabulary the engine would have to interpret.
    #[serde(default, skip_serializing_if = "std::ops::Not::not")]
    pub voids: bool,
    /// The words of this article that establish the override, verbatim.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub legal_text_excerpt: Option<String>,
}

/// A required input for a procedure stage
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StageRequirement {
    /// Name of the required input
    pub name: String,
    /// Data type of the required input
    #[serde(rename = "type")]
    pub req_type: ParameterType,
}

/// A stage in an AWB-defined procedure lifecycle (RFC-008)
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Stage {
    /// Stage name (e.g., "AANVRAAG", "BESLUIT", "BEKENDMAKING")
    pub name: String,
    /// Human-readable description
    #[serde(default)]
    pub description: Option<String>,
    /// External inputs required to enter this stage
    #[serde(default)]
    pub requires: Option<Vec<StageRequirement>>,
}

/// Filter for which legal character a procedure applies to
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcedureAppliesTo {
    /// Legal character (e.g., "BESCHIKKING")
    pub legal_character: String,
}

/// A procedure definition — an AWB-defined lifecycle for a legal character (RFC-008)
///
/// Procedures are defined by the AWB, not by specific laws. Laws declare which
/// procedure they participate in via `produces.legal_character`.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ProcedureDefinition {
    /// Unique identifier for this procedure (e.g., "beschikking", "beschikking_uov")
    pub id: String,
    /// Whether this is the default procedure for its legal_character
    #[serde(default)]
    pub default: Option<bool>,
    /// Which legal character this procedure governs
    pub applies_to: ProcedureAppliesTo,
    /// Ordered sequence of lifecycle stages
    pub stages: Vec<Stage>,
}

/// What has to change before a marked article can be translated in full.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum MarkingResolution {
    /// The operation does not exist and must be built (statutory rounding,
    /// extracting the year from a date). Building one reaches past the engine:
    /// schema, model, evaluation, the BDD grammar and the frontend all carry
    /// it, so the value names what has to be added rather than where it runs.
    Operation,
    /// The operation set is not the problem; the format has no shape for this
    /// construct (quantification over persons, a rule about a set rather than a
    /// value, a legal fiction).
    Model,
}

impl MarkingResolution {
    /// The value as it is written in YAML.
    pub fn as_str(&self) -> &'static str {
        match self {
            MarkingResolution::Operation => "operation",
            MarkingResolution::Model => "model",
        }
    }
}

/// A construct in an article that the format itself cannot express (schema v0.6.0).
///
/// A marking is a flag on an article that is otherwise worked out: it names the
/// one thing that does not fit and leaves everything that does fit standing. It
/// is the opposite of an [`OpenTerm`], which says the language expresses this
/// fine and the content is filled elsewhere.
///
/// The engine parses markings but does not act on them: what execution should do
/// with a marked article is a separate decision. Until it is taken, the RFC-012
/// taint machinery keeps running off [`UntranslatableEntry`], which markings
/// replace from v0.6.0 onwards.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Marking {
    /// The construct that cannot be expressed, in the words the article uses.
    pub about: String,
    /// Why the construct does not fit, in terms of what the format does have.
    ///
    /// The diagnosis, and the half that cannot be recovered from the other
    /// fields: `resolved_by` follows from it and not the other way round.
    /// Without it a marking states a wish, and a gap someone examined reads
    /// the same as one nobody did.
    pub reason: String,
    /// Whether resolving this needs a new engine operation or a new model shape.
    pub resolution: MarkingResolution,
    /// The change that would resolve it, named concretely enough to become work.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub resolved_by: Option<String>,
    /// The values in this article that cannot be produced because of this
    /// marking. An empty list is a statement and not an omission: it says the
    /// article stays executable and only its explanation is incomplete.
    pub target: Vec<String>,
    /// The words from this article's own legal text that the marking hangs on.
    pub legal_text_excerpt: String,
    /// Whether a human has reviewed and acknowledged this gap.
    #[serde(default)]
    pub accepted: bool,
}

/// A top-level document property an article establishes (schema v0.6.0).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum DeclaredProperty {
    Name,
    OfficieleTitel,
    ValidFrom,
    ValidTo,
    RegulatoryLayer,
    LegalBasis,
}

/// Declaration that this article fixes a document property (schema v0.6.0).
///
/// A citation title, a commencement date or a scope in time is not a
/// calculation, and it is not nothing either: it fixes a value the rest of the
/// corpus and every trace depend on.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Declaration {
    /// The document property this article fixes.
    pub property: DeclaredProperty,
    /// The value the article gives it, verbatim from the text.
    pub value: Value,
    /// Where the article limits the property in time beyond the value itself.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub applies_from: Option<String>,
}

/// One container in an article's [`Placement`]: its number and its opschrift.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct PlacementContainer {
    /// Number of this container as the law gives it (e.g. "3.3").
    pub number: String,
    /// Opschrift of this container (e.g. "Advisering").
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub heading: Option<String>,
}

/// Where an article sits in the document: the containers that enclose it
/// (schema v0.6.0).
///
/// The opschrift is condensed legal classification written by the legislator and
/// decides questions the article text alone cannot answer. Absent for an article
/// that no container encloses, which is normal in a short law.
#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize)]
pub struct Placement {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub boek: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub deel: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub titeldeel: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<PlacementContainer>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<PlacementContainer>,
}

/// Structured reference to another law/article for runtime resolution.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct ArticleReference {
    /// Reference ID used in markdown links (e.g. "ref1").
    pub id: String,
    /// BWB identifier of the referenced law.
    pub bwb_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub artikel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub lid: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub onderdeel: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub hoofdstuk: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub paragraaf: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub afdeling: Option<String>,
}

/// A legal construct that cannot be expressed with the engine's current operation set (RFC-012)
///
/// Superseded by [`Marking`] in schema v0.6.0. It stays on the model because the
/// model is one struct for every version in
/// `regelrecht_engine::config::SUPPORTED_SCHEMAS`, and every law in the corpus
/// is still on v0.5.x; dropping the field would silently discard what those
/// laws declare and disable the RFC-012 taint modes that run off it.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UntranslatableEntry {
    /// The legal construct that cannot be translated
    pub construct: String,
    /// Why this construct is untranslatable
    pub reason: String,
    /// Suggested engine operation or approach to resolve this
    #[serde(default)]
    pub suggestion: Option<String>,
    /// Relevant excerpt from the article's legal text
    #[serde(default)]
    pub legal_text_excerpt: Option<String>,
    /// Whether a human has reviewed and acknowledged this gap
    #[serde(default)]
    pub accepted: bool,
}

/// Machine-readable section of an article
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct MachineReadable {
    /// Named endpoint for this article, making it callable from other regulations
    #[serde(default)]
    pub endpoint: Option<String>,
    #[serde(default)]
    pub definitions: Option<HashMap<String, Definition>>,
    #[serde(default)]
    pub execution: Option<Execution>,
    /// Dependencies on other articles, regelingen or external sources
    #[serde(default)]
    pub requires: Option<Vec<ArticleRequirement>>,
    #[serde(default)]
    pub competent_authority: Option<CompetentAuthority>,
    /// Open terms that can or must be filled by implementing regulations
    #[serde(default)]
    pub open_terms: Option<Vec<OpenTerm>>,
    /// Declares which open terms from higher-level laws this article fills
    #[serde(default)]
    pub implements: Option<Vec<ImplementsDeclaration>>,
    /// Hook declarations: this article fires when matching lifecycle events occur (RFC-007)
    #[serde(default)]
    pub hooks: Option<Vec<HookDeclaration>>,
    /// Override declarations: this article replaces another article's output (RFC-007)
    #[serde(default)]
    pub overrides: Option<Vec<OverrideDeclaration>>,
    /// Legal constructs that cannot be expressed with the current operation set (RFC-012).
    /// Superseded by [`MachineReadable::markings`] in schema v0.6.0.
    #[serde(default)]
    pub untranslatables: Option<Vec<UntranslatableEntry>>,
    /// Constructs that the format itself cannot express (schema v0.6.0)
    #[serde(default)]
    pub markings: Option<Vec<Marking>>,
    /// Document properties this article establishes (schema v0.6.0)
    #[serde(default)]
    pub declares: Option<Vec<Declaration>>,
}

/// Represents a single article in a law
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Article {
    pub number: String,
    pub text: String,
    /// URL to the official source (also supports 'ref' for backward compatibility)
    #[serde(default, alias = "ref")]
    pub url: Option<String>,
    /// The containers that enclose this article, each with its number and
    /// opschrift (schema v0.6.0)
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub placement: Option<Placement>,
    #[serde(default)]
    pub machine_readable: Option<MachineReadable>,
    /// Structured references to other laws/articles for runtime resolution
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub references: Option<Vec<ArticleReference>>,
}

impl Article {
    /// Extract execution specification from machine_readable section
    pub fn get_execution_spec(&self) -> Option<&Execution> {
        self.machine_readable.as_ref()?.execution.as_ref()
    }

    /// Get definitions from this article.
    ///
    /// Returns a reference to avoid unnecessary allocations.
    pub fn get_definitions(&self) -> Option<&HashMap<String, Definition>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.definitions.as_ref())
    }

    /// Get the declared dependencies of this article
    pub fn get_requires(&self) -> &[ArticleRequirement] {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.requires.as_deref())
            .unwrap_or_default()
    }

    /// Get all output names from this article - these are the public endpoints
    pub fn get_output_names(&self) -> Vec<&str> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.execution.as_ref())
            .and_then(|exec| exec.output.as_ref())
            .map(|outputs| outputs.iter().map(|o| o.name.as_str()).collect())
            .unwrap_or_default()
    }

    /// Check if this article produces a specific output (allocation-free).
    ///
    /// More efficient than `get_output_names().contains(&name)` as it
    /// doesn't allocate a Vec.
    pub fn has_output(&self, output_name: &str) -> bool {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.execution.as_ref())
            .and_then(|exec| exec.output.as_ref())
            .is_some_and(|outputs| outputs.iter().any(|o| o.name == output_name))
    }

    /// Check if this article is publicly callable (has outputs)
    pub fn is_public(&self) -> bool {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.execution.as_ref())
            .and_then(|exec| exec.output.as_ref())
            .is_some_and(|outputs| !outputs.is_empty())
    }

    /// Get the competent authority for this article
    pub fn get_competent_authority(&self) -> Option<&CompetentAuthority> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.competent_authority.as_ref())
    }

    /// Get inputs from this article's execution spec.
    pub fn get_inputs(&self) -> &[Input] {
        self.get_execution_spec()
            .and_then(|exec| exec.input.as_deref())
            .unwrap_or(&[])
    }

    /// Get open terms declared by this article.
    pub fn get_open_terms(&self) -> Option<&Vec<OpenTerm>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.open_terms.as_ref())
    }

    /// Get implements declarations from this article.
    pub fn get_implements(&self) -> Option<&Vec<ImplementsDeclaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.implements.as_ref())
    }

    /// Get hook declarations from this article.
    pub fn get_hooks(&self) -> Option<&Vec<HookDeclaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.hooks.as_ref())
    }

    /// Get override declarations from this article.
    pub fn get_overrides(&self) -> Option<&Vec<OverrideDeclaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.overrides.as_ref())
    }

    /// Get the markings declared by this article (schema v0.6.0).
    ///
    /// Read-only: the engine parses markings but does not yet act on them.
    pub fn get_markings(&self) -> Option<&Vec<Marking>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.markings.as_ref())
    }

    /// Get the document properties this article declares (schema v0.6.0).
    pub fn get_declares(&self) -> Option<&Vec<Declaration>> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.declares.as_ref())
    }

    /// Get the produces specification from this article.
    pub fn get_produces(&self) -> Option<&Produces> {
        self.get_execution_spec()
            .and_then(|exec| exec.produces.as_ref())
    }
}

/// Aanhef of a law: the text that precedes article 1.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Preamble {
    /// Preamble text in markdown, as published
    pub text: String,
    /// URL to the preamble in the official publication
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub url: Option<String>,
    /// The schema allows the aanhef its own machine-readable section
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub machine_readable: Option<MachineReadable>,
}

/// Represents an article-based law document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleBasedLaw {
    /// JSON Schema URL
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    /// Law identifier (slug for referencing).
    ///
    /// The schema does not list `$id` as required, so a schema-valid document
    /// may omit it and the model must still load it (completeness). An empty
    /// id means "unnamed law": it can be executed directly but nothing can
    /// reference it.
    #[serde(rename = "$id", default)]
    pub id: String,
    /// Unique UUID
    #[serde(default)]
    pub uuid: Option<String>,
    /// Regulatory layer type
    pub regulatory_layer: RegulatoryLayer,
    /// Publication date
    pub publication_date: String,
    /// Date from which law is valid
    #[serde(default)]
    pub valid_from: Option<String>,
    /// Last date on which the law is in force (inclusive). Set only when the law is
    /// terminated (it vervalt or wordt ingetrokken — a horizonbepaling or repeal by
    /// another instrument); absent means valid indefinitely until superseded by a
    /// newer version. Not about whether a successor exists. See RFC-019.
    #[serde(default)]
    pub valid_to: Option<String>,
    /// Law name (can be a reference like "#wet_naam")
    #[serde(default)]
    pub name: Option<String>,
    /// Competent authority
    #[serde(default)]
    pub competent_authority: Option<CompetentAuthority>,
    /// BWB identifier for national laws
    #[serde(default)]
    pub bwb_id: Option<String>,
    /// URL to official source
    #[serde(default)]
    pub url: Option<String>,
    /// Additional identifiers
    #[serde(default)]
    pub identifiers: Option<HashMap<String, String>>,
    /// Municipality code for gemeentelijke verordeningen
    #[serde(default)]
    pub gemeente_code: Option<String>,
    /// CBS province code for provinciale verordeningen
    #[serde(default)]
    pub provincie_code: Option<String>,
    /// Water board code for waterschapsverordeningen
    #[serde(default)]
    pub waterschap_code: Option<String>,
    /// CELEX number, required by the schema for `EU_VERORDENING`/`EU_RICHTLIJN`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub celex_nummer: Option<String>,
    /// European Legislation Identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub eli: Option<String>,
    /// Tractatenblad identifier, required by the schema for `VERDRAG`
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub tractatenblad_id: Option<String>,
    /// UN Treaty Series number
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unts_nummer: Option<String>,
    /// Staatscourant identifier
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub stcrt_id: Option<String>,
    /// Issuing organisation
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub organisation: Option<String>,
    /// Official title for local regulations
    #[serde(default)]
    pub officiele_titel: Option<String>,
    /// Year for versioned regulations (e.g., tariffs)
    #[serde(default)]
    pub jaar: Option<i32>,
    /// Legal basis references
    #[serde(default)]
    pub legal_basis: Option<Vec<LegalBasis>>,
    /// AWB-defined procedure lifecycles (RFC-008)
    #[serde(default)]
    pub procedure: Option<Vec<ProcedureDefinition>>,
    /// Aanhef preceding article 1. Carries the "Gelet op"-chain, and the schema
    /// allows it a `machine_readable` section of its own.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub preamble: Option<Preamble>,
    /// Articles in the law
    #[serde(default)]
    pub articles: Vec<Article>,
    /// SHA-256 hash of the source YAML, for provenance (RFC-013).
    ///
    /// This is the one runtime-populated field on the document model: it is left
    /// empty by plain deserialization and filled in by the engine's loader
    /// (`regelrecht_engine::article::LawLoad`) at parse time. It lives on the
    /// document (rather than an engine-side wrapper) so the hash travels with the
    /// law value through every consumer, and is `#[serde(skip)]` so it never
    /// round-trips into the YAML.
    #[serde(skip)]
    pub content_hash: Option<String>,
}

impl ArticleBasedLaw {
    /// Extract schema version (e.g., "v0.5.0") from the `$schema` URL.
    ///
    /// Looks for a `/vN.N.N` pattern (semver with v prefix) in the URL,
    /// skipping false matches like `/vendor/` or `/riva/`.
    pub fn schema_version(&self) -> Option<&str> {
        let url = self.schema.as_deref()?;
        let mut search_from = 0;
        loop {
            let pos = url[search_from..].find("/v")?;
            let abs_pos = search_from + pos;
            let version_start = abs_pos + 1;
            let rest = &url[version_start..];
            let end = rest.find('/').unwrap_or(rest.len());
            let candidate = &rest[..end];
            if candidate.starts_with('v') && Self::is_semver(&candidate[1..]) {
                return Some(candidate);
            }
            search_from = abs_pos + 2;
            if search_from >= url.len() {
                return None;
            }
        }
    }

    /// Check if a string looks like a semver version (N.N.N).
    fn is_semver(s: &str) -> bool {
        let mut parts = s.split('.');
        let valid = |p: &str| !p.is_empty() && p.chars().all(|c| c.is_ascii_digit());
        matches!((parts.next(), parts.next(), parts.next(), parts.next()),
            (Some(a), Some(b), Some(c), None) if valid(a) && valid(b) && valid(c))
    }

    /// Find article that produces the given output.
    ///
    /// Uses allocation-free search via `Article::has_output()`.
    pub fn find_article_by_output(&self, output_name: &str) -> Option<&Article> {
        self.articles
            .iter()
            .find(|article| article.has_output(output_name))
    }

    /// Find article by article number
    pub fn find_article_by_number(&self, number: &str) -> Option<&Article> {
        self.articles
            .iter()
            .find(|article| article.number == number)
    }

    /// Get mapping of output names to articles
    pub fn get_all_outputs(&self) -> HashMap<String, &Article> {
        let mut outputs = HashMap::new();
        for article in &self.articles {
            for output_name in article.get_output_names() {
                outputs.insert(output_name.to_string(), article);
            }
        }
        outputs
    }

    /// Get all publicly callable articles
    pub fn get_public_articles(&self) -> Vec<&Article> {
        self.articles.iter().filter(|art| art.is_public()).collect()
    }

    /// Get BWB identifier if available
    pub fn get_bwb_id(&self) -> Option<&str> {
        self.bwb_id
            .as_deref()
            .or_else(|| self.identifiers.as_ref()?.get("bwb_id").map(|s| s.as_str()))
    }

    /// Get official URL if available
    pub fn get_url(&self) -> Option<&str> {
        self.url.as_deref().or_else(|| {
            let ids = self.identifiers.as_ref()?;
            ids.get("url")
                .or_else(|| ids.get("ref"))
                .map(|s| s.as_str())
        })
    }
}
