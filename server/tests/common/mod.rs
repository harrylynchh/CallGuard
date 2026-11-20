//! Shared helpers for the server's integration tests.
#![allow(dead_code)]

use axum::{
    Router,
    body::Body,
    http::{Request, Response, header},
};
use http_body_util::BodyExt;
use server::{config::Config, routes, state::AppState, twilio::signature};
use tower::ServiceExt;

pub const TOKEN: &str = "test-auth-token";
pub const PUBLIC_URL: &str = "https://cg.example.com";
pub const TWILIO_NUMBER: &str = "+16175550100";
pub const USER_PHONE: &str = "+16175550199";

pub fn config() -> Config {
    Config::from_lookup(|key| {
        Some(
            match key {
                "TWILIO_AUTH_TOKEN" => TOKEN,
                "TWILIO_NUMBER" => TWILIO_NUMBER,
                "USER_PHONE" => USER_PHONE,
                "PUBLIC_URL" => PUBLIC_URL,
                _ => return None,
            }
            .to_string(),
        )
    })
    .unwrap()
}

pub fn app() -> Router {
    routes::router(AppState::new(config()))
}

/// Builds a form POST signed exactly the way Twilio would sign it.
pub fn signed_post(path: &str, params: &[(&str, &str)]) -> Request<Body> {
    let owned: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let sig = signature::compute(TOKEN, &format!("{PUBLIC_URL}{path}"), &owned);
    form_post(path, params, &sig)
}

pub fn form_post(path: &str, params: &[(&str, &str)], signature: &str) -> Request<Body> {
    let body = params
        .iter()
        .map(|(k, v)| {
            format!(
                "{}={}",
                server::twilio::form::encode(k),
                server::twilio::form::encode(v)
            )
        })
        .collect::<Vec<_>>()
        .join("&");
    Request::post(path)
        .header(header::CONTENT_TYPE, "application/x-www-form-urlencoded")
        .header("x-twilio-signature", signature)
        .body(Body::from(body))
        .unwrap()
}

pub async fn send(app: Router, request: Request<Body>) -> (u16, String) {
    let response: Response<Body> = app.oneshot(request).await.unwrap();
    let status = response.status().as_u16();
    let bytes = response.into_body().collect().await.unwrap().to_bytes();
    (status, String::from_utf8(bytes.to_vec()).unwrap())
}
