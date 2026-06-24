#![no_std]
#![no_main]

use defmt_rtt as _; // defmt поверх RTT
use panic_probe as _; // panic-обработчик с defmt-выводом

use embassy_nrf::config::{Config, HfclkSource, LfclkSource};

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal; // HFXO 64MHz от 32MHz кристалла (§5.4.1.1 p.80; schematic p.3 X1)
    config.lfclk_source = LfclkSource::InternalRC; // LFRC: нет LFXO на micro:bit (§5.4.2 p.81)

    let _p = embassy_nrf::init(config);

    defmt::info!("firmware boot: HFXO + LFRC ready");

    loop {
        embassy_time::Timer::after_secs(1).await;
        defmt::info!("alive");
    }
}
