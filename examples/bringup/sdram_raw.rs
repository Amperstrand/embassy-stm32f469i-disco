//! SDRAM initialization and memory test using SdramCtrl BSP API.
#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_time::Timer;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    defmt::info!("Starting SDRAM test...");

    let mut sdram = embassy_stm32f469i_disco::sdram_init!(p);
    let ok = sdram.test_quick();
    defmt::info!("SDRAM test: {}", ok);

    let mut led = embassy_stm32::gpio::Output::new(
        p.PG6,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );

    loop {
        Timer::after(embassy_time::Duration::from_secs(1)).await;
        led.toggle();
    }
}
