//! Two Deepgram streams per call, one per speaker, sharing one result channel.

use detector::Speaker;
use tokio::sync::mpsc;

use crate::{
    config::Config,
    deepgram::{DeepgramStream, messages::DeepgramResult},
};

pub struct Transcriber {
    caller: Option<DeepgramStream>,
    user: Option<DeepgramStream>,
    results_rx: mpsc::Receiver<(Speaker, DeepgramResult)>,
}

impl Transcriber {
    /// Connects both streams. Returns `None` (and the call goes on untranscribed)
    /// when there is no API key or the caller stream can't connect. If only the
    /// user stream fails we keep transcribing the caller, which is what we score.
    pub async fn start(config: &Config, call_sid: &str) -> Option<Self> {
        let Some(api_key) = config.deepgram_api_key.as_deref() else {
            tracing::warn!(%call_sid, "DEEPGRAM_API_KEY not set; skipping transcription");
            return None;
        };
        let (results_tx, results_rx) = mpsc::channel(256);
        let url = &config.deepgram_url;
        let (caller, user) = tokio::join!(
            DeepgramStream::connect(url, api_key, Speaker::Caller, results_tx.clone()),
            DeepgramStream::connect(url, api_key, Speaker::User, results_tx),
        );

        let caller = caller
            .inspect_err(
                |err| tracing::error!(%call_sid, "caller transcription unavailable: {err:#}"),
            )
            .ok()?;
        let user = user
            .inspect_err(|err| tracing::warn!(%call_sid, "user transcription unavailable: {err:#}"))
            .ok();
        Some(Self {
            caller: Some(caller),
            user,
            results_rx,
        })
    }

    pub fn send(&self, speaker: Speaker, bytes: Vec<u8>) {
        let stream = match speaker {
            Speaker::Caller => &self.caller,
            Speaker::User => &self.user,
        };
        if let Some(stream) = stream {
            stream.send(bytes);
        }
    }

    /// Next result from either stream; `None` once both have closed.
    pub async fn next_result(&mut self) -> Option<(Speaker, DeepgramResult)> {
        self.results_rx.recv().await
    }

    /// Stops sending audio. Each stream sends `CloseStream` and Deepgram
    /// flushes its remaining results before closing.
    pub fn close_audio(&mut self) {
        self.caller = None;
        self.user = None;
    }
}
