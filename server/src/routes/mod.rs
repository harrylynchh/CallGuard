//! HTTP routing.

mod voice;

use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::{state::AppState, twilio::verify::require_twilio_signature};

pub fn router(state: AppState) -> Router {
    let twilio = Router::new()
        .route("/voice", post(voice::voice))
        .route("/whisper", post(voice::whisper).get(voice::whisper))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_twilio_signature,
        ));

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(twilio)
        .with_state(state)
}
