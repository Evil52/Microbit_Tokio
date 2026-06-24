//! web — хост-сервис (Tokio): читает CDC-порт micro:bit, парсит телеметрию
//! через `shared`, отдаёт live-дашборд по axum + SSE.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("web boot ");
    Ok(())
}
