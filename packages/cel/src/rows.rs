//! Synthesis per row: a table field of a lexostatus of the case becomes an
//! array parameter, with per row columns from other cells.
//!
//! An article can ask for a table as a parameter of which the submitter fills
//! in only part; the agency establishes the rest itself, per row, from
//! registers of other cells. The process does not build that table in code:
//! `synthesis.yaml` of the deployment (until RFC-045) says which table field
//! delivers the rows, which column goes
//! along under which name, and which source is queried per row with which
//! input.
//!
//! The rows are queried concurrently and returned in the order of the table
//! field. Per row the process goes along the sources, in order. The input of
//! a source comes from the row itself (`column`), from a lexostatus of the
//! case (`lexostatus` and `field`) or from the combined parameters
//! (`parameter`); a source whose turn came earlier can thus deliver a column
//! that a later source uses as input. An input can also be an output of a
//! regulation (`regulation` and `output`): the law derives it from the
//! combined parameters, like a reference date from a year, in a run of its
//! own before the rows; or a fixed value (`value`). If an input is absent, a
//! source is unreachable, or it does not deliver the value, that column stays
//! out: nothing is filled in. Which columns those were is in `missing`.
//!
//! The block sits under an action (`handling.actions[].rows`) or under the
//! assessment (`portal.assessment.rows`); both run it with [`apply`], before
//! the engine.

use std::collections::{BTreeMap, BTreeSet};

use futures_util::stream::{self, StreamExt};

use serde::Serialize;
use serde_json::{Map, Value};

use regelrecht_engine::LawExecutionService;

use crate::cell::Cell;
use crate::config::{ProcessDefinition, RowInput, RowSource, RowsDefinition};
use crate::reduction::{AsOf, Lexostatus};
use crate::synthesis::{Combination, Provenance, Status};
use crate::transport::TransportError;

/// A source that is queried per row, with the transport the runtime chose
/// for it.
pub type Source = crate::synthesis::Source<RowSource>;

/// A rows definition with its sources.
#[derive(Clone)]
pub struct Rows {
    pub definition: RowsDefinition,
    pub sources: Vec<Source>,
}

/// How querying a source went across all rows.
#[derive(Debug, Clone, Serialize)]
pub struct SourceResult {
    pub cell: String,
    pub lexostatus: String,
    pub transport: &'static str,
    /// The number of rows for which the source was queried.
    pub queried: usize,
    /// The worst status across the rows.
    pub status: Status,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// Along which route(s) the source reduced, if it says so (a runtime
    /// with the engine route, experiment A): `engine`, `dsl`, or both with a
    /// comma.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub reduction: Option<String>,
}

/// What a rows definition yielded.
#[derive(Debug, Clone, Serialize)]
pub struct RowsOutcome {
    pub parameter: String,
    /// Why the table was not built: the table field is not a list of rows.
    /// Then the parameter does not go to the engine; leaving out a row would
    /// silently change the output.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
    /// The rows, each with the columns a source delivered.
    pub rows: Vec<Value>,
    pub sources: Vec<SourceResult>,
    /// Columns that are not in every row, without duplicates.
    #[serde(skip_serializing_if = "Vec::is_empty")]
    pub missing: Vec<String>,
    /// The input the law derived, per `<regulation>#<output>`, with the
    /// value or why there was none.
    #[serde(skip_serializing_if = "BTreeMap::is_empty")]
    pub from_law: BTreeMap<String, Result<Value, String>>,
}

/// What the rows are built against: the corpus and the date on which the
/// engine reads the regulation for an input from the law (that of the
/// assessment or the decision), and the as-of at which every source reduces
/// its chronicle (see [`AsOf`]).
#[derive(Clone, Copy)]
pub struct Environment<'a> {
    pub service: &'a LawExecutionService,
    pub date: &'a str,
    pub as_of: &'a AsOf,
}

/// The key of an input from the law.
fn law_key(regulation: &str, output: &str) -> String {
    format!("{regulation}#{output}")
}

