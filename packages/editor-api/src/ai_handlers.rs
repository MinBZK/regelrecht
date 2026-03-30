use std::collections::HashMap;

use axum::extract::State;
use axum::http::StatusCode;
use axum::Json;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::state::AppState;
use crate::vlam_client::VlamError;

#[derive(Deserialize)]
pub struct GenerateTitlesRequest {
    pub operations: Value,
}

#[derive(Serialize)]
pub struct GenerateTitlesResponse {
    pub titles: HashMap<String, String>,
}

#[derive(Serialize)]
pub struct ErrorResponse {
    pub error: String,
}

pub async fn generate_operation_titles(
    State(state): State<AppState>,
    Json(req): Json<GenerateTitlesRequest>,
) -> Result<Json<GenerateTitlesResponse>, (StatusCode, Json<ErrorResponse>)> {
    let vlam = state.vlam.as_ref().ok_or_else(|| {
        (
            StatusCode::NOT_IMPLEMENTED,
            Json(ErrorResponse {
                error: VlamError::NotConfigured.to_string(),
            }),
        )
    })?;

    let titles = vlam.generate_titles(&req.operations).await.map_err(|e| {
        tracing::warn!(error = %e, "VLAM title generation failed");
        (
            StatusCode::BAD_GATEWAY,
            Json(ErrorResponse {
                error: e.to_string(),
            }),
        )
    })?;

    Ok(Json(GenerateTitlesResponse { titles }))
}
