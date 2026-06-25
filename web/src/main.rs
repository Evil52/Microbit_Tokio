//! web — хост-сервис (Tokio): читает CDC-порт micro:bit, парсит телеметрию
//! через `shared`, отдаёт live-дашборд по axum + SSE.

mod flash;
mod serial;
mod web_server;

use flash::FlashGate;
use tokio::net::TcpListener;
use tokio::sync::broadcast;
use web_server::{router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Порт CDC: из аргумента или env. На macOS использовать cu.* (tty.* блокирует open).
    let port = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("MICROBIT_PORT").ok())
        .unwrap_or_else(|| "/dev/cu.usbmodem312102".to_string());

    let (tx, _rx) = broadcast::channel(16);
    let gate = FlashGate::new();

    // Фоновый ридер серийника (устойчив к flash/обрыву — переоткрывает порт).
    {
        let tx = tx.clone();
        let port = port.clone();
        let gate = gate.clone();
        tokio::spawn(async move {
            serial::run(port, tx, gate).await;
        });
    }

    let app = router(AppState { tx, gate });
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    tracing::info!("dashboard on http://127.0.0.1:8080");
    axum::serve(listener, app).await?;
    Ok(())
}