/// Computes every input from the law of this rows definition, once, with the
/// combined parameters. Nothing is filled in: if the output lacks a fact,
/// the error says which.
fn from_law(
    rows: &Rows,
    parameters: &BTreeMap<String, Value>,
    law: Environment<'_>,
) -> BTreeMap<String, Result<Value, String>> {
    let mut out = BTreeMap::new();
    for b in rows.sources.iter().map(|b| &b.definition) {
        for i in b.input.values() {
            let RowInput::Law { regulation, output } = i else {
                continue;
            };
            let key = law_key(regulation, output);
            if out.contains_key(&key) {
                continue;
            }
            let e = crate::assessment::evaluate(
                law.service,
                regulation,
                &[output],
                parameters,
                law.date,
            );
            let value = match e.values.get(output.as_str()) {
                Some(w) if !w.is_null() => Ok(w.clone()),
                _ => Err(e.reason(&format!("{key} has no value"))),
            };
            out.insert(key, value);
        }
    }
    out
}

/// The value a lexostatus delivers under a name: a parameter or an extra
/// field.
fn from_lexostatus<'l>(
    lexostatuses: &'l [Lexostatus],
    name: &str,
    field: &str,
) -> Option<&'l Value> {
    lexostatuses.iter().find(|l| l.name == name)?.field(field)
}

/// The input for a source at a row. An error names what is absent.
fn input(
    source: &RowSource,
    row: &Map<String, Value>,
    lexostatuses: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
    law: &BTreeMap<String, Result<Value, String>>,
) -> Result<Map<String, Value>, String> {
    let mut out = Map::new();
    for (name, reference) in &source.input {
        let (value, what) = match reference {
            RowInput::Column { column } => (
                row.get(column).filter(|w| !w.is_null()),
                format!("column '{column}'"),
            ),
            RowInput::Own { lexostatus, field } => (
                from_lexostatus(lexostatuses, lexostatus, field),
                format!("lexostatus '{lexostatus}', field '{field}'"),
            ),
            RowInput::Parameter { parameter } => (
                parameters.get(parameter).filter(|w| !w.is_null()),
                format!("parameter '{parameter}'"),
            ),
            RowInput::Law { regulation, output } => match law.get(&law_key(regulation, output)) {
                Some(Ok(w)) => (Some(w), String::new()),
                Some(Err(f)) => return Err(format!("input '{name}' is absent: {f}")),
                None => (None, format!("output '{output}' of {regulation}")),
            },
            RowInput::Value { value } => (Some(value), String::new()),
        };
        let Some(value) = value else {
            return Err(format!("input '{name}' is absent: {what} has no value"));
        };
        out.insert(name.clone(), value.clone());
    }
    Ok(out)
}

/// The worse of two statuses: queried is best, then error, not queried and
/// unreachable.
fn worst(a: Status, b: Status) -> Status {
    fn rank(s: Status) -> u8 {
        match s {
            Status::Queried => 0,
            Status::Error => 1,
            Status::NotQueried => 2,
            Status::Unreachable => 3,
        }
    }
    if rank(b) > rank(a) {
        b
    } else {
        a
    }
}

/// How many rows query their sources concurrently.
const CONCURRENT: usize = 16;

/// How querying a source at a row went.
enum Query {
    /// With the route of the reduction, if the source named it.
    Queried(Option<String>),
    /// Not queried: an input was absent.
    NotQueried(String),
    /// Queried, without a lexostatus.
    Failed(Status, String),
}

/// What a row yielded: the row, the columns that are absent, and per source
/// (in the order of the definition) how querying went.
struct RowResult {
    row: Map<String, Value>,
    missing: BTreeSet<String>,
    sources: Vec<Query>,
}

