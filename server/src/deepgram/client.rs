//! One live Deepgram WebSocket per speaker. Audio goes in over a channel and
//! results come back tagged with the speaker.

use std::time::Duration;

use anyhow::{Context, Result};
use detector::Speaker;
use futures_util::{SinkExt, StreamExt};
use tokio::sync::mpsc;
use tokio_tungstenite::tungstenite::{
    Message, client::IntoClientRequest, http::header::AUTHORIZATION,
};

use super::messages::{self, DeepgramResult};

const QUERY: &str =
    "encoding=mulaw&sample_rate=8000&channels=1&interim_results=true&punctuate=true";
const CONNECT_TIMEOUT: Duration = Duration::from_secs(5);
const AUDIO_QUEUE: usize = 500;

pub type ResultSender = mpsc::Sender<(Speaker, DeepgramResult)>;

/// Write side of a live connection. Dropping it sends `CloseStream`, after which
/// Deepgram flushes its last results and closes.
pub struct DeepgramStream {
    audio_tx: mpsc::Sender<Vec<u8>>,
}

impl DeepgramStream {
    pub async fn connect(
        base_url: &str,
        api_key: &str,
        speaker: Speaker,
        results: ResultSender,
    ) -> Result<Self> {
        let mut request = format!("{base_url}?{QUERY}").into_client_request()?;
        request
            .headers_mut()
            .insert(AUTHORIZATION, format!("Token {api_key}").parse()?);
        let (socket, _) =
            tokio::time::timeout(CONNECT_TIMEOUT, tokio_tungstenite::connect_async(request))
                .await
                .context("timed out connecting to Deepgram")?
                .context("connecting to Deepgram")?;

        let (sink, stream) = socket.split();
        let (audio_tx, audio_rx) = mpsc::channel(AUDIO_QUEUE);
        tokio::spawn(write_audio(sink, audio_rx, speaker));
        tokio::spawn(read_results(stream, speaker, results));
        Ok(Self { audio_tx })
    }

    /// Queues audio without waiting; if Deepgram falls far behind we drop frames
    /// rather than stall the call.
    pub fn send(&self, bytes: Vec<u8>) {
        if let Err(mpsc::error::TrySendError::Full(_)) = self.audio_tx.try_send(bytes) {
            tracing::warn!("Deepgram is behind; dropped an audio frame");
        }
    }
}

async fn write_audio<S>(mut sink: S, mut audio_rx: mpsc::Receiver<Vec<u8>>, speaker: Speaker)
where
    S: SinkExt<Message> + Unpin,
{
    while let Some(bytes) = audio_rx.recv().await {
        if sink.send(Message::Binary(bytes.into())).await.is_err() {
            tracing::warn!(?speaker, "Deepgram connection closed while sending audio");
            return;
        }
    }
    let _ = sink.send(Message::text(r#"{"type":"CloseStream"}"#)).await;
}

async fn read_results<S, E>(mut stream: S, speaker: Speaker, results: ResultSender)
where
    S: StreamExt<Item = Result<Message, E>> + Unpin,
    E: std::fmt::Display,
{
    while let Some(message) = stream.next().await {
        match message {
            Ok(Message::Text(text)) => {
                if let Some(result) = messages::parse(&text)
                    && results.send((speaker, result)).await.is_err()
                {
                    return;
                }
            }
            Ok(Message::Close(_)) => return,
            Ok(_) => {}
            Err(err) => {
                tracing::warn!(?speaker, %err, "Deepgram connection error");
                return;
            }
        }
    }
}
