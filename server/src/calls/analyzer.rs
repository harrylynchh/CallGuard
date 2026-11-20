//! Runs transcripts through the detector and publishes the results. Shared by
//! live calls (Deepgram) and replays (scripts).

use std::{sync::Arc, time::Instant};

use detector::{CallDetector, Flag, RuleSet, Speaker};

use super::{events::CallEvent, record::CallRecord};

/// A transcript, from speech-to-text or a replay script.
pub struct Transcript {
    pub speaker: Speaker,
    pub text: String,
    pub is_final: bool,
    /// When the audio for the end of each word arrived, one entry per
    /// whitespace-separated word of `text` where known. Used for latency.
    pub word_ends: Vec<Instant>,
}

pub struct CallAnalyzer {
    detector: CallDetector,
    record: Arc<CallRecord>,
}

impl CallAnalyzer {
    pub fn new(record: Arc<CallRecord>, rules: Arc<RuleSet>) -> Self {
        let detector = CallDetector::new(rules);
        record.publish(CallEvent::Risk(detector.risk()));
        Self { detector, record }
    }

    pub fn handle(&mut self, transcript: Transcript) {
        let observation =
            self.detector
                .observe(transcript.speaker, &transcript.text, transcript.is_final);
        let flagged_at = Instant::now();

        self.record.publish(CallEvent::Transcript {
            speaker: transcript.speaker,
            highlights: observation.matches.into_iter().map(|m| m.phrase).collect(),
            text: transcript.text.clone(),
            is_final: transcript.is_final,
        });
        for flag in observation.flags {
            let latency_ms = audio_end_of(&flag, &transcript)
                .map(|at| flagged_at.duration_since(at).as_millis() as u64);
            self.record.publish(flag_event(flag, latency_ms));
        }
        if observation.risk_changed {
            self.record.publish(CallEvent::Risk(observation.risk));
        }
    }

    pub fn finish(self) {
        self.record.publish(CallEvent::Ended);
    }
}

/// When the audio containing the last word of the flagged phrase arrived.
fn audio_end_of(flag: &Flag, transcript: &Transcript) -> Option<Instant> {
    let word_index = transcript.text[..flag.match_end]
        .split_whitespace()
        .count()
        .saturating_sub(1);
    transcript
        .word_ends
        .get(word_index)
        .or(transcript.word_ends.last())
        .copied()
}

fn flag_event(flag: Flag, latency_ms: Option<u64>) -> CallEvent {
    CallEvent::Flag {
        rule: flag.rule_id,
        description: flag.description,
        phrase: flag.phrase,
        weight: flag.weight,
        added: flag.added,
        latency_ms,
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::calls::record::CallSource;
    use std::time::Duration;

    #[test]
    fn publishes_transcript_flag_and_risk_with_latency() {
        let record = Arc::new(CallRecord::new("CA1".into(), "+1".into(), CallSource::Live));
        let mut analyzer =
            CallAnalyzer::new(Arc::clone(&record), Arc::new(RuleSet::default_rules()));

        // "IRS" is the fourth word (index 3); pretend its audio arrived 250 ms ago.
        let now = Instant::now();
        let word_ends = vec![now, now, now, now - Duration::from_millis(250)];
        let text = "This is the IRS".to_string();
        analyzer.handle(Transcript {
            speaker: Speaker::Caller,
            text,
            is_final: true,
            word_ends,
        });
        analyzer.finish();

        let (history, _) = record.subscribe();
        let names: Vec<_> = history.iter().map(CallEvent::name).collect();
        assert_eq!(names, ["risk", "transcript", "flag", "risk", "ended"]);
        let CallEvent::Flag {
            latency_ms: Some(latency),
            ..
        } = &history[2]
        else {
            panic!("{history:?}")
        };
        assert!((250..1000).contains(latency), "{latency}");
    }
}
