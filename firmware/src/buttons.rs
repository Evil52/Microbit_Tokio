//! Кнопки A/B: ждём фронт нажатия и глушим дребезг таймером.
//! Active-low (schematic p.3): нажатие = переход High→Low (falling edge).
//!
//! Прерывание GPIOTE настраивается самим `embassy_nrf::init()` (хендлер зашит
//! в крейте под фичей `rt`), поэтому `bind_interrupts!` здесь НЕ нужен.
//! Фронты ловятся через механизм SENSE/PORT (nRF52833 PS §6.9 p.148):
//! `Input::wait_for_falling_edge()` ставит SENSE и засыпает до события порта.

use embassy_nrf::gpio::Input;

use crate::state::STATE;

/// Антидребезг: окно тишины после фиксации фронта (Making Embedded Systems Ch.6).
const DEBOUNCE_MS: u64 = 20;

#[embassy_executor::task]
pub async fn button_a_task(mut btn: Input<'static>) {
    loop {
        btn.wait_for_falling_edge().await;
        {
            let mut s = STATE.lock().await;
            s.btn_a = s.btn_a.wrapping_add(1);
        }
        defmt::info!("BTN_A pressed");
        embassy_time::Timer::after_millis(DEBOUNCE_MS).await;
        btn.wait_for_high().await;
    }
}

#[embassy_executor::task]
pub async fn button_b_task(mut btn: Input<'static>) {
    loop {
        btn.wait_for_falling_edge().await;
        {
            let mut s = STATE.lock().await;
            s.btn_b = s.btn_b.wrapping_add(1);
        }
        defmt::info!("BTN_B pressed");
        embassy_time::Timer::after_millis(DEBOUNCE_MS).await;
        btn.wait_for_high().await;
    }
}
