//! Pretends to be Twilio Media Streams so the `/media` path can be tested
//! without a phone. Sends `connected` and `start`, streams both tracks at
//! real-time pace, then sends `stop`.
//!
//! ```text
//! cargo run -p server --bin fake_twilio -- [--url ws://localhost:3000/media]
//!     [--caller caller.ulaw] [--user user.ulaw] [--seconds 6] [--from +16175550123]
//! ```
//!
//! Audio files are raw 8 kHz mono μ-law, e.g.
//! `ffmpeg -i in.wav -ar 8000 -ac 1 -f mulaw caller.ulaw`. Without files the
//! client sends generated tones and silence. If `TWILIO_AUTH_TOKEN` and
//! `PUBLIC_URL` are set (and signatures aren't skipped), the upgrade request is
//! signed just like Twilio would sign it.

use std::time::Duration;

use anyhow::{Context, Result, bail};
use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use futures_util::SinkExt;
use serde_json::{Value, json};
use server::{
    audio::{self, FRAME_BYTES, FRAME_MS},
    twilio::signature,
};
use tokio_tungstenite::tungstenite::{Message, client::IntoClientRequest};

struct Args {
    url: String,
    caller_file: Option<String>,
    user_file: Option<String>,
    seconds: u64,
    from: String,
}

#[tokio::main]
async fn main() -> Result<()> {
    let _ = dotenvy::dotenv();
    let args = parse_args()?;
    let caller = load_track(args.caller_file.as_deref(), args.seconds, 440.0, 0)?;
    let user = load_track(args.user_file.as_deref(), args.seconds, 660.0, 750)?;
    stream_call(&args, &caller, &user).await
}

fn parse_args() -> Result<Args> {
    let port = std::env::var("PORT").unwrap_or_else(|_| "3000".into());
    let mut args = Args {
        url: format!("ws://localhost:{port}/media"),
        caller_file: None,
        user_file: None,
        seconds: 6,
        from: "+16175550123".into(),
    };
    let mut raw = std::env::args().skip(1);
    while let Some(flag) = raw.next() {
        let mut value = || raw.next().with_context(|| format!("{flag} needs a value"));
        match flag.as_str() {
            "--url" => args.url = value()?,
            "--caller" => args.caller_file = Some(value()?),
            "--user" => args.user_file = Some(value()?),
            "--seconds" => args.seconds = value()?.parse().context("--seconds must be a number")?,
            "--from" => args.from = value()?,
            other => bail!("unknown flag {other} (see the doc comment in fake_twilio.rs)"),
        }
    }
    Ok(args)
}

/// Reads a raw μ-law file, or generates `seconds` of a tone that alternates
/// with silence. `offset_ms` delays the first tone so the two tracks take turns.
fn load_track(file: Option<&str>, seconds: u64, frequency: f32, offset_ms: u64) -> Result<Vec<u8>> {
    if let Some(path) = file {
        return std::fs::read(path).with_context(|| format!("reading {path}"));
    }
    let mut track = audio::silence(offset_ms);
    while (track.len() as u64) < seconds * u64::from(audio::SAMPLE_RATE) {
        track.extend(audio::tone(frequency, 750, 0.4));
        track.extend(audio::silence(750));
    }
    track.truncate((seconds * u64::from(audio::SAMPLE_RATE)) as usize);
    Ok(track)
}

