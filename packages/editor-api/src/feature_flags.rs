use std::collections::HashMap;
use std::sync::LazyLock;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;
use serde::Deserialize;

use crate::state::AppState;

/// Key of the GitHub user-OAuth flag (PR #887). Shared with the write path
/// (`credentials::write_requires_user_token`), which reads this flag to
/// decide whether traject writes must carry the acting user's own token.
pub const GITHUB_USER_OAUTH: &str = "github.user_oauth";

static DEFAULTS: LazyLock<HashMap<String, bool>> = LazyLock::new(|| {
    HashMap::from([
        ("panel.article_text".into(), true),
        ("panel.scenario_form".into(), true),
        ("panel.yaml_editor".into(), true),
        ("panel.machine_readable".into(), true),
        ("panel.law_graph".into(), false),
        // Note authoring (RFC-018 write path, MVP: localStorage + manual
        // export). Same allow-list rule: without this key the toggle PUT 400s
        // and the frontend silently reverts it.
        ("notes.create".into(), false),
        // Editor capability flags — visibility is panel.*, editability is editor.*.
        // The frontend wires editor.article_text_edit (default off) to gate
        // write access on the Tekst pane. Without the key here the backend's
        // allow-list check rejects the toggle PUT with 400, and the frontend
        // treats that as a failure and silently reverts the user's change —
        // so the editor would stay read-only no matter how many times the
        // menu toggle is flipped.
        ("editor.article_text_edit".into(), false),
        // GitHub user-OAuth (spike, PR #887). One switch, two effects: it
        // shows the "Koppel GitHub-account" affordance in the account menu
        // AND makes traject writes require the acting user's own GitHub token
        // (`credentials::write_requires_user_token`) — linking is never
        // offered-but-inert. Default off so the spike stays invisible until
        // opted in. Same allow-list rule: without this key the toggle PUT
        // 400s and the frontend silently reverts it, so the toggle would
        // never stick.
        (GITHUB_USER_OAUTH.into(), false),
    ])
});

fn defaults() -> HashMap<String, bool> {
    DEFAULTS.clone()
}

/// Effective value of a single flag: the stored row when present, else the
/// registered default. Errors propagate — the write path treats "can't read
/// the flag" as a failure, never as "assume the default".
pub async fn flag_enabled(
    pool: &sqlx::PgPool,
    key: &str,
) -> regelrecht_pipeline::error::Result<bool> {
    Ok(regelrecht_pipeline::feature_flags::get_flag(pool, key)
        .await?
        .map(|flag| flag.enabled)
        .unwrap_or_else(|| DEFAULTS.get(key).copied().unwrap_or(false)))
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
