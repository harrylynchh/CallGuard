//! One call's shared, in-memory record: who called, its event history and a
//! broadcast channel for live subscribers. Holds text only, never audio.

use std::{
    sync::Mutex,
    time::{SystemTime, UNIX_EPOCH},
};

use detector::{Risk, RiskLevel};
use serde::Serialize;
use tokio::sync::broadcast;

use super::events::CallEvent;

const LIVE_BUFFER: usize = 256;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum CallSource {
    Live,
    Replay,
}

/// What the call list shows.
#[derive(Debug, Clone, Serialize)]
pub struct CallSummary {
    pub id: String,
    pub from: String,
    pub source: CallSource,
    pub started_at_ms: u64,
    pub risk: Risk,
    pub ended: bool,
}

pub struct CallRecord {
    pub id: String,
    pub from: String,
    pub source: CallSource,
    pub started_at_ms: u64,
    inner: Mutex<Inner>,
    live: broadcast::Sender<CallEvent>,
}

struct Inner {
    history: Vec<CallEvent>,
    risk: Risk,
    ended: bool,
}

impl CallRecord {
    pub fn new(id: String, from: String, source: CallSource) -> Self {
        let started_at_ms = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .map_or(0, |d| d.as_millis() as u64);
        let inner = Inner {
            history: Vec::new(),
            risk: Risk {
                level: RiskLevel::Low,
                score: 0,
            },
            ended: false,
        };
        Self {
            id,
            from,
            source,
            started_at_ms,
            inner: Mutex::new(inner),
            live: broadcast::channel(LIVE_BUFFER).0,
        }
    }

    pub fn publish(&self, event: CallEvent) {
        let mut inner = self.lock();
        match &event {
            CallEvent::Risk(risk) => inner.risk = *risk,
            CallEvent::Ended => inner.ended = true,
            _ => {}
        }
        if event.is_kept() {
            inner.history.push(event.clone());
        }
        // Sent while holding the lock so subscribers never miss or repeat an event.
        let _ = self.live.send(event);
    }

    /// Everything so far plus a receiver for what comes next, with no gap.
    pub fn subscribe(&self) -> (Vec<CallEvent>, broadcast::Receiver<CallEvent>) {
        let inner = self.lock();
        (inner.history.clone(), self.live.subscribe())
    }

    pub fn is_ended(&self) -> bool {
        self.lock().ended
    }

    pub fn summary(&self) -> CallSummary {
        let inner = self.lock();
        CallSummary {
            id: self.id.clone(),
            from: self.from.clone(),
            source: self.source,
            started_at_ms: self.started_at_ms,
            risk: inner.risk,
            ended: inner.ended,
        }
    }

    fn lock(&self) -> std::sync::MutexGuard<'_, Inner> {
        self.inner
            .lock()
            .unwrap_or_else(|poisoned| poisoned.into_inner())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use detector::Speaker;

    fn transcript(text: &str, is_final: bool) -> CallEvent {
        CallEvent::Transcript {
            speaker: Speaker::Caller,
            text: text.into(),
            is_final,
            highlights: vec![],
        }
    }

    #[test]
    fn late_subscribers_get_history_without_interims() {
        let record = CallRecord::new("CA1".into(), "+1".into(), CallSource::Live);
        record.publish(transcript("hel", false));
        record.publish(transcript("hello", true));
        record.publish(CallEvent::Risk(Risk {
            level: RiskLevel::Medium,
            score: 3,
        }));

        let (history, mut live) = record.subscribe();
        assert_eq!(history.len(), 2);
        record.publish(CallEvent::Ended);
        assert!(matches!(live.try_recv(), Ok(CallEvent::Ended)));

        let summary = record.summary();
        assert_eq!((summary.risk.score, summary.ended), (3, true));
    }
}
