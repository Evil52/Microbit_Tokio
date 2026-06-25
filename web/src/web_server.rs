//! HTTP: отдаёт дашборд (/) и SSE-поток телеметрии (/events).

use std::convert::Infallible;
use std::time::Duration;

use axum::extract::State;
use axum::http::StatusCode;
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::Html;
use axum::routing::{get, post};
use axum::Router;
use futures_util::stream::Stream;
use serde_json::json;
use shared::Telemetry;
use tokio::sync::broadcast;
use tokio_stream::wrappers::BroadcastStream;
use tokio_stream::StreamExt;

use crate::flash::FlashGate;

#[derive(Clone)]
pub struct AppState {
    pub tx: broadcast::Sender<Telemetry>,
    pub gate: FlashGate,
}

pub fn router(state: AppState) -> Router {
    Router::new()
        .route("/", get(index))
        .route("/events", get(events))
        .route("/flash", post(flash_handler))
        .with_state(state)
}

async fn flash_handler(State(state): State<AppState>) -> (StatusCode, String) {
    match crate::flash::flash(&state.gate).await {
        Ok(msg) => (StatusCode::OK, msg),
        Err(e) => (StatusCode::INTERNAL_SERVER_ERROR, e),
    }
}

async fn index() -> Html<&'static str> {
    Html(include_str!("../static/index.html"))
}

/// JSON-представление кадра для дашборда. Это публичный контракт с фронтендом
/// (static/index.html ждёт именно эти ключи), поэтому вынесено и протестировано.
fn telemetry_json(t: &Telemetry) -> serde_json::Value {
    json!({
        "temp_c": t.temp_celsius(),
        "accel_mg": t.accel_mg,
        "btn_a": t.btn_a,
        "btn_b": t.btn_b,
    })
}

async fn events(
    State(state): State<AppState>,
) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let rx = state.tx.subscribe();
    let stream = BroadcastStream::new(rx).filter_map(|res| {
        let t = res.ok()?;
        Some(Ok(Event::default().data(telemetry_json(&t).to_string())))
    });

    Sse::new(stream).keep_alive(KeepAlive::new().interval(Duration::from_secs(15)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Telemetry {
        Telemetry {
            temp_q4: 84, // 21.0 °C
            accel_mg: [10, -20, 1000],
            btn_a: 7,
            btn_b: 3,
        }
    }

    #[test]
    fn json_has_dashboard_contract_keys() {
        let v = telemetry_json(&sample());
        // Ключи, которые читает static/index.html. Меняешь — ломаешь фронт.
        assert!(v.get("temp_c").is_some());
        assert!(v.get("accel_mg").is_some());
        assert!(v.get("btn_a").is_some());
        assert!(v.get("btn_b").is_some());
    }

    #[test]
    fn json_values_are_correct() {
        let v = telemetry_json(&sample());
        assert_eq!(v["temp_c"], 21.0);
        assert_eq!(v["accel_mg"], json!([10, -20, 1000]));
        assert_eq!(v["btn_a"], 7);
        assert_eq!(v["btn_b"], 3);
    }

    #[test]
    fn json_temp_is_celsius_not_raw_q4() {
        // Регрессия: на провод идёт temp_q4 (шаги 0.25°), а в JSON — °C.
        let mut t = sample();
        t.temp_q4 = 100; // 25.0 °C
        assert_eq!(telemetry_json(&t)["temp_c"], 25.0);
    }

    #[test]
    fn negative_temp_serializes() {
        let mut t = sample();
        t.temp_q4 = -40; // -10.0 °C
        assert_eq!(telemetry_json(&t)["temp_c"], -10.0);
    }

    /// Сквозной путь: published Telemetry → broadcast → тот же filter_map, что в SSE.
    /// Проверяем, что подписчик получает корректный JSON-payload.
    #[tokio::test]
    async fn broadcast_to_json_pipeline() {
        let (tx, rx) = broadcast::channel(4);
        let t = sample();
        tx.send(t).unwrap();

        let mut stream = BroadcastStream::new(rx).filter_map(|res| {
            let t = res.ok()?;
            Some(telemetry_json(&t))
        });

        let v = stream.next().await.expect("должен прийти кадр");
        assert_eq!(v["temp_c"], 21.0);
        assert_eq!(v["btn_a"], 7);
    }

    /// Несколько подписчиков получают один и тот же кадр (fan-out SSE).
    #[tokio::test]
    async fn multiple_subscribers_receive_same_frame() {
        let (tx, _) = broadcast::channel::<Telemetry>(4);
        let mut a = BroadcastStream::new(tx.subscribe());
        let mut b = BroadcastStream::new(tx.subscribe());

        let t = sample();
        tx.send(t).unwrap();

        assert_eq!(a.next().await.unwrap().unwrap(), t);
        assert_eq!(b.next().await.unwrap().unwrap(), t);
    }
}
