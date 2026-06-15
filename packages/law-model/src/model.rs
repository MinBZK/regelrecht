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
    Structured { name: String },
}

/// Legal basis reference to another law
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct LegalBasis {
    pub law_id: String,
    pub article: String,
    #[serde(default)]
    pub description: Option<String>,
}

/// Type specification for input/output fields.
///
/// Currently only contains unit specification, but may be extended
/// with additional type metadata (precision, range, format) as the schema evolves.
#[derive(Debug, Clone, PartialEq, Default, Serialize, Deserialize)]
pub struct TypeSpec {
    /// Unit of measurement (e.g., "eurocent", "days", "percentage")
    #[serde(default)]
    pub unit: Option<String>,
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

/// Definition value in definitions section
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Definition {
    /// Definition with explicit value field
    Structured { value: Value },
    /// Simple value (for backward compatibility)
    Simple(Value),
}

impl Definition {
    /// Get the value from this definition
    pub fn value(&self) -> &Value {
        match self {
            Definition::Structured { value } => value,
            Definition::Simple(v) => v,
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

/// A legal construct that cannot be expressed with the engine's current operation set (RFC-012)
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
    #[serde(default)]
    pub definitions: Option<HashMap<String, Definition>>,
    #[serde(default)]
    pub execution: Option<Execution>,
    #[serde(default)]
    pub requires: Option<Vec<String>>,
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
    /// Legal constructs that cannot be expressed with the current operation set (RFC-012)
    #[serde(default)]
    pub untranslatables: Option<Vec<UntranslatableEntry>>,
}

/// Represents a single article in a law
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct Article {
    pub number: String,
    pub text: String,
    /// URL to the official source (also supports 'ref' for backward compatibility)
    #[serde(default, alias = "ref")]
    pub url: Option<String>,
    #[serde(default)]
    pub machine_readable: Option<MachineReadable>,
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

    /// Get required URI dependencies
    pub fn get_requires(&self) -> Vec<&str> {
        self.machine_readable
            .as_ref()
            .and_then(|mr| mr.requires.as_ref())
            .map(|reqs| reqs.iter().map(|s| s.as_str()).collect())
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

    /// Get the produces specification from this article.
    pub fn get_produces(&self) -> Option<&Produces> {
        self.get_execution_spec()
            .and_then(|exec| exec.produces.as_ref())
    }
}

/// Represents an article-based law document
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ArticleBasedLaw {
    /// JSON Schema URL
    #[serde(rename = "$schema", default)]
    pub schema: Option<String>,
    /// Law identifier (slug for referencing)
    #[serde(rename = "$id")]
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
    /// Water board code for waterschapsverordeningen
    #[serde(default)]
    pub waterschap_code: Option<String>,
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
    /// Articles in the law
    #[serde(default)]
    pub articles: Vec<Article>,
    /// SHA-256 hash of the YAML content (computed at load time, not serialized)
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
