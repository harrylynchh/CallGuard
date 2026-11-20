//! A stand-in for Deepgram's live API. Each connection answers with a
//! transcript once it has received some audio: "caller" text for audio that
//! isn't silence, "user" text otherwise. It records what it saw so tests can
//! check the protocol.

use std::sync::{Arc, Mutex};

use futures_util::{SinkExt, StreamExt};
use serde_json::json;
use tokio::net::TcpListener;
use tokio_tungstenite::tungstenite::{
    Message,
    handshake::server::{Request, Response},
};

pub const CALLER_TEXT: &str = "This is the IRS, buy Google Play gift cards right now.";
pub const USER_TEXT: &str = "Okay, what should I do?";

#[derive(Debug, Default, Clone)]
pub struct ConnectionLog {
    pub authorization: String,
    pub query: String,
    pub audio_bytes: usize,
    pub got_close_stream: bool,
}

#[derive(Clone, Default)]
pub struct MockDeepgram {
    pub connections: Arc<Mutex<Vec<ConnectionLog>>>,
}

impl MockDeepgram {
    /// Starts the mock and returns it with its `ws://` URL.
    pub async fn start() -> (Self, String) {
        let listener = TcpListener::bind("127.0.0.1:0").await.unwrap();
        let url = format!("ws://{}/v1/listen", listener.local_addr().unwrap());
        let mock = Self::default();
        let accepting = mock.clone();
        tokio::spawn(async move {
            while let Ok((tcp, _)) = listener.accept().await {
                tokio::spawn(accepting.clone().serve(tcp));
            }
        });
        (mock, url)
    }

    pub fn logs(&self) -> Vec<ConnectionLog> {
        self.connections.lock().unwrap().clone()
    }

    // The handshake callback signature is fixed by tungstenite.
    #[allow(clippy::result_large_err)]
    async fn serve(self, tcp: tokio::net::TcpStream) {
        let mut log = ConnectionLog::default();
        let callback = |req: &Request, res: Response| {
            log.authorization = req
                .headers()
                .get("authorization")
                .and_then(|v| v.to_str().ok())
                .unwrap_or_default()
                .to_string();
            log.query = req.uri().query().unwrap_or_default().to_string();
            Ok(res)
        };
        let mut ws = tokio_tungstenite::accept_hdr_async(tcp, callback)
            .await
            .unwrap();

        let mut answered = false;
        let mut saw_speech = false;
        while let Some(Ok(message)) = ws.next().await {
            match message {
                Message::Binary(bytes) => {
                    log.audio_bytes += bytes.len();
                    saw_speech |= bytes.iter().any(|&b| b != 0xFF);
                    // Answer after half a second of audio, so tones have started.
                    if !answered && log.audio_bytes >= 4000 {
                        answered = true;
                        let text = if saw_speech { CALLER_TEXT } else { USER_TEXT };
                        ws.send(Message::text(results(text, false))).await.ok();
                        ws.send(Message::text(results(text, true))).await.ok();
                    }
                }
                Message::Text(text) if text.contains("CloseStream") => {
                    log.got_close_stream = true;
                    ws.close(None).await.ok();
                    break;
                }
                _ => {}
            }
        }
        self.connections.lock().unwrap().push(log);
    }
}

fn results(text: &str, is_final: bool) -> String {
    let words: Vec<_> = text
        .split_whitespace()
        .enumerate()
        .map(|(i, w)| json!({"word": w.to_lowercase(), "punctuated_word": w, "start": 0.1 * i as f64, "end": 0.1 * (i + 1) as f64}))
        .collect();
    json!({
        "type": "Results",
        "is_final": is_final,
        "start": 0.0,
        "duration": 0.1 * words.len() as f64,
        "channel": {"alternatives": [{"transcript": text, "words": words}]}
    })
    .to_string()
}
