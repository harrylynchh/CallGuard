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

#[tokio::test]
async fn voice_dials_user_with_stream_and_whisper() {
    let (status, body) = send(app(), signed_post("/voice", &call_params(CALLER))).await;
    assert_eq!(status, 200);
    assert!(
        body.contains(r#"<Stream url="wss://cg.example.com/media" track="both_tracks">"#),
        "{body}"
    );
    assert!(
        body.contains(r#"<Parameter name="from" value="+16175550123"/>"#),
        "{body}"
    );
    assert!(body.contains(r#"<Dial callerId="+16175550100">"#), "{body}");
    assert!(
        body.contains(
            r#"url="https://cg.example.com/whisper?from=%2B16175550123">+16175550199</Number>"#
        ),
        "{body}"
    );
}

#[tokio::test]
async fn loop_guard_hangs_up_on_our_own_number() {
    let (status, body) = send(app(), signed_post("/voice", &call_params(TWILIO_NUMBER))).await;
    assert_eq!(
        (status, body.as_str()),
        (200, "<Response><Hangup/></Response>")
    );
}

#[tokio::test]
async fn whisper_announces_caller_and_checks_signature_with_query() {
    let path = "/whisper?from=%2B16175550123";
    let (status, body) = send(app(), signed_post(path, &[("CallSid", "CA0002")])).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("Screened call from 6 1 7 5 5 5 0 1 2 3"),
        "{body}"
    );

    let (status, _) = send(app(), form_post(path, &[("CallSid", "CA0002")], "")).await;
    assert_eq!(status, 403);
}

#[tokio::test]
async fn voice_fails_open_on_unparseable_body() {
    let sig = server::twilio::signature::compute(TOKEN, &format!("{PUBLIC_URL}/voice"), &[]);
    let request = Request::post("/voice")
        .header("content-type", "text/plain")
        .header("x-twilio-signature", sig)
        .body(Body::from("garbage"))
        .unwrap();
    let (status, body) = send(app(), request).await;
    assert_eq!(status, 200);
    assert!(
        body.contains("<Dial") && body.contains(USER_PHONE),
        "{body}"
    );
}
