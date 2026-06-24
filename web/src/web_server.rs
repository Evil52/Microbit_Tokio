//! HTTP: отдаёт дашборд (/) и SSE-поток телеметрии (/events).

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::Html;
use axum::routing::get;
use axum::Router;
use futures_util::stream::Stream;
use serde_json::json;
use shared::Telemetry;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

#[derive(Clone)]
pub struct AppState {
    pub tx: broadcast::Sender<Telemetry>,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/events", get(events))
        .with_state(state)
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| {
        let t = res.ok()?; // пропускаем lagged-ошибки broadcast
        let payload = json!({
            "temp_c": t.temp_celsius(),
            "accel_mg": t.accel_mg,
            "btn_a": t.btn_a,
            "btn_b": t.btn_b,
        });
        Some(Ok(Event::default().data(payload.to_string())))
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}
