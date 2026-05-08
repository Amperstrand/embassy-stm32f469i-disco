//! Poll touch controller and print coordinates using the Board API.
//!
//! Build:
//!   cargo build --target thumbv7em-none-eabihf --example board_touch
//!
//! Flash:
//!   probe-rs run --chip STM32F469NIHx --example board_touch

#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32f469i_disco::{config_180, Board, BoardHint};
use embassy_time::{Duration, Timer};
use {defmt_rtt as _, panic_probe as _};

const POLL_MS: u64 = 50;

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(config_180());
    let mut board = Board::new(p, BoardHint::Auto);
    info!("board_touch: init complete");

    loop {
        match board.touch.get_touch() {
            Ok(Some(pt)) => info!("touch: x={}, y={}", pt.x, pt.y),
            Ok(None) => {}
            Err(_) => info!("touch: I2C error"),
        }
        Timer::after(Duration::from_millis(POLL_MS)).await;
    }
}
