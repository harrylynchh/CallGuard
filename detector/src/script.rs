//! Parser for the call scripts in `fixtures/scripts/`: one line per turn,
//! `CALLER: ...` or `USER: ...`.

use crate::Speaker;

#[derive(Debug, Clone, PartialEq)]
pub struct Line {
    pub speaker: Speaker,
    pub text: String,
}

/// Parses a script, skipping blank lines and lines without a known speaker.
pub fn parse(script: &str) -> Vec<Line> {
    script
        .lines()
        .filter_map(|line| {
            let (who, text) = line.split_once(':')?;
            let speaker = match who.trim().to_ascii_uppercase().as_str() {
                "CALLER" => Speaker::Caller,
                "USER" => Speaker::User,
                _ => return None,
            };
            let text = text.trim();
            (!text.is_empty()).then(|| Line {
                speaker,
                text: text.to_string(),
            })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_speakers_and_skips_noise() {
        let lines =
            parse("CALLER: Hello there.\n\nUSER: Hi: who is this?\n# comment\nNARRATOR: ignored\n");
        assert_eq!(
            lines,
            vec![
                Line {
                    speaker: Speaker::Caller,
                    text: "Hello there.".into()
                },
                Line {
                    speaker: Speaker::User,
                    text: "Hi: who is this?".into()
                },
            ]
        );
    }
}