/// Composes a row: the columns from the table, and then those of each
/// source, one after the other.
async fn compose_row(
    rows: &Rows,
    source: &Map<String, Value>,
    lexostatuses: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
    law: &BTreeMap<String, Result<Value, String>>,
    as_of: &AsOf,
) -> RowResult {
    let mut out = RowResult {
        row: Map::new(),
        missing: BTreeSet::new(),
        sources: Vec::new(),
    };
    for (column, name) in &rows.definition.columns {
        if let Some(w) = source.get(column) {
            out.row.insert(name.clone(), w.clone());
        } else {
            out.missing.insert(name.clone());
        }
    }
    for b in &rows.sources {
        let input = match input(&b.definition, &out.row, lexostatuses, parameters, law) {
            Ok(i) => i,
            Err(f) => {
                out.sources.push(Query::NotQueried(f));
                out.missing.extend(b.definition.columns.values().cloned());
                continue;
            }
        };
        let delivered = match b.request(&input, as_of).await {
            Ok(l) => {
                out.sources.push(Query::Queried(
                    l.reduction.as_ref().map(|r| r.route.clone()),
                ));
                crate::synthesis::delivered(l)
            }
            Err(f) => {
                let status = match f {
                    TransportError::Unreachable(_) => Status::Unreachable,
                    TransportError::Response { .. } | TransportError::Json(_) => Status::Error,
                };
                out.sources.push(Query::Failed(status, f.to_string()));
                BTreeMap::new()
            }
        };
        for (of, to) in &b.definition.columns {
            match delivered.get(of).filter(|w| !w.is_null()) {
                Some(w) => {
                    out.row.insert(to.clone(), w.clone());
                }
                None => {
                    out.missing.insert(to.clone());
                }
            }
        }
    }
    out
}

/// Builds the array parameter. For every row of the table field the columns
/// from the configuration go along, and then the columns the sources deliver
/// per row. If something is absent, that column stays out.
pub async fn compose(
    rows: &Rows,
    lexostatuses: &[Lexostatus],
    parameters: &BTreeMap<String, Value>,
    law: Environment<'_>,
) -> Option<RowsOutcome> {
    let d = &rows.definition;
    let where_ = format!(
        "lexostatus '{}', field '{}'",
        d.table.lexostatus, d.table.field
    );
    let not_buildable = |error: String| RowsOutcome {
        parameter: d.parameter.clone(),
        error: Some(error),
        rows: Vec::new(),
        sources: Vec::new(),
        missing: Vec::new(),
        from_law: BTreeMap::new(),
    };
    let table = from_lexostatus(lexostatuses, &d.table.lexostatus, &d.table.field)?;
    let Some(table) = table.as_array() else {
        return Some(not_buildable(format!("{where_} is not a list of rows")));
    };
    let mut table_rows = Vec::new();
    for (i, r) in table.iter().enumerate() {
        match r.as_object() {
            Some(r) => table_rows.push(r),
            None => {
                return Some(not_buildable(format!(
                    "{where_}: row {i} is not an object with columns"
                )))
            }
        }
    }

    let mut results: Vec<SourceResult> = rows
        .sources
        .iter()
        .map(|b| SourceResult {
            cell: b.definition.cell.clone(),
            lexostatus: b.definition.lexostatus.clone(),
            transport: b.transport.kind(),
            queried: 0,
            status: Status::Queried,
            error: None,
            reduction: None,
        })
        .collect();
    let mut out_rows = Vec::new();
    let mut missing: BTreeSet<String> = BTreeSet::new();

    // The rows concurrently (at most CONCURRENT), in the order of the table;
    // within a row the sources one after the other, because a source can
    // take a column of an earlier one as input.
    // First the futures in a list: a stream over a closure with references
    // makes the future of a route not Send.
    let derived = from_law(rows, parameters, law);
    let mut ask = Vec::with_capacity(table_rows.len());
    for r in table_rows {
        ask.push(compose_row(
            rows,
            r,
            lexostatuses,
            parameters,
            &derived,
            law.as_of,
        ));
    }
    let per_row: Vec<RowResult> = stream::iter(ask).buffered(CONCURRENT).collect().await;
    for r in per_row {
        for (result, query) in results.iter_mut().zip(r.sources) {
            match query {
                Query::Queried(route) => {
                    result.queried += 1;
                    if let Some(route) = route {
                        let r = result.reduction.get_or_insert_with(String::new);
                        if !r.split(", ").any(|x| x == route) {
                            if !r.is_empty() {
                                r.push_str(", ");
                            }
                            r.push_str(&route);
                        }
                    }
                }
                Query::NotQueried(f) => {
                    result.status = worst(result.status, Status::NotQueried);
                    result.error.get_or_insert(f);
                }
                Query::Failed(status, f) => {
                    result.queried += 1;
                    result.status = worst(result.status, status);
                    result.error.get_or_insert(f);
                }
            }
        }
        missing.extend(r.missing);
        out_rows.push(Value::Object(r.row));
    }
    Some(RowsOutcome {
        parameter: d.parameter.clone(),
        error: None,
        rows: out_rows,
        sources: results,
        missing: missing.into_iter().collect(),
        from_law: derived,
    })
}

