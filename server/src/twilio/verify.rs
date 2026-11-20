//! Axum middleware that rejects requests without a valid `X-Twilio-Signature`.

use axum::{
    body::{Body, to_bytes},
    extract::{Request, State},
    http::{StatusCode, header, request::Parts},
    middleware::Next,
    response::{IntoResponse, Response},
};

use super::{form, signature};
use crate::state::AppState;

/// Twilio webhook bodies are small; anything bigger is not from Twilio.
const MAX_BODY_BYTES: usize = 64 * 1024;

pub async fn require_twilio_signature(
    State(state): State<AppState>,
    request: Request,
    next: Next,
) -> Response {
    let config = &state.config;
    if config.skip_twilio_signature {
        return next.run(request).await;
    }

    let (parts, body) = request.into_parts();
    let Ok(body) = to_bytes(body, MAX_BODY_BYTES).await else {
        return StatusCode::PAYLOAD_TOO_LARGE.into_response();
    };

    let url = signed_url(&config.public_url, &parts);
    let params = if is_form(&parts) {
        form::parse(&body)
    } else {
        Vec::new()
    };
    let received = parts
        .headers
        .get("x-twilio-signature")
        .and_then(|v| v.to_str().ok())
        .unwrap_or_default();

    if !signature::is_valid(&config.twilio_auth_token, &url, &params, received) {
        tracing::warn!(%url, "rejected request with missing or invalid Twilio signature");
        return StatusCode::FORBIDDEN.into_response();
    }
    next.run(Request::from_parts(parts, Body::from(body))).await
}

/// Rebuilds the URL Twilio signed from `PUBLIC_URL`, because behind a tunnel the
/// request we see has a different host and scheme. WebSocket upgrades are signed
/// with the `wss://` URL from our TwiML.
pub fn signed_url(public_url: &str, parts: &Parts) -> String {
    let path = parts.uri.path_and_query().map_or("/", |pq| pq.as_str());
    let base = if is_websocket_upgrade(parts) {
        public_url
            .replacen("https://", "wss://", 1)
            .replacen("http://", "ws://", 1)
    } else {
        public_url.to_string()
    };
    format!("{base}{path}")
}

fn is_form(parts: &Parts) -> bool {
    parts
        .headers
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|ct| ct.starts_with("application/x-www-form-urlencoded"))
}

fn is_websocket_upgrade(parts: &Parts) -> bool {
    parts
        .headers
        .get(header::UPGRADE)
        .and_then(|v| v.to_str().ok())
        .is_some_and(|v| v.eq_ignore_ascii_case("websocket"))
}
