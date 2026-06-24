//! LED-матрица 5×5: построчное сканирование (row scan, Making Embedded Systems Ch.8).
//! Одна строка активна за раз, ~2 мс на строку → ~100 Гц на кадр, без призраков.

use crate::board::LedPins;

#[embassy_executor::task]
pub async fn display_task(pins: LedPins) {
    let LedPins { mut rows, mut cols } = pins;
    let frame = [[true; 5]; 5];

    loop {
        for r in 0..5 {
            // Выставляем столбцы текущей строки до подачи питания на строку.
            for c in 0..5 {
                if frame[r][c] {
                    cols[c].set_low(); // active-low: зажечь
                } else {
                    cols[c].set_high(); // погасить
                }
            }

            rows[r].set_high(); // active-high: подать питание на строку
            embassy_time::Timer::after_millis(2).await;
            rows[r].set_low(); // снять питание перед следующей строкой
        }
    }
}
