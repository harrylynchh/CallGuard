mod common;

use axum::{body::Body, http::Request};
use common::*;

const CALLER: &str = "+16175550123";

fn call_params(from: &str) -> Vec<(&str, &str)> {
    vec![
        ("CallSid", "CA0001"),
        ("From", from),
        ("To", TWILIO_NUMBER),
        ("StirVerstat", "TN-Validation-Passed-A"),
    ]
}

#[tokio::test]
async fn health_is_ok() {
    let (status, body) = send(app(), Request::get("/health").body(Body::empty()).unwrap()).await;
    assert_eq!((status, body.as_str()), (200, "ok"));
}

#[tokio::test]
async fn voice_plays_disclosure_when_signed() {
    let (status, body) = send(app(), signed_post("/voice", &call_params(CALLER))).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("<Say>This call is screened by CallGuard"),
        "{body}"
    );
}

#[tokio::test]
async fn voice_rejects_missing_or_bad_signature() {
    let (status, _) = send(app(), form_post("/voice", &call_params(CALLER), "")).await;
    assert_eq!(status, 403);
    let (status, _) = send(
        app(),
        form_post("/voice", &call_params(CALLER), "bm90LXJlYWw="),
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn signature_covers_the_body() {
    // Signed for one caller, but the body claims another.
    let params = call_params(CALLER);
    let owned: Vec<(String, String)> = params
        .iter()
        .map(|(k, v)| (k.to_string(), v.to_string()))
        .collect();
    let sig = server::twilio::signature::compute(TOKEN, &format!("{PUBLIC_URL}/voice"), &owned);
    let (status, _) = send(
        app(),
        form_post("/voice", &call_params("+19995550000"), &sig),
    )
    .await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn skip_flag_disables_signature_check() {
    let mut config = config();
    config.skip_twilio_signature = true;
    let app = server::routes::router(server::state::AppState::new(config));
    let (status, _) = send(app, form_post("/voice", &call_params(CALLER), "")).await;
    assert_eq!(status, 200);
}
