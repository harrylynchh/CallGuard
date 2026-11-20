//! Per-call detection state.

use std::{collections::HashSet, sync::Arc};

use serde::Serialize;

use crate::{
    Speaker,
    risk::Risk,
    rules::{Rule, RuleSet},
};

/// A rule match inside one transcript.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Match {
    pub rule_id: String,
    /// The text that matched, as it appears in the transcript.
    pub phrase: String,
    /// Byte offsets of the match within the transcript.
    pub start: usize,
    pub end: usize,
}

/// A newly raised flag, to show on the live page.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Flag {
    pub rule_id: String,
    pub description: String,
    pub phrase: String,
    /// The rule's configured weight.
    pub weight: u32,
    /// What this flag added to the score: the weight the first time the rule
    /// fires in a call, zero after that.
    pub added: u32,
    /// Byte offset in the transcript where the matched phrase ends.
    pub match_end: usize,
}

/// What one transcript changed.
#[derive(Debug, Clone, PartialEq)]
pub struct Observation {
    /// Every match in the transcript, including ones flagged earlier (for highlighting).
    pub matches: Vec<Match>,
    /// Flags not raised before for this utterance.
    pub flags: Vec<Flag>,
    pub risk: Risk,
    pub risk_changed: bool,
}

/// Tracks one call. Feed it every transcript, interim and final, in order.
pub struct CallDetector {
    rules: Arc<RuleSet>,
    /// Rules that have already added their weight in this call.
    scored_rules: HashSet<String>,
    /// `(rule, phrase)` already flagged in the caller's current utterance, so
    /// that interim results and the final for the same words flag only once.
    flagged_in_utterance: HashSet<(String, String)>,
    score: u32,
}

impl CallDetector {
    pub fn new(rules: Arc<RuleSet>) -> Self {
        Self {
            rules,
            scored_rules: HashSet::new(),
            flagged_in_utterance: HashSet::new(),
            score: 0,
        }
    }

    pub fn risk(&self) -> Risk {
        Risk {
            level: self.rules.thresholds.level(self.score),
            score: self.score,
        }
    }

    pub fn observe(&mut self, speaker: Speaker, text: &str, is_final: bool) -> Observation {
        let before = self.risk();
        if speaker == Speaker::User {
            return Observation {
                matches: Vec::new(),
                flags: Vec::new(),
                risk: before,
                risk_changed: false,
            };
        }

        let rules = Arc::clone(&self.rules);
        let matches: Vec<(&Rule, Match)> = rules
            .rules
            .iter()
            .flat_map(|rule| rule.find_all(text).map(move |m| (rule, to_match(rule, m))))
            .collect();
        let flags = matches
            .iter()
            .filter_map(|(rule, m)| self.raise(rule, m))
            .collect();

        if is_final {
            self.flagged_in_utterance.clear();
        }
        let risk = self.risk();
        Observation {
            matches: matches.into_iter().map(|(_, m)| m).collect(),
            flags,
            risk,
            risk_changed: risk != before,
        }
    }

    /// Raises a flag for `m` unless this utterance already flagged it, adding
    /// the rule's weight the first time the rule fires in the call.
    fn raise(&mut self, rule: &Rule, m: &Match) -> Option<Flag> {
        let key = (rule.id.clone(), m.phrase.to_lowercase());
        if !self.flagged_in_utterance.insert(key) {
            return None;
        }
        let added = if self.scored_rules.insert(rule.id.clone()) {
            rule.weight
        } else {
            0
        };
        self.score += added;
        Some(Flag {
            rule_id: rule.id.clone(),
            description: rule.description.clone(),
            phrase: m.phrase.clone(),
            weight: rule.weight,
            added,
            match_end: m.end,
        })
    }
}

fn to_match(rule: &Rule, m: regex::Match<'_>) -> Match {
    Match {
        rule_id: rule.id.clone(),
        phrase: m.as_str().to_string(),
        start: m.start(),
        end: m.end(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::RiskLevel;

    fn detector() -> CallDetector {
        CallDetector::new(Arc::new(RuleSet::default_rules()))
    }

    #[test]
    fn scores_each_rule_once_per_call() {
        let mut d = detector();
        let first = d.observe(Speaker::Caller, "Buy gift cards.", true);
        assert_eq!(first.flags.len(), 1);
        assert_eq!((first.flags[0].added, first.risk.score), (4, 4));
        assert!(first.risk_changed);

        let again = d.observe(Speaker::Caller, "I said Google Play gift cards!", true);
        assert_eq!(again.flags.len(), 2, "still flagged so the user sees it");
        assert!(again.flags.iter().all(|f| f.added == 0));
        assert_eq!(again.risk.score, 4);
        assert!(!again.risk_changed);
    }

    #[test]
    fn interim_then_final_does_not_double_count_or_reflag() {
        let mut d = detector();
        assert!(
            d.observe(Speaker::Caller, "this is the", false)
                .flags
                .is_empty()
        );
        let interim = d.observe(Speaker::Caller, "this is the IRS", false);
        assert_eq!(interim.flags.len(), 1, "partials may fire early");
        let interim2 = d.observe(Speaker::Caller, "this is the IRS calling", false);
        assert!(interim2.flags.is_empty());
        let fin = d.observe(Speaker::Caller, "This is the IRS calling.", true);
        assert!(fin.flags.is_empty(), "final must not re-flag");
        assert_eq!(
            fin.matches.len(),
            1,
            "but still reports the match for highlighting"
        );
        assert_eq!(fin.risk.score, 3);
    }

    #[test]
    fn user_speech_is_not_scored() {
        let mut d = detector();
        let obs = d.observe(
            Speaker::User,
            "Should I buy gift cards right now for the IRS?",
            true,
        );
        assert!(obs.flags.is_empty() && obs.matches.is_empty());
        assert_eq!(
            obs.risk,
            Risk {
                level: RiskLevel::Low,
                score: 0
            }
        );
    }

    #[test]
    fn flag_reports_match_position() {
        let mut d = detector();
        let text = "Download AnyDesk now";
        let flag = &d.observe(Speaker::Caller, text, true).flags[0];
        assert_eq!(
            (flag.rule_id.as_str(), flag.phrase.as_str()),
            ("tech_support", "AnyDesk")
        );
        assert_eq!(&text[..flag.match_end], "Download AnyDesk");
    }

    #[test]
    fn contractions_and_spoken_variants_match() {
        let mut d = detector();
        let flags = d
            .observe(
                Speaker::Caller,
                "Don't tell anyone and do not hang up. Use any desk.",
                true,
            )
            .flags;
        let rules: HashSet<_> = flags.iter().map(|f| f.rule_id.as_str()).collect();
        assert!(
            rules.contains("secrecy") && rules.contains("tech_support"),
            "{flags:?}"
        );
    }
}
