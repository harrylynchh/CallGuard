//! Plays a parsed script word by word, like live speech-to-text would:
//! growing interim transcripts, then a final one per line.
//!
//! There is no audio in a replay, so flags carry no latency.

use detector::script::Line;

use super::Pace;
use crate::calls::{CallAnalyzer, Transcript};

pub async fn play(script: Vec<Line>, mut analyzer: CallAnalyzer, pace: Pace) {
    for line in script {
        speak_line(&line, &mut analyzer, pace).await;
        tokio::time::sleep(pace.between_turns).await;
    }
    analyzer.finish();
}

async fn speak_line(line: &Line, analyzer: &mut CallAnalyzer, pace: Pace) {
    let words: Vec<&str> = line.text.split_whitespace().collect();
    for i in 0..words.len() {
        tokio::time::sleep(pace.per_word).await;
        let is_final = i + 1 == words.len();
        let text = if is_final {
            line.text.clone()
        } else {
            words[..=i].join(" ")
        };
        analyzer.handle(Transcript {
            speaker: line.speaker,
            text,
            is_final,
            word_ends: Vec::new(),
        });
    }
}
