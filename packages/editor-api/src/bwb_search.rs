use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use std::collections::HashMap;

use crate::state::AppState;

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

pub async fn search_bwb(
    State(state): State<AppState>,
    Query(params): Query<SearchParams>,
) -> Result<Json<Vec<BwbSearchResult>>, (StatusCode, String)> {
    let q = params.q.trim();
    if q.is_empty() || q.len() < 2 {
        return Ok(Json(vec![]));
    }

    // Build CQL query: search by title, optionally filter to active laws
    let cql = format!("overheidbwb.titel any \"{}\"", q.replace('"', ""));

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
    .map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("URL build error: {e}"),
        )
    })?;

    let response = state
        .http_client
        .get(url)
        .send()
        .await
        .map_err(|e| (StatusCode::BAD_GATEWAY, format!("BWB search failed: {e}")))?;

    let xml_text = response.text().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("BWB response read failed: {e}"),
        )
    })?;

    let results = parse_sru_response(&xml_text).map_err(|e| {
        (
            StatusCode::INTERNAL_SERVER_ERROR,
            format!("XML parse error: {e}"),
        )
    })?;

    Ok(Json(results))
}

/// Parse SRU XML response and extract unique laws (deduplicated by BWBR ID).
fn parse_sru_response(xml: &str) -> Result<Vec<BwbSearchResult>, String> {
    let doc = roxmltree::Document::parse(xml).map_err(|e| e.to_string())?;

    let mut seen: HashMap<String, BwbSearchResult> = HashMap::new();

    for node in doc.descendants() {
        if !node.is_element() {
            continue;
        }
        let tag = node.tag_name().name();
        if tag != "owmskern" {
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
            seen.entry(bwb_id.clone()).or_insert(BwbSearchResult {
                bwb_id,
                title,
                law_type: law_type.unwrap_or_default(),
            });
        }
    }

    let mut results: Vec<BwbSearchResult> = seen.into_values().collect();
    results.sort_by(|a, b| a.title.cmp(&b.title));
    Ok(results)
}
