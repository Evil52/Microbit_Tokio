//! web — хост-сервис (Tokio): читает CDC-порт micro:bit, парсит телеметрию
//! через `shared`, отдаёт live-дашборд по axum + SSE.
//!
//! Фаза 0: минимальный скелет — компилируется и сразу завершается.
//! Реальный конвейер serial -> shared::decode -> broadcast -> SSE — Фаза 5.

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    tracing_subscriber::fmt::init();
    tracing::info!("web boot (Фаза 0 skeleton)");
    Ok(())
}
