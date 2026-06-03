use std::collections::HashMap;
use std::sync::LazyLock;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::state::AppState;

static DEFAULTS: LazyLock<HashMap<String, bool>> = LazyLock::new(|| {
    HashMap::from([
        ("panel.article_text".into(), true),
        ("panel.scenario_form".into(), true),
        ("panel.yaml_editor".into(), true),
        ("panel.machine_readable".into(), true),
        ("panel.law_graph".into(), false),
        // Notes pane (RFC-005/RFC-018). Default off: display-only MVP, notes
        // exist for one law so far. Must be registered here or the toggle PUT
        // 400s and the frontend silently reverts it (see the editor.* note
        // below — same allow-list mechanism).
        ("panel.notes".into(), false),
        // AI-suggestiepane: toont de door de pipeline gegenereerde suggesties
        // (aanwijzingen + machine_readable) als annotaties met accept/reject.
        // Default off: dark-launch tot de suggest-pipeline op productie draait.
        // Moet hier staan of de toggle-PUT 400't (allow-list).
        ("panel.suggestions".into(), false),
        // Note authoring (RFC-018 write path, MVP: localStorage + manual
        // export). Separate gate from panel.notes so notes can be shown
        // read-only without exposing creation. Same allow-list rule: without
        // this key the toggle PUT 400s and the frontend silently reverts it.
        ("notes.create".into(), false),
        // Editor capability flags — visibility is panel.*, editability is editor.*.
        // The frontend wires editor.article_text_edit (default off) to gate
        // write access on the Tekst pane. Without the key here the backend's
        // allow-list check rejects the toggle PUT with 400, and the frontend
        // treats that as a failure and silently reverts the user's change —
        // so the editor would stay read-only no matter how many times the
        // menu toggle is flipped.
        ("editor.article_text_edit".into(), false),
    ])
});

fn defaults() -> HashMap<String, bool> {
    DEFAULTS.clone()
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
    if !DEFAULTS.contains_key(&key) {
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

    match regelrecht_pipeline::feature_flags::upsert_flag(pool, &key, body.enabled, None).await {
        Ok(_) => {
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
        Err(e) => {
            tracing::error!(error = %e, "failed to update feature flag");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        }
    }
}
