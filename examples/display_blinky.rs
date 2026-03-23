#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::Config;
use embassy_stm32f469i_disco::{display::SdramCtrl, DisplayCtrl, FB_WIDTH, FB_HEIGHT};
use embassy_time::Ticker;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{rectangle::Rectangle, PrimitiveStyle},
};

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let p = embassy_stm32::init(Config::default());

    let sdram = SdramCtrl::new(&mut unsafe { embassy_stm32::Peripherals::steal() }, 180_000_000);
    defmt::info!("SDRAM test: {}", sdram.test_quick());

    let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() });
    let mut fb = display.fb();

    fb.clear(Rgb565::CSS_NAVY);
    defmt::info!("Display: {}x{} ready", FB_WIDTH, FB_HEIGHT);

    let style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(Rgb565::WHITE)
        .background_color(Rgb565::CSS_NAVY)
        .build();
    embedded_graphics::text::Text::new("embassy-stm32f469i-disco", Point::new(20, 400), style)
        .draw(&mut fb)
        .ok();

    let mut ticker = Ticker::every(embassy_time::Duration::from_secs(1));
    let mut on = false;
    loop {
        ticker.next().await;
        let color = if on { Rgb565::RED } else { Rgb565::CSS_NAVY };
        Rectangle::new(Point::new(100, 350), Size::new(280, 100))
            .into_styled(PrimitiveStyle::with_fill(color))
            .draw(&mut fb)
            .ok();
        on = !on;
    }
}
