#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_time::Timer;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    defmt::info!("Blink test");

    let mut led = embassy_stm32::gpio::Output::new(
        p.PG6,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );

    loop {
        Timer::after(embassy_time::Duration::from_secs(1)).await;
        led.toggle();
        defmt::info!("toggle");
    }
}
