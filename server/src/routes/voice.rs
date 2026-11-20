//! Twilio voice webhooks: `/voice` answers the incoming call.

use axum::{
    Form,
    extract::{State, rejection::FormRejection},
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

pub async fn voice(
    State(state): State<AppState>,
    form: Result<Form<VoiceRequest>, FormRejection>,
) -> impl IntoResponse {
    let from = form.map(|Form(req)| req.from).unwrap_or_default();
    xml(twiml::screening(&state.config, &from))
}

pub fn xml(body: String) -> impl IntoResponse {
    ([(header::CONTENT_TYPE, "application/xml")], body)
}
