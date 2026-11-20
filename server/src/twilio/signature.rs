//! `X-Twilio-Signature` validation.
//!
//! Twilio signs each request with base64(HMAC-SHA1(auth_token, url + sorted params)),
//! where the params are appended as `key` then `value` for every POST parameter,
//! sorted by key. See <https://www.twilio.com/docs/usage/webhooks/webhooks-security>.

use base64::{Engine, engine::general_purpose::STANDARD as BASE64};
use hmac::{Hmac, KeyInit, Mac};
use sha1::Sha1;

type HmacSha1 = Hmac<Sha1>;

/// Computes the signature Twilio would send for this URL and parameter set.
pub fn compute(auth_token: &str, url: &str, params: &[(String, String)]) -> String {
    BASE64.encode(mac_for(auth_token, url, params).finalize().into_bytes())
}

/// Checks a received signature in constant time.
pub fn is_valid(auth_token: &str, url: &str, params: &[(String, String)], signature: &str) -> bool {
    let Ok(expected) = BASE64.decode(signature.trim()) else {
        return false;
    };
    mac_for(auth_token, url, params)
        .verify_slice(&expected)
        .is_ok()
}

fn mac_for(auth_token: &str, url: &str, params: &[(String, String)]) -> HmacSha1 {
    let mut sorted: Vec<&(String, String)> = params.iter().collect();
    sorted.sort_by(|a, b| a.0.cmp(&b.0)); // stable, so duplicate keys keep their order

    let mut mac =
        HmacSha1::new_from_slice(auth_token.as_bytes()).expect("HMAC accepts any key length");
    mac.update(url.as_bytes());
    for (key, value) in sorted {
        mac.update(key.as_bytes());
        mac.update(value.as_bytes());
    }
    mac
}

#[cfg(test)]
mod tests {
    use super::*;

    fn params(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect()
    }

    /// Example request from Twilio's webhook security docs. The expected value was
    /// cross-checked against Twilio's official `RequestValidator` (twilio-python).
    const TOKEN: &str = "12345";
    const URL: &str = "https://mycompany.com/myapp.php?foo=1&bar=2";
    const DOCS_PARAMS: &[(&str, &str)] = &[
        ("CallSid", "CA1234567890ABCDE"),
        ("Caller", "+12349013030"),
        ("Digits", "1234"),
        ("From", "+12349013030"),
        ("To", "+18005551212"),
    ];
    const EXPECTED: &str = "0/KCTR6DLpKmkAf8muzZqo1nDgQ=";

    #[test]
    fn matches_twilio_docs_example() {
        assert_eq!(compute(TOKEN, URL, &params(DOCS_PARAMS)), EXPECTED);
        assert!(is_valid(TOKEN, URL, &params(DOCS_PARAMS), EXPECTED));
    }

    #[test]
    fn parameter_order_does_not_matter() {
        let mut reversed = params(DOCS_PARAMS);
        reversed.reverse();
        assert!(is_valid(TOKEN, URL, &reversed, EXPECTED));
    }

    #[test]
    fn rejects_tampering() {
        let mut tampered = params(DOCS_PARAMS);
        tampered[3].1 = "+15555555555".into();
        assert!(!is_valid(TOKEN, URL, &tampered, EXPECTED));
        assert!(!is_valid(
            TOKEN,
            "https://evil.example.com/myapp.php?foo=1&bar=2",
            &params(DOCS_PARAMS),
            EXPECTED
        ));
        assert!(!is_valid(
            "wrong-token",
            URL,
            &params(DOCS_PARAMS),
            EXPECTED
        ));
        assert!(!is_valid(TOKEN, URL, &params(DOCS_PARAMS), "not base64!"));
    }
}
