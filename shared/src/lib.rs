//! shared — контракт телеметрии между firmware (no_std) и web (std).
//!
//! Формат провода: postcard-COBS кадр.
//!   COBS{ version(1) | postcard(Telemetry) | crc16(2 LE) } 0x00
//! COBS даёт самосинхронизацию потока UART (0x00 — разделитель кадров, внутри
//! не встречается). CRC16 ловит битые байты, version — эволюцию схемы.

#![cfg_attr(not(feature = "std"), no_std)]

mod frame;

pub use frame::{decode_frame, encode_frame, DecodeError};

use serde::{Deserialize, Serialize};

/// Версия схемы кадра. Растёт при несовместимом изменении Telemetry.
pub const PROTOCOL_VERSION: u8 = 1;

/// Максимальный размер закодированного COBS-кадра (payload + version + crc + overhead).
pub const MAX_FRAME: usize = 64;

/// Один кадр телеметрии с платы.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub struct Telemetry {
    /// Температура кристалла SoC в шагах 0.25°C (как отдаёт nRF TEMP).
    pub temp_q4: i16,
    /// Ускорение по осям X/Y/Z в mg (LSM303AGR, ±2g normal mode).
    pub accel_mg: [i16; 3],
    /// Счётчик нажатий кнопки A (монотонный, wrap при переполнении).
    pub btn_a: u16,
    /// Счётчик нажатий кнопки B.
    pub btn_b: u16,
}

impl Telemetry {
    /// Температура в °C как f32 (для дашборда).
    pub fn temp_celsius(&self) -> f32 {
        self.temp_q4 as f32 / 4.0
    }
}
