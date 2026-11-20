//! Runs the fixture scripts through the detector.

use std::sync::Arc;

use detector::{CallDetector, RiskLevel, RuleSet, Speaker, script};

fn fixture(name: &str) -> String {
    let path = format!(
        "{}/../fixtures/scripts/{name}.txt",
        env!("CARGO_MANIFEST_DIR")
    );
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("reading {path}: {e}"))
}

/// Plays a script as final transcripts only.
fn run_finals(name: &str) -> (CallDetector, Vec<String>) {
    let mut detector = CallDetector::new(Arc::new(RuleSet::default_rules()));
    let mut fired = Vec::new();
    for line in script::parse(&fixture(name)) {
        let obs = detector.observe(line.speaker, &line.text, true);
        fired.extend(obs.flags.into_iter().map(|f| f.rule_id));
    }
    (detector, fired)
}

/// Plays a script the way live speech-to-text would: growing interim
/// transcripts word by word, then the final.
fn run_with_interims(name: &str) -> CallDetector {
    let mut detector = CallDetector::new(Arc::new(RuleSet::default_rules()));
    for line in script::parse(&fixture(name)) {
        let words: Vec<&str> = line.text.split_whitespace().collect();
        for n in 1..words.len() {
            detector.observe(line.speaker, &words[..n].join(" "), false);
        }
        detector.observe(line.speaker, &line.text, true);
    }
    detector
}

#[test]
fn irs_scam_reaches_high() {
    let (detector, fired) = run_finals("scam_irs");
    assert_eq!(
        detector.risk().level,
        RiskLevel::High,
        "score {}",
        detector.risk().score
    );
    for rule in [
        "government_impersonation",
        "unusual_payment",
        "urgency",
        "secrecy",
    ] {
        assert!(
            fired.iter().any(|r| r == rule),
            "{rule} should fire: {fired:?}"
        );
    }
}

#[test]
fn tech_support_scam_reaches_high() {
    let (detector, fired) = run_finals("scam_tech_support");
    assert_eq!(
        detector.risk().level,
        RiskLevel::High,
        "score {}",
        detector.risk().score
    );
    for rule in [
        "tech_support",
        "sensitive_info",
        "unusual_payment",
        "secrecy",
        "urgency",
    ] {
        assert!(
            fired.iter().any(|r| r == rule),
            "{rule} should fire: {fired:?}"
        );
    }
}

#[test]
fn legitimate_pharmacy_call_stays_low() {
    let (detector, fired) = run_finals("legit_pharmacy");
    assert_eq!(detector.risk().level, RiskLevel::Low, "flags: {fired:?}");
    assert_eq!(detector.risk().score, 0, "flags: {fired:?}");
}

#[test]
fn interim_transcripts_give_the_same_score_as_finals() {
    for name in ["scam_irs", "scam_tech_support", "legit_pharmacy"] {
        assert_eq!(
            run_with_interims(name).risk(),
            run_finals(name).0.risk(),
            "{name}"
        );
    }
}

#[test]
fn user_lines_never_flag() {
    let mut detector = CallDetector::new(Arc::new(RuleSet::default_rules()));
    for line in script::parse(&fixture("scam_irs"))
        .into_iter()
        .filter(|l| l.speaker == Speaker::User)
    {
        assert!(
            detector
                .observe(line.speaker, &line.text, true)
                .flags
                .is_empty(),
            "{}",
            line.text
        );
    }
}
