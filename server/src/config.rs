//! Runtime configuration, loaded from the environment (and `.env` via dotenvy).

use anyhow::{Context, Result, bail};

#[derive(Debug, Clone)]
pub struct Config {
    /// Used to verify `X-Twilio-Signature`. Empty only when signatures are skipped.
    pub twilio_auth_token: String,
    /// Our Twilio phone number, in E.164 form.
    pub twilio_number: String,
    /// The protected user's phone, in E.164 form.
    pub user_phone: String,
    /// Optional: without it calls still connect, they just aren't transcribed.
    pub deepgram_api_key: Option<String>,
    /// Public base URL (e.g. `https://callguard.example.com`), no trailing slash.
    pub public_url: String,
    pub port: u16,
    /// Local testing only: disables Twilio signature checks.
    pub skip_twilio_signature: bool,
}

impl Config {
    /// Loads `.env` (if present) and then reads the process environment.
    pub fn from_env() -> Result<Self> {
        let _ = dotenvy::dotenv();
        Self::from_lookup(|key| std::env::var(key).ok())
    }

    /// Builds a config from any key lookup, which keeps tests independent of the real env.
    pub fn from_lookup(lookup: impl Fn(&str) -> Option<String>) -> Result<Self> {
        let get = |key: &str| {
            lookup(key)
                .map(|v| v.trim().to_string())
                .filter(|v| !v.is_empty())
        };
        let require =
            |key: &str| get(key).with_context(|| format!("missing required env var {key}"));

        let skip_twilio_signature = get("SKIP_TWILIO_SIGNATURE").is_some_and(|v| v == "1");
        let twilio_auth_token = match get("TWILIO_AUTH_TOKEN") {
            Some(token) => token,
            None if skip_twilio_signature => String::new(),
            None => bail!(
                "missing TWILIO_AUTH_TOKEN (or set SKIP_TWILIO_SIGNATURE=1 for local testing)"
            ),
        };
        let port = match get("PORT") {
            Some(p) => p
                .parse()
                .with_context(|| format!("PORT is not a valid port: {p}"))?,
            None => 3000,
        };

        Ok(Self {
            twilio_auth_token,
            twilio_number: require("TWILIO_NUMBER")?,
            user_phone: require("USER_PHONE")?,
            deepgram_api_key: get("DEEPGRAM_API_KEY"),
            public_url: require("PUBLIC_URL")?.trim_end_matches('/').to_string(),
            port,
            skip_twilio_signature,
        })
    }

    /// Host part of `PUBLIC_URL`, e.g. `callguard.example.com`.
    pub fn public_host(&self) -> &str {
        self.public_url
            .split_once("://")
            .map_or(self.public_url.as_str(), |(_, rest)| rest)
    }

    /// WebSocket URL Twilio should stream media to.
    pub fn media_stream_url(&self) -> String {
        format!("wss://{}/media", self.public_host())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    fn config_from(pairs: &[(&str, &str)]) -> Result<Config> {
        let map: HashMap<String, String> = pairs
            .iter()
            .map(|(k, v)| (k.to_string(), v.to_string()))
            .collect();
        Config::from_lookup(|k| map.get(k).cloned())
    }

    const BASE: &[(&str, &str)] = &[
        ("TWILIO_AUTH_TOKEN", "secret"),
        ("TWILIO_NUMBER", "+16175550100"),
        ("USER_PHONE", "+16175550199"),
        ("PUBLIC_URL", "https://cg.example.com/"),
    ];

    #[test]
    fn loads_defaults_and_trims_trailing_slash() {
        let config = config_from(BASE).unwrap();
        assert_eq!(config.port, 3000);
        assert_eq!(config.public_url, "https://cg.example.com");
        assert_eq!(config.public_host(), "cg.example.com");
        assert_eq!(config.media_stream_url(), "wss://cg.example.com/media");
        assert!(config.deepgram_api_key.is_none());
        assert!(!config.skip_twilio_signature);
    }

    #[test]
    fn auth_token_is_optional_only_when_skipping_signatures() {
        let without_token: Vec<_> = BASE[1..].to_vec();
        assert!(config_from(&without_token).is_err());

        let mut skipping = without_token;
        skipping.push(("SKIP_TWILIO_SIGNATURE", "1"));
        assert!(config_from(&skipping).unwrap().skip_twilio_signature);
    }

    #[test]
    fn rejects_bad_port() {
        let mut pairs = BASE.to_vec();
        pairs.push(("PORT", "not-a-port"));
        assert!(config_from(&pairs).is_err());
    }
}
