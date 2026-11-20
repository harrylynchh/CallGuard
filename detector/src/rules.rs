//! Loading and compiling `rules.toml`.

use std::fmt;

use regex::{Regex, RegexBuilder};
use serde::Deserialize;

use crate::risk::Thresholds;

/// The rules shipped with the crate.
pub const DEFAULT_RULES: &str = include_str!("../rules.toml");

#[derive(Debug)]
pub struct Rule {
    pub id: String,
    pub description: String,
    pub weight: u32,
    regex: Regex,
}

impl Rule {
    /// Every place this rule matches in `text`, as byte ranges.
    pub fn find_all<'t>(&self, text: &'t str) -> impl Iterator<Item = regex::Match<'t>> {
        self.regex.find_iter(text)
    }
}

#[derive(Debug)]
pub struct RuleSet {
    pub rules: Vec<Rule>,
    pub thresholds: Thresholds,
}

#[derive(Debug)]
pub enum RulesError {
    Toml(toml::de::Error),
    EmptyRule(String),
    BadPattern { rule: String, error: regex::Error },
}

impl fmt::Display for RulesError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            RulesError::Toml(e) => write!(f, "invalid rules file: {e}"),
            RulesError::EmptyRule(id) => write!(f, "rule {id} has no phrases or patterns"),
            RulesError::BadPattern { rule, error } => {
                write!(f, "rule {rule} has a bad pattern: {error}")
            }
        }
    }
}

impl std::error::Error for RulesError {}

#[derive(Deserialize)]
struct RawRuleSet {
    thresholds: Thresholds,
    rules: Vec<RawRule>,
}

#[derive(Deserialize)]
struct RawRule {
    id: String,
    description: String,
    weight: u32,
    #[serde(default)]
    phrases: Vec<String>,
    #[serde(default)]
    patterns: Vec<String>,
}

impl RuleSet {
    pub fn from_toml(source: &str) -> Result<Self, RulesError> {
        let raw: RawRuleSet = toml::from_str(source).map_err(RulesError::Toml)?;
        let rules = raw
            .rules
            .into_iter()
            .map(compile)
            .collect::<Result<_, _>>()?;
        Ok(Self {
            rules,
            thresholds: raw.thresholds,
        })
    }

    /// The built-in `rules.toml`.
    pub fn default_rules() -> Self {
        Self::from_toml(DEFAULT_RULES).expect("bundled rules.toml is valid")
    }
}

/// Folds a rule's phrases and patterns into one case-insensitive regex.
fn compile(raw: RawRule) -> Result<Rule, RulesError> {
    let alternatives: Vec<String> = raw
        .phrases
        .iter()
        .map(|p| phrase_pattern(p))
        .chain(raw.patterns)
        .collect();
    if alternatives.is_empty() {
        return Err(RulesError::EmptyRule(raw.id));
    }
    let pattern = format!(r"\b(?:{})\b", alternatives.join("|"));
    let regex = RegexBuilder::new(&pattern)
        .case_insensitive(true)
        .build()
        .map_err(|error| RulesError::BadPattern {
            rule: raw.id.clone(),
            error,
        })?;
    Ok(Rule {
        id: raw.id,
        description: raw.description,
        weight: raw.weight,
        regex,
    })
}

/// A literal phrase where any run of whitespace matches its spaces.
fn phrase_pattern(phrase: &str) -> String {
    phrase
        .split_whitespace()
        .map(regex::escape)
        .collect::<Vec<_>>()
        .join(r"\s+")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn rule(phrases: &str, patterns: &str) -> String {
        format!(
            "[thresholds]\nmedium = 3\nhigh = 6\n[[rules]]\nid = \"r\"\ndescription = \"d\"\nweight = 1\n\
             phrases = {phrases}\npatterns = {patterns}\n"
        )
    }

    #[test]
    fn bundled_rules_load() {
        let rules = RuleSet::default_rules();
        assert_eq!(rules.rules.len(), 6);
        assert_eq!((rules.thresholds.medium, rules.thresholds.high), (3, 6));
    }

    #[test]
    fn phrases_are_case_insensitive_whole_words_with_flexible_spaces() {
        let set = RuleSet::from_toml(&rule(r#"["gift card", "IRS"]"#, "[]")).unwrap();
        let found = |text: &str| {
            set.rules[0]
                .find_all(text)
                .map(|m| m.as_str().to_string())
                .collect::<Vec<_>>()
        };
        assert_eq!(
            found("buy a GIFT  card from the irs"),
            ["GIFT  card", "irs"]
        );
        assert!(found("first card").is_empty());
        assert!(found("the stirs").is_empty());
    }

    #[test]
    fn reports_bad_rules() {
        assert!(matches!(
            RuleSet::from_toml(&rule("[]", "[]")),
            Err(RulesError::EmptyRule(_))
        ));
        assert!(matches!(
            RuleSet::from_toml(&rule("[]", r#"["(unclosed"]"#)),
            Err(RulesError::BadPattern { .. })
        ));
        assert!(matches!(
            RuleSet::from_toml("nope"),
            Err(RulesError::Toml(_))
        ));
    }
}
