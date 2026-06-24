//! Общее состояние телеметрии: каждая сенсорная таска пишет своё поле,
//! uart_task снимает целостный снимок и отправляет кадр.

use embassy_sync::blocking_mutex::raw::ThreadModeRawMutex;
use embassy_sync::mutex::Mutex;

use shared::Telemetry;

/// Глобальный снимок. ThreadModeRawMutex: все таски в thread-mode (один executor,
/// нет писателей из прерываний) → достаточно лёгкого мьютекса без critical-section.
pub static STATE: Mutex<ThreadModeRawMutex, Telemetry> = Mutex::new(Telemetry {
    temp_q4: 0,
    accel_mg: [0; 3],
    btn_a: 0,
    btn_b: 0,
});
