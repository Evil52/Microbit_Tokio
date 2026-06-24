//! Передача телеметрии по UART → DAPLink CDC (виден как /dev/tty.usbmodem*).
//! Раз в период снимаем STATE, кодируем shared::encode_frame, шлём + 0x00.

use embassy_nrf::peripherals::{P1_08, UARTE0};
use embassy_nrf::uarte::{Config, UarteTx};
use embassy_nrf::Peri;

use shared::{encode_frame, MAX_FRAME};

use crate::state::STATE;
use crate::Irqs;

const PERIOD_MS: u64 = 200; // 5 кадров/с

#[embassy_executor::task]
pub async fn uart_task(uarte: Peri<'static, UARTE0>, tx: Peri<'static, P1_08>) {
    let config = Config::default(); // 115200 8N1 — стандарт DAPLink CDC
    let mut tx = UarteTx::new(uarte, Irqs, tx, config);

    let mut out = [0u8; MAX_FRAME];
    loop {
        // Снимок состояния: копируем под мьютексом и сразу отпускаем.
        let snapshot = { *STATE.lock().await };

        if let Ok(frame) = encode_frame(&snapshot, &mut out) {
            // COBS-тело + разделитель кадров 0x00.
            let _ = tx.write(frame).await;
            let _ = tx.write(&[0x00]).await;
        }

        embassy_time::Timer::after_millis(PERIOD_MS).await;
    }
}
