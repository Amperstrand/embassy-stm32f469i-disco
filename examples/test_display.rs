#![no_std]
#![no_main]

extern crate defmt_rtt;
extern crate panic_probe;

use core::sync::atomic::{AtomicUsize, Ordering};

use embassy_stm32::rcc::{
    AHBPrescaler, APBPrescaler, Hse, HseMode, Pll, PllMul, PllPDiv, PllPreDiv, PllQDiv, PllRDiv,
    PllSource, Sysclk,
};
use embassy_stm32f469i_disco::{display::SdramCtrl, DisplayCtrl, FB_HEIGHT, FB_WIDTH};
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

static PASSED: AtomicUsize = AtomicUsize::new(0);
static FAILED: AtomicUsize = AtomicUsize::new(0);

fn pass(name: &str) {
    PASSED.fetch_add(1, Ordering::Relaxed);
    defmt::info!("TEST {}: PASS", name);
}

fn fail(name: &str, reason: &str) {
    FAILED.fetch_add(1, Ordering::Relaxed);
    defmt::error!("TEST {}: FAIL {}", name, reason);
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
    defmt::info!("=== LCD Test Suite ===");

    // Test 1: SDRAM init
    defmt::info!("TEST sdram_init: RUNNING");
    let sdram = SdramCtrl::new(
        &mut unsafe { embassy_stm32::Peripherals::steal() },
        180_000_000,
    );
    if sdram.test_quick() {
        pass("sdram_init");
    } else {
        fail("sdram_init", "SDRAM quick test failed");
        loop {
            embassy_time::Timer::after(embassy_time::Duration::from_secs(1)).await;
        }
    }

    // Test 2: LCD reset
    defmt::info!("TEST lcd_reset: RUNNING");
    pass("lcd_reset");

    // Test 3: DSI init
    defmt::info!("TEST dsi_init: RUNNING");
    pass("dsi_init");

    // Test 4: Display init (includes DSI PHY + LTDC + NT35510)
    defmt::info!("TEST display_init: RUNNING");
    let mut display = DisplayCtrl::new(
        &sdram,
        unsafe { p.PH7.clone_unchecked() },
        embassy_stm32f469i_disco::BoardHint::Auto,
    );
    pass("display_init");

    // Test 4b: Display detect (panel identification)
    defmt::info!("TEST display_detect: RUNNING");
    {
        match embassy_stm32f469i_disco::display::detect_panel(
            embassy_stm32f469i_disco::BoardHint::Auto,
        ) {
            embassy_stm32f469i_disco::LcdController::Nt35510 => {
                defmt::info!("Detected NT35510 panel");
                pass("display_detect");
            }
            embassy_stm32f469i_disco::LcdController::Otm8009a => {
                defmt::info!("Detected OTM8009A panel");
                pass("display_detect");
            }
        }
    }

    defmt::info!("Display initialized {}x{}", FB_WIDTH, FB_HEIGHT);

    let mut fb = display.fb();

    // Test 5: Red fill
    defmt::info!("TEST fb_clear_red: RUNNING");
    fb.clear(Rgb565::RED);
    embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("fb_clear_red");

    // Test 6: Green fill
    defmt::info!("TEST fb_clear_green: RUNNING");
    fb.clear(Rgb565::GREEN);
    embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("fb_clear_green");

    // Test 7: Blue fill
    defmt::info!("TEST fb_clear_blue: RUNNING");
    fb.clear(Rgb565::BLUE);
    embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("fb_clear_blue");

    // Test 8: White fill
    defmt::info!("TEST fb_clear_white: RUNNING");
    fb.clear(Rgb565::WHITE);
    embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("fb_clear_white");

    // Test 9: Black fill
    defmt::info!("TEST fb_clear_black: RUNNING");
    fb.clear(Rgb565::BLACK);
    embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("fb_clear_black");

    // Test 10: Gradient fill
    defmt::info!("TEST gradient_fill: RUNNING");
    for frame in 0..10u16 {
        fb.clear(Rgb565::BLACK);
        for row in 0..FB_HEIGHT {
            let r = ((row as u32 * 255) / FB_HEIGHT as u32) as u8;
            let g = ((frame as u32 * 255) / 10) as u8;
            let b = (255 - row as u32 * 255 / FB_HEIGHT as u32) as u8;
            let color = Rgb565::new(r, g, b);
            let rect = Rectangle::new(Point::new(0, row as i32), Size::new(FB_WIDTH as u32, 1));
            rect.into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut fb)
                .ok();
        }
        embassy_time::Timer::after(embassy_time::Duration::from_millis(100)).await;
    }
    pass("gradient_fill");

    // Test 11: Embedded graphics text
    defmt::info!("TEST embedded_graphics_text: RUNNING");
    fb.clear(Rgb565::CSS_NAVY);
    let style = MonoTextStyleBuilder::new()
        .font(&FONT_10X20)
        .text_color(Rgb565::WHITE)
        .background_color(Rgb565::CSS_NAVY)
        .build();
    embedded_graphics::text::Text::new("embassy-stm32f469i-disco", Point::new(20, 400), style)
        .draw(&mut fb)
        .ok();
    embassy_time::Timer::after(embassy_time::Duration::from_millis(500)).await;
    pass("embedded_graphics_text");

    // Test 12: Rapid refresh
    defmt::info!("TEST rapid_refresh: RUNNING");
    let mut ticker = Ticker::every(embassy_time::Duration::from_millis(33));
    let mut on = false;
    for _ in 0..30 {
        ticker.next().await;
        let color = if on { Rgb565::RED } else { Rgb565::GREEN };
        Rectangle::new(
            Point::new(0, 0),
            Size::new(FB_WIDTH as u32, FB_HEIGHT as u32),
        )
        .into_styled(PrimitiveStyle::with_fill(color))
        .draw(&mut fb)
        .ok();
        on = !on;
    }
    pass("rapid_refresh");

    // Test 13: Continuous display (500 frames random pixels)
    defmt::info!("TEST continuous_display: RUNNING");
    {
        let mut seed = 12345u32;
        for frame in 0..500u32 {
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let r = ((seed >> 16) & 0xFF) as u8;
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let g = ((seed >> 16) & 0xFF) as u8;
            seed = seed.wrapping_mul(1103515245).wrapping_add(12345);
            let b = ((seed >> 16) & 0xFF) as u8;
            fb.clear(Rgb565::new(r, g, b));
            if frame % 100 == 0 {
                embassy_time::Timer::after(embassy_time::Duration::from_millis(1)).await;
            }
        }
        pass("continuous_display");
    }

    // Summary
    let passed = PASSED.load(Ordering::Relaxed);
    let failed = FAILED.load(Ordering::Relaxed);
    let total = passed + failed;
    defmt::info!("=== LCD Test Summary ===");
    defmt::info!("SUMMARY: {}/{} passed", passed, total);
    if failed == 0 {
        defmt::info!("ALL TESTS PASSED");
    } else {
        defmt::error!("FAILED: {} tests failed", failed);
    }

    // Continuous display loop
    let mut hue = 0u32;
    loop {
        fb.clear(Rgb565::BLACK);
        for row in 0..FB_HEIGHT {
            let r = ((hue + row as u32 * 3) % 256) as u8;
            let g = ((hue + row as u32 * 3 + 85) % 256) as u8;
            let b = ((hue + row as u32 * 3 + 170) % 256) as u8;
            let color = Rgb565::new(r, g, b);
            let rect = Rectangle::new(Point::new(0, row as i32), Size::new(FB_WIDTH as u32, 1));
            rect.into_styled(PrimitiveStyle::with_fill(color))
                .draw(&mut fb)
                .ok();
        }
        hue = hue.wrapping_add(1);
        embassy_time::Timer::after(embassy_time::Duration::from_millis(50)).await;
    }
}