async fn stream_call(args: &Args, caller: &[u8], user: &[u8]) -> Result<()> {
    let mut request = args.url.as_str().into_client_request()?;
    if let Some(sig) = upgrade_signature() {
        request
            .headers_mut()
            .insert("x-twilio-signature", sig.parse()?);
    }
    let (mut ws, _) = tokio_tungstenite::connect_async(request)
        .await
        .context("connecting to /media")?;
    println!("connected to {}", args.url);

    let call_sid = format!("CAfake{:x}", std::process::id());
    let stream_sid = format!("MZfake{:x}", std::process::id());
    let mut messages = MessageBuilder::new(&stream_sid);
    ws.send(text(
        json!({"event": "connected", "protocol": "Call", "version": "1.0.0"}),
    ))
    .await?;
    ws.send(text(messages.start(&call_sid, &args.from))).await?;

    let frames = caller.len().max(user.len()).div_ceil(FRAME_BYTES);
    let mut tick = tokio::time::interval(Duration::from_millis(FRAME_MS));
    for i in 0..frames {
        tick.tick().await;
        let timestamp = i as u64 * FRAME_MS;
        ws.send(text(messages.media(
            "inbound",
            timestamp,
            frame_at(caller, i),
        )))
        .await?;
        ws.send(text(messages.media(
            "outbound",
            timestamp,
            frame_at(user, i),
        )))
        .await?;
    }

    ws.send(text(messages.stop(&call_sid))).await?;
    ws.close(None).await.ok();
    println!("sent {frames} frames per track ({call_sid}), then stop");
    Ok(())
}

/// Signs the upgrade the way Twilio does: the `wss://` URL from `PUBLIC_URL`, no params.
fn upgrade_signature() -> Option<String> {
    if std::env::var("SKIP_TWILIO_SIGNATURE").is_ok_and(|v| v == "1") {
        return None;
    }
    let token = std::env::var("TWILIO_AUTH_TOKEN").ok()?;
    let public_url = std::env::var("PUBLIC_URL").ok()?;
    let host = public_url
        .trim_end_matches('/')
        .split_once("://")?
        .1
        .to_string();
    Some(signature::compute(
        &token,
        &format!("wss://{host}/media"),
        &[],
    ))
}

/// The `i`th 20 ms frame of a track, padded with silence past its end.
fn frame_at(track: &[u8], i: usize) -> Vec<u8> {
    let start = (i * FRAME_BYTES).min(track.len());
    let mut frame = track[start..(start + FRAME_BYTES).min(track.len())].to_vec();
    frame.resize(FRAME_BYTES, audio::encode(0));
    frame
}

fn text(value: Value) -> Message {
    Message::text(value.to_string())
}

/// Builds Twilio-shaped messages with increasing sequence numbers.
struct MessageBuilder {
    stream_sid: String,
    sequence: u64,
    chunks: [u64; 2],
}

impl MessageBuilder {
    fn new(stream_sid: &str) -> Self {
        Self {
            stream_sid: stream_sid.into(),
            sequence: 1,
            chunks: [0, 0],
        }
    }

    fn next_sequence(&mut self) -> String {
        self.sequence += 1;
        self.sequence.to_string()
    }

    fn start(&mut self, call_sid: &str, from: &str) -> Value {
        json!({
            "event": "start",
            "sequenceNumber": self.next_sequence(),
            "streamSid": self.stream_sid,
            "start": {
                "accountSid": "ACfake",
                "streamSid": self.stream_sid,
                "callSid": call_sid,
                "tracks": ["inbound", "outbound"],
                "customParameters": { "from": from },
                "mediaFormat": { "encoding": "audio/x-mulaw", "sampleRate": 8000, "channels": 1 }
            }
        })
    }

    fn media(&mut self, track: &str, timestamp: u64, payload: Vec<u8>) -> Value {
        let chunk = &mut self.chunks[usize::from(track == "outbound")];
        *chunk += 1;
        let chunk = chunk.to_string();
        json!({
            "event": "media",
            "sequenceNumber": self.next_sequence(),
            "streamSid": self.stream_sid,
            "media": { "track": track, "chunk": chunk, "timestamp": timestamp.to_string(), "payload": BASE64.encode(payload) }
        })
    }

    fn stop(&mut self, call_sid: &str) -> Value {
        json!({
            "event": "stop",
            "sequenceNumber": self.next_sequence(),
            "streamSid": self.stream_sid,
            "stop": { "accountSid": "ACfake", "callSid": call_sid }
        })
    }
}
