//! Twilio voice webhooks: `/voice` answers the incoming call and `/whisper`
//! announces the caller to the protected user.

use axum::{
    Form,
    extract::{Query, State, rejection::FormRejection},
    http::header,
    response::IntoResponse,
};
use serde::Deserialize;

use crate::{state::AppState, twilio::twiml};

/// The subset of Twilio's voice webhook parameters we use.
#[derive(Debug, Deserialize)]
pub struct VoiceRequest {
    #[serde(rename = "From", default)]
    pub from: String,
}

#[derive(Debug, Deserialize)]
pub struct WhisperQuery {
    #[serde(default)]
    pub from: String,
}

/// Answers an incoming call. This must never drop a legitimate call, so a body
/// we can't parse falls back to a plain `<Dial>` to the user.
pub async fn voice(
    State(state): State<AppState>,
    form: Result<Form<VoiceRequest>, FormRejection>,
) -> impl IntoResponse {
    let config = &state.config;
    let twiml = match form {
        Ok(Form(req)) if req.from == config.twilio_number => {
            tracing::warn!("loop guard: call from our own number, hanging up");
            twiml::hangup()
        }
        Ok(Form(req)) => twiml::screening(config, &req.from),
        Err(err) => {
            tracing::error!(%err, "could not parse /voice request; failing open");
            twiml::dial_only(config)
        }
    };
    xml(twiml)
}

pub async fn whisper(Query(query): Query<WhisperQuery>) -> impl IntoResponse {
    xml(twiml::whisper(&query.from))
}

fn xml(body: String) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/xml")], body)
}
