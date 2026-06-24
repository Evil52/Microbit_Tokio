//! web — хост-сервис (Tokio): читает CDC-порт micro:bit, парсит телеметрию
//! через `shared`, отдаёт live-дашборд по axum + SSE.

mod serial;
mod web_server;

use tokio::net::TcpListener;
use tokio::sync::broadcast;
use web_server::{router, AppState};

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();

    // Порт CDC: из аргумента или env, иначе подсказка по умолчанию.
    let port = std::env::args()
        .nth(1)
        .or_else(|| std::env::var("MICROBIT_PORT").ok())
        .unwrap_or_else(|| "/dev/tty.usbmodem1102".to_string());

    let (tx, _rx) = broadcast::channel(16);

    // Фоновый ридер серийника. При обрыве порта — лог и выход из задачи.
    {
        let tx = tx.clone();
        let port = port.clone();
        tokio::spawn(async move {
            if let Err(e) = serial::run(port, tx).await {
                tracing::error!(?e, "serial task ended");
            }
        });
    }

    let app = router(AppState { tx });
    let listener = TcpListener::bind("127.0.0.1:8080").await?;
    tracing::info!("dashboard on http://127.0.0.1:8080");
    axum::serve(listener, app).await?;
    Ok(())
}