/// Runs the rows definitions on a combination, before the engine: every
/// array parameter is added, with provenance per row. `own` are the
/// lexostatuses of the case (for the assessment: the trial reduction of the
/// draft); a table can also be an extra field a synthesis source passed on,
/// which then sits next to the own lexostatuses under the name of that
/// source. Shared by the assessment and the decision.
pub async fn apply(
    rows: &[Rows],
    own: &[Lexostatus],
    combined: &mut Combination,
    law: Environment<'_>,
) -> Vec<RowsOutcome> {
    let mut with_sources = own.to_vec();
    for u in combined
        .sources
        .iter()
        .filter(|u| !u.extra_fields.is_empty())
    {
        with_sources.push(Lexostatus {
            extra_fields: u
                .extra_fields
                .iter()
                .map(|(k, v)| (k.clone(), v.clone()))
                .collect(),
            ..Lexostatus::empty(&u.lexostatus)
        });
    }
    let mut results = Vec::new();
    for r in rows {
        let Some(result) = compose(r, &with_sources, &combined.parameters, law).await else {
            continue;
        };
        if result.error.is_some() {
            results.push(result);
            continue;
        }
        combined
            .parameters
            .insert(result.parameter.clone(), Value::Array(result.rows.clone()));
        combined.provenance.insert(
            result.parameter.clone(),
            Provenance::PerRow {
                lexostatus: r.definition.table.lexostatus.clone(),
                field: r.definition.table.field.clone(),
            },
        );
        results.push(result);
    }
    results
}

