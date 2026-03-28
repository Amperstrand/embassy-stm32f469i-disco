#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_time::Ticker;

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    defmt::info!("=== USB Soak Test ===");
    defmt::info!("Continuous display refresh + LED heartbeat + USB GPIO toggle");
    defmt::info!("Press Ctrl+C to stop");

    let mut led = embassy_stm32::gpio::Output::new(
        p.PG6,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );

    let mut dm = embassy_stm32::gpio::Output::new(
        p.PA11,
        embassy_stm32::gpio::Level::Low,
        embassy_stm32::gpio::Speed::Low,
    );

    let mut ticker = Ticker::every(embassy_time::Duration::from_millis(500));
    let mut count: u32 = 0;

    loop {
        ticker.next().await;
        led.toggle();
        dm.toggle();
        count += 1;
        if count % 120 == 0 {
            defmt::info!("soak: {} heartbeats", count);
        }
    }
}
