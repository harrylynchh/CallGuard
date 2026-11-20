//! The live page: call list, per-call page, SSE stream and replay trigger.

use std::{convert::Infallible, sync::Arc};

use axum::{
    Json,
    extract::{Path, Query, State},
    http::StatusCode,
    response::{
        Html, IntoResponse, Response,
        sse::{Event, KeepAlive, Sse},
    },
};
use futures_util::{Stream, StreamExt, stream};
use serde::Deserialize;
use serde_json::json;
use tokio::sync::broadcast::{Receiver, error::RecvError};

use crate::{
    calls::{CallEvent, CallRecord},
    replay,
    state::AppState,
};

const INDEX_HTML: &str = include_str!("../../static/index.html");
const CALL_HTML: &str = include_str!("../../static/call.html");

pub async fn index() -> Html<&'static str> {
    Html(INDEX_HTML)
}

pub async fn call_page(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.calls.get(&id) {
        Some(_) => Html(CALL_HTML).into_response(),
        None => (StatusCode::NOT_FOUND, "No such call").into_response(),
    }
}

pub async fn list_calls(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.calls.summaries())
}

pub async fn get_call(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    match state.calls.get(&id) {
        Some(record) => Json(record.summary()).into_response(),
        None => StatusCode::NOT_FOUND.into_response(),
    }
}

pub async fn list_scripts(State(state): State<AppState>) -> impl IntoResponse {
    Json(state.replay.scripts())
}

pub async fn events(State(state): State<AppState>, Path(id): Path<String>) -> Response {
    let Some(record) = state.calls.get(&id) else {
        return StatusCode::NOT_FOUND.into_response();
    };
    let sse_events = event_stream(&record).map(|event| {
        let data = serde_json::to_string(&event).unwrap_or_default();
        Ok::<_, Infallible>(Event::default().event(event.name()).data(data))
    });
    Sse::new(sse_events)
        .keep_alive(KeepAlive::default())
        .into_response()
}

/// Past events, then live ones, ending right after `ended`.
pub fn event_stream(record: &Arc<CallRecord>) -> impl Stream<Item = CallEvent> + use<> {
    struct Cursor {
        history: std::vec::IntoIter<CallEvent>,
        live: Receiver<CallEvent>,
        done: bool,
    }

    let (history, live) = record.subscribe();
    let cursor = Cursor {
        history: history.into_iter(),
        live,
        done: false,
    };
    stream::unfold(cursor, |mut cursor| async move {
        if cursor.done {
            return None;
        }
        let event = match cursor.history.next() {
            Some(event) => event,
            None => next_live(&mut cursor.live).await?,
        };
        cursor.done = matches!(event, CallEvent::Ended);
        Some((event, cursor))
    })
}

async fn next_live(live: &mut Receiver<CallEvent>) -> Option<CallEvent> {
    loop {
        match live.recv().await {
            Ok(event) => return Some(event),
            // A slow client missed some interim updates; carry on.
            Err(RecvError::Lagged(_)) => continue,
            Err(RecvError::Closed) => return None,
        }
    }
}

#[derive(Deserialize)]
pub struct ReplayQuery {
    script: String,
}

pub async fn start_replay(
    State(state): State<AppState>,
    Query(query): Query<ReplayQuery>,
) -> Response {
    match replay::start(&state, &query.script) {
        Ok(record) => {
            let body = json!({ "id": record.id, "url": format!("/calls/{}", record.id) });
            (StatusCode::CREATED, Json(body)).into_response()
        }
        Err(err) => (StatusCode::BAD_REQUEST, err.to_string()).into_response(),
    }
}
