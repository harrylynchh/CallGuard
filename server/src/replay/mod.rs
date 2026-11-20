//! Replay mode: plays a script from `fixtures/scripts/` as a fake call through
//! the same detector and live page as a real one. The demo backup.

mod player;

use std::{
    path::PathBuf,
    sync::{
        Arc,
        atomic::{AtomicU64, Ordering},
    },
    time::Duration,
};

use crate::{
    calls::{CallAnalyzer, CallRecord, CallSource},
    state::AppState,
};

/// How fast scripts are "spoken".
#[derive(Debug, Clone, Copy)]
pub struct Pace {
    pub per_word: Duration,
    pub between_turns: Duration,
}

impl Pace {
    /// About 150 words per minute with a short pause between speakers.
    pub const REALISTIC: Pace = Pace {
        per_word: Duration::from_millis(400),
        between_turns: Duration::from_millis(700),
    };
    /// For tests.
    pub const INSTANT: Pace = Pace {
        per_word: Duration::ZERO,
        between_turns: Duration::ZERO,
    };
}

#[derive(Debug, Clone)]
pub struct ReplayConfig {
    pub scripts_dir: PathBuf,
    pub pace: Pace,
}

impl Default for ReplayConfig {
    fn default() -> Self {
        Self {
            scripts_dir: concat!(env!("CARGO_MANIFEST_DIR"), "/../fixtures/scripts").into(),
            pace: Pace::REALISTIC,
        }
    }
}

#[derive(Debug)]
pub enum ReplayError {
    InvalidName,
    NotFound(String),
}

impl std::fmt::Display for ReplayError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            ReplayError::InvalidName => {
                write!(f, "script names may only contain letters, digits, - and _")
            }
            ReplayError::NotFound(name) => write!(f, "no script named {name}"),
        }
    }
}

impl ReplayConfig {
    /// Script names (file stems), sorted.
    pub fn scripts(&self) -> Vec<String> {
        let Ok(entries) = std::fs::read_dir(&self.scripts_dir) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|e| {
                e.ok()?
                    .file_name()
                    .to_str()?
                    .strip_suffix(".txt")
                    .map(String::from)
            })
            .collect();
        names.sort();
        names
    }

    fn load(&self, name: &str) -> Result<String, ReplayError> {
        let valid = !name.is_empty()
            && name
                .chars()
                .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_');
        if !valid {
            return Err(ReplayError::InvalidName);
        }
        std::fs::read_to_string(self.scripts_dir.join(format!("{name}.txt")))
            .map_err(|_| ReplayError::NotFound(name.to_string()))
    }
}

/// Creates a replay call and starts playing it in the background.
pub fn start(state: &AppState, name: &str) -> Result<Arc<CallRecord>, ReplayError> {
    static NEXT_ID: AtomicU64 = AtomicU64::new(1);

    let script = detector::script::parse(&state.replay.load(name)?);
    let id = format!("replay-{name}-{}", NEXT_ID.fetch_add(1, Ordering::Relaxed));
    let record = state
        .calls
        .create(&id, &format!("Replay: {name}"), CallSource::Replay);
    let analyzer = CallAnalyzer::new(Arc::clone(&record), Arc::clone(&state.rules));
    tokio::spawn(player::play(script, analyzer, state.replay.pace));
    Ok(record)
}
