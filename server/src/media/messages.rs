//! JSON messages Twilio sends over the Media Streams WebSocket.
//! See <https://www.twilio.com/docs/voice/media-streams/websocket-messages>.

use std::collections::HashMap;

use serde::{Deserialize, Deserializer};

#[derive(Debug, Deserialize)]
#[serde(tag = "event", rename_all = "lowercase")]
pub enum StreamMessage {
    Connected,
    Start {
        start: StartInfo,
    },
    Media {
        media: MediaInfo,
    },
    Stop,
    Mark,
    #[serde(other)]
    Unknown,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct StartInfo {
    pub call_sid: String,
    pub stream_sid: String,
    #[serde(default)]
    pub custom_parameters: HashMap<String, String>,
}

impl StartInfo {
    /// The caller's number, passed through our `<Parameter name="from">`.
    pub fn from(&self) -> &str {
        self.custom_parameters
            .get("from")
            .map_or("unknown", String::as_str)
    }
}

#[derive(Debug, Deserialize)]
pub struct MediaInfo {
    pub track: Track,
    /// Milliseconds since the stream started. Twilio sends this as a string.
    #[serde(deserialize_with = "u64_from_string_or_number")]
    pub timestamp: u64,
    /// Base64-encoded μ-law audio.
    pub payload: String,
}

/// With `<Dial>`, inbound is the caller and outbound is the protected user.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Track {
    Inbound,
    Outbound,
}

fn u64_from_string_or_number<'de, D: Deserializer<'de>>(deserializer: D) -> Result<u64, D::Error> {
    #[derive(Deserialize)]
    #[serde(untagged)]
    enum Raw {
        Number(u64),
        Text(String),
    }
    match Raw::deserialize(deserializer)? {
        Raw::Number(n) => Ok(n),
        Raw::Text(s) => s.parse().map_err(serde::de::Error::custom),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parse(json: &str) -> StreamMessage {
        serde_json::from_str(json).unwrap()
    }

    #[test]
    fn parses_start() {
        let msg = parse(
            r#"{"event":"start","sequenceNumber":"1","streamSid":"MZ1",
                "start":{"accountSid":"AC1","streamSid":"MZ1","callSid":"CA1",
                "tracks":["inbound","outbound"],"customParameters":{"from":"+16175550123"},
                "mediaFormat":{"encoding":"audio/x-mulaw","sampleRate":8000,"channels":1}}}"#,
        );
        let StreamMessage::Start { start } = msg else {
            panic!("expected start")
        };
        assert_eq!(
            (
                start.call_sid.as_str(),
                start.stream_sid.as_str(),
                start.from()
            ),
            ("CA1", "MZ1", "+16175550123")
        );
    }

    #[test]
    fn parses_media_with_string_timestamp() {
        let msg = parse(
            r#"{"event":"media","sequenceNumber":"3","streamSid":"MZ1",
                "media":{"track":"outbound","chunk":"2","timestamp":"140","payload":"//8="}}"#,
        );
        let StreamMessage::Media { media } = msg else {
            panic!("expected media")
        };
        assert_eq!(
            (media.track, media.timestamp, media.payload.as_str()),
            (Track::Outbound, 140, "//8=")
        );
    }

    #[test]
    fn parses_control_messages() {
        assert!(matches!(
            parse(r#"{"event":"connected","protocol":"Call","version":"1.0.0"}"#),
            StreamMessage::Connected
        ));
        assert!(matches!(
            parse(r#"{"event":"stop","stop":{"callSid":"CA1"}}"#),
            StreamMessage::Stop
        ));
        assert!(matches!(
            parse(r#"{"event":"mark","mark":{"name":"x"}}"#),
            StreamMessage::Mark
        ));
        assert!(matches!(
            parse(r#"{"event":"dtmf","dtmf":{"digit":"1"}}"#),
            StreamMessage::Unknown
        ));
    }
}
