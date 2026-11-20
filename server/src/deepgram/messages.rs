//! Parsing Deepgram live transcription results.
//! See <https://developers.deepgram.com/docs/live-streaming-audio>.

use serde::Deserialize;

/// One transcription result for one speaker's stream.
#[derive(Debug, Clone, PartialEq)]
pub struct DeepgramResult {
    pub text: String,
    pub is_final: bool,
    /// Seconds from the start of the audio we sent on this connection.
    pub start: f64,
    pub duration: f64,
    pub words: Vec<Word>,
}

#[derive(Debug, Clone, PartialEq, Deserialize)]
pub struct Word {
    pub start: f64,
    pub end: f64,
}

impl DeepgramResult {
    /// End of this result in stream seconds.
    pub fn end(&self) -> f64 {
        self.start + self.duration
    }
}

#[derive(Deserialize)]
struct RawMessage {
    #[serde(rename = "type")]
    kind: String,
    #[serde(default)]
    is_final: bool,
    #[serde(default)]
    start: f64,
    #[serde(default)]
    duration: f64,
    channel: Option<RawChannel>,
}

#[derive(Deserialize)]
struct RawChannel {
    alternatives: Vec<RawAlternative>,
}

#[derive(Deserialize)]
struct RawAlternative {
    transcript: String,
    #[serde(default)]
    words: Vec<Word>,
}

/// Returns the transcript in a Deepgram message, or `None` for metadata,
/// empty results and anything we don't understand.
pub fn parse(json: &str) -> Option<DeepgramResult> {
    let raw: RawMessage = serde_json::from_str(json).ok()?;
    if raw.kind != "Results" {
        return None;
    }
    let best = raw.channel?.alternatives.into_iter().next()?;
    let text = best.transcript.trim().to_string();
    if text.is_empty() {
        return None;
    }
    Some(DeepgramResult {
        text,
        is_final: raw.is_final,
        start: raw.start,
        duration: raw.duration,
        words: best.words,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_final_result() {
        let json = r#"{"type":"Results","channel_index":[0,1],"duration":1.5,"start":2.0,
            "is_final":true,"speech_final":true,
            "channel":{"alternatives":[{"transcript":"Buy gift cards.","confidence":0.98,
            "words":[{"word":"buy","start":2.1,"end":2.3,"confidence":0.9,"punctuated_word":"Buy"},
                     {"word":"gift","start":2.3,"end":2.6,"confidence":0.9,"punctuated_word":"gift"},
                     {"word":"cards","start":2.6,"end":3.1,"confidence":0.9,"punctuated_word":"cards."}]}]},
            "metadata":{"request_id":"x"}}"#;
        let result = parse(json).unwrap();
        assert_eq!(result.text, "Buy gift cards.");
        assert!(result.is_final);
        assert_eq!(result.end(), 3.5);
        assert_eq!(result.words.len(), 3);
        assert_eq!(result.words[2].end, 3.1);
    }

    #[test]
    fn ignores_metadata_and_empty_results() {
        assert!(parse(r#"{"type":"Metadata","request_id":"x"}"#).is_none());
        assert!(
            parse(r#"{"type":"Results","channel":{"alternatives":[{"transcript":""}]}}"#).is_none()
        );
        assert!(parse("not json").is_none());
    }
}
