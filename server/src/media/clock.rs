//! Maps positions in a Deepgram audio stream back to when that audio arrived
//! from Twilio, so we can measure detection latency.
//!
//! Deepgram reports word times in seconds from the start of the audio we sent
//! on that connection. We record, for every frame we forward, the stream
//! position where it ends and the instant it arrived.

use std::{collections::VecDeque, time::Instant};

use crate::audio::SAMPLE_RATE;

/// Two minutes of 20 ms frames. Older entries are dropped; we never need them.
const MAX_FRAMES: usize = 6000;

#[derive(Debug, Default)]
pub struct FrameClock {
    sent_ms: f64,
    frames: VecDeque<(f64, Instant)>,
}

impl FrameClock {
    /// Records a frame of `len` μ-law bytes that arrived at `received_at`.
    pub fn record(&mut self, len: usize, received_at: Instant) {
        self.sent_ms += len as f64 * 1000.0 / f64::from(SAMPLE_RATE);
        self.frames.push_back((self.sent_ms, received_at));
        if self.frames.len() > MAX_FRAMES {
            self.frames.pop_front();
        }
    }

    /// When the frame containing stream time `seconds` arrived.
    pub fn arrival_of(&self, seconds: f64) -> Option<Instant> {
        let target_ms = seconds * 1000.0;
        let index = self
            .frames
            .partition_point(|(end_ms, _)| *end_ms < target_ms);
        self.frames
            .get(index)
            .or(self.frames.back())
            .map(|(_, at)| *at)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::time::Duration;

    #[test]
    fn finds_frame_containing_time() {
        let t0 = Instant::now();
        let mut clock = FrameClock::default();
        for i in 0..10 {
            clock.record(160, t0 + Duration::from_millis(i * 20));
        }
        // 0.05 s falls in the third frame (40–60 ms).
        assert_eq!(clock.arrival_of(0.05), Some(t0 + Duration::from_millis(40)));
        assert_eq!(clock.arrival_of(0.0), Some(t0));
        // Past the end clamps to the newest frame.
        assert_eq!(clock.arrival_of(9.0), Some(t0 + Duration::from_millis(180)));
        assert_eq!(FrameClock::default().arrival_of(1.0), None);
    }

    #[test]
    fn keeps_memory_bounded() {
        let mut clock = FrameClock::default();
        for _ in 0..MAX_FRAMES + 10 {
            clock.record(160, Instant::now());
        }
        assert_eq!(clock.frames.len(), MAX_FRAMES);
    }
}