/// Checks a rows definition at load time: the table comes from one of the
/// `own` lexostatuses (which delivers it) or from an extra field a synthesis
/// source passes on, every column comes from only one place, a source is
/// another cell, and every input of a source is filled (by something before
/// it; a parameter by the synthesis). `who` is the
/// execution (assessment or decision), `before` are the rows definitions of
/// that execution that run before `r`, `otherwise` says what another
/// lexostatus then is not.
pub fn check(
    who: &str,
    r: &RowsDefinition,
    before: &[RowsDefinition],
    own: &[&str],
    otherwise: &str,
    d: &ProcessDefinition,
    cell: &Cell,
) -> Vec<String> {
    let mut errors = Vec::new();
    let who = format!("{who}, rows '{}'", r.parameter);
    // A table or input may also come from an extra field a synthesis source
    // passes on (for example the rows of a result from a register).
    let passed_on_field = |lexostatus: &str, field: &str| {
        d.other_sources()
            .any(|s| s.lexostatus == lexostatus && s.extra_fields.iter().any(|e| e == field))
    };
    // A parameter input comes from what the synthesis combines before these
    // rows: an own lexostatus, a synthesis source, or a rows definition of
    // the same execution that runs before it.
    let parameter_known = |p: &str| {
        own.iter().any(|l| {
            cell.lexostatuses
                .lexostatus(l)
                .is_some_and(|l| l.reduction.derivations.contains_key(p))
        }) || d
            .other_sources()
            .any(|s| s.parameters.iter().any(|x| x == p))
            || before.iter().any(|x| x.parameter == p)
    };
    if passed_on_field(&r.table.lexostatus, &r.table.field) {
        // From a source: the synthesis checks the source itself.
    } else if !own.contains(&r.table.lexostatus.as_str()) {
        errors.push(format!(
            "{who}: the table comes from lexostatus '{}', and that is {otherwise}",
            r.table.lexostatus
        ));
    } else if !cell
        .lexostatuses
        .lexostatus(&r.table.lexostatus)
        .is_some_and(|l| l.delivers(&r.table.field))
    {
        errors.push(format!(
            "{who}: lexostatus '{}' does not deliver '{}' (no derivation and no extra field)",
            r.table.lexostatus, r.table.field
        ));
    }
    // Every column name comes from only one place: the table or a source.
    let mut columns: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for name in r.columns.values() {
        columns.entry(name).or_default().push("the table".into());
    }
    for source in &r.sources {
        let source_who = format!("{who}, source {}/{}", source.cell, source.lexostatus);
        if source.cell == cell.id() {
            errors.push(format!(
                "{source_who}: a source is another cell, not the cell itself"
            ));
        }
        for (i, v) in &source.input {
            match v {
                RowInput::Column { column } if !columns.contains_key(column.as_str()) => {
                    errors.push(format!(
                        "{source_who}, input '{i}': column '{column}' is not filled by anything before it"
                    ));
                }
                RowInput::Law { regulation, output }
                    if cell
                        .service
                        .resolver()
                        .get_article_by_output(regulation, output, None)
                        .is_none() =>
                {
                    errors.push(format!(
                        "{source_who}, input '{i}': regulation '{regulation}' has no output '{output}'"
                    ));
                }
                RowInput::Own { lexostatus, field }
                    if !passed_on_field(lexostatus, field)
                        && !cell
                            .lexostatuses
                            .lexostatus(lexostatus)
                            .is_some_and(|l| l.delivers(field)) =>
                {
                    errors.push(format!(
                        "{source_who}, input '{i}': lexostatus '{lexostatus}' does not deliver '{field}'"
                    ));
                }
                RowInput::Parameter { parameter } if !parameter_known(parameter) => {
                    errors.push(format!(
                        "{source_who}, input '{i}': parameter '{parameter}' comes from no own lexostatus, synthesis source or other rows"
                    ));
                }
                _ => {}
            }
        }
        // A source's own columns count only after its inputs: an input
        // cannot be filled by the source it is the input of.
        for name in source.columns.values() {
            columns
                .entry(name)
                .or_default()
                .push(format!("source {}/{}", source.cell, source.lexostatus));
        }
    }
    for (name, where_) in &columns {
        if where_.len() > 1 {
            errors.push(format!(
                "{who}: column '{name}' comes from more than one place: {}",
                where_.join(", ")
            ));
        }
    }
    errors
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::trial::Fixed;
    use serde_json::json;
    use std::sync::Arc;

    /// A fictional regulation that derives a reference date from a year.
    const AS_OF: &str = r#"
$id: testregeling_peil
regulatory_layer: WET
publication_date: '2025-01-01'
articles:
  - number: '1'
    text: Het tarief geldt op 1 januari van het jaar.
    machine_readable:
      execution:
        parameters:
          - {name: jaar, type: number, required: true}
        output:
          - {name: peildatum, type: date}
        actions:
          - output: peildatum
            value: {operation: DATE, year: $jaar, month: 1, day: 1}
"#;

    const NO_AS_OF: AsOf = AsOf {
        as_of: None,
        known_at: None,
    };

    fn service() -> LawExecutionService {
        let mut s = LawExecutionService::new();
        s.load_law(AS_OF).unwrap();
        s
    }

    fn law(service: &LawExecutionService) -> Environment<'_> {
        Environment {
            service,
            date: "2026-05-01",
            as_of: &NO_AS_OF,
        }
    }

    fn own() -> Vec<Lexostatus> {
        vec![serde_json::from_value(json!({
            "name": "aanvraag",
            "parameters": {},
            "extra_fields": {
                "aanduiding": "EEN LIJST",
                "organen": [
                    {"orgaan": "raad", "gebied": "A", "zetels": 10, "aantal": null},
                    {"orgaan": "raad", "gebied": "B", "zetels": 3, "aantal": 2},
                ],
            },
        }))
        .unwrap()]
    }

    fn definition() -> RowsDefinition {
        serde_json::from_value(json!({
            "parameter": "tabel",
            "table": {"lexostatus": "aanvraag", "field": "organen"},
            "columns": {"orgaan": "orgaan", "gebied": "gebiedscode", "zetels": "zetels", "aantal": "samenstellende"},
        }))
        .unwrap()
    }

    fn source(response: Result<Value, TransportError>, input: Value) -> (Source, Arc<Fixed>) {
        let t = Arc::new(Fixed::new(response));
        let definition: RowSource = serde_json::from_value(json!({
            "cell": "register",
            "lexostatus": "per_gebied",
            "input": input,
            "columns": {"bedrag": "tarief"},
        }))
        .unwrap();
        (
            Source {
                definition,
                transport: t.clone(),
            },
            t,
        )
    }

    #[tokio::test]
    async fn every_row_gets_its_columns() {
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {}, "extra_fields": {"bedrag": 5}})),
            json!({"gebied": {"column": "gebiedscode"}}),
        );
        let rows = Rows {
            definition: definition(),
            sources: vec![b],
        };
        let u = compose(&rows, &own(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert_eq!(
            t.ask().as_slice(),
            [
                "/cells/register/api/lexostatus/per_gebied?gebied=A",
                "/cells/register/api/lexostatus/per_gebied?gebied=B"
            ]
        );
        assert_eq!(
            u.rows[0],
            json!({"orgaan": "raad", "gebiedscode": "A", "zetels": 10, "samenstellende": null, "tarief": 5})
        );
        assert_eq!(u.rows[1]["samenstellende"], json!(2));
        assert!(u.missing.is_empty(), "{:?}", u.missing);
        assert_eq!(u.sources[0].queried, 2);
        assert_eq!(u.sources[0].status, Status::Queried);
    }

    #[tokio::test]
    async fn an_unreachable_source_fills_in_nothing() {
        let (b, _) = source(
            Err(TransportError::Unreachable("weg".into())),
            json!({"gebied": {"column": "gebiedscode"}}),
        );
        let rows = Rows {
            definition: definition(),
            sources: vec![b],
        };
        let u = compose(&rows, &own(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(!u.rows[0].as_object().unwrap().contains_key("tarief"));
        assert_eq!(u.missing, ["tarief"]);
        assert_eq!(u.sources[0].status, Status::Unreachable);
    }

    #[tokio::test]
    async fn input_from_the_law_and_a_fixed_value() {
        // The regulation derives the reference date from the year; the
        // configuration converts nothing.
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({
                "gebied": {"column": "gebiedscode"},
                "peildatum": {"regulation": "testregeling_peil", "output": "peildatum"},
                "naam": {"lexostatus": "aanvraag", "field": "aanduiding"},
                "soort": {"value": "raad"},
            }),
        );
        let rows = Rows {
            definition: definition(),
            sources: vec![b],
        };
        let mut parameters = BTreeMap::new();
        parameters.insert("jaar".to_string(), json!(2026));
        let u = compose(&rows, &own(), &parameters, law(&service()))
            .await
            .unwrap();
        assert_eq!(
            t.ask()[0],
            "/cells/register/api/lexostatus/per_gebied?gebied=A&naam=EEN+LIJST&peildatum=2026-01-01&soort=raad"
        );
        assert_eq!(u.rows[0]["tarief"], json!(7));
        assert_eq!(
            u.from_law["testregeling_peil#peildatum"],
            Ok(json!("2026-01-01"))
        );
    }

    /// If the law cannot derive the input, the source is not queried and the
    /// error says what the law lacked.
    #[tokio::test]
    async fn input_from_the_law_that_lacks_a_fact() {
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({"peildatum": {"regulation": "testregeling_peil", "output": "peildatum"}}),
        );
        let rows = Rows {
            definition: definition(),
            sources: vec![b],
        };
        let u = compose(&rows, &own(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(t.ask().is_empty());
        let error = u.sources[0].error.as_deref().unwrap();
        assert!(error.contains("input 'peildatum' is absent"), "{error}");
        assert!(error.contains("jaar"), "{error}");
        assert_eq!(u.missing, ["tarief"]);
    }

    #[tokio::test]
    async fn without_input_the_source_is_not_queried() {
        let (b, t) = source(
            Ok(json!({"name": "per_gebied", "parameters": {"bedrag": 7}})),
            json!({"peildatum": {"parameter": "ontbreekt"}}),
        );
        let rows = Rows {
            definition: definition(),
            sources: vec![b],
        };
        let u = compose(&rows, &own(), &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(t.ask().is_empty());
        assert_eq!(u.sources[0].status, Status::NotQueried);
        assert!(u.sources[0]
            .error
            .as_ref()
            .unwrap()
            .contains("input 'peildatum' is absent"));
        assert_eq!(u.missing, ["tarief"]);
    }

    #[tokio::test]
    async fn a_row_that_is_not_an_object_builds_no_table() {
        let mut l = own();
        l[0].extra_fields.insert(
            "organen".into(),
            json!([{"orgaan": "raad", "gebied": "A", "zetels": 1}, "raad B"]),
        );
        let rows = Rows {
            definition: definition(),
            sources: Vec::new(),
        };
        let u = compose(&rows, &l, &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(u.rows.is_empty());
        assert!(
            u.error
                .as_deref()
                .unwrap()
                .contains("row 1 is not an object"),
            "{u:?}"
        );
        // Then the parameter does not go to the engine.
        let mut combined = Combination {
            parameters: BTreeMap::new(),
            provenance: BTreeMap::new(),
            sources: Vec::new(),
        };
        let out = apply(
            std::slice::from_ref(&rows),
            &l,
            &mut combined,
            law(&service()),
        )
        .await;
        assert!(out[0].error.is_some());
        assert!(!combined.parameters.contains_key("tabel"));
    }

    /// A source that waits a moment, counts how many queries run at once,
    /// and returns the area from the query as the amount.
    struct Slow {
        busy: std::sync::atomic::AtomicUsize,
        highest: std::sync::atomic::AtomicUsize,
    }

    impl crate::transport::Transport for Slow {
        fn kind(&self) -> &'static str {
            "internal"
        }
        fn fetch<'a>(&'a self, path: &'a str) -> crate::transport::Response<'a> {
            use std::sync::atomic::Ordering::SeqCst;
            Box::pin(async move {
                let current = self.busy.fetch_add(1, SeqCst) + 1;
                self.highest.fetch_max(current, SeqCst);
                tokio::time::sleep(std::time::Duration::from_millis(50)).await;
                self.busy.fetch_sub(1, SeqCst);
                let area = path.rsplit_once("gebied=").unwrap().1.to_string();
                Ok(
                    json!({"name": "per_gebied", "parameters": {}, "extra_fields": {"bedrag": area}}),
                )
            })
        }
        fn send<'a>(&'a self, path: &'a str, _body: &'a Value) -> crate::transport::Response<'a> {
            self.fetch(path)
        }
    }

    #[tokio::test]
    async fn the_rows_are_queried_concurrently_in_their_own_order() {
        let organen: Vec<Value> = (0..8)
            .map(|i| json!({"orgaan": "raad", "gebied": format!("G{i}"), "zetels": i}))
            .collect();
        let mut l = own();
        l[0].extra_fields
            .insert("organen".into(), Value::Array(organen));
        let t = Arc::new(Slow {
            busy: 0.into(),
            highest: 0.into(),
        });
        let (mut b, _) = source(
            Ok(Value::Null),
            json!({"gebied": {"column": "gebiedscode"}}),
        );
        b.transport = t.clone();
        let rows = Rows {
            definition: definition(),
            sources: vec![b],
        };
        let u = compose(&rows, &l, &BTreeMap::new(), law(&service()))
            .await
            .unwrap();
        assert!(t.highest.load(std::sync::atomic::Ordering::SeqCst) > 1);
        // Every row keeps its own response, in the order of the table.
        let rates: Vec<&str> = u
            .rows
            .iter()
            .map(|r| r["tarief"].as_str().unwrap())
            .collect();
        assert_eq!(rates, ["G0", "G1", "G2", "G3", "G4", "G5", "G6", "G7"]);
        assert_eq!(u.sources[0].queried, 8);
        assert_eq!(u.sources[0].status, Status::Queried);
    }

    #[tokio::test]
    async fn no_table_no_parameter() {
        let mut d = definition();
        d.table.field = "bestaat_niet".into();
        let rows = Rows {
            definition: d,
            sources: Vec::new(),
        };
        assert!(compose(&rows, &own(), &BTreeMap::new(), law(&service()))
            .await
            .is_none());
    }
}
