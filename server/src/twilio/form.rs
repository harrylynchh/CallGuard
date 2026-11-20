//! Minimal `application/x-www-form-urlencoded` helpers.
//!
//! The signature check needs the raw, ordered list of POST parameters, so we
//! decode the body ourselves instead of going through a typed extractor.

/// Decodes a form body into `(key, value)` pairs, preserving order and duplicates.
pub fn parse(body: &[u8]) -> Vec<(String, String)> {
    body.split(|&b| b == b'&')
        .filter(|pair| !pair.is_empty())
        .map(|pair| {
            let mut parts = pair.splitn(2, |&b| b == b'=');
            let key = decode(parts.next().unwrap_or_default());
            let value = decode(parts.next().unwrap_or_default());
            (key, value)
        })
        .collect()
}

/// Percent-encodes a value for use in a URL query string.
pub fn encode(value: &str) -> String {
    let mut out = String::with_capacity(value.len());
    for byte in value.bytes() {
        match byte {
            b'A'..=b'Z' | b'a'..=b'z' | b'0'..=b'9' | b'-' | b'_' | b'.' | b'~' => {
                out.push(byte as char)
            }
            _ => out.push_str(&format!("%{byte:02X}")),
        }
    }
    out
}

fn decode(raw: &[u8]) -> String {
    let mut out = Vec::with_capacity(raw.len());
    let mut i = 0;
    while i < raw.len() {
        match raw[i] {
            b'+' => out.push(b' '),
            b'%' if i + 2 < raw.len() => match hex_pair(raw[i + 1], raw[i + 2]) {
                Some(byte) => {
                    out.push(byte);
                    i += 2;
                }
                None => out.push(b'%'),
            },
            other => out.push(other),
        }
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn hex_pair(hi: u8, lo: u8) -> Option<u8> {
    let digit = |c: u8| (c as char).to_digit(16).map(|d| d as u8);
    Some(digit(hi)? * 16 + digit(lo)?)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn parses_twilio_style_body() {
        let pairs = parse(b"CallSid=CA123&From=%2B16175550123&Caller+Name=Jane+Doe&Empty=");
        assert_eq!(
            pairs,
            vec![
                ("CallSid".into(), "CA123".into()),
                ("From".into(), "+16175550123".into()),
                ("Caller Name".into(), "Jane Doe".into()),
                ("Empty".into(), "".into()),
            ]
        );
    }

    #[test]
    fn leaves_malformed_escapes_alone() {
        assert_eq!(
            parse(b"a=100%&b=%zz"),
            vec![("a".into(), "100%".into()), ("b".into(), "%zz".into())]
        );
    }

    #[test]
    fn encode_round_trips() {
        let encoded = encode("+1 (617) 555-0123");
        assert_eq!(encoded, "%2B1%20%28617%29%20555-0123");
        assert_eq!(
            parse(format!("x={encoded}").as_bytes())[0].1,
            "+1 (617) 555-0123"
        );
    }
}
