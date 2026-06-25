//! Датчик температуры кристалла SoC (nRF52833 PS §6.20 TEMP).
//! Разрешение 0.25°C; read() отдаёт fixed-point I30F2 в шагах 0.25°,
//! поэтому сразу сводим к f32 через .to_num() (без зависимости от крейта `fixed`).

use embassy_nrf::peripherals::TEMP;
use embassy_nrf::temp::Temp;
use embassy_nrf::twim::{self, Twim};
use embassy_nrf::Peri;

use crate::board::I2cPins;
use crate::state::STATE;
use crate::Irqs;

const PERIOD_SECS: u64 = 1;

const LSM303_ACCEL_ADDR: u8 = 0x19;
const WHO_AM_I_A: u8 = 0x0F;
const CTRL_REG1_A: u8 = 0x20;
const CTRL_REG1_A_VAL: u8 = 0x57;
const OUT_X_L_A: u8 = 0x28;
const AUTO_INC: u8 = 0x80;
const MG_PER_DIGIT: i16 = 4;

#[embassy_executor::task]
pub async fn temp_task(peri: Peri<'static, TEMP>, irq: Irqs) {
    let mut temp = Temp::new(peri, irq);

    loop {
        let raw = temp.read().await;
        let q4 = raw.to_bits() as i16;
        STATE.lock().await.temp_q4 = q4;
        defmt::info!("SoC temp: {} C", raw.to_num::<f32>());
        embassy_time::Timer::after_secs(PERIOD_SECS).await;
    }
}

/// LSM303AGR через TWIM (внутренняя I²C). Bring-up: проверяем WHO_AM_I.
#[embassy_executor::task]
pub async fn accel_task(pins: I2cPins) {
    let config = twim::Config::default();
    let mut buf = [0u8; 16];
    let mut twim = Twim::new(pins.twim, Irqs, pins.sda, pins.scl, config, &mut buf);

    let mut id = [0u8; 1];
    match twim
        .write_read(LSM303_ACCEL_ADDR, &[WHO_AM_I_A], &mut id)
        .await
    {
        Ok(()) if id[0] == 0x33 => defmt::info!("LSM303AGR online, WHO_AM_I=0x{:02x}", id[0]),
        Ok(()) => defmt::error!("LSM303AGR wrong ID: 0x{:02x} (ждали 0x33)", id[0]),

        Err(e) => defmt::error!("LSM303AGR I2C error: {:?}", defmt::Debug2Format(&e)),
    }

    if let Err(e) = twim
        .write(LSM303_ACCEL_ADDR, &[CTRL_REG1_A, CTRL_REG1_A_VAL])
        .await
    {
        defmt::error!("LSM303AGR config error: {:?}", defmt::Debug2Format(&e));
    }

    loop {
        let mut raw = [0u8; 6];

        match twim
            .write_read(LSM303_ACCEL_ADDR, &[OUT_X_L_A | AUTO_INC], &mut raw)
            .await
        {
            Ok(()) => {
                let x = (i16::from_le_bytes([raw[0], raw[1]]) >> 6) * MG_PER_DIGIT;
                let y = (i16::from_le_bytes([raw[2], raw[3]]) >> 6) * MG_PER_DIGIT;
                let z = (i16::from_le_bytes([raw[4], raw[5]]) >> 6) * MG_PER_DIGIT;
                STATE.lock().await.accel_mg = [x, y, z];
                defmt::info!("accel: x={} y={} z={} mg", x, y, z);
            }
            Err(e) => defmt::error!("LSM303AGR read error: {:?}", defmt::Debug2Format(&e)),
        }
        embassy_time::Timer::after_millis(500).await;
    }
}
