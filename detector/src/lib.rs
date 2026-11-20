//! Rules-based scam phrase detector. Pure library: no networking, no async.
//!
//! Feed each transcript to a [`CallDetector`] as `(speaker, text, is_final)`;
//! it returns new [`Flag`]s and the call's current [`Risk`]. Rules and risk
//! thresholds live in `rules.toml`.

mod detector;
mod risk;
mod rules;
pub mod script;
mod speaker;

pub use detector::{CallDetector, Flag, Match, Observation};
pub use risk::{Risk, RiskLevel, Thresholds};
pub use rules::{DEFAULT_RULES, Rule, RuleSet, RulesError};
pub use speaker::Speaker;
