//! TwiML responses. Kept as plain string builders: the documents are tiny and
//! this makes the exact XML easy to read and test.

use super::form;
use crate::config::Config;

pub const DISCLOSURE: &str =
    "This call is screened by CallGuard and may be transcribed to help protect against fraud.";

/// Full screening flow: disclosure, start the media stream, then ring the
/// user with a whisper that announces who is calling.
pub fn screening(config: &Config, from: &str) -> String {
    let whisper_url = format!("{}/whisper?from={}", config.public_url, form::encode(from));
    format!(
        concat!(
            "<Response>",
            "<Say>{disclosure}</Say>",
            "<Start><Stream url=\"{stream}\" track=\"both_tracks\">",
            "<Parameter name=\"from\" value=\"{from}\"/>",
            "</Stream></Start>",
            "<Dial callerId=\"{caller_id}\"><Number url=\"{whisper}\">{user}</Number></Dial>",
            "</Response>"
        ),
        disclosure = escape(DISCLOSURE),
        stream = escape(&config.media_stream_url()),
        from = escape(from),
        caller_id = escape(&config.twilio_number),
        whisper = escape(&whisper_url),
        user = escape(&config.user_phone),
    )
}

/// Fail-open fallback: just connect the caller to the user, no screening.
pub fn dial_only(config: &Config) -> String {
    format!(
        "<Response><Dial callerId=\"{}\"><Number>{}</Number></Dial></Response>",
        escape(&config.twilio_number),
        escape(&config.user_phone),
    )
}

/// Played only to the user, before the call connects.
pub fn whisper(from: &str) -> String {
    format!(
        "<Response><Say>Screened call from {}</Say></Response>",
        escape(&speakable_number(from))
    )
}

/// Ends the call (used by the loop guard).
pub fn hangup() -> String {
    "<Response><Hangup/></Response>".to_string()
}

/// Turns `+16175550123` into `6 1 7 5 5 5 0 1 2 3` so text-to-speech reads it
/// digit by digit. The US country code is dropped; non-numbers become "an unknown number".
pub fn speakable_number(number: &str) -> String {
    let digits: String = number.chars().filter(char::is_ascii_digit).collect();
    let digits = match digits.strip_prefix('1') {
        Some(rest) if digits.len() == 11 => rest,
        _ => digits.as_str(),
    };
    if digits.is_empty() {
        return "an unknown number".to_string();
    }
    digits
        .chars()
        .map(String::from)
        .collect::<Vec<_>>()
        .join(" ")
}

/// Escapes text for use in XML content and attribute values.
pub fn escape(text: &str) -> String {
    let mut out = String::with_capacity(text.len());
    for c in text.chars() {
        match c {
            '&' => out.push_str("&amp;"),
            '<' => out.push_str("&lt;"),
            '>' => out.push_str("&gt;"),
            '"' => out.push_str("&quot;"),
            '\'' => out.push_str("&apos;"),
            _ => out.push(c),
        }
    }
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::config::Config;

    fn config() -> Config {
        Config::from_lookup(|key| {
            let value = match key {
                "TWILIO_AUTH_TOKEN" => "token",
                "TWILIO_NUMBER" => "+16175550100",
                "USER_PHONE" => "+16175550199",
                "PUBLIC_URL" => "https://cg.example.com",
                _ => return None,
            };
            Some(value.to_string())
        })
        .unwrap()
    }

    #[test]
    fn screening_twiml_matches_spec() {
        let expected = concat!(
            "<Response>",
            "<Say>This call is screened by CallGuard and may be transcribed to help protect against fraud.</Say>",
            "<Start><Stream url=\"wss://cg.example.com/media\" track=\"both_tracks\">",
            "<Parameter name=\"from\" value=\"+16175550123\"/>",
            "</Stream></Start>",
            "<Dial callerId=\"+16175550100\">",
            "<Number url=\"https://cg.example.com/whisper?from=%2B16175550123\">+16175550199</Number>",
            "</Dial>",
            "</Response>"
        );
        assert_eq!(screening(&config(), "+16175550123"), expected);
    }

    #[test]
    fn screening_escapes_hostile_caller_id() {
        let twiml = screening(&config(), "\"/><Hangup/>");
        assert!(!twiml.contains("<Hangup/>"), "{twiml}");
    }

    #[test]
    fn dial_only_connects_without_screening() {
        assert_eq!(
            dial_only(&config()),
            "<Response><Dial callerId=\"+16175550100\"><Number>+16175550199</Number></Dial></Response>"
        );
    }

    #[test]
    fn whisper_reads_digits() {
        assert_eq!(
            whisper("+16175550123"),
            "<Response><Say>Screened call from 6 1 7 5 5 5 0 1 2 3</Say></Response>"
        );
    }

    #[test]
    fn speakable_number_handles_odd_inputs() {
        assert_eq!(speakable_number("+442071838750"), "4 4 2 0 7 1 8 3 8 7 5 0");
        assert_eq!(speakable_number("Anonymous"), "an unknown number");
        assert_eq!(speakable_number(""), "an unknown number");
    }

    #[test]
    fn escapes_xml_special_characters() {
        assert_eq!(escape(r#"a<b>&"c'"#), "a&lt;b&gt;&amp;&quot;c&apos;");
    }
}
