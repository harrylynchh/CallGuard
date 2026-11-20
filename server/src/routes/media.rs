//! `/media`: the WebSocket Twilio streams call audio to.

use axum::{
    extract::{State, WebSocketUpgrade},
    response::Response,
};

use crate::{media::stream, state::AppState};

pub async fn media(ws: WebSocketUpgrade, State(state): State<AppState>) -> Response {
    ws.on_upgrade(move |socket| stream::handle_socket(socket, state))
}
