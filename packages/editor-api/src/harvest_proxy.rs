use axum::body::Body;
use axum::extract::State;
use axum::http::{Request, StatusCode};
use axum::response::{IntoResponse, Response};

use crate::state::AppState;

/// Generic reverse proxy handler for `/api/harvest/*` requests.
///
/// Strips the `/api` prefix and forwards the remaining path + query string
/// to the pipeline-api service. Returns 503 if pipeline-api is not configured.
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
        .unwrap_or(req.uri().path());
    let query = req
        .uri()
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let upstream_url = format!("{pipeline_url}{path}{query}");

    // Forward the request
    let method = req.method().clone();
    let mut builder = state.http_client.request(method, &upstream_url);

    // Forward content-type header if present
    if let Some(ct) = req.headers().get("content-type") {
        builder = builder.header("content-type", ct);
    }

    // Forward the body
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
        tracing::error!(error = %e, url = %upstream_url, "pipeline-api request failed");
        (
            StatusCode::BAD_GATEWAY,
            format!("Pipeline API request failed: {e}"),
        )
    })?;

    // Convert reqwest response to axum response
    let status = StatusCode::from_u16(upstream_response.status().as_u16())
        .unwrap_or(StatusCode::INTERNAL_SERVER_ERROR);

    let mut response_builder = Response::builder().status(status);

    // Forward content-type from upstream
    if let Some(ct) = upstream_response.headers().get("content-type") {
        response_builder = response_builder.header("content-type", ct);
    }

    let response_body = upstream_response.bytes().await.map_err(|e| {
        (
            StatusCode::BAD_GATEWAY,
            format!("failed to read pipeline-api response: {e}"),
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
