//! Чтение CDC-порта micro:bit: байты → COBS-кадры (раздел 0x00) →
//! shared::decode_frame → broadcast подписчикам (SSE).
//! Устойчив к flash/обрыву: внешний цикл переоткрывает порт.

use std::sync::atomic::Ordering;
use std::time::Duration;

use shared::{decode_frame, Telemetry};
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast;
use tokio_serial::SerialPortBuilderExt;

use crate::flash::FlashGate;

/// Внешний цикл: держит соединение, переоткрывает при flash/обрыве.
pub async fn run(port: String, tx: broadcast::Sender<Telemetry>, gate: FlashGate) {
    loop {
        // Если идёт прошивка — ждём её завершения, не трогаем порт.
        if gate.flashing.load(Ordering::SeqCst) {
            gate.resume.notified().await;
            tokio::time::sleep(Duration::from_millis(500)).await; // дать USB переподняться
        }

        if let Err(e) = read_loop(&port, &tx, &gate).await {
            tracing::warn!(?e, "serial disconnected, retrying in 1s");
            tokio::time::sleep(Duration::from_secs(1)).await;
        }
    }
}

/// Внутренний цикл чтения. Возвращается при ошибке или начале flash.
async fn read_loop(
    port: &str,
    tx: &broadcast::Sender<Telemetry>,
    gate: &FlashGate,
) -> anyhow::Result<()> {
    let mut serial = tokio_serial::new(port, 115_200).open_native_async()?;
    tracing::info!(port = %port, "serial opened");

    let mut acc: Vec<u8> = Vec::with_capacity(128); // накопитель текущего кадра
    let mut byte = [0u8; 1];

    loop {
        // Прерываем чтение, если началась прошивка → отпускаем порт.
        if gate.flashing.load(Ordering::SeqCst) {
            tracing::info!("pausing serial for flash");
            return Ok(());
        }

        let n = serial.read(&mut byte).await?;
        if n == 0 {
            continue;
        }
        let b = byte[0];

        if b == 0x00 {
            // конец кадра: декодируем накопленное.
            if !acc.is_empty() {
                match decode_frame(&acc) {
                    Ok(frame) => {
                        let _ = tx.send(frame);
                    }
                    Err(e) => tracing::warn!(?e, "bad frame"),
                }
                acc.clear();
            }
        } else {
            acc.push(b);
            if acc.len() > 128 {
                tracing::warn!("frame overflow, resync");
                acc.clear();
            }
        }
    }
}
