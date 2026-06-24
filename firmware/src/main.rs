#![no_std]
#![no_main]

use defmt_rtt as _; // defmt поверх RTT
use panic_probe as _; // panic-обработчик с defmt-выводом

#[cortex_m_rt::entry]
fn main() -> ! {
    let _p = embassy_nrf::init(Default::default());
    defmt::info!("firmware boot ");
    loop {
        cortex_m::asm::wfi(); // ждём прерывание — не крутим CPU впустую (Ch.13)
    }
}
