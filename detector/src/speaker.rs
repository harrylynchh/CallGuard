use serde::Serialize;

/// Who said something. Only the caller's speech is scored.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Speaker {
    Caller,
    User,
}
