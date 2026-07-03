use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// Reverse proxy handler for `/api/harvest/*` requests.
///
/// Strips the `/api` prefix and forwards the remaining path + query string
/// to the pipeline-api service. Returns 503 if pipeline-api is not configured.
/// Only `content-type` is forwarded (pipeline-api has no auth of its own).
pub async fn proxy_harvest(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    let pipeline_url = state.pipeline_api_url.as_deref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Pipeline API not configured".to_string(),
    ))?;

    // Build the upstream URL: strip /api prefix, keep /harvest/... path + query
    let path = req
        .uri()
        .path()
        .strip_prefix("/api")
        .unwrap_or(req.uri().path())
        .to_string();

    forward(&state, pipeline_url, &path, req, false, "pipeline-api").await
}

/// Reverse proxy handler for `/api/harvest-admin/*` requests → the standalone
/// harvester-admin API.
///
/// Rewrites `/api/harvest-admin/<x>` → `/api/<x>` and, unlike the pipeline
/// proxy, forwards the caller's session `Cookie`. Because editor-api and the
/// harvester-admin service share the same Postgres session store (and default
/// session cookie name), the harvester-admin service resolves that cookie to
/// the same authenticated session — including the user's realm roles — and
/// enforces its own `harvester-*` gates, so OIDC-only writes work without any
/// service token. editor-api still gates these routes on `harvester-reader`
/// up front as defence-in-depth. Returns 503 if the harvester-admin URL is
/// not configured.
pub async fn proxy_harvest_admin(
    State(state): State<AppState>,
    req: Request<Body>,
) -> Result<Response, (StatusCode, String)> {
    let base = state.harvest_admin_url.as_deref().ok_or((
        StatusCode::SERVICE_UNAVAILABLE,
        "Harvester-admin API not configured".to_string(),
    ))?;

    let upstream_path = harvest_admin_upstream_path(req.uri().path());

    forward(&state, base, &upstream_path, req, true, "harvester-admin").await
}

/// Map an editor-side `/api/harvest-admin/...` path to the harvester-admin
/// service's own `/api/...` path. `/api/harvest-admin` (no trailing segment)
/// maps to `/api`.
fn harvest_admin_upstream_path(req_path: &str) -> String {
    let rest = req_path
        .strip_prefix("/api/harvest-admin")
        .unwrap_or(req_path);
    format!("/api{rest}")
}

/// Shared reverse-proxy body: builds `{upstream_base}{upstream_path}?{query}`,
/// forwards method, `content-type` and (when `forward_cookie`) the `cookie`
/// header + request body, and streams the upstream response back. Only
/// `content-type` is forwarded from the upstream response — the editor-api
/// owns the browser's session cookie, so any upstream `Set-Cookie` is dropped.
async fn forward(
    state: &AppState,
    upstream_base: &str,
    upstream_path: &str,
    req: Request<Body>,
    forward_cookie: bool,
    upstream_label: &str,
) -> Result<Response, (StatusCode, String)> {
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let upstream_url = format!("{upstream_base}{upstream_path}{query}");

    let method = req.method().clone();
    let content_type = req.headers().get("content-type").cloned();
    let cookie = if forward_cookie {
        req.headers().get("cookie").cloned()
    } else {
        None
    };

    let mut builder = state.http_client.request(method, &upstream_url);
    if let Some(ct) = content_type {
        builder = builder.header("content-type", ct);
    }
    if let Some(cookie) = cookie {
        builder = builder.header("cookie", cookie);
    }

    let body_bytes = axum::body::to_bytes(req.into_body(), 1024 * 1024)
        .await
        .map_err(|e| {
            (
                StatusCode::BAD_REQUEST,
                format!("failed to read request body: {e}"),
            )
        })?;

    if !body_bytes.is_empty() {
        builder = builder.body(body_bytes);
    }

    let upstream_response = builder.send().await.map_err(|e| {
        tracing::error!(error = %e, url = %upstream_url, upstream = upstream_label, "upstream request failed");
        (
            StatusCode::BAD_GATEWAY,
            format!("{upstream_label} request failed: {e}"),
        )
    })?;

    let status = StatusCode::from_u16(upstream_response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    // Clone the upstream content-type before we consume the response body below.
    let response_content_type = upstream_response.headers().get("content-type").cloned();
    let mut response_builder = Response::builder().status(status);
    if let Some(ct) = response_content_type {
        response_builder = response_builder.header("content-type", ct);
    }

    let response_body = upstream_response.bytes().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("failed to read {upstream_label} response: {e}"),
        )
    })?;

    response_builder
        .body(Body::from(response_body))
        .map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                format!("failed to build response: {e}"),
            )
        })
        .map(IntoResponse::into_response)
}

#[cfg(test)]
mod tests {
    use super::harvest_admin_upstream_path;

    #[test]
    fn rewrites_law_entries() {
        assert_eq!(
            harvest_admin_upstream_path("/api/harvest-admin/law_entries"),
            "/api/law_entries"
        );
    }

    #[test]
    fn rewrites_nested_jobs_summary() {
        assert_eq!(
            harvest_admin_upstream_path("/api/harvest-admin/jobs/summary"),
            "/api/jobs/summary"
        );
    }

    #[test]
    fn rewrites_job_by_id() {
        assert_eq!(
            harvest_admin_upstream_path("/api/harvest-admin/jobs/abc-123"),
            "/api/jobs/abc-123"
        );
    }

    #[test]
    fn bare_prefix_maps_to_api_root() {
        assert_eq!(harvest_admin_upstream_path("/api/harvest-admin"), "/api");
    }

    #[test]
    fn unexpected_path_is_prefixed_with_api() {
        // Defensive: a path that doesn't carry the prefix still lands under
        // /api rather than silently hitting an unexpected upstream route.
        assert_eq!(harvest_admin_upstream_path("/other"), "/api/other");
    }
}
