use std::collections::HashMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::state::AppState;

fn defaults() -> HashMap<String, bool> {
    HashMap::from([
        ("panel.article_text".into(), true),
        ("panel.scenario_form".into(), true),
        ("panel.yaml_editor".into(), true),
        ("panel.execution_trace".into(), true),
        ("panel.machine_readable".into(), false),
    ])
}

pub async fn list_feature_flags(State(state): State<AppState>) -> Json<HashMap<String, bool>> {
    let Some(pool) = &state.pool else {
        return Json(defaults());
    };

    match regelrecht_pipeline::feature_flags::list_flags(pool).await {
        Ok(rows) => {
            let mut flags = defaults();
            for flag in rows {
                flags.insert(flag.key, flag.enabled);
            }
            Json(flags)
        }
        Err(e) => {
            tracing::warn!(error = %e, "failed to fetch feature flags, using defaults");
            Json(defaults())
        }
    }
}

#[derive(Deserialize)]
pub struct UpdateFlag {
    pub enabled: bool,
}

pub async fn update_feature_flag(
    State(state): State<AppState>,
    Path(key): Path<String>,
    Json(body): Json<UpdateFlag>,
) -> impl IntoResponse {
    if !defaults().contains_key(&key) {
        return (
            StatusCode::BAD_REQUEST,
            Json(serde_json::json!({"error": format!("unknown flag key '{}'", key)})),
        )
            .into_response();
    }

    let Some(pool) = &state.pool else {
        return (
            StatusCode::SERVICE_UNAVAILABLE,
            Json(serde_json::json!({"error": "no database configured"})),
        )
            .into_response();
    };

    match regelrecht_pipeline::feature_flags::set_flag(pool, &key, body.enabled).await {
        Ok(Some(_)) => {
            // Return the full flag map after update
            match regelrecht_pipeline::feature_flags::list_flags(pool).await {
                Ok(rows) => {
                    let mut flags = defaults();
                    for flag in rows {
                        flags.insert(flag.key, flag.enabled);
                    }
                    Json(flags).into_response()
                }
                Err(e) => {
                    tracing::warn!(error = %e, "failed to list flags after update");
                    StatusCode::INTERNAL_SERVER_ERROR.into_response()
                }
            }
        }
        Ok(None) => (
            StatusCode::NOT_FOUND,
            Json(serde_json::json!({"error": format!("flag '{}' not found", key)})),
        )
            .into_response(),
        Err(e) => {
            tracing::error!(error = %e, "failed to update feature flag");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
