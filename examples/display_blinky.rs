#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use embassy_stm32::rcc::{
    AHBPrescaler, APBPrescaler, Hse, HseMode, Pll, PllMul, PllPDiv, PllPreDiv, PllQDiv, PllRDiv,
    PllSource, Sysclk,
};
use embassy_stm32f469i_disco::{display::SdramCtrl, DisplayCtrl, FB_WIDTH, FB_HEIGHT};
use embassy_time::Ticker;
use embedded_graphics::{
    mono_font::{ascii::FONT_10X20, MonoTextStyleBuilder},
    pixelcolor::Rgb565,
    prelude::*,
    primitives::{rectangle::Rectangle, PrimitiveStyle},
};

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn LTDC() {
    cortex_m::asm::nop();
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn LTDC_ER() {
    cortex_m::asm::nop();
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DSI() {
    cortex_m::asm::nop();
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DSIHOST() {
    cortex_m::asm::nop();
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn DMA2D() {
    cortex_m::asm::nop();
}

#[allow(non_snake_case)]
#[no_mangle]
unsafe extern "C" fn FMC() {
    cortex_m::asm::nop();
}

#[embassy_executor::main]
async fn main(_spawner: embassy_executor::Spawner) {
    let mut config = embassy_stm32::Config::default();
    config.rcc.hse = Some(Hse {
        freq: embassy_stm32::time::mhz(8),
        mode: HseMode::Oscillator,
    });
    config.rcc.pll_src = PllSource::HSE;
    config.rcc.pll = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL360,
        divp: Some(PllPDiv::DIV2),
        divq: Some(PllQDiv::DIV7),
        divr: Some(PllRDiv::DIV6),
    });
    config.rcc.pllsai = Some(Pll {
        prediv: PllPreDiv::DIV8,
        mul: PllMul::MUL384,
        divp: None,
        divq: None,
        divr: Some(PllRDiv::DIV7),
    });
    config.rcc.sys = Sysclk::PLL1_P;
    config.rcc.ahb_pre = AHBPrescaler::DIV1;
    config.rcc.apb1_pre = APBPrescaler::DIV4;
    config.rcc.apb2_pre = APBPrescaler::DIV2;

    let p = embassy_stm32::init(config);
    defmt::info!("embassy init done");

    let sdram = SdramCtrl::new(&mut unsafe { embassy_stm32::Peripherals::steal() }, 180_000_000);
    defmt::info!("SDRAM test: {}", sdram.test_quick());

    let mut display = DisplayCtrl::new(&sdram, unsafe { p.PH7.clone_unchecked() }, embassy_stm32f469i_disco::BoardHint::Auto);
    defmt::info!("Display init done, {}x{}", FB_WIDTH, FB_HEIGHT);

    let mut fb = display.fb();

    fb.clear(Rgb565::CSS_NAVY);
    defmt::info!("Framebuffer cleared");

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
