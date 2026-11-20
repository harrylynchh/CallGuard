//! HTTP routing.

mod live;
mod media;
mod voice;

use axum::{
    Router, middleware,
    routing::{get, post},
};

use crate::{state::AppState, twilio::verify::require_twilio_signature};

pub fn router(state: AppState) -> Router {
    // Everything Twilio calls is signature-checked.
    let twilio = Router::new()
        .route("/voice", post(voice::voice))
        .route("/whisper", post(voice::whisper).get(voice::whisper))
        .route("/media", get(media::media))
        .route_layer(middleware::from_fn_with_state(
            state.clone(),
            require_twilio_signature,
        ));

    let live = Router::new()
        .route("/", get(live::index))
        .route("/calls/{id}", get(live::call_page))
        .route("/calls/{id}/events", get(live::events))
        .route("/api/calls", get(live::list_calls))
        .route("/api/calls/{id}", get(live::get_call))
        .route("/api/scripts", get(live::list_scripts))
        .route("/replay", post(live::start_replay));

    Router::new()
        .route("/health", get(|| async { "ok" }))
        .merge(twilio)
        .merge(live)
        .with_state(state)
}
