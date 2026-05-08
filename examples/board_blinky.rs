//! Blink all 4 user LEDs in sequence using the Board API.
//!
//! Build:
//!   cargo build --target thumbv7em-none-eabihf --example board_blinky
//!
//! Flash:
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
    let mut board = Board::new(p, BoardHint::Auto);

    info!("board_blinky: starting LED sequence");

    // LEDs are active-low: set_low = on, set_high = off
    loop {
        board.leds.green.set_low();
        Timer::after_millis(250).await;
        board.leds.green.set_high();
        board.leds.orange.set_low();
        Timer::after_millis(250).await;
        board.leds.orange.set_high();
        board.leds.red.set_low();
        Timer::after_millis(250).await;
        board.leds.red.set_high();
        board.leds.blue.set_low();
        Timer::after_millis(250).await;
        board.leds.blue.set_high();
    }
}
