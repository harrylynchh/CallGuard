//! One task per call. The WebSocket handler pushes decoded audio frames in;
//! the task routes them to speech-to-text and handles the transcripts.

use std::time::{Duration, Instant};

use detector::Speaker;
use tokio::sync::mpsc;

use super::{
    clock::FrameClock,
    messages::{StartInfo, Track},
    transcriber::Transcriber,
};
use crate::{deepgram::messages::DeepgramResult, state::AppState};

/// About 10 s of audio before the WebSocket reader waits on us.
const AUDIO_QUEUE: usize = 1000;
/// How long to wait for Deepgram's last results after the call ends.
const FLUSH_TIMEOUT: Duration = Duration::from_secs(3);

/// One 20 ms chunk of μ-law audio from one track. Held in memory only.
pub struct AudioFrame {
    pub track: Track,
    pub timestamp_ms: u64,
    pub received_at: Instant,
    pub bytes: Vec<u8>,
}

/// Handle used by the WebSocket reader. Dropping it ends the call.
pub struct CallSession {
    audio_tx: mpsc::Sender<AudioFrame>,
}

impl CallSession {
    pub fn spawn(state: AppState, start: &StartInfo) -> Self {
        let (audio_tx, audio_rx) = mpsc::channel(AUDIO_QUEUE);
        let call_sid = start.call_sid.clone();
        tokio::spawn(async move {
            let task = CallTask::start(state, call_sid).await;
            task.run(audio_rx).await;
        });
        Self { audio_tx }
    }

    pub async fn push(&self, frame: AudioFrame) {
        // An error only means the call task already ended, so the frame is moot.
        let _ = self.audio_tx.send(frame).await;
    }
}

/// Per-speaker bookkeeping.
#[derive(Default)]
struct TrackState {
    frames: u64,
    clock: FrameClock,
}

struct CallTask {
    call_sid: String,
    transcriber: Option<Transcriber>,
    caller: TrackState,
    user: TrackState,
}

impl CallTask {
    async fn start(state: AppState, call_sid: String) -> Self {
        tracing::info!(%call_sid, "call started");
        let transcriber = Transcriber::start(&state.config, &call_sid).await;
        Self {
            call_sid,
            transcriber,
            caller: TrackState::default(),
            user: TrackState::default(),
        }
    }

    async fn run(mut self, mut audio_rx: mpsc::Receiver<AudioFrame>) {
        loop {
            tokio::select! {
                frame = audio_rx.recv() => match frame {
                    Some(frame) => self.on_frame(frame),
                    None => break,
                },
                Some((speaker, result)) = next_result(&mut self.transcriber) => self.on_result(speaker, result),
            }
        }
        self.finish().await;
    }

    fn on_frame(&mut self, frame: AudioFrame) {
        let speaker = frame.track.speaker();
        let track = self.track(speaker);
        track.frames += 1;
        track.clock.record(frame.bytes.len(), frame.received_at);
        if let Some(transcriber) = &self.transcriber {
            transcriber.send(speaker, frame.bytes);
        }
    }

    fn on_result(&mut self, speaker: Speaker, result: DeepgramResult) {
        if result.is_final {
            tracing::info!(call_sid = %self.call_sid, ?speaker, text = %result.text, "transcript");
        } else {
            tracing::debug!(call_sid = %self.call_sid, ?speaker, text = %result.text, "interim transcript");
        }
    }

    /// Lets Deepgram flush its last results, then logs a summary. All audio
    /// state is dropped with `self`.
    async fn finish(mut self) {
        if let Some(mut transcriber) = self.transcriber.take() {
            transcriber.close_audio();
            let flush = async {
                while let Some((speaker, result)) = transcriber.next_result().await {
                    self.on_result(speaker, result);
                }
            };
            if tokio::time::timeout(FLUSH_TIMEOUT, flush).await.is_err() {
                tracing::warn!(call_sid = %self.call_sid, "Deepgram did not close in time");
            }
        }
        tracing::info!(
            call_sid = %self.call_sid,
            inbound_frames = self.caller.frames,
            outbound_frames = self.user.frames,
            "call ended"
        );
    }

    fn track(&mut self, speaker: Speaker) -> &mut TrackState {
        match speaker {
            Speaker::Caller => &mut self.caller,
            Speaker::User => &mut self.user,
        }
    }
}

/// Waits for the next transcript. Without a transcriber (or once its streams
/// have closed) this never resolves, so `select!` just waits on audio.
async fn next_result(transcriber: &mut Option<Transcriber>) -> Option<(Speaker, DeepgramResult)> {
    match transcriber {
        Some(t) => match t.next_result().await {
            Some(result) => Some(result),
            None => {
                *transcriber = None;
                std::future::pending().await
            }
        },
        None => std::future::pending().await,
    }
}
