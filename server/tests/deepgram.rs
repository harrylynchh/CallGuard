mod common;

use std::time::Duration;

use common::{mock_deepgram::MockDeepgram, *};
use futures_util::SinkExt;
use tokio_tungstenite::tungstenite::Message;

/// Streams `seconds` of audio: a tone-like pattern on the caller track and
/// silence on the user track.
pub async fn fake_call(addr: std::net::SocketAddr, call_sid: &str, seconds: usize) {
    let (mut ws, _) = tokio_tungstenite::connect_async(format!("ws://{addr}/media"))
        .await
        .unwrap();
    let start = format!(
        r#"{{"event":"start","start":{{"callSid":"{call_sid}","streamSid":"MZ1","customParameters":{{"from":"+16175550123"}}}}}}"#
    );
    ws.send(Message::text(start)).await.unwrap();
    // Base64 of 160 bytes: non-silent (0x10) for the caller, silent (0xFF) for the user.
    let speech = base64_frame(0x10);
    let silence = base64_frame(0xFF);
    for i in 0..seconds * 50 {
        for (track, payload) in [("inbound", &speech), ("outbound", &silence)] {
            let msg = format!(
                r#"{{"event":"media","media":{{"track":"{track}","timestamp":"{}","payload":"{payload}"}}}}"#,
                i * 20
            );
            ws.send(Message::text(msg)).await.unwrap();
        }
    }
    ws.send(Message::text(r#"{"event":"stop"}"#)).await.unwrap();
    ws.close(None).await.ok();
}

fn base64_frame(byte: u8) -> String {
    use base64::Engine;
    base64::engine::general_purpose::STANDARD.encode([byte; 160])
}

#[tokio::test]
async fn opens_one_deepgram_stream_per_speaker_and_closes_them() {
    let (mock, url) = MockDeepgram::start().await;
    let mut config = config();
    config.skip_twilio_signature = true;
    config.deepgram_api_key = Some("test-key".into());
    config.deepgram_url = url;
    let addr = spawn_server(server::routes::router(server::state::AppState::new(config))).await;

    fake_call(addr, "CA_DG", 1).await;
    tokio::time::sleep(Duration::from_millis(500)).await;

    let logs = mock.logs();
    assert_eq!(logs.len(), 2, "{logs:?}");
    for log in logs {
        assert_eq!(log.authorization, "Token test-key");
        assert_eq!(
            log.query,
            "encoding=mulaw&sample_rate=8000&channels=1&interim_results=true&punctuate=true"
        );
        assert_eq!(
            log.audio_bytes, 8000,
            "one second of 8 kHz mu-law per speaker"
        );
        assert!(log.got_close_stream);
    }
}
