//! Reverse proxy to a PoC that runs as its own (unpublished) component.
//!
//! Deliberately not a reuse of `editor-api`'s `harvest_proxy::forward`. That
//! one drops the upstream's `Set-Cookie` on purpose, because editor-api owns
//! the browser's session cookie and the service behind it has none of its own.
//! Here the opposite holds: napp keeps its own `tower-sessions` cookie and its
//! own login, so cookies have to pass in both directions or nobody stays
//! logged in.
//!
//! The path is forwarded unchanged. napp is built with `base: /napp/` and
//! serves itself under that prefix, so stripping it here would leave its own
//! HTML pointing at assets this portal would then fail to find.

use axum::body::Body;
use axum::extract::Request;
use axum::http::{HeaderMap, HeaderName, StatusCode};
use axum::response::{IntoResponse, Response};
use futures_util::TryStreamExt as _;

/// Hop-by-hop headers: meaningful between two endpoints of one connection, and
/// wrong to copy onto the next hop (RFC 9110 §7.6.1). `content-length` goes
/// too, because the body is re-framed as a stream.
const NIET_DOORGEVEN: &[&str] = &[
    "connection",
    "keep-alive",
    "proxy-authenticate",
    "proxy-authorization",
    "te",
    "trailer",
    "transfer-encoding",
    "upgrade",
    "content-length",
    "host",
];

fn is_doorgeefbaar(name: &HeaderName) -> bool {
    !NIET_DOORGEVEN.contains(&name.as_str())
}

fn kopieer(from: &HeaderMap, to: &mut HeaderMap) {
    for (name, value) in from {
        if is_doorgeefbaar(name) {
            to.append(name.clone(), value.clone());
        }
    }
}

/// Forward one request to `base` and stream the answer back.
pub async fn forward(client: &reqwest::Client, base: &str, request: Request) -> Response {
    let (parts, body) = request.into_parts();
    let query = parts
        .uri
        .query()
        .map(|q| format!("?{q}"))
        .unwrap_or_default();
    let url = format!("{base}{}{query}", parts.uri.path());

    // Streamed rather than buffered: the beleidsassistent answers with SSE, and
    // a buffered proxy would hold the whole stream until the model is done.
    let body = reqwest::Body::wrap_stream(body.into_data_stream().map_err(std::io::Error::other));

    let mut upstream_request = client.request(parts.method.clone(), &url).body(body);
    let mut headers = HeaderMap::new();
    kopieer(&parts.headers, &mut headers);
    upstream_request = upstream_request.headers(headers);

    let upstream = match upstream_request.send().await {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(error = %e, url = %url, "upstream request failed");
            return (
                StatusCode::BAD_GATEWAY,
                "De achterliggende omgeving antwoordde niet.",
            )
                .into_response();
        }
    };

    let status =
        StatusCode::from_u16(upstream.status().as_u16()).unwrap_or(StatusCode::BAD_GATEWAY);
    let mut response = Response::builder().status(status);
    if let Some(headers) = response.headers_mut() {
        kopieer(upstream.headers(), headers);
    }

    response
        .body(Body::from_stream(upstream.bytes_stream()))
        .unwrap_or_else(|e| {
            tracing::error!(error = %e, "failed to build proxy response");
            StatusCode::INTERNAL_SERVER_ERROR.into_response()
        })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use axum::http::header;

    #[test]
    fn cookies_pass_in_both_directions() {
        // The difference from editor-api's proxy, and the reason this module
        // exists: napp owns its own session cookie.
        assert!(is_doorgeefbaar(&header::COOKIE));
        assert!(is_doorgeefbaar(&header::SET_COOKIE));
    }

    #[test]
    fn hop_by_hop_headers_are_not_forwarded() {
        for name in ["connection", "transfer-encoding", "upgrade", "host"] {
            let h = HeaderName::from_static(name);
            assert!(!is_doorgeefbaar(&h), "{name} must not be forwarded");
        }
    }

    #[test]
    fn content_length_is_dropped_because_the_body_is_restreamed() {
        assert!(!is_doorgeefbaar(&header::CONTENT_LENGTH));
    }

    #[test]
    fn ordinary_headers_survive() {
        for h in [
            header::CONTENT_TYPE,
            header::ACCEPT,
            header::CACHE_CONTROL,
            header::LOCATION,
        ] {
            assert!(is_doorgeefbaar(&h), "{h} should be forwarded");
        }
    }

    #[test]
    fn a_repeated_header_keeps_every_value() {
        // `Set-Cookie` is the header that actually repeats; using `insert`
        // here instead of `append` would silently keep only the last one.
        let mut from = HeaderMap::new();
        from.append(header::SET_COOKIE, "a=1".parse().expect("valid"));
        from.append(header::SET_COOKIE, "b=2".parse().expect("valid"));
        let mut to = HeaderMap::new();
        kopieer(&from, &mut to);
        assert_eq!(to.get_all(header::SET_COOKIE).iter().count(), 2);
    }
}
