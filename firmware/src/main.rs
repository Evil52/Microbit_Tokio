#![no_std]
#![no_main]

use defmt_rtt as _;
use panic_probe as _;

mod board;
mod buttons;
mod led;
mod sensor;
mod state;
mod uart;

use embassy_nrf::peripherals::{TWISPI0, UARTE0};
use embassy_nrf::{
    bind_interrupts,
    config::{Config, HfclkSource, LfclkSource},
    temp, twim, uarte,
};

bind_interrupts!(struct Irqs{
    TEMP => temp::InterruptHandler;
    TWISPI0 => twim::InterruptHandler<TWISPI0>;
    UARTE0 => uarte::InterruptHandler<UARTE0>;
});

#[embassy_executor::main]
async fn main(spawner: embassy_executor::Spawner) {
    let mut config = Config::default();
    config.hfclk_source = HfclkSource::ExternalXtal;
    config.lfclk_source = LfclkSource::InternalRC;

    let p = embassy_nrf::init(config);
    let board = board::split(p);

    defmt::info!("firmware boot: HFXO + LFRC ready");

    spawner.spawn(led::display_task(board.leds).unwrap());
    spawner.spawn(buttons::button_a_task(board.buttons.a).unwrap());
    spawner.spawn(buttons::button_b_task(board.buttons.b).unwrap());
    spawner.spawn(sensor::temp_task(board.temp, Irqs).unwrap());
    spawner.spawn(sensor::accel_task(board.i2c).unwrap());
    spawner.spawn(uart::uart_task(board.uart.uarte, board.uart.tx).unwrap());

    loop {
        embassy_time::Timer::after_secs(10).await;
        defmt::info!("alive");
    }
}
