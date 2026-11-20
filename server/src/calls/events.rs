//! Events published for each call and streamed to the live page.

use detector::{Risk, Speaker};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "lowercase")]
pub enum CallEvent {
    Transcript {
        speaker: Speaker,
        text: String,
        #[serde(rename = "final")]
        is_final: bool,
        /// Matched phrases in this text, for highlighting.
        highlights: Vec<String>,
    },
    Flag {
        rule: String,
        description: String,
        phrase: String,
        weight: u32,
        /// What the flag added to the score (0 if the rule already counted).
        added: u32,
        /// From the end of the matched words' audio arriving to this flag.
        latency_ms: Option<u64>,
    },
    Risk(Risk),
    Ended,
}

impl CallEvent {
    /// SSE event name.
    pub fn name(&self) -> &'static str {
        match self {
            CallEvent::Transcript { .. } => "transcript",
            CallEvent::Flag { .. } => "flag",
            CallEvent::Risk(_) => "risk",
            CallEvent::Ended => "ended",
        }
    }

    /// Whether late subscribers should see this event. Interim transcripts are
    /// superseded by their final, so they're only sent live.
    pub fn is_kept(&self) -> bool {
        !matches!(
            self,
            CallEvent::Transcript {
                is_final: false,
                ..
            }
        )
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use detector::RiskLevel;

    #[test]
    fn serializes_with_type_tags() {
        let risk = CallEvent::Risk(Risk {
            level: RiskLevel::High,
            score: 7,
        });
        assert_eq!(
            serde_json::to_string(&risk).unwrap(),
            r#"{"type":"risk","level":"High","score":7}"#
        );
        let transcript = CallEvent::Transcript {
            speaker: Speaker::Caller,
            text: "hi".into(),
            is_final: true,
            highlights: vec![],
        };
        assert_eq!(
            serde_json::to_string(&transcript).unwrap(),
            r#"{"type":"transcript","speaker":"caller","text":"hi","final":true,"highlights":[]}"#
        );
        assert_eq!(
            serde_json::to_string(&CallEvent::Ended).unwrap(),
            r#"{"type":"ended"}"#
        );
    }
}
