//! Чтение CDC-порта micro:bit: байты → COBS-кадры (раздел 0x00) →
//! shared::decode_frame → broadcast подписчикам (SSE).

use shared::{decode_frame, Telemetry};
use tokio::io::AsyncReadExt;
use tokio::sync::broadcast;
use tokio_serial::SerialPortBuilderExt;

/// Открыть порт и качать кадры в broadcast. Запускается в фоне (tokio::spawn).
pub async fn run(port: String, tx: broadcast::Sender<Telemetry>) -> anyhow::Result<()> {
    let mut serial = tokio_serial::new(&port, 115_200).open_native_async()?;
    tracing::info!(port = %port, "serial opened");

    let mut acc: Vec<u8> = Vec::with_capacity(128); // накопитель текущего кадра
    let mut byte = [0u8; 1];

    loop {
        // Читаем по байту: просто и достаточно для 5 кадров/с.
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
                        // нет подписчиков — не страшно, ошибку send игнорируем.
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
