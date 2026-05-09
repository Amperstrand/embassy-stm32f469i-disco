//! Minimal LED blink using the Board API.
//!
//! Toggles the green LED (PG6, LD1) once per second and logs the blink count
//! over RTT so a logic analyzer is not required to confirm heartbeat.
//!
//! Build:
//!   cargo build --target thumbv7em-none-eabihf --example board_blinky
//!
//! Flash and run:
//!   probe-rs run --chip STM32F469NIHx --example board_blinky

#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32f469i_disco::{config_180, Board, BoardHint};
use embassy_time::Timer;
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(config_180());
    let mut board = Board::try_new(p, BoardHint::Auto).expect("board init");

    // LEDs are active-low: set_low = on, set_high = off.
    let mut n: u32 = 0;
    loop {
        board.leds.green.set_low();
        Timer::after_secs(1).await;
        board.leds.green.set_high();
        Timer::after_secs(1).await;
        info!("blink {}", n);
        n = n.wrapping_add(1);
    }
}
