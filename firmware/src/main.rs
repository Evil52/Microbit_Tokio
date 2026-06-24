#![no_std]
#![no_main]

use defmt_rtt as _; // defmt поверх RTT
use panic_probe as _; // panic-обработчик с defmt-выводом

mod board;
mod buttons;
mod led;

use embassy_nrf::config::{Config, HfclkSource, LfclkSource};

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal; // HFXO 64MHz от 32MHz кристалла (§5.4.1.1 p.80; schematic p.3 X1)
    config.lfclk_source = LfclkSource::InternalRC; // LFRC: нет LFXO на micro:bit (§5.4.2 p.81)

    // init() настраивает в т.ч. прерывание GPIOTE для async-фронтов кнопок.
    let p = embassy_nrf::init(config);
    let board = board::split(p);

    defmt::info!("firmware boot: HFXO + LFRC ready");

    spawner.spawn(led::display_task(board.leds).unwrap());
    spawner.spawn(buttons::button_a_task(board.buttons.a).unwrap());
    spawner.spawn(buttons::button_b_task(board.buttons.b).unwrap());

    loop {
        embassy_time::Timer::after_secs(10).await;
        defmt::info!("alive");
    }
}
