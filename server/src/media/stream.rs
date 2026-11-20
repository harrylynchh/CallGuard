//! Reads the Twilio Media Streams WebSocket and feeds the call session.

use std::time::Instant;

use axum::extract::ws::{Message, WebSocket};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};

use super::{
    messages::{MediaInfo, StreamMessage},
    session::{AudioFrame, CallSession},
};
use crate::state::AppState;

pub async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let mut session: Option<CallSession> = None;

    while let Some(Ok(message)) = socket.recv().await {
        let Message::Text(text) = message else {
            continue;
        };
        match serde_json::from_str::<StreamMessage>(&text) {
            Ok(StreamMessage::Start { start }) => {
                tracing::info!(call_sid = %start.call_sid, stream_sid = %start.stream_sid, "media stream started");
                session = Some(CallSession::spawn(state.clone(), &start));
            }
            Ok(StreamMessage::Media { media }) => {
                if let (Some(session), Some(frame)) = (&session, decode_frame(media)) {
                    session.push(frame).await;
                }
            }
            Ok(StreamMessage::Stop) => break,
            Ok(StreamMessage::Connected | StreamMessage::Mark | StreamMessage::Unknown) => {}
            // Never log the message itself: it may contain audio.
            Err(err) => tracing::warn!(%err, "ignoring unparseable media stream message"),
        }
    }
    // Dropping the session closes its audio channel, which ends the call task
    // and frees every buffer.
    drop(session);
}

fn decode_frame(media: MediaInfo) -> Option<AudioFrame> {
    let bytes = BASE64.decode(media.payload).ok()?;
    Some(AudioFrame {
        track: media.track,
        timestamp_ms: media.timestamp,
        received_at: Instant::now(),
        bytes,
    })
}
