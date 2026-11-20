//! TwiML responses. Kept as plain string builders: the documents are tiny and
//! this makes the exact XML easy to read and test.

use crate::config::Config;

pub const DISCLOSURE: &str =
    "This call is screened by CallGuard and may be transcribed to help protect against fraud.";

/// Disclosure only (build step 1).
pub fn screening(_config: &Config, _from: &str) -> String {
    format!("<Response><Say>{}</Say></Response>", escape(DISCLOSURE))
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

    #[test]
    fn escapes_xml_special_characters() {
        assert_eq!(escape(r#"a<b>&"c'"#), "a&lt;b&gt;&amp;&quot;c&apos;");
    }
}
