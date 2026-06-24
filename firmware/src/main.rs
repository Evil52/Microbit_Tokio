#![no_std]
#![no_main]

use defmt_rtt as _; // defmt поверх RTT
use panic_probe as _; // panic-обработчик с defmt-выводом

use embassy_nrf::config::{Config, HfclkSource, LfclkSource};
use embassy_nrf::gpio::{Level, Output, OutputDrive};

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal; // HFXO 64MHz от 32MHz кристалла (§5.4.1.1 p.80; schematic p.3 X1)
    config.lfclk_source = LfclkSource::InternalRC; // LFRC: нет LFXO на micro:bit (§5.4.2 p.81)

    let p = embassy_nrf::init(config);

    // LEd

    let rows: [Output<'static>; 5] = [
        Output::new(p.P0_21, Level::Low, OutputDrive::Standard),
        Output::new(p.P0_22, Level::Low, OutputDrive::Standard),
        Output::new(p.P0_15, Level::Low, OutputDrive::Standard),
        Output::new(p.P0_24, Level::Low, OutputDrive::Standard),
        Output::new(p.P0_19, Level::Low, OutputDrive::Standard),
    ];

    let cols: [Output<'static>; 5] = [
        Output::new(p.P0_28, Level::High, OutputDrive::Standard), // COL1
        Output::new(p.P0_11, Level::High, OutputDrive::Standard), // COL2
        Output::new(p.P0_31, Level::High, OutputDrive::Standard), // COL3
        Output::new(p.P1_05, Level::High, OutputDrive::Standard), // COL4
        Output::new(p.P0_30, Level::High, OutputDrive::Standard), // COL5
    ];

    defmt::info!("firmware boot: HFXO + LFRC ready");
    spawner.spawn(display_task(rows, cols).unwrap());

    loop {
        embassy_time::Timer::after_secs(10).await;
        defmt::info!("alive");
    }
}

/// Сканирование LED-матрицы 5x5 (row scan, Ch.8).

#[embassy_executor::task]
async fn display_task(mut rows: [Output<'static>; 5], mut cols: [Output<'static>; 5]) {
    let frame = [[true; 5]; 5];

    loop {
        for r in 0..5 {
            for c in 0..5 {
                if frame[r][c] {
                    cols[c].set_low();
                } else {
                    cols[c].set_high();
                }
            }

            rows[r].set_high();
            embassy_time::Timer::after_millis(2).await;
            rows[r].set_low();
        }
    }
}
