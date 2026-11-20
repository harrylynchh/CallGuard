//! One task per call. The WebSocket handler pushes decoded audio frames in;
//! the task owns everything else about the call.

use std::time::Instant;

use tokio::sync::mpsc;

use super::messages::{StartInfo, Track};
use crate::state::AppState;

/// About 10 s of audio per track before the WebSocket reader waits on us.
const AUDIO_QUEUE: usize = 1000;

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
    pub fn spawn(_state: AppState, start: &StartInfo) -> Self {
        let (audio_tx, audio_rx) = mpsc::channel(AUDIO_QUEUE);
        tokio::spawn(run(start.call_sid.clone(), audio_rx));
        Self { audio_tx }
    }

    pub async fn push(&self, frame: AudioFrame) {
        // An error only means the call task already ended, so the frame is moot.
        let _ = self.audio_tx.send(frame).await;
    }
}

#[derive(Debug, Default)]
struct FrameCounts {
    inbound: u64,
    outbound: u64,
}

async fn run(call_sid: String, mut audio_rx: mpsc::Receiver<AudioFrame>) {
    tracing::info!(%call_sid, "call started");
    let mut counts = FrameCounts::default();
    while let Some(frame) = audio_rx.recv().await {
        match frame.track {
            Track::Inbound => counts.inbound += 1,
            Track::Outbound => counts.outbound += 1,
        }
    }
    tracing::info!(%call_sid, inbound_frames = counts.inbound, outbound_frames = counts.outbound, "call ended");
}
