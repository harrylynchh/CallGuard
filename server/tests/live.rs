//! Replay mode, the SSE stream and the full live-call pipeline.

mod common;

use std::time::Duration;

use axum::{Router, body::Body, http::Request};
use common::{mock_deepgram::MockDeepgram, *};
use futures_util::SinkExt;
use server::{
    replay::{Pace, ReplayConfig},
    state::AppState,
};
use tokio_tungstenite::tungstenite::Message;

fn instant_replay_state() -> AppState {
    AppState::with_replay(
        config(),
        ReplayConfig {
            pace: Pace::INSTANT,
            ..ReplayConfig::default()
        },
    )
}

async fn get(app: Router, path: &str) -> (u16, String) {
    send(app, Request::get(path).body(Body::empty()).unwrap()).await
}

async fn post(app: Router, path: &str) -> (u16, String) {
    send(app, Request::post(path).body(Body::empty()).unwrap()).await
}

/// Polls until the call exists and has ended, and returns its summary JSON.
async fn wait_for_end(app: &Router, id: &str) -> serde_json::Value {
    for _ in 0..100 {
        let (status, body) = get(app.clone(), &format!("/api/calls/{id}")).await;
        if status == 200 {
            let summary: serde_json::Value = serde_json::from_str(&body).unwrap();
            if summary["ended"] == true {
                return summary;
            }
        }
        tokio::time::sleep(Duration::from_millis(50)).await;
    }
    panic!("call {id} never ended");
}

async fn replay(app: &Router, script: &str) -> serde_json::Value {
    let (status, body) = post(app.clone(), &format!("/replay?script={script}")).await;
    assert_eq!(status, 201, "{body}");
    let created: serde_json::Value = serde_json::from_str(&body).unwrap();
    wait_for_end(app, created["id"].as_str().unwrap()).await
}

#[tokio::test]
async fn replays_score_like_the_fixtures() {
    let app = server::routes::router(instant_replay_state());
    for (script, level) in [
        ("scam_irs", "High"),
        ("scam_tech_support", "High"),
        ("legit_pharmacy", "Low"),
    ] {
        let summary = replay(&app, script).await;
        assert_eq!(summary["risk"]["level"], level, "{script}: {summary}");
        assert_eq!(summary["source"], "replay");
    }
    let (_, list) = get(app, "/api/calls").await;
    assert_eq!(
        serde_json::from_str::<Vec<serde_json::Value>>(&list)
            .unwrap()
            .len(),
        3
    );
}

#[tokio::test]
async fn sse_replays_history_and_ends() {
    let app = server::routes::router(instant_replay_state());
    let summary = replay(&app, "scam_irs").await;
    let id = summary["id"].as_str().unwrap();

    let (status, body) = get(app.clone(), &format!("/calls/{id}/events")).await;
    assert_eq!(status, 200);
    assert!(body.contains("event: transcript"), "{body}");
    assert!(body.contains("event: flag"), "{body}");
    assert!(body.contains(r#""level":"High""#), "{body}");
    assert!(
        body.trim_end().ends_with(r#"data: {"type":"ended"}"#),
        "{body}"
    );
    assert!(
        !body.contains(r#""final":false"#),
        "interims are not replayed to late subscribers"
    );

    let (status, page) = get(app, &format!("/calls/{id}")).await;
    assert_eq!(status, 200);
    assert!(page.contains("EventSource"));
}

#[tokio::test]
async fn rejects_bad_replay_and_unknown_calls() {
    let app = server::routes::router(instant_replay_state());
    assert_eq!(
        post(app.clone(), "/replay?script=..%2F..%2FCargo").await.0,
        400
    );
    assert_eq!(post(app.clone(), "/replay?script=nope").await.0, 400);
    assert_eq!(get(app.clone(), "/calls/CA_missing").await.0, 404);
    assert_eq!(get(app.clone(), "/calls/CA_missing/events").await.0, 404);

    let (status, scripts) = get(app.clone(), "/api/scripts").await;
    assert_eq!(status, 200);
    assert_eq!(
        scripts,
        r#"["legit_pharmacy","scam_irs","scam_tech_support"]"#
    );
    let (status, index) = get(app, "/").await;
    assert!(status == 200 && index.contains("CallGuard"));
}

#[tokio::test]
async fn live_call_flows_from_media_stream_to_risk() {
    let (_mock, url) = MockDeepgram::start().await;
    let mut config = config();
    config.skip_twilio_signature = true;
    config.deepgram_api_key = Some("test-key".into());
    config.deepgram_url = url;
    let app = server::routes::router(AppState::new(config));
    let addr = spawn_server(app.clone()).await;

    stream_fake_call(addr, "CA_LIVE").await;
    let summary = wait_for_end(&app, "CA_LIVE").await;
    assert_eq!(summary["risk"]["level"], "High", "{summary}");
    assert_eq!(summary["from"], "+16175550123");

    let (_, body) = get(app, "/calls/CA_LIVE/events").await;
    assert!(
        body.contains(r#""speaker":"caller","text":"This is the IRS"#),
        "{body}"
    );
    assert!(body.contains(r#""speaker":"user""#), "{body}");
    assert!(
        body.contains(r#""latency_ms":"#) && !body.contains(r#""latency_ms":null"#),
        "{body}"
    );
}

/// One second of "speech" on the caller track and silence on the user track.
async fn stream_fake_call(addr: std::net::SocketAddr, call_sid: &str) {
    use base64::Engine;
    let frame = |byte: u8| base64::engine::general_purpose::STANDARD.encode([byte; 160]);
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/media"))
        .await
        .unwrap();
    let start = serde_json::json!({"event": "start", "start": {
        "callSid": call_sid, "streamSid": "MZ1", "customParameters": {"from": "+16175550123"}}});
    ws.send(Message::text(start.to_string())).await.unwrap();
    for i in 0..50 {
        for (track, payload) in [("inbound", frame(0x10)), ("outbound", frame(0xFF))] {
            let media = serde_json::json!({"event": "media", "media": {
                "track": track, "timestamp": (i * 20).to_string(), "payload": payload}});
            ws.send(Message::text(media.to_string())).await.unwrap();
        }
    }
    ws.send(Message::text(r#"{"event":"stop"}"#)).await.unwrap();
    ws.close(None).await.ok();
}
