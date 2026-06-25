//! Передача телеметрии по UART → DAPLink CDC (виден как /dev/tty.usbmodem*).
//! Раз в период снимаем STATE, кодируем shared::encode_frame, шлём + 0x00.

use embassy_nrf::peripherals::{P0_06, UARTE0};
use embassy_nrf::uarte::{Config, UarteTx};
use embassy_nrf::Peri;

use shared::{encode_frame, MAX_FRAME};

use crate::state::STATE;
use crate::Irqs;

const PERIOD_MS: u64 = 200;

#[embassy_executor::task]
pub async fn uart_task(uarte: Peri<'static, UARTE0>, tx: Peri<'static, P0_06>) {
    let config = Config::default();
    let mut tx = UarteTx::new(uarte, Irqs, tx, config);

    let mut out = [0u8; MAX_FRAME + 1];
    loop {
        let snapshot = { *STATE.lock().await };

        let mut frame_buf = [0u8; MAX_FRAME];
        if let Ok(frame) = encode_frame(&snapshot, &mut frame_buf) {
            let len = frame.len();
            out[..len].copy_from_slice(frame);
            out[len] = 0x00;

            if let Err(e) = tx.write(&out[..len + 1]).await {
                defmt::error!("UART write error: {:?}", defmt::Debug2Format(&e));
            }
        }

        embassy_time::Timer::after_millis(PERIOD_MS).await;
    }
}
