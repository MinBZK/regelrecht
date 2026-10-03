//! The cell client: how a process queries the cell in which it records. A
//! process never reads a chronicle itself; it asks the cell along its
//! routes, through a [`Transport`], like any consumer.
//!
//! This holds the requests and responses of those routes as types, so that a
//! response that does not have the expected shape is an error and does not
//! silently become an empty value.

use std::collections::BTreeMap;

use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};

use crate::gram::{Gram, Input, Receipt};
use crate::reduction::{CaseState, Lexostatus, CASE_STATE, OWNER, OWNER_PATH};
use crate::transport::{Transport, TransportError};

/// What a process asks the cell to record (`POST /api/grams`), or to reduce
/// on trial. The cell builds the gram from its stream: the process only
/// gives the input.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct RecordRequest {
    /// Who has it recorded. The cell refuses if that is not the
    /// `recording_actor` of the stream.
    pub actor: String,
    pub stream: String,
    pub event: String,
    /// The intake channel (`$intake.*`): who submitted and by which route.
    #[serde(default)]
    pub intake: Value,
    /// The content (`$external.*`).
    #[serde(default)]
    pub external: Map<String, Value>,
    /// Per reference of the event, the id of the gram the new gram refers to
    /// (the application, the decision). The cell gives the id of the new
    /// gram.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub refers_to: BTreeMap<String, String>,
    /// Only for an action the process computed: the input with provenance and
    /// the receipt, and for a decision what makes it a decision. The process
    /// runs the engine, so the process assembles this.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub decision: Option<DecisionFields>,
    /// How many grams the group of the root had when the process read it.
    /// The cell only records if that is still so under its lock: what the
    /// process computed held for the group as it was then.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub root_grams: Option<usize>,
}

/// The fields of an action on a gram, besides the stream shape (see
/// [`crate::action::take`]).
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
pub struct DecisionFields {
    #[serde(default)]
    pub legal_character: Option<String>,
    #[serde(default)]
    pub decision_type: Option<String>,
    #[serde(default)]
    pub regulation: Option<String>,
    #[serde(default)]
    pub regulation_valid_from: Option<String>,
    #[serde(default)]
    pub competent_authority: Option<String>,
    #[serde(default)]
    pub acting_actor: Option<crate::gram::ActingActor>,
    #[serde(default)]
    pub inputs: BTreeMap<String, Input>,
    #[serde(default)]
    pub receipt: Option<Receipt>,
}

/// A gram as the cell returns it: with its YAML.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct WithYaml {
    pub gram: Gram,
    pub yaml: String,
}

/// The response to a trial reduction: the gram of the draft and the
/// lexostatus of the chronicle with that gram.
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct TrialReduction {
    pub gram: Gram,
    pub lexostatus: Lexostatus,
}

/// A route of a cell.
pub fn cell_path(cell: &str, route: &str) -> String {
    format!("/cells/{}/api/{route}", url_segment(cell))
}

fn read<T: DeserializeOwned>(v: Value, what: &str) -> Result<T, TransportError> {
    serde_json::from_value(v).map_err(|e| TransportError::Json(format!("{what}: {e}")))
}

fn write<T: Serialize>(v: &T) -> Result<Value, TransportError> {
    serde_json::to_value(v)
        .map_err(|e| TransportError::Json(format!("the request is not JSON: {e}")))
}

/// The grams with this root, each with YAML, as the cell filters them
/// (`GET cases/{root}`). A process never reads the whole chronicle: the
/// filtering is the cell's work. A 404: the cell does not know the root.
pub async fn read_case(
    cell: &dyn Transport,
    id: &str,
    root: &str,
) -> Result<Vec<WithYaml>, TransportError> {
    let v = cell
        .fetch(&cell_path(id, &format!("cases/{}", url_segment(root))))
        .await?;
    read(
        v,
        &format!("the cell gave no list of grams for root {root}"),
    )
}

