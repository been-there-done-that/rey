use crate::state::AppState;
use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::Event;
use axum::response::Sse;
use axum::Json;
use futures::{Stream, StreamExt};
use std::convert::Infallible;
use std::time::Duration;
use tokio_stream::wrappers::BroadcastStream;
use types::error::ErrorResponse;
use types::sse::SseEvent;
use uuid::Uuid;

pub async fn sse_stream(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
) -> Result<Sse<impl Stream<Item = Result<Event, Infallible>>>, (StatusCode, Json<ErrorResponse>)> {
    let rx = state.sse_hub.subscribe(&user_id.to_string());

    let stream = BroadcastStream::new(rx)
        .filter_map(|result| async move {
            match result {
                Ok(event) => serde_json::to_string(&event)
                    .ok()
                    .map(|json| Ok(Event::default().data(json))),
                Err(_) => None,
            }
        })
        .chain(tokio_stream::once(Ok(
            Event::default().data("connection closed")
        )));

    Ok(Sse::new(stream).keep_alive(
        axum::response::sse::KeepAlive::new()
            .interval(Duration::from_secs(15))
            .text("ping"),
    ))
}

pub async fn send_test_event(
    State(state): State<AppState>,
    axum::extract::Extension(user_id): axum::extract::Extension<Uuid>,
) -> Result<StatusCode, (StatusCode, Json<ErrorResponse>)> {
    let event = SseEvent::Heartbeat {
        timestamp: chrono::Utc::now().timestamp_millis(),
    };
    state.sse_hub.broadcast(&user_id.to_string(), event);
    Ok(StatusCode::ACCEPTED)
}
