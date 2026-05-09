//! Draw "Hello STM32F469!" on the display using the Board API.
//!
//! Build:
//!   cargo build --target thumbv7em-none-eabihf --example board_display
//!
//! Flash:
//!   probe-rs run --chip STM32F469NIHx --example board_display

#![no_std]
#![no_main]

use defmt::info;
use embassy_executor::Spawner;
use embassy_stm32f469i_disco::{config_180, Board, BoardHint};
use embassy_time::Timer;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyle},
    pixelcolor::Rgb888,
    prelude::*,
    primitives::{PrimitiveStyle, Rectangle},
    text::{Baseline, Text},
};
use {defmt_rtt as _, panic_probe as _};

#[embassy_executor::main]
async fn main(_spawner: Spawner) {
    let p = embassy_stm32::init(config_180());
    let mut board = Board::new(p, BoardHint::Auto);
    info!("board_display: init complete");

    let mut fb = board.display.fb();
    fb.clear(Rgb888::BLACK);

    let style = PrimitiveStyle::with_fill(Rgb888::new(0x20, 0x20, 0x40));
    Rectangle::new(Point::new(0, 0), Size::new(480, 800))
        .into_styled(style)
        .draw(&mut fb)
        .unwrap();

    let text_style = MonoTextStyle::new(&FONT_10X20, Rgb888::WHITE);
    Text::with_baseline(
        "Hello STM32F469!",
        Point::new(100, 400),
        text_style,
        Baseline::Top,
    )
    .draw(&mut fb)
    .unwrap();

    info!("board_display: text drawn, blinking LED");

    loop {
        board.leds.green.set_low();
        Timer::after_millis(500).await;
        board.leds.green.set_high();
        Timer::after_millis(500).await;
    }
}