/// The state of the group around a root, as the cell derives it (the
/// lexostatus [`CASE_STATE`], see [`CaseState`]). With `owner` (an
/// `$intake` path without `$intake.` and a value) the cell also says whether
/// someone with that value knows the group. A 404: the cell does not know the root.
pub async fn case_state(
    cell: &dyn Transport,
    id: &str,
    root: &str,
    owner: Option<(&str, &str)>,
) -> Result<CaseState, TransportError> {
    let mut query = vec![("root", root)];
    if let Some((path, value)) = owner {
        query.push((OWNER_PATH, path));
        query.push((OWNER, value));
    }
    let query = serde_urlencoded::to_string(&query)
        .map_err(|e| TransportError::Json(format!("the query cannot be written: {e}")))?;
    let v = cell
        .fetch(&cell_path(id, &format!("lexostatus/{CASE_STATE}?{query}")))
        .await?;
    let l: Lexostatus = read(v, "the cell gave no lexostatus for the root")?;
    CaseState::out(&l).map_err(TransportError::Json)
}

/// Have the cell record a gram (`POST grams`). Response: the
/// recorded gram with its YAML.
pub async fn record(
    cell: &dyn Transport,
    id: &str,
    request: &RecordRequest,
) -> Result<WithYaml, TransportError> {
    let v = cell.send(&cell_path(id, "grams"), &write(request)?).await?;
    read(v, "the recorded gram is unreadable")
}

/// Have the cell reduce a draft on trial to `lexostatus`
/// (`POST lexostatus/<name>/trial`), with `inputs` (and the as-of if needed);
/// nothing is recorded.
pub async fn trial(
    cell: &dyn Transport,
    id: &str,
    lexostatus: &str,
    concept: &RecordRequest,
    inputs: &Map<String, Value>,
) -> Result<TrialReduction, TransportError> {
    let body = json!({"draft": write(concept)?, "inputs": inputs});
    let v = cell
        .send(
            &cell_path(id, &format!("lexostatus/{}/trial", url_segment(lexostatus))),
            &body,
        )
        .await?;
    read(v, "the trial reduction of the cell is unreadable")
}

/// A path segment as it appears in a url: only letters, digits and
/// `_-.` remain.
pub fn url_segment(t: &str) -> String {
    t.bytes()
        .map(|b| {
            if b.is_ascii_alphanumeric() || b"_-.".contains(&b) {
                (b as char).to_string()
            } else {
                format!("%{b:02X}")
            }
        })
        .collect()
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::transport::Http;
    use axum::routing::get;
    use axum::{Json, Router};
    use std::time::Duration;

    /// A cell over HTTP that answers every request with something that is not
    /// a gram or a list of grams.
    async fn wrong_cell() -> Http {
        let router = Router::new()
            .route(
                "/cells/c/api/cases/{z}",
                get(|| async { Json(json!({"niet": "een lijst"})) }),
            )
            .route(
                "/cells/c/api/grams",
                axum::routing::post(|| async { Json(json!([1, 2])) }),
            );
        let listener = tokio::net::TcpListener::bind("127.0.0.1:0").await.unwrap();
        let address = listener.local_addr().unwrap();
        tokio::spawn(async move { axum::serve(listener, router).await });
        Http::new(&format!("http://{address}"), Duration::from_secs(5)).unwrap()
    }

    #[tokio::test]
    async fn an_unreadable_response_over_http_is_an_error() {
        let t = wrong_cell().await;
        let error = read_case(&t, "c", "z").await.unwrap_err();
        assert!(
            matches!(&error, TransportError::Json(r) if r.contains("no list of grams")),
            "{error:?}"
        );
        let error = record(&t, "c", &RecordRequest::default())
            .await
            .unwrap_err();
        assert!(
            matches!(&error, TransportError::Json(r) if r.contains("unreadable")),
            "{error:?}"
        );
    }

    /// A root and a cell are path segments: what is not safe in one is
    /// encoded, so a root cannot reach another route.
    #[tokio::test]
    async fn a_root_is_one_path_segment() {
        let t = crate::transport::trial::Fixed::new(Ok(json!([])));
        read_case(&t, "c/d", "../grams?x=1").await.unwrap();
        assert_eq!(t.ask(), ["/cells/c%2Fd/api/cases/..%2Fgrams%3Fx%3D1"]);
    }
}
