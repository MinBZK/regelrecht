use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

use crate::ApiState;

const SRU_BASE: &str = "https://zoekservice.overheid.nl/sru/Search";
const MAX_RESULTS: u32 = 20;

#[derive(Deserialize)]
pub struct SearchParams {
    pub q: String,
}

#[derive(Serialize, Clone)]
pub struct BwbSearchResult {
    pub bwb_id: String,
    pub title: String,
    #[serde(rename = "type")]
    pub law_type: String,
}

/// GET /harvest/search?q=...
///
/// Search wetten.overheid.nl via the SRU API for laws matching the query.
pub async fn search_bwb(
    State(state): State<ApiState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<BwbSearchResult>>, (StatusCode, String)> {
    match search_bwb_by_name(&state.http_client, params.q.trim()).await {
        Ok(results) => Ok(Json(results)),
        Err(e) => Err((StatusCode::BAD_GATEWAY, e)),
    }
}

/// Search wetten.overheid.nl via the SRU API for laws matching `q`.
///
/// The client-taking core shared by the axum handler and the enrich worker's
/// related-legislation resolution. Queries shorter than 3 characters (after the
/// same sanitize as the handler) return an empty list rather than an error.
pub async fn search_bwb_by_name(
    client: &reqwest::Client,
    q: &str,
) -> Result<Vec<BwbSearchResult>, String> {
    let q = q.trim();
    if q.len() < 3 {
        return Ok(vec![]);
    }

    let sanitized: String = q
        .chars()
        .filter(|c| c.is_alphanumeric() || *c == ' ' || *c == '-' || *c == '.')
        .collect();
    let cql = format!("overheidbwb.titel any \"{sanitized}\"");

    let url = url::Url::parse_with_params(
        SRU_BASE,
        &[
            ("operation", "searchRetrieve"),
            ("version", "1.2"),
            ("x-connection", "BWB"),
            ("query", &cql),
            ("maximumRecords", &MAX_RESULTS.to_string()),
        ],
    )
    .map_err(|e| format!("URL build error: {e}"))?;

    let response = client
        .get(url)
        .send()
        .await
        .map_err(|e| format!("BWB search failed: {e}"))?;

    let xml_text = response
        .text()
        .await
        .map_err(|e| format!("BWB response read failed: {e}"))?;

    parse_sru_response(&xml_text).map_err(|e| format!("XML parse error: {e}"))
}

/// Parse SRU XML response and extract unique laws (deduplicated by BWBR ID).
fn parse_sru_response(xml: &str) -> Result<Vec<BwbSearchResult>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;

    // SRU returns results in relevance order — preserve that ordering by
    // deduplicating via a HashSet of seen IDs while pushing into a Vec.
    let mut seen: HashSet<String> = HashSet::new();
    let mut results: Vec<BwbSearchResult> = Vec::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        if node.tag_name().name() != "owmskern" {
            continue;
        }

        let mut identifier = None;
        let mut title = None;
        let mut law_type = None;

        for child in node.children().filter(|n| n.is_element()) {
            match child.tag_name().name() {
                "identifier" => identifier = child.text().map(|s| s.trim().to_string()),
                "title" => title = child.text().map(|s| s.trim().to_string()),
                "type" => law_type = child.text().map(|s| s.trim().to_string()),
                _ => {}
            }
        }

        if let (Some(bwb_id), Some(title)) = (identifier, title) {
            if !bwb_id.starts_with("BWBR") {
                continue;
            }
            if seen.insert(bwb_id.clone()) {
                results.push(BwbSearchResult {
                    bwb_id,
                    title,
                    law_type: law_type.unwrap_or_default(),
                });
            }
        }
    }

    Ok(results)
}
